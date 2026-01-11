use crate::auth;
use crate::core::error::Result;
use clap::Parser;

/// Arguments for the logout command
#[derive(Parser, Debug)]
pub struct LogoutArgs {}

pub fn run(_args: LogoutArgs) -> Result<()> {
    auth::logout()
}
