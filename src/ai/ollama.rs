//! Ollama Local LLM Client
//!
//! Provides local LLM inference via Ollama for search reranking and trace enhancement.
//! Supports models like codellama, deepseek-coder, llama3, etc.
//!
//! @module ai/ollama

use crate::core::error::{Error, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// =============================================================================
// CONSTANTS
// =============================================================================

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "codellama";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

/// Ollama generate request
#[derive(Debug, Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<GenerateOptions>,
}

/// Ollama generation options
#[derive(Debug, Serialize)]
struct GenerateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
}

/// Ollama generate response
#[derive(Debug, Deserialize)]
struct GenerateResponse {
    response: String,
    #[allow(dead_code)]
    done: bool,
}

/// Ollama chat request (alternative API)
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<GenerateOptions>,
}

/// Chat message
#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Ollama chat response
#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: ChatMessageResponse,
    #[allow(dead_code)]
    done: bool,
}

/// Chat message response
#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: String,
}

/// Ollama model list response
#[derive(Debug, Deserialize)]
struct ModelsResponse {
    models: Vec<ModelInfo>,
}

/// Model information
#[derive(Debug, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    #[allow(dead_code)]
    pub size: Option<u64>,
}

// =============================================================================
// OLLAMA CLIENT
// =============================================================================

/// Ollama client for local LLM inference
pub struct OllamaClient {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaClient {
    /// Create a new Ollama client with default settings
    pub fn new() -> Self {
        Self::with_config(DEFAULT_OLLAMA_URL, DEFAULT_MODEL)
    }

