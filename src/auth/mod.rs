pub mod google;
pub mod server;
pub mod storage;

use crate::core::error::{Error, Result};

pub async fn login() -> Result<()> {
    println!("Initiating Google OAuth login...");
    let token = google::authenticate().await.map_err(Error::Auth)?;
    storage::save_token(&token).map_err(Error::Auth)?;
    println!("Successfully logged in!");
    Ok(())
}

pub fn logout() -> Result<()> {
    storage::delete_token().map_err(Error::Auth)?;
    println!("Logged out.");
    Ok(())
}

pub fn get_token() -> Result<String> {
    storage::load_token().map_err(Error::Auth)
}
