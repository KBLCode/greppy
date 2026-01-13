use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE_NAME: &str = "greppy";

/// Provider-specific user names for token storage
const GOOGLE_USER: &str = "google_oauth_token";
const ANTHROPIC_USER: &str = "anthropic_oauth_token";
// Legacy key for backwards compatibility
const LEGACY_USER: &str = "oauth_token";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Google,
    Anthropic,
}

impl Provider {
    fn storage_key(&self) -> &'static str {
        match self {
            Provider::Google => GOOGLE_USER,
            Provider::Anthropic => ANTHROPIC_USER,
        }
    }
}

pub fn save_token(provider: Provider, token: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, provider.storage_key())?;
    entry.set_password(token)?;
    Ok(())
}

pub fn load_token(provider: Provider) -> Result<String> {
    let entry = Entry::new(SERVICE_NAME, provider.storage_key())?;
    entry.get_password().context(format!(
        "No {:?} auth token found. Please run 'greppy login'.",
        provider
    ))
}

pub fn delete_token(provider: Provider) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, provider.storage_key())?;
    match entry.delete_password() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
        Err(e) => Err(e.into()),
    }
}

/// Check if a token exists for a provider
pub fn has_token(provider: Provider) -> bool {
    load_token(provider).is_ok()
}

/// Delete all tokens (logout from all providers)
pub fn delete_all_tokens() -> Result<()> {
    let _ = delete_token(Provider::Google);
    let _ = delete_token(Provider::Anthropic);
    // Also try to delete legacy token
    if let Ok(entry) = Entry::new(SERVICE_NAME, LEGACY_USER) {
        let _ = entry.delete_password();
    }
    Ok(())
}

// Legacy functions for backwards compatibility
pub fn save_token_legacy(token: &str) -> Result<()> {
    save_token(Provider::Google, token)
}

pub fn load_token_legacy() -> Result<String> {
    // Try Google first, then legacy key
    load_token(Provider::Google).or_else(|_| {
        let entry = Entry::new(SERVICE_NAME, LEGACY_USER)?;
        entry
            .get_password()
            .context("No auth token found. Please run 'greppy login'.")
    })
}

pub fn delete_token_legacy() -> Result<()> {
    delete_token(Provider::Google)
}
