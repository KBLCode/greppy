//! List command implementation

use crate::cli::ListArgs;
use crate::core::config::Config;
use crate::core::error::Result;
use serde::Serialize;
use std::fs;

/// Run the list command
pub fn run(args: ListArgs) -> Result<()> {
    let home = Config::greppy_home()?;
    let indexes_dir = home.join("indexes");

    if !indexes_dir.exists() {
        println!("No indexed projects found.");
        return Ok(());
    }

    let mut projects = Vec::new();

    for entry in fs::read_dir(&indexes_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() && path.join("meta.json").exists() {
            // Try to read project info
            // For now, just list the hash directories
            projects.push(IndexedProject {
                index_path: path.to_string_lossy().to_string(),
                // TODO: Store and retrieve original project path
            });
        }
    }

    match args.format {
        crate::cli::OutputFormat::Human => {
            if projects.is_empty() {
                println!("No indexed projects found.");
            } else {
                println!("Indexed projects:");
                for project in &projects {
                    println!("  {}", project.index_path);
                }
            }
        }
        crate::cli::OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&projects)?);
        }
    }

    Ok(())
}

#[derive(Serialize)]
struct IndexedProject {
    index_path: String,
}
