use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE_NAME: &str = "greppy";
const USER_NAME: &str = "oauth_token";

pub fn save_token(token: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, USER_NAME)?;
    entry.set_password(token)?;
    Ok(())
}

pub fn load_token() -> Result<String> {
    let entry = Entry::new(SERVICE_NAME, USER_NAME)?;
    entry
        .get_password()
        .context("No auth token found. Please run 'greppy login'.")
}

pub fn delete_token() -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, USER_NAME)?;
    match entry.delete_password() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
        Err(e) => Err(e.into()),
    }
}
