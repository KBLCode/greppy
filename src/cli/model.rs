//! Interactive AI model switcher

use crate::ai::ollama::OllamaClient;
use crate::core::config::{AiProfile, AiProvider, Config};
use crate::core::error::{Error, Result};
use dialoguer::{theme::ColorfulTheme, Input, Select};

/// Run the interactive model switcher
pub async fn run() -> Result<()> {
    let mut config = Config::load()?;

    // Show current model
    println!();
    println!(" Current: {}", format_current(&config));
    println!();

    // Build menu options (only selectable items)
    let mut options = Vec::new();
    let mut actions = Vec::new();

    // === Saved Profiles ===
    let mut profile_names: Vec<_> = config.ai.profiles.keys().cloned().collect();
    profile_names.sort();

    for name in &profile_names {
        if let Some(profile) = config.ai.profiles.get(name) {
            let label = format_profile_label(name, profile);
            let is_active = is_profile_active(&config, name);
            if is_active {
                options.push(format!("{} ✓", label));
            } else {
                options.push(label);
            }
            actions.push(Action::SwitchProfile(name.clone()));
        }
    }

    // === Cloud Providers ===
    let has_claude = config.ai.anthropic_token.is_some();
    let has_gemini = config.ai.google_token.is_some();

    if has_claude {
        let active = config.ai.provider == AiProvider::Claude;
        let label = "Claude (Anthropic)".to_string();
        if active {
            options.push(format!("{} ✓", label));
        } else {
            options.push(label);
        }
        actions.push(Action::SwitchProvider(AiProvider::Claude));
    }

    if has_gemini {
        let active = config.ai.provider == AiProvider::Gemini;
        let label = "Gemini (Google)".to_string();
        if active {
            options.push(format!("{} ✓", label));
        } else {
            options.push(label);
        }
        actions.push(Action::SwitchProvider(AiProvider::Gemini));
    }

    // === Ollama Models ===
    let ollama_client = OllamaClient::new();
    let ollama_available = ollama_client.is_available().await;

    if ollama_available {
        if let Ok(models) = ollama_client.list_models().await {
            for model in models {
                let is_active = config.ai.provider == AiProvider::Ollama
                    && config.ai.ollama_model == model.name;
                let label = format!("Ollama: {}", model.name);
                if is_active {
                    options.push(format!("{} ✓", label));
                } else {
                    options.push(label);
                }
                actions.push(Action::SwitchOllama(model.name));
            }
        }
    }

    // === Actions ===
    options.push("+ Add account (login)".to_string());
    actions.push(Action::AddAccount);

    options.push("+ Save as profile".to_string());
    actions.push(Action::SaveProfile);

    if !profile_names.is_empty() {
        options.push("- Delete profile".to_string());
        actions.push(Action::DeleteProfile);
    }

    // Show selection menu
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Switch to")
        .items(&options)
        .default(0)
        .interact()
        .map_err(|e| Error::DaemonError {
            message: format!("Selection failed: {}", e),
        })?;

    // Handle selection
    match &actions[selection] {
        Action::SwitchProfile(name) => {
            if let Some(profile) = config.ai.profiles.get(name).cloned() {
                apply_profile(&mut config, &profile);
                config.save()?;
                println!("\n✓ Switched to '{}'", name);
            }
        }
        Action::SwitchProvider(provider) => {
            config.ai.provider = provider.clone();
            config.save()?;
            println!("\n✓ Switched to {}", format_provider_name(provider));
        }
        Action::SwitchOllama(model) => {
            config.ai.provider = AiProvider::Ollama;
            config.ai.ollama_model = model.clone();
            config.save()?;
            println!("\n✓ Switched to Ollama '{}'", model);
        }
        Action::SaveProfile => {
            save_current_as_profile(&mut config).await?;
        }
        Action::AddAccount => {
            // Run login flow
            drop(config);
            crate::cli::login::run().await?;
        }
        Action::DeleteProfile => {
            delete_profile(&mut config).await?;
        }
    }

    Ok(())
}

