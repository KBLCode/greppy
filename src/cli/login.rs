//! Login command implementation

use crate::ai::ollama::OllamaClient;
use crate::auth::{self, Provider};
use crate::core::config::{AiProvider, Config};
use crate::core::error::Result;
use dialoguer::{theme::ColorfulTheme, Input, Select};

/// Run the login command - let user choose provider with arrow keys
pub async fn run() -> Result<()> {
    // Check if already logged in
    let providers = auth::get_authenticated_providers();
    let config = Config::load()?;
    let has_ollama = config.ai.provider == AiProvider::Ollama;

    // Show current config if any
    if !providers.is_empty() || has_ollama {
        println!("Current providers:");
        for p in &providers {
            match p {
                Provider::Anthropic => println!("  ✓ Claude (Anthropic)"),
                Provider::Google => println!("  ✓ Gemini (Google)"),
            }
        }
        if has_ollama {
            println!("  ✓ Ollama ({})", config.ai.ollama_model);
        }
        println!();
    }

    // Interactive selection - show what's available to add
    let mut options: Vec<&str> = Vec::new();

    // Always show Claude option
    if providers.contains(&Provider::Anthropic) {
        options.push("Claude (Anthropic) ✓ configured");
    } else {
        options.push("Claude (Anthropic) - OAuth, free tier");
    }

    // Always show Gemini option
    if providers.contains(&Provider::Google) {
        options.push("Gemini (Google) ✓ configured");
    } else {
        options.push("Gemini (Google) - OAuth, free tier");
    }

    // Always show Ollama option
    let ollama_label = if has_ollama {
        format!("Ollama ({}) ✓ configured", config.ai.ollama_model)
    } else {
        "Ollama (Local) - No internet, runs on your machine".to_string()
    };
    options.push(&ollama_label);

    options.push("Cancel");

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Add/configure AI provider")
        .items(&options)
        .default(0)
        .interact()
        .map_err(|e| crate::core::error::Error::DaemonError {
            message: format!("Selection failed: {}", e),
        })?;

    match selection {
        0 => {
            // Claude
            println!("\nAuthenticating with Claude...\n");
            auth::login_anthropic().await?;
            // Set as active provider
            let mut config = Config::load()?;
            config.ai.provider = AiProvider::Claude;
            config.save()?;
        }
        1 => {
            // Gemini
            println!("\nAuthenticating with Google...\n");
            auth::login_google().await?;
            let mut config = Config::load()?;
            config.ai.provider = AiProvider::Gemini;
            config.save()?;
        }
        2 => {
            // Ollama
            setup_ollama().await?;
        }
        3 => {
            // Cancel
            println!("Cancelled.");
            return Ok(());
        }
        _ => unreachable!(),
    }

    println!("\nYou can now use semantic search:");
    println!("  greppy search \"your query\"");
    println!("\nSwitch models anytime with: greppy model");

    Ok(())
}

/// Setup Ollama local model
async fn setup_ollama() -> Result<()> {
    println!("\n Setting up Ollama (local AI)...\n");

    // Check if Ollama is running
    let client = OllamaClient::new();

    print!("Checking Ollama connection... ");
    if !client.is_available().await {
        println!("NOT FOUND\n");
        println!("Ollama is not running. Please:");
        println!("  1. Install Ollama: https://ollama.ai");
        println!("  2. Start Ollama: ollama serve");
        println!("  3. Pull a model: ollama pull qwen2.5-coder:0.5b");
        println!("  4. Run 'greppy login' again");
        return Err(crate::core::error::Error::ConfigError {
            message: "Ollama not available".to_string(),
        });
    }
    println!("OK\n");

    // List available models
    let models = client.list_models().await.unwrap_or_default();

    if models.is_empty() {
        println!("No models found. Please pull a model first:");
        println!("  ollama pull qwen2.5-coder:0.5b   # Small, fast (400MB)");
        println!("  ollama pull codellama:7b         # Better quality (4GB)");
        println!("  ollama pull deepseek-coder:6.7b  # Code-focused (4GB)");
        return Err(crate::core::error::Error::ConfigError {
            message: "No Ollama models available".to_string(),
        });
    }

    println!("Available models:");
    let model_names: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select model for AI reranking")
        .items(&model_names)
        .default(0)
        .interact()
        .map_err(|e| crate::core::error::Error::DaemonError {
            message: format!("Selection failed: {}", e),
        })?;

    let selected_model = model_names[selection].to_string();

    // Ask for custom URL (advanced)
    let url: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Ollama URL")
        .default("http://localhost:11434".to_string())
        .interact_text()
        .map_err(|e| crate::core::error::Error::DaemonError {
            message: format!("Input failed: {}", e),
        })?;

    // Test the model
    print!("\nTesting model '{}'... ", selected_model);
    let test_client = OllamaClient::with_config(&url, &selected_model);

    match test_client
        .rerank(
            "test query",
            &["code snippet 1".to_string(), "code snippet 2".to_string()],
        )
        .await
    {
        Ok(_) => println!("OK"),
        Err(e) => {
            println!("FAILED\n");
            println!("Error: {}", e);
            println!("\nThe model may need to be loaded first. Try:");
            println!("  ollama run {}", selected_model);
            return Err(e);
        }
    }

    // Save to config
    let mut config = Config::load()?;
    config.ai.provider = AiProvider::Ollama;
    config.ai.ollama_model = selected_model.clone();
    config.ai.ollama_url = url;
    config.save()?;

    println!("\n Ollama configured successfully!");
    println!("  Model: {}", selected_model);

    Ok(())
}

/// Run the logout command - remove all stored credentials
pub fn logout() -> Result<()> {
    let providers = auth::get_authenticated_providers();
    let config = Config::load()?;
    let has_ollama = config.ai.provider == AiProvider::Ollama;

    if providers.is_empty() && !has_ollama {
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
    if has_ollama {
        println!("  - Ollama (local)");
    }

    // Clear OAuth tokens
    auth::logout()?;

    // Clear Ollama config
    if has_ollama {
        let mut config = Config::load()?;
        config.ai.provider = AiProvider::Claude; // Reset to default
        config.save()?;
    }

    println!("\nSuccessfully logged out.");
    Ok(())
}

/// Check if user is authenticated with any provider (including Ollama)
pub fn is_authenticated() -> bool {
    if !auth::get_authenticated_providers().is_empty() {
        return true;
    }
    // Check if Ollama is configured
    if let Ok(config) = Config::load() {
        if config.ai.provider == AiProvider::Ollama {
            return true;
        }
    }
    false
}

/// Get the preferred provider (Anthropic > Google > Ollama)
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

/// Check if Ollama is configured as the provider
pub fn is_ollama_configured() -> bool {
    Config::load()
        .map(|c| c.ai.provider == AiProvider::Ollama)
        .unwrap_or(false)
}

/// Get Ollama client if configured
pub fn get_ollama_client() -> Option<OllamaClient> {
    let config = Config::load().ok()?;
    if config.ai.provider == AiProvider::Ollama {
        Some(OllamaClient::with_config(
            &config.ai.ollama_url,
            &config.ai.ollama_model,
        ))
    } else {
        None
    }
}
