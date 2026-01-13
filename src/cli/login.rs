//! Login command implementation

use crate::auth::{self, Provider};
use crate::core::error::Result;
use dialoguer::{theme::ColorfulTheme, Select};

/// Run the login command - let user choose provider with arrow keys
pub async fn run() -> Result<()> {
    // Check if already logged in
    let providers = auth::get_authenticated_providers();

    if !providers.is_empty() {
        println!("Already logged in with:");
        for p in &providers {
            match p {
                Provider::Anthropic => println!("  - Claude (Anthropic)"),
                Provider::Google => println!("  - Gemini (Google)"),
            }
        }
        println!("\nTo switch providers, run 'greppy logout' first.");
        return Ok(());
    }

    // Interactive selection - Claude first (most common)
    let options = &["Claude (Anthropic) - OAuth", "Gemini (Google) - OAuth"];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose AI provider")
        .items(options)
        .default(0)
        .interact()
        .map_err(|e| crate::core::error::Error::DaemonError {
            message: format!("Selection failed: {}", e),
        })?;

    match selection {
        0 => {
            println!("\nAuthenticating with Claude...\n");
            auth::login_anthropic().await?;
        }
        1 => {
            println!("\nAuthenticating with Google...\n");
            auth::login_google().await?;
        }
        _ => unreachable!(),
    }

    println!("\nYou can now use semantic search:");
    println!("  greppy search \"your query\"");

    Ok(())
}

/// Run the logout command - remove all stored credentials
pub fn logout() -> Result<()> {
    let providers = auth::get_authenticated_providers();

    if providers.is_empty() {
        println!("Not logged in to any provider.");
        return Ok(());
    }

    println!("Logging out from:");
    for p in &providers {
        match p {
            Provider::Anthropic => println!("  - Claude (Anthropic)"),
            Provider::Google => println!("  - Gemini (Google)"),
        }
    }

    auth::logout()?;
    println!("\nSuccessfully logged out.");
    Ok(())
}

/// Check if user is authenticated with any provider
pub fn is_authenticated() -> bool {
    !auth::get_authenticated_providers().is_empty()
}

/// Get the preferred provider (Anthropic > Google)
pub fn get_preferred_provider() -> Option<Provider> {
    let providers = auth::get_authenticated_providers();
    if providers.contains(&Provider::Anthropic) {
        Some(Provider::Anthropic)
    } else if providers.contains(&Provider::Google) {
        Some(Provider::Google)
    } else {
        None
    }
}