#[derive(Debug)]
enum Action {
    SwitchProfile(String),
    SwitchProvider(AiProvider),
    SwitchOllama(String),
    SaveProfile,
    AddAccount,
    DeleteProfile,
}

fn format_current(config: &Config) -> String {
    match config.ai.provider {
        AiProvider::Claude => "Claude (Anthropic)".to_string(),
        AiProvider::Gemini => "Gemini (Google)".to_string(),
        AiProvider::Ollama => format!("Ollama ({})", config.ai.ollama_model),
    }
}

fn format_provider_name(provider: &AiProvider) -> &'static str {
    match provider {
        AiProvider::Claude => "Claude (Anthropic)",
        AiProvider::Gemini => "Gemini (Google)",
        AiProvider::Ollama => "Ollama",
    }
}

fn format_profile_label(name: &str, profile: &AiProfile) -> String {
    match profile.provider {
        AiProvider::Claude => format!("[{}] Claude", name),
        AiProvider::Gemini => format!("[{}] Gemini", name),
        AiProvider::Ollama => {
            let model = profile.ollama_model.as_deref().unwrap_or("default");
            format!("[{}] Ollama: {}", name, model)
        }
    }
}

fn is_profile_active(config: &Config, name: &str) -> bool {
    if let Some(profile) = config.ai.profiles.get(name) {
        if config.ai.provider != profile.provider {
            return false;
        }
        match config.ai.provider {
            AiProvider::Ollama => profile.ollama_model.as_deref() == Some(&config.ai.ollama_model),
            AiProvider::Claude => {
                profile.anthropic_token.is_some()
                    && profile.anthropic_token == config.ai.anthropic_token
            }
            AiProvider::Gemini => {
                profile.google_token.is_some() && profile.google_token == config.ai.google_token
            }
        }
    } else {
        false
    }
}

fn apply_profile(config: &mut Config, profile: &AiProfile) {
    config.ai.provider = profile.provider.clone();

    if let Some(model) = &profile.ollama_model {
        config.ai.ollama_model = model.clone();
    }
    if let Some(url) = &profile.ollama_url {
        config.ai.ollama_url = url.clone();
    }
    if let Some(token) = &profile.google_token {
        config.ai.google_token = Some(token.clone());
    }
    if let Some(token) = &profile.anthropic_token {
        config.ai.anthropic_token = Some(token.clone());
    }
}

async fn save_current_as_profile(config: &mut Config) -> Result<()> {
    let name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Profile name")
        .interact_text()
        .map_err(|e| Error::DaemonError {
            message: format!("Input failed: {}", e),
        })?;

    let profile = AiProfile {
        provider: config.ai.provider.clone(),
        ollama_model: if config.ai.provider == AiProvider::Ollama {
            Some(config.ai.ollama_model.clone())
        } else {
            None
        },
        ollama_url: if config.ai.provider == AiProvider::Ollama {
            Some(config.ai.ollama_url.clone())
        } else {
            None
        },
        google_token: if config.ai.provider == AiProvider::Gemini {
            config.ai.google_token.clone()
        } else {
            None
        },
        anthropic_token: if config.ai.provider == AiProvider::Claude {
            config.ai.anthropic_token.clone()
        } else {
            None
        },
    };

    config.ai.profiles.insert(name.clone(), profile);
    config.save()?;
    println!("\n✓ Saved profile '{}'", name);

    Ok(())
}

async fn delete_profile(config: &mut Config) -> Result<()> {
    let profile_names: Vec<_> = config.ai.profiles.keys().cloned().collect();

    if profile_names.is_empty() {
        println!("No profiles to delete.");
        return Ok(());
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Delete which profile?")
        .items(&profile_names)
        .interact()
        .map_err(|e| Error::DaemonError {
            message: format!("Selection failed: {}", e),
        })?;

    let name = &profile_names[selection];
    config.ai.profiles.remove(name);
    config.save()?;
    println!("\n✓ Deleted profile '{}'", name);

    Ok(())
}
