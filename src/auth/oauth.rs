//! OAuth 2.0 PKCE flow implementation for Anthropic Claude
//!
//! Implements the authorization code flow with PKCE (Proof Key for Code Exchange)
//! for secure authentication without client secrets.

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use super::storage::{self, OAuthTokens};

/// Anthropic OAuth client ID (public, used by CLI tools)
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

/// OAuth authorization endpoint
const AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";

/// OAuth token endpoint
const TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";

/// OAuth callback redirect URI
const REDIRECT_URI: &str = "https://console.anthropic.com/oauth/code/callback";

/// OAuth scopes required for API access
const SCOPES: &str = "org:create_api_key user:profile user:inference";

/// PKCE verifier and challenge pair
#[derive(Debug)]
pub struct PkceChallenge {
    /// The verifier string (random, kept secret)
    pub verifier: String,
    /// The challenge (SHA256 hash of verifier, sent to server)
    pub challenge: String,
}

/// Generate a PKCE challenge pair
fn generate_pkce() -> PkceChallenge {
    // Generate 32 random bytes for the verifier
    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    
    // Base64url encode the verifier
    let verifier = URL_SAFE_NO_PAD.encode(&random_bytes);
    
    // SHA256 hash the verifier and base64url encode for challenge
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    let challenge = URL_SAFE_NO_PAD.encode(hash);
    
    PkceChallenge { verifier, challenge }
}

/// Authorization result containing URL and verifier
#[derive(Debug)]
pub struct AuthorizationRequest {
    /// URL to open in browser for user authorization
    pub url: String,
    /// PKCE verifier to use when exchanging the code
    pub verifier: String,
}

/// Start the OAuth authorization flow
///
/// Returns a URL to open in the browser and a verifier to use when exchanging the code.
pub fn authorize() -> AuthorizationRequest {
    let pkce = generate_pkce();
    
    let mut url = url::Url::parse(AUTHORIZE_URL).unwrap();
    url.query_pairs_mut()
        .append_pair("code", "true")
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("scope", SCOPES)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &pkce.verifier);
    
    debug!("Generated authorization URL");
    
    AuthorizationRequest {
        url: url.to_string(),
        verifier: pkce.verifier,
    }
}

/// Token response from the OAuth server
#[derive(Debug, serde::Deserialize)]
struct TokenResponse {
    refresh_token: String,
    access_token: String,
    expires_in: u64,
}

/// Exchange an authorization code for tokens
///
/// The code format is "code#state" as returned by the callback page.
pub async fn exchange(code: &str, verifier: &str) -> Result<OAuthTokens> {
    // Split code and state
    let parts: Vec<&str> = code.split('#').collect();
    let auth_code = parts.first().ok_or_else(|| anyhow!("Invalid code format"))?;
    let state = parts.get(1);
    
    let client = reqwest::Client::new();
    
    let mut body = serde_json::json!({
        "code": auth_code,
        "grant_type": "authorization_code",
        "client_id": CLIENT_ID,
        "redirect_uri": REDIRECT_URI,
        "code_verifier": verifier,
    });
    
    if let Some(s) = state {
        body["state"] = serde_json::json!(s);
    }
    
    debug!("Exchanging authorization code for tokens");
    
    let response = client
        .post(TOKEN_URL)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("Failed to send token request")?;
    
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!("Token exchange failed: {} - {}", status, text));
    }
    
    let token_response: TokenResponse = response
        .json()
        .await
        .context("Failed to parse token response")?;
    
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    
    let tokens = OAuthTokens {
        token_type: "oauth".to_string(),
        refresh: token_response.refresh_token,
        access: token_response.access_token,
        expires: now + (token_response.expires_in * 1000),
    };
    
    info!("Successfully exchanged code for tokens");
    Ok(tokens)
}

/// Refresh an expired access token
pub async fn refresh(refresh_token: &str) -> Result<OAuthTokens> {
    let client = reqwest::Client::new();
    
    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "refresh_token": refresh_token,
        "client_id": CLIENT_ID,
    });
    
    debug!("Refreshing access token");
    
    let response = client
        .post(TOKEN_URL)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("Failed to send refresh request")?;
    
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!("Token refresh failed: {} - {}", status, text));
    }
    
    let token_response: TokenResponse = response
        .json()
        .await
        .context("Failed to parse refresh response")?;
    
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    
    let tokens = OAuthTokens {
        token_type: "oauth".to_string(),
        refresh: token_response.refresh_token,
        access: token_response.access_token,
        expires: now + (token_response.expires_in * 1000),
    };
    
    info!("Successfully refreshed access token");
    Ok(tokens)
}

/// Get a valid access token, refreshing if necessary
///
/// Returns None if not authenticated or refresh fails.
pub async fn get_access_token() -> Option<String> {
    let tokens = match storage::load_tokens() {
        Ok(Some(t)) => t,
        Ok(None) => {
            debug!("No stored tokens found");
            return None;
        }
        Err(e) => {
            warn!("Failed to load tokens: {}", e);
            return None;
        }
    };
    
    // Check if token is still valid
    if !tokens.is_expired() {
        return Some(tokens.access);
    }
    
    // Try to refresh
    debug!("Access token expired, attempting refresh");
    match refresh(&tokens.refresh).await {
        Ok(new_tokens) => {
            if let Err(e) = storage::store_tokens(&new_tokens) {
                warn!("Failed to store refreshed tokens: {}", e);
            }
            Some(new_tokens.access)
        }
        Err(e) => {
            warn!("Failed to refresh token: {}", e);
            // Clear invalid tokens
            if let Err(e) = storage::clear_tokens() {
                warn!("Failed to clear invalid tokens: {}", e);
            }
            None
        }
    }
}

/// Check if the user is authenticated
pub async fn is_authenticated() -> bool {
    get_access_token().await.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pkce_generation() {
        let pkce = generate_pkce();
        
        // Verifier should be base64url encoded (43 chars for 32 bytes)
        assert!(!pkce.verifier.is_empty());
        assert!(pkce.verifier.len() >= 40);
        
        // Challenge should be base64url encoded SHA256 (43 chars)
        assert!(!pkce.challenge.is_empty());
        assert_eq!(pkce.challenge.len(), 43);
        
        // Verify the challenge is correct
        let mut hasher = Sha256::new();
        hasher.update(pkce.verifier.as_bytes());
        let hash = hasher.finalize();
        let expected_challenge = URL_SAFE_NO_PAD.encode(hash);
        assert_eq!(pkce.challenge, expected_challenge);
    }
    
    #[test]
    fn test_authorize_url() {
        let auth = authorize();
        
        assert!(auth.url.starts_with(AUTHORIZE_URL));
        assert!(auth.url.contains("client_id="));
        assert!(auth.url.contains("code_challenge="));
        assert!(auth.url.contains("code_challenge_method=S256"));
        assert!(!auth.verifier.is_empty());
    }
}
