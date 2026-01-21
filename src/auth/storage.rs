//! Token storage using config file (no keychain)

use crate::core::config::Config;
use anyhow::{Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Google,
    Anthropic,
}

pub fn save_token(provider: Provider, token: &str) -> Result<()> {
    let mut config = Config::load().unwrap_or_default();
    match provider {
        Provider::Google => config.ai.google_token = Some(token.to_string()),
        Provider::Anthropic => config.ai.anthropic_token = Some(token.to_string()),
    }
    config.save().context("Failed to save config")?;
    Ok(())
}

pub fn load_token(provider: Provider) -> Result<String> {
    let config = Config::load().unwrap_or_default();
    match provider {
        Provider::Google => config
            .ai
            .google_token
            .context("No Google auth token found. Please run 'greppy login'."),
        Provider::Anthropic => config
            .ai
            .anthropic_token
            .context("No Anthropic auth token found. Please run 'greppy login'."),
    }
}

pub fn delete_token(provider: Provider) -> Result<()> {
    let mut config = Config::load().unwrap_or_default();
    match provider {
        Provider::Google => config.ai.google_token = None,
        Provider::Anthropic => config.ai.anthropic_token = None,
    }
    config.save().context("Failed to save config")?;
    Ok(())
}

/// Check if a token exists for a provider
pub fn has_token(provider: Provider) -> bool {
    load_token(provider).is_ok()
}

/// Delete all tokens (logout from all providers)
pub fn delete_all_tokens() -> Result<()> {
    let mut config = Config::load().unwrap_or_default();
    config.ai.google_token = None;
    config.ai.anthropic_token = None;
    config.save().context("Failed to save config")?;
    Ok(())
}

// Legacy functions for backwards compatibility
pub fn save_token_legacy(token: &str) -> Result<()> {
    save_token(Provider::Google, token)
}

pub fn load_token_legacy() -> Result<String> {
    load_token(Provider::Google)
}

pub fn delete_token_legacy() -> Result<()> {
    delete_token(Provider::Google)
}
