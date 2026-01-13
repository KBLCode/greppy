use crate::core::error::{Error, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

// Gemini CLI OAuth credentials (for token refresh)
const GEMINI_CLIENT_ID: &str =
    "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com";
const GEMINI_CLIENT_SECRET: &str = "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";

// Cloud Code Assist API endpoints
const CODE_ASSIST_BASE: &str = "https://cloudcode-pa.googleapis.com";

/// Cached access token with expiry
struct CachedToken {
    token: String,
    expires_at: Instant,
}

// Inner request structure for Cloud Code Assist API
#[derive(Debug, Serialize)]
struct InnerRequest {
    contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "systemInstruction")]
    system_instruction: Option<Content>,
}

// Wrapped request for Cloud Code Assist API
#[derive(Debug, Serialize)]
struct CodeAssistRequest {
    project: String,
    model: String,
    request: InnerRequest,
}

#[derive(Debug, Serialize)]
struct Content {
    role: String,
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
struct Part {
    text: String,
}

// Cloud Code Assist wraps response in a "response" field
#[derive(Debug, Deserialize)]
struct CodeAssistResponse {
    response: Option<GenerateContentResponse>,
}

#[derive(Debug, Deserialize)]
struct GenerateContentResponse {
    candidates: Option<Vec<Candidate>>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: ResponseContent,
}

#[derive(Debug, Deserialize)]
struct ResponseContent {
    parts: Vec<PartResponse>,
}

#[derive(Debug, Deserialize)]
struct PartResponse {
    text: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    expires_in: u64,
}

// loadCodeAssist response
#[derive(Debug, Deserialize)]
struct LoadCodeAssistResponse {
    #[serde(rename = "cloudaicompanionProject")]
    cloudaicompanion_project: Option<String>,
    #[serde(rename = "allowedTiers")]
    allowed_tiers: Option<Vec<AllowedTier>>,
}

#[derive(Debug, Deserialize)]
struct AllowedTier {
    id: Option<String>,
    #[serde(rename = "isDefault")]
    is_default: Option<bool>,
}

// onboardUser response
#[derive(Debug, Deserialize)]
struct OnboardUserResponse {
    done: Option<bool>,
    response: Option<OnboardResponseInner>,
}

#[derive(Debug, Deserialize)]
struct OnboardResponseInner {
    #[serde(rename = "cloudaicompanionProject")]
    cloudaicompanion_project: Option<CloudaicompanionProject>,
}

#[derive(Debug, Deserialize)]
struct CloudaicompanionProject {
    id: Option<String>,
}

// Metadata for Code Assist requests
#[derive(Debug, Clone, Serialize)]
struct CodeAssistMetadata {
    #[serde(rename = "ideType")]
    ide_type: String,
    platform: String,
    #[serde(rename = "pluginType")]
    plugin_type: String,
}

#[derive(Debug, Serialize)]
struct LoadCodeAssistRequest {
    metadata: CodeAssistMetadata,
}

#[derive(Debug, Serialize)]
struct OnboardUserRequest {
    #[serde(rename = "tierId")]
    tier_id: String,
    metadata: CodeAssistMetadata,
}

pub struct GeminiClient {
    client: Client,
    refresh_token: String,
    cached_token: std::sync::Mutex<Option<CachedToken>>,
    cached_project_id: std::sync::Mutex<Option<String>>,
}

impl GeminiClient {
    pub fn new(refresh_token: String) -> Self {
        Self {
            client: Client::new(),
            refresh_token,
            cached_token: std::sync::Mutex::new(None),
            cached_project_id: std::sync::Mutex::new(None),
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
        let params = [
            ("client_id", GEMINI_CLIENT_ID),
            ("client_secret", GEMINI_CLIENT_SECRET),
            ("refresh_token", &self.refresh_token),
            ("grant_type", "refresh_token"),
        ];

        let res = self
            .client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
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

        // Cache the token (expires_in is typically 3600 seconds, cache for 50 min)
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

    /// Get or create a managed project ID for Cloud Code Assist
    async fn get_project_id(&self, access_token: &str) -> Result<String> {
        // Check cache first
        if let Ok(guard) = self.cached_project_id.lock() {
            if let Some(ref project_id) = *guard {
                return Ok(project_id.clone());
            }
        }

        let metadata = CodeAssistMetadata {
            ide_type: "IDE_UNSPECIFIED".to_string(),
            platform: "PLATFORM_UNSPECIFIED".to_string(),
            plugin_type: "GEMINI".to_string(),
        };

        // Try to load existing project
        let load_url = format!("{}/v1internal:loadCodeAssist", CODE_ASSIST_BASE);
        let load_request = LoadCodeAssistRequest {
            metadata: metadata.clone(),
        };

        let res = self
            .client
            .post(&load_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .header("User-Agent", "greppy/0.9.0")
            .json(&load_request)
            .send()
            .await
            .map_err(|e| Error::DaemonError {
                message: format!("loadCodeAssist failed: {}", e),
            })?;

        if res.status().is_success() {
            if let Ok(load_response) = res.json::<LoadCodeAssistResponse>().await {
                if let Some(project_id) = load_response.cloudaicompanion_project {
                    // Cache and return
                    if let Ok(mut guard) = self.cached_project_id.lock() {
                        *guard = Some(project_id.clone());
                    }
                    return Ok(project_id);
                }

                // Need to onboard - get default tier
                let tier_id = load_response
                    .allowed_tiers
                    .as_ref()
                    .and_then(|tiers| {
                        tiers
                            .iter()
                            .find(|t| t.is_default == Some(true))
                            .or(tiers.first())
                    })
                    .and_then(|t| t.id.clone())
                    .unwrap_or_else(|| "FREE".to_string());

                // Onboard user
                let onboard_url = format!("{}/v1internal:onboardUser", CODE_ASSIST_BASE);
                let onboard_request = OnboardUserRequest { tier_id, metadata };

                let onboard_res = self
                    .client
                    .post(&onboard_url)
                    .header("Authorization", format!("Bearer {}", access_token))
                    .header("Content-Type", "application/json")
                    .header("User-Agent", "greppy/0.9.0")
                    .json(&onboard_request)
                    .send()
                    .await
                    .map_err(|e| Error::DaemonError {
                        message: format!("onboardUser failed: {}", e),
                    })?;

                if onboard_res.status().is_success() {
                    if let Ok(onboard_response) = onboard_res.json::<OnboardUserResponse>().await {
                        if onboard_response.done == Some(true) {
                            if let Some(project_id) = onboard_response
                                .response
                                .and_then(|r| r.cloudaicompanion_project)
                                .and_then(|p| p.id)
                            {
                                // Cache and return
                                if let Ok(mut guard) = self.cached_project_id.lock() {
                                    *guard = Some(project_id.clone());
                                }
                                return Ok(project_id);
                            }
                        }
                    }
                }
            }
        }

        Err(Error::DaemonError {
            message: "Failed to get Gemini project ID. You may need to enable Gemini API in Google Cloud Console.".to_string(),
        })
    }

    /// Rerank search results by relevance to query
    /// Returns JSON array of indices in order of relevance: [2, 0, 5, 1, ...]
    pub async fn rerank(&self, query: &str, chunks: &[String]) -> Result<Vec<usize>> {
        let access_token = self.get_access_token().await?;
        let project_id = self.get_project_id(&access_token).await?;

        let system_prompt =
            "You are a code search reranker. Given a query and numbered code chunks, \
            return ONLY a JSON array of chunk indices ordered by relevance to the query. \
            Most relevant first. Example response: [2, 0, 5, 1, 3, 4]";

        let mut user_prompt = format!("Query: {}\n\nCode chunks:\n", query);
        for (i, chunk) in chunks.iter().enumerate() {
            user_prompt.push_str(&format!("\n--- Chunk {} ---\n{}\n", i, chunk));
        }
        user_prompt.push_str("\nReturn ONLY the JSON array of indices, nothing else.");

        // Build the inner request
        let inner_request = InnerRequest {
            contents: vec![Content {
                role: "user".to_string(),
                parts: vec![Part { text: user_prompt }],
            }],
            system_instruction: Some(Content {
                role: "user".to_string(),
                parts: vec![Part {
                    text: system_prompt.to_string(),
                }],
            }),
        };

        // Wrap for Cloud Code Assist API
        let request_body = CodeAssistRequest {
            project: project_id,
            model: "gemini-2.0-flash".to_string(),
            request: inner_request,
        };

        let url = format!("{}/v1internal:generateContent", CODE_ASSIST_BASE);

        let res = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .header("User-Agent", "greppy/0.9.0")
            .header("X-Goog-Api-Client", "greppy/0.9.0")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| Error::DaemonError {
                message: format!("API request failed: {}", e),
            })?;

        if !res.status().is_success() {
            let text = res.text().await.unwrap_or_default();
            return Err(Error::DaemonError {
                message: format!("Gemini API Error: {}", text),
            });
        }

        // Cloud Code Assist wraps response in "response" field
        let wrapper: CodeAssistResponse = res.json().await.map_err(|e| Error::DaemonError {
            message: format!("Failed to parse response: {}", e),
        })?;

        // Parse the JSON array from response
        if let Some(response) = wrapper.response {
            if let Some(candidates) = response.candidates {
                if let Some(candidate) = candidates.first() {
                    if let Some(part) = candidate.content.parts.first() {
                        let text = part.text.trim();
                        // Try direct parse
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
        }

        // Fallback: return original order
        Ok((0..chunks.len()).collect())
    }
}
