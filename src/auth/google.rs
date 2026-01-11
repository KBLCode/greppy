use crate::auth::server;
use anyhow::{Context, Result};
use oauth2::reqwest::async_http_client;
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use std::net::TcpListener;

// Standard Google OAuth constants
// Note: For a real production app, we might want to use a specific Client ID
// or allow the user to provide one. For now, we'll use a placeholder or a known public one if available.
// The reference 'opencode-gemini-auth' uses the Gemini CLI client ID.
const GOOGLE_CLIENT_ID: &str =
    "947318989803-6bn6qk8qdgf4n4g3pfee6491hc0brc4i.apps.googleusercontent.com"; // Common public client ID for Google Cloud SDK
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

pub async fn authenticate() -> Result<String> {
    // 1. Setup Client
    let client_id = ClientId::new(GOOGLE_CLIENT_ID.to_string());
    let client_secret = None; // Public client, no secret
    let auth_url = AuthUrl::new(AUTH_URL.to_string())?;
    let token_url = TokenUrl::new(TOKEN_URL.to_string())?;

    // Bind to a random port to get the redirect URI
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let redirect_url = RedirectUrl::new(format!("http://127.0.0.1:{}/callback", port))?;

    let client = BasicClient::new(client_id, client_secret, auth_url, Some(token_url))
        .set_redirect_uri(redirect_url);

    // 2. Generate PKCE challenge
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // 3. Generate Auth URL
    let (authorize_url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/cloud-platform".to_string(),
        ))
        .set_pkce_challenge(pkce_challenge)
        .url();

    println!("Opening browser to: {}", authorize_url);
    open::that(authorize_url.to_string())?;

    // 4. Start Server and wait for code
    let code = server::run_server(listener, csrf_state.secret().clone()).await?;

    // 5. Exchange code for token
    let token_result = client
        .exchange_code(code)
        .set_pkce_verifier(pkce_verifier)
        .request_async(async_http_client)
        .await
        .context("Failed to exchange code for token")?;

    Ok(token_result.access_token().secret().clone())
}
