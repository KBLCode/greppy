use crate::auth;
use crate::core::error::Result;
use clap::Parser;

/// Arguments for the login command
#[derive(Parser, Debug)]
pub struct LoginArgs {}

pub async fn run(_args: LoginArgs) -> Result<()> {
    auth::login().await
}
