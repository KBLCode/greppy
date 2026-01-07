//! Claude API client for query enhancement
//!
//! Uses the Anthropic Messages API with support for:
//! - ANTHROPIC_API_KEY environment variable (standard API key auth)
//! - OAuth tokens from Claude Pro/Max subscription (Bearer auth with beta headers)
//!
//! Optimized for speed with:
//! - HTTP/2 with connection pooling and keep-alive
//! - Pre-warmed connections
//! - Minimal token usage

use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::auth;

/// Claude API endpoint
const API_URL: &str = "https://api.anthropic.com/v1/messages";

/// Default model for query enhancement (fastest)
const DEFAULT_MODEL: &str = "claude-3-5-haiku-20241022";

/// Maximum tokens for query enhancement response
const MAX_TOKENS: u32 = 256;

/// Request timeout in seconds
const TIMEOUT_SECS: u64 = 3;

/// Global HTTP client with connection pooling (reused across all requests)
static HTTP_CLIENT: Lazy<Arc<reqwest::Client>> = Lazy::new(|| {
    Arc::new(
        reqwest::Client::builder()
            // Connection pooling - keep connections warm
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            // HTTP/2 optimizations (Anthropic supports HTTP/2)
            .http2_prior_knowledge()
            .http2_keep_alive_interval(Duration::from_secs(30))
            .http2_keep_alive_timeout(Duration::from_secs(10))
            .http2_keep_alive_while_idle(true)
            .http2_adaptive_window(true)
            // TCP optimizations
            .tcp_nodelay(true)
            .tcp_keepalive(Duration::from_secs(60))
            // Timeouts
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .build()
            .expect("Failed to build HTTP client")
    )
});

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

/// Claude API client (uses global connection pool)
pub struct ClaudeClient {
    model: String,
}

impl ClaudeClient {
    /// Create a new Claude client (reuses global HTTP connection pool)
    pub fn new() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
        }
    }

    /// Create a client with a custom model
    pub fn with_model(model: &str) -> Self {
        Self {
            model: model.to_string(),
        }
    }
    
    /// Get the shared HTTP client
    fn client() -> &'static reqwest::Client {
        &HTTP_CLIENT
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

        // Build request with appropriate headers (uses global connection pool)
        let mut req = Self::client()
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
