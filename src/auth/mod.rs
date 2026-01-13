pub mod anthropic;
pub mod google;
pub mod server;
pub mod storage;

use crate::core::error::{Error, Result};
pub use storage::Provider;

/// Login with Google (Gemini) via OAuth
pub async fn login_google() -> Result<()> {
    println!("Initiating Google OAuth login...");
    let token = google::authenticate().await.map_err(Error::Auth)?;
    storage::save_token(Provider::Google, &token).map_err(Error::Auth)?;
    println!("Successfully logged in with Google!");
    Ok(())
}

/// Login with Anthropic (Claude) via OAuth
pub async fn login_anthropic() -> Result<()> {
    println!("Initiating Claude OAuth login...");
    let token = anthropic::authenticate().await.map_err(Error::Auth)?;
    storage::save_token(Provider::Anthropic, &token).map_err(Error::Auth)?;
    println!("Successfully logged in with Claude!");
    Ok(())
}

/// Legacy login function (defaults to Google OAuth)
pub async fn login() -> Result<()> {
    login_google().await
}

/// Logout from a specific provider
pub fn logout_provider(provider: Provider) -> Result<()> {
    storage::delete_token(provider).map_err(Error::Auth)?;
    println!("Logged out from {:?}.", provider);
    Ok(())
}

/// Logout from all providers
pub fn logout() -> Result<()> {
    storage::delete_all_tokens().map_err(Error::Auth)?;
    println!("Logged out from all providers.");
    Ok(())
}

/// Get token for a specific provider
pub fn get_token(provider: Provider) -> Result<String> {
    storage::load_token(provider).map_err(Error::Auth)
}

/// Get Google token (for Gemini)
pub fn get_google_token() -> Result<String> {
    get_token(Provider::Google)
}

/// Get Anthropic token (for Claude)
pub fn get_anthropic_token() -> Result<String> {
    get_token(Provider::Anthropic)
}

/// Check which providers are authenticated
pub fn get_authenticated_providers() -> Vec<Provider> {
    let mut providers = Vec::new();
    if storage::has_token(Provider::Google) {
        providers.push(Provider::Google);
    }
    if storage::has_token(Provider::Anthropic) {
        providers.push(Provider::Anthropic);
    }
    providers
}
