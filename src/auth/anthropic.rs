use anyhow::{Context, Result};
use oauth2::PkceCodeChallenge;
use reqwest::Client;
use serde::{Deserialize, Serialize};

// Claude OAuth constants (from opencode-anthropic-auth)
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const AUTH_URL: &str = "https://claude.ai/oauth/authorize";
const TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const REDIRECT_URI: &str = "https://console.anthropic.com/oauth/code/callback";

#[derive(Debug, Serialize)]
struct TokenRequest {
    code: String,
    state: String,
    grant_type: String,
    client_id: String,
    redirect_uri: String,
    code_verifier: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

pub async fn authenticate() -> Result<String> {
    // Generate PKCE challenge
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Build authorization URL
    let mut auth_url = url::Url::parse(AUTH_URL)?;
    auth_url
        .query_pairs_mut()
        .append_pair("code", "true")
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair(
            "scope",
            "org:create_api_key user:profile user:inference user:sessions:claude_code",
        )
        .append_pair("code_challenge", pkce_challenge.as_str())
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", pkce_verifier.secret());

    println!("Opening browser for Claude authentication...");
    println!("If browser doesn't open, visit:\n{}\n", auth_url);

    // Open browser
    if let Err(e) = open::that(auth_url.as_str()) {
        eprintln!(
            "Failed to open browser: {}. Please open the URL manually.",
            e
        );
    }

    // Prompt user to paste the code
    println!("After authorizing, you'll see a code in the browser.");
    println!("Copy the FULL code (including any # and text after it) and paste it here:\n");

    let code: String = dialoguer::Input::new()
        .with_prompt("Authorization code")
        .interact_text()
        .context("Failed to read authorization code")?;

    // Parse code - it may contain state after #
    let (auth_code, state) = if code.contains('#') {
        let parts: Vec<&str> = code.split('#').collect();
        (
            parts[0].to_string(),
            parts.get(1).unwrap_or(&"").to_string(),
        )
    } else {
        (code.clone(), pkce_verifier.secret().to_string())
    };

    // Exchange code for tokens
    let client = Client::new();
    let token_request = TokenRequest {
        code: auth_code,
        state,
        grant_type: "authorization_code".to_string(),
        client_id: CLIENT_ID.to_string(),
        redirect_uri: REDIRECT_URI.to_string(),
        code_verifier: pkce_verifier.secret().to_string(),
    };

    let response = client
        .post(TOKEN_URL)
        .header("Content-Type", "application/json")
        .json(&token_request)
        .send()
        .await
        .context("Failed to exchange code for token")?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Token exchange failed: {}", error_text);
    }

    let token_response: TokenResponse = response
        .json()
        .await
        .context("Failed to parse token response")?;

    // Return refresh token for storage
    Ok(token_response.refresh_token)
}
