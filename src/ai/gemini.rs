use crate::core::error::{Error, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

// const GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent";

#[derive(Debug, Serialize)]
struct GenerateContentRequest {
    contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<Content>,
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

#[derive(Debug, Deserialize)]
struct GenerateContentResponse {
    candidates: Option<Vec<Candidate>>,
    #[allow(dead_code)]
    error: Option<ErrorResponse>,
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
struct ErrorResponse {
    #[allow(dead_code)]
    message: String,
}

pub struct GeminiClient {
    client: Client,
    token: String,
}

impl GeminiClient {
    pub fn new(token: String) -> Self {
        Self {
            client: Client::new(),
            token,
        }
    }

    pub async fn ask(&self, question: &str, context: &str) -> Result<String> {
        let system_prompt = "You are Greppy, an expert AI code assistant. \
            You are given a user question and some relevant code snippets from the codebase. \
            Answer the question based ONLY on the provided context. \
            If the context doesn't contain the answer, say so. \
            Be concise and technical.";

        let user_prompt = format!("Context:\n{}\n\nQuestion: {}", context, question);

        let request_body = GenerateContentRequest {
            contents: vec![Content {
                role: "user".to_string(),
                parts: vec![Part { text: user_prompt }],
            }],
            system_instruction: Some(Content {
                role: "user".to_string(), // Gemini uses 'user' or 'model', system instructions are passed differently in v1beta but this structure is often accepted or we use a specific field.
                // Actually, for gemini-1.5-flash, system_instruction is a top-level field.
                parts: vec![Part {
                    text: system_prompt.to_string(),
                }],
            }),
        };

        // Note: Google Cloud Vertex AI vs AI Studio.
        // If using OAuth token from gcloud/user login, we typically hit Vertex AI endpoints:
        // `https://us-central1-aiplatform.googleapis.com/v1/projects/{PROJECT_ID}/locations/us-central1/publishers/google/models/gemini-1.5-flash:generateContent`
        // The `generativelanguage.googleapis.com` is usually for API Keys.
        // Since we implemented OAuth with `https://www.googleapis.com/auth/cloud-platform`, we should use Vertex AI.
        // However, that requires a PROJECT_ID.
        // We don't have a project ID in the config yet.
        //
        // Alternative: Use the generic `generativelanguage` API with the OAuth token?
        // Often `generativelanguage` accepts OAuth tokens if the user has enabled the API in their project.
        // But usually it expects an API Key `?key=...`.
        //
        // Let's try to use the Vertex AI endpoint if we can get the project ID, or ask the user for it.
        // Or, we can try `generativelanguage` with `Authorization: Bearer TOKEN`.
        //
        // Let's assume for now we might need to ask the user for a Project ID in `greppy.toml` or CLI arg.
        // But to make it "just work" like `opencode`, maybe we can infer it or use a default?
        // `opencode` uses `https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent` with an API Key usually.
        // Wait, our `auth/google.rs` gets an OAuth token.
        // If we use OAuth, we MUST use Vertex AI or have the user provide an API Key instead of OAuth.
        //
        // Let's check `docs/0.4.0/USER_GUIDE.md`. It mentions `project_id` in config.
        // So we should load it from config.

        // For this implementation, I'll assume we need a Project ID for Vertex AI if we use OAuth.
        // If no Project ID, maybe we fail or try a default?
        // Let's stick to the plan: Use OAuth.
        // We need to get the Project ID.
        //
        // Let's look at `src/core/config.rs` again to see if we added `ai` section.
        // We haven't. We need to add it.

        // Temporary: I will use a placeholder URL logic that tries to use the config.
        // But first, let's write this client assuming we can get the URL.

        let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent";

        // NOTE: With OAuth token, `generativelanguage` might reject if not using API Key.
        // But let's try sending the token as Bearer.

        let res = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            //.header("x-goog-user-project", "...") // Might be needed
            .json(&request_body)
            .send()
            .await
            .map_err(|e| Error::DaemonError {
                message: e.to_string(),
            })?; // Reusing DaemonError for now or add AIError

        if !res.status().is_success() {
            let text = res.text().await.unwrap_or_default();
            return Err(Error::DaemonError {
                message: format!("API Error: {}", text),
            });
        }

        let response: GenerateContentResponse =
            res.json().await.map_err(|e| Error::DaemonError {
                message: e.to_string(),
            })?;

        if let Some(candidates) = response.candidates {
            if let Some(candidate) = candidates.first() {
                if let Some(part) = candidate.content.parts.first() {
                    return Ok(part.text.clone());
                }
            }
        }

        Ok("No response generated.".to_string())
    }
}
