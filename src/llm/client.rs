//! Claude API client for query enhancement
//!
//! Uses the Anthropic Messages API with support for:
//! - ANTHROPIC_API_KEY environment variable (standard API key auth)
//! - OAuth tokens from Claude Pro/Max subscription (Bearer auth with beta headers)

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::auth;

/// Claude API endpoint
const API_URL: &str = "https://api.anthropic.com/v1/messages";

/// Default model for query enhancement (fastest/cheapest)
const DEFAULT_MODEL: &str = "claude-3-haiku-20240307";

/// Maximum tokens for query enhancement response
const MAX_TOKENS: u32 = 256;

/// Request timeout in seconds
const TIMEOUT_SECS: u64 = 5;

/// Required system prompt prefix for OAuth authentication
/// This MUST be the first element in the system array for OAuth to work with Sonnet/Opus
const CLAUDE_CODE_SYSTEM_PREFIX: &str = "You are Claude Code, Anthropic's official CLI for Claude.";

/// Beta header required for OAuth authentication
const OAUTH_BETA_HEADER: &str = "oauth-2025-04-20,claude-code-20250219,interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14";

/// Authentication method
#[derive(Debug, Clone)]
enum AuthMethod {
    /// Standard API key (x-api-key header)
    ApiKey(String),
    /// OAuth Bearer token (Authorization: Bearer header)
    OAuth(String),
}

/// Claude API client
pub struct ClaudeClient {
    client: reqwest::Client,
    model: String,
}

impl ClaudeClient {
    /// Create a new Claude client
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            client,
            model: DEFAULT_MODEL.to_string(),
        }
    }

    /// Create a client with a custom model
    pub fn with_model(model: &str) -> Self {
        let mut client = Self::new();
        client.model = model.to_string();
        client
    }

    /// Get authentication method (API key or OAuth token)
    async fn get_auth() -> Result<AuthMethod> {
        // First, check for ANTHROPIC_API_KEY environment variable
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            debug!("Using ANTHROPIC_API_KEY from environment");
            return Ok(AuthMethod::ApiKey(api_key));
        }

        // Fall back to OAuth token
        if let Some(token) = auth::get_access_token().await {
            info!("Using OAuth token for Claude Pro/Max authentication");
            return Ok(AuthMethod::OAuth(token));
        }

        Err(anyhow!(
            "No API key found. Set ANTHROPIC_API_KEY environment variable or run 'greppy auth login'"
        ))
    }

    /// Send a message to Claude and get a response
    pub async fn send_message(&self, system: &str, user_message: &str) -> Result<String> {
        let auth = Self::get_auth().await?;

        // Build system prompt based on auth method
        let (system_content, use_oauth_headers) = match &auth {
            AuthMethod::ApiKey(_) => {
                // Standard API key - use system prompt as-is
                (vec![SystemBlock { 
                    block_type: "text".to_string(), 
                    text: system.to_string() 
                }], false)
            }
            AuthMethod::OAuth(_) => {
                // OAuth - prepend required Claude Code system prompt
                (vec![
                    SystemBlock { 
                        block_type: "text".to_string(), 
                        text: CLAUDE_CODE_SYSTEM_PREFIX.to_string() 
                    },
                    SystemBlock { 
                        block_type: "text".to_string(), 
                        text: system.to_string() 
                    },
                ], true)
            }
        };

        let request = MessageRequest {
            model: &self.model,
            max_tokens: MAX_TOKENS,
            system: system_content,
            messages: vec![Message {
                role: "user",
                content: user_message,
            }],
        };

        debug!("Sending request to Claude API (oauth={})", use_oauth_headers);

        // Build request with appropriate headers
        let mut req = self.client
            .post(API_URL)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01");

        // Add auth headers based on method
        match &auth {
            AuthMethod::ApiKey(key) => {
                req = req.header("x-api-key", key);
            }
            AuthMethod::OAuth(token) => {
                req = req
                    .header("Authorization", format!("Bearer {}", token))
                    .header("anthropic-beta", OAUTH_BETA_HEADER);
            }
        }

        let response = req
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Claude API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!("Claude API error: {} - {}", status, text);
            return Err(anyhow!("Claude API error: {} - {}", status, text));
        }

        let response: MessageResponse = response
            .json()
            .await
            .context("Failed to parse Claude API response")?;

        // Extract text from response
        let text = response.content
            .into_iter()
            .filter_map(|block| {
                if block.content_type == "text" {
                    Some(block.text)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        debug!("Received response from Claude API ({} chars)", text.len());
        Ok(text)
    }
}

impl Default for ClaudeClient {
    fn default() -> Self {
        Self::new()
    }
}

/// System block for structured system prompts (required for OAuth)
#[derive(Debug, Serialize)]
struct SystemBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
}

/// Message request to Claude API
#[derive(Debug, Serialize)]
struct MessageRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: Vec<SystemBlock>,
    messages: Vec<Message<'a>>,
}

/// A single message in the conversation
#[derive(Debug, Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

/// Response from Claude API
#[derive(Debug, Deserialize)]
struct MessageResponse {
    content: Vec<ContentBlock>,
}

/// Content block in response
#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = ClaudeClient::new();
        assert_eq!(client.model, DEFAULT_MODEL);
    }

    #[test]
    fn test_client_with_model() {
        let client = ClaudeClient::with_model("claude-3-sonnet-20240229");
        assert_eq!(client.model, "claude-3-sonnet-20240229");
    }
}
