use crate::core::error::{Error, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};

// Claude OAuth constants
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

// Cache access token for 50 minutes (tokens typically expire in 1 hour)
const TOKEN_CACHE_DURATION: Duration = Duration::from_secs(50 * 60);

#[derive(Debug, Serialize)]
struct MessageRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct MessageResponse {
    content: Option<Vec<ContentBlock>>,
    error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

/// Cached access token with expiry
struct CachedToken {
    token: String,
    expires_at: Instant,
}

pub struct ClaudeClient {
    client: Client,
    refresh_token: String,
    cached_token: Mutex<Option<CachedToken>>,
}

impl ClaudeClient {
    /// Create a new Claude client with OAuth refresh token
    pub fn new(refresh_token: String) -> Self {
        Self {
            client: Client::new(),
            refresh_token,
            cached_token: Mutex::new(None),
        }
    }

    /// Get access token, using cache if valid
    async fn get_access_token(&self) -> Result<String> {
        // Check cache first
        if let Ok(guard) = self.cached_token.lock() {
            if let Some(ref cached) = *guard {
                if Instant::now() < cached.expires_at {
                    return Ok(cached.token.clone());
                }
            }
        }

        // Refresh token
        let params = serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": self.refresh_token,
            "client_id": CLIENT_ID,
        });

        let res = self
            .client
            .post("https://console.anthropic.com/v1/oauth/token")
            .header("Content-Type", "application/json")
            .json(&params)
            .send()
            .await
            .map_err(|e| Error::DaemonError {
                message: format!("Token refresh failed: {}", e),
            })?;

        if !res.status().is_success() {
            let text = res.text().await.unwrap_or_default();
            return Err(Error::DaemonError {
                message: format!("Token refresh error: {}", text),
            });
        }

        let token_response: TokenResponse = res.json().await.map_err(|e| Error::DaemonError {
            message: format!("Failed to parse token response: {}", e),
        })?;

        // Cache the token
        let expires_at = Instant::now()
            + Duration::from_secs(token_response.expires_in.saturating_sub(600).max(60));
        if let Ok(mut guard) = self.cached_token.lock() {
            *guard = Some(CachedToken {
                token: token_response.access_token.clone(),
                expires_at,
            });
        }

        Ok(token_response.access_token)
    }

    /// Rerank search results by relevance to query
    /// Returns JSON array of indices in order of relevance: [2, 0, 5, 1, ...]
    pub async fn rerank(&self, query: &str, chunks: &[String]) -> Result<Vec<usize>> {
        let access_token = self.get_access_token().await?;

        let system_prompt =
            "You are a code search reranker. Given a query and numbered code chunks, \
            return ONLY a JSON array of chunk indices ordered by relevance to the query. \
            Most relevant first. Example response: [2, 0, 5, 1, 3, 4]";

        let mut user_prompt = format!("Query: {}\n\nCode chunks:\n", query);
        for (i, chunk) in chunks.iter().enumerate() {
            user_prompt.push_str(&format!("\n--- Chunk {} ---\n{}\n", i, chunk));
        }
        user_prompt.push_str("\nReturn ONLY the JSON array of indices, nothing else.");

        let request_body = MessageRequest {
            model: "claude-3-5-haiku-latest".to_string(),
            max_tokens: 256,
            system: system_prompt.to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: user_prompt,
            }],
        };

        let res = self
            .client
            .post(ANTHROPIC_API_URL)
            .query(&[("beta", "true")])
            .header("Authorization", format!("Bearer {}", access_token))
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("User-Agent", "greppy/0.9.0")
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| Error::DaemonError {
                message: format!("API request failed: {}", e),
            })?;

        if !res.status().is_success() {
            let text = res.text().await.unwrap_or_default();
            return Err(Error::DaemonError {
                message: format!("Claude API Error: {}", text),
            });
        }

        let response: MessageResponse = res.json().await.map_err(|e| Error::DaemonError {
            message: format!("Failed to parse response: {}", e),
        })?;

        if let Some(error) = response.error {
            return Err(Error::DaemonError {
                message: format!("Claude API Error: {}", error.message),
            });
        }

        // Parse the JSON array from response
        if let Some(content) = response.content {
            if let Some(block) = content.first() {
                if let Some(text) = &block.text {
                    // Extract JSON array from response
                    let text = text.trim();
                    if let Ok(indices) = serde_json::from_str::<Vec<usize>>(text) {
                        return Ok(indices);
                    }
                    // Try to find JSON array in the text
                    if let Some(start) = text.find('[') {
                        if let Some(end) = text.rfind(']') {
                            let json_str = &text[start..=end];
                            if let Ok(indices) = serde_json::from_str::<Vec<usize>>(json_str) {
                                return Ok(indices);
                            }
                        }
                    }
                }
            }
        }

        // Fallback: return original order
        Ok((0..chunks.len()).collect())
    }
}
