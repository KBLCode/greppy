//! Token storage for OAuth credentials
//!
//! Stores OAuth tokens securely in ~/.config/greppy/auth.json with
//! restricted file permissions (0600).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, warn};

/// OAuth token data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    /// Token type identifier
    #[serde(rename = "type")]
    pub token_type: String,
    /// Refresh token for obtaining new access tokens
    pub refresh: String,
    /// Current access token
    pub access: String,
    /// Expiration timestamp in milliseconds since epoch
    pub expires: u64,
}

impl OAuthTokens {
    /// Check if the token is expired (with 5 minute buffer)
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Add 5 minute buffer
        self.expires < now + (5 * 60 * 1000)
    }
}

/// Auth data stored in auth.json
#[derive(Debug, Default, Serialize, Deserialize)]
struct AuthData {
    #[serde(skip_serializing_if = "Option::is_none")]
    anthropic: Option<OAuthTokens>,
}

/// Get the greppy config directory path
fn get_config_dir() -> PathBuf {
    // Use XDG_CONFIG_HOME if set, otherwise ~/.config/greppy
    if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg_config).join("greppy")
    } else if let Some(home) = dirs::home_dir() {
        home.join(".config").join("greppy")
    } else {
        // Fallback to current directory
        PathBuf::from(".greppy")
    }
}

/// Get the auth file path
fn get_auth_file_path() -> PathBuf {
    get_config_dir().join("auth.json")
}

/// Load OAuth tokens from storage
pub fn load_tokens() -> Result<Option<OAuthTokens>> {
    let path = get_auth_file_path();

    if !path.exists() {
        debug!("Auth file does not exist: {:?}", path);
        return Ok(None);
    }

    let content = fs::read_to_string(&path).context("Failed to read auth file")?;

    let data: AuthData = serde_json::from_str(&content).context("Failed to parse auth file")?;

    match &data.anthropic {
        Some(tokens) if tokens.token_type == "oauth" => {
            debug!("Loaded OAuth tokens from {:?}", path);
            Ok(Some(tokens.clone()))
        }
        Some(_) => {
            warn!("Invalid token type in auth file");
            Ok(None)
        }
        None => {
            debug!("No Anthropic tokens in auth file");
            Ok(None)
        }
    }
}

/// Store OAuth tokens to disk
pub fn store_tokens(tokens: &OAuthTokens) -> Result<()> {
    let path = get_auth_file_path();
    let dir = path.parent().unwrap();

    // Ensure directory exists
    fs::create_dir_all(dir).context("Failed to create config directory")?;

    // Load existing data or create new
    let mut data = if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        AuthData::default()
    };

    // Update tokens
    data.anthropic = Some(tokens.clone());

    // Serialize
    let content = serde_json::to_string_pretty(&data).context("Failed to serialize auth data")?;

    // Write with secure permissions
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .context("Failed to create auth file")?;

        file.write_all(content.as_bytes())
            .context("Failed to write auth file")?;
    }

    #[cfg(not(unix))]
    {
        fs::write(&path, content).context("Failed to write auth file")?;
    }

    debug!("Stored OAuth tokens to {:?}", path);
    Ok(())
}

/// Clear stored OAuth tokens
pub fn clear_tokens() -> Result<()> {
    let path = get_auth_file_path();

    if !path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&path).unwrap_or_default();
    let mut data: AuthData = serde_json::from_str(&content).unwrap_or_default();

    data.anthropic = None;

    let content = serde_json::to_string_pretty(&data).context("Failed to serialize auth data")?;

    fs::write(&path, content).context("Failed to write auth file")?;

    debug!("Cleared OAuth tokens from {:?}", path);
    Ok(())
}

/// Get the path to the auth file (for display purposes)
#[allow(dead_code)]
pub fn get_auth_path() -> PathBuf {
    get_auth_file_path()
}

// Add dirs crate for home_dir
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_expiry() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Token that expires in 10 minutes - not expired
        let valid_token = OAuthTokens {
            token_type: "oauth".to_string(),
            refresh: "refresh".to_string(),
            access: "access".to_string(),
            expires: now + (10 * 60 * 1000),
        };
        assert!(!valid_token.is_expired());

        // Token that expires in 2 minutes - expired (within 5 min buffer)
        let expiring_token = OAuthTokens {
            token_type: "oauth".to_string(),
            refresh: "refresh".to_string(),
            access: "access".to_string(),
            expires: now + (2 * 60 * 1000),
        };
        assert!(expiring_token.is_expired());

        // Token that already expired
        let expired_token = OAuthTokens {
            token_type: "oauth".to_string(),
            refresh: "refresh".to_string(),
            access: "access".to_string(),
            expires: now - 1000,
        };
        assert!(expired_token.is_expired());
    }
}
