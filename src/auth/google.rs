use crate::auth::server;
use anyhow::{Context, Result};
use oauth2::reqwest::async_http_client;
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    Scope, TokenResponse, TokenUrl,
};
use std::net::TcpListener;

// Gemini CLI OAuth credentials (from opencode-gemini-auth)
const GEMINI_CLIENT_ID: &str =
    "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com";
const GEMINI_CLIENT_SECRET: &str = "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";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

pub async fn authenticate() -> Result<String> {
    // Use fixed port 8085 to match Gemini CLI redirect URI
    let listener =
        TcpListener::bind("127.0.0.1:8085").or_else(|_| TcpListener::bind("127.0.0.1:0"))?;
    let port = listener.local_addr()?.port();
    let redirect_url = format!("http://localhost:{}/oauth2callback", port);

    let client = BasicClient::new(
        ClientId::new(GEMINI_CLIENT_ID.to_string()),
        Some(ClientSecret::new(GEMINI_CLIENT_SECRET.to_string())),
        AuthUrl::new(AUTH_URL.to_string())?,
        Some(TokenUrl::new(TOKEN_URL.to_string())?),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url)?);

    // Generate PKCE challenge
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate Auth URL with required scopes
    let (authorize_url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/cloud-platform".to_string(),
        ))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/userinfo.email".to_string(),
        ))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/userinfo.profile".to_string(),
        ))
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
        .set_pkce_challenge(pkce_challenge)
        .url();

    println!("Opening browser for Google authentication...");
    println!("If browser doesn't open, visit: {}", authorize_url);

    if let Err(e) = open::that(authorize_url.to_string()) {
        eprintln!(
            "Failed to open browser: {}. Please open the URL manually.",
            e
        );
    }

    // Start Server and wait for code
    let code = server::run_server(listener, csrf_state.secret().clone()).await?;

    // Exchange code for token
    let token_result = client
        .exchange_code(code)
        .set_pkce_verifier(pkce_verifier)
        .request_async(async_http_client)
        .await
        .context("Failed to exchange code for token")?;

    // Return refresh token if available, otherwise access token
    if let Some(refresh_token) = token_result.refresh_token() {
        Ok(refresh_token.secret().clone())
    } else {
        Ok(token_result.access_token().secret().clone())
    }
}