    /// Create a new Ollama client with custom URL and model
    pub fn with_config(base_url: &str, model: &str) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
        }
    }

    /// Check if Ollama is running and accessible
    pub async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        self.client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// List available models
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/api/tags", self.base_url);

        let res = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| self.connection_error(e))?;

        if !res.status().is_success() {
            return Err(self.api_error("Failed to list models", res).await);
        }

        let models: ModelsResponse = res.json().await.map_err(|e| Error::DaemonError {
            message: format!("Failed to parse models response: {}", e),
        })?;

        Ok(models.models)
    }

    /// Check if a specific model is available
    pub async fn has_model(&self, model: &str) -> bool {
        self.list_models()
            .await
            .map(|models| models.iter().any(|m| m.name.starts_with(model)))
            .unwrap_or(false)
    }

    /// Generate completion using the generate API
    pub async fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);

        let request = GenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
            system: system.map(|s| s.to_string()),
            options: Some(GenerateOptions {
                temperature: Some(0.1), // Low temperature for deterministic output
                num_predict: Some(512), // Limit response length
            }),
        };

        let res = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| self.connection_error(e))?;

        if !res.status().is_success() {
            return Err(self.api_error("Generation failed", res).await);
        }

        let response: GenerateResponse = res.json().await.map_err(|e| Error::DaemonError {
            message: format!("Failed to parse generate response: {}", e),
        })?;

        Ok(response.response)
    }

    /// Generate completion using the chat API
    pub async fn chat(&self, user_message: &str, system: Option<&str>) -> Result<String> {
        let url = format!("{}/api/chat", self.base_url);

        let mut messages = Vec::new();

        if let Some(sys) = system {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: sys.to_string(),
            });
        }

        messages.push(ChatMessage {
            role: "user".to_string(),
            content: user_message.to_string(),
        });

        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            stream: false,
            options: Some(GenerateOptions {
                temperature: Some(0.1),
                num_predict: Some(512),
            }),
        };

        let res = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| self.connection_error(e))?;

        if !res.status().is_success() {
            return Err(self.api_error("Chat failed", res).await);
        }

        let response: ChatResponse = res.json().await.map_err(|e| Error::DaemonError {
            message: format!("Failed to parse chat response: {}", e),
        })?;

        Ok(response.message.content)
    }

    /// Rerank search results by relevance to query
    /// Returns indices in order of relevance: [2, 0, 5, 1, ...]
    ///
    /// This matches the interface of ClaudeClient and GeminiClient
    pub async fn rerank(&self, query: &str, chunks: &[String]) -> Result<Vec<usize>> {
        // First check if Ollama is available
        if !self.is_available().await {
            // Graceful fallback: return original order if Ollama not running
            return Ok((0..chunks.len()).collect());
        }

        let system_prompt =
            "You are a code search reranker. Given a query and numbered code chunks, \
            return ONLY a JSON array of chunk indices ordered by relevance to the query. \
            Most relevant first. Example response: [2, 0, 5, 1, 3, 4]";

        let mut user_prompt = format!("Query: {}\n\nCode chunks:\n", query);
        for (i, chunk) in chunks.iter().enumerate() {
            user_prompt.push_str(&format!("\n--- Chunk {} ---\n{}\n", i, chunk));
        }
        user_prompt.push_str("\nReturn ONLY the JSON array of indices, nothing else.");

        // Try chat API first (better for instruction following)
        let response = match self.chat(&user_prompt, Some(system_prompt)).await {
            Ok(r) => r,
            Err(_) => {
                // Fallback to generate API
                let full_prompt = format!("{}\n\n{}", system_prompt, user_prompt);
                self.generate(&full_prompt, None).await?
            }
        };

        // Parse the JSON array from response
        self.parse_rerank_response(&response, chunks.len())
    }

    // =========================================================================
    // PRIVATE HELPERS
    // =========================================================================

    /// Parse rerank response, extracting JSON array
    fn parse_rerank_response(&self, text: &str, chunk_count: usize) -> Result<Vec<usize>> {
        let text = text.trim();

        // Try direct parse
        if let Ok(indices) = serde_json::from_str::<Vec<usize>>(text) {
            return Ok(self.validate_indices(indices, chunk_count));
        }

        // Try to find JSON array in the text
        if let Some(start) = text.find('[') {
            if let Some(end) = text.rfind(']') {
                let json_str = &text[start..=end];
                if let Ok(indices) = serde_json::from_str::<Vec<usize>>(json_str) {
                    return Ok(self.validate_indices(indices, chunk_count));
                }
            }
        }

        // Fallback: return original order
        Ok((0..chunk_count).collect())
    }

    /// Validate and filter indices to ensure they're within bounds
    fn validate_indices(&self, indices: Vec<usize>, chunk_count: usize) -> Vec<usize> {
        let mut seen = std::collections::HashSet::new();
        let mut valid: Vec<usize> = indices
            .into_iter()
            .filter(|&i| i < chunk_count && seen.insert(i))
            .collect();

        // Add any missing indices at the end
        for i in 0..chunk_count {
            if !seen.contains(&i) {
                valid.push(i);
            }
        }

        valid
    }

    /// Create connection error with helpful message
    fn connection_error(&self, e: reqwest::Error) -> Error {
        if e.is_connect() {
            Error::DaemonError {
                message: format!(
                    "Cannot connect to Ollama at {}. \
                    Make sure Ollama is running (ollama serve) or check your config.",
                    self.base_url
                ),
            }
        } else if e.is_timeout() {
            Error::DaemonError {
                message: format!(
                    "Ollama request timed out. The model '{}' may be loading or too slow.",
                    self.model
                ),
            }
        } else {
            Error::DaemonError {
                message: format!("Ollama request failed: {}", e),
            }
        }
    }

    /// Create API error from response
    async fn api_error(&self, context: &str, res: reqwest::Response) -> Error {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();

        if status.as_u16() == 404 && text.contains("model") {
            Error::DaemonError {
                message: format!(
                    "Model '{}' not found. Run 'ollama pull {}' to download it.",
                    self.model, self.model
                ),
            }
        } else {
            Error::DaemonError {
                message: format!("{}: HTTP {} - {}", context, status, text),
            }
        }
    }
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_indices() {
        let client = OllamaClient::new();

        // Valid indices
        let result = client.validate_indices(vec![2, 0, 1], 3);
        assert_eq!(result, vec![2, 0, 1]);

        // Out of bounds filtered
        let result = client.validate_indices(vec![5, 0, 1], 3);
        assert_eq!(result, vec![0, 1, 2]);

        // Duplicates removed
        let result = client.validate_indices(vec![0, 0, 1], 3);
        assert_eq!(result, vec![0, 1, 2]);

        // Missing indices added
        let result = client.validate_indices(vec![2], 3);
        assert_eq!(result, vec![2, 0, 1]);
    }

    #[test]
    fn test_parse_rerank_response() {
        let client = OllamaClient::new();

        // Clean JSON
        let result = client.parse_rerank_response("[2, 0, 1]", 3).unwrap();
        assert_eq!(result, vec![2, 0, 1]);

        // JSON with surrounding text
        let result = client
            .parse_rerank_response("Here's the ranking: [2, 0, 1] based on relevance", 3)
            .unwrap();
        assert_eq!(result, vec![2, 0, 1]);

        // Invalid response falls back to original order
        let result = client
            .parse_rerank_response("I cannot rank these", 3)
            .unwrap();
        assert_eq!(result, vec![0, 1, 2]);
    }
}
