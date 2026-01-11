use crate::core::error::{Error, Result};
use clap::Parser;
use regex::Regex;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Arguments for the read command
#[derive(Parser, Debug)]
pub struct ReadArgs {
    /// The location to read (e.g., "src/main.rs", "src/main.rs:10", "src/main.rs:10-20")
    pub location: String,

    /// Number of context lines to show around a single line (default: 20)
    #[arg(short, long, default_value_t = 20)]
    pub context: usize,
}

pub async fn run(args: ReadArgs) -> Result<()> {
    let (path, start, end) = parse_location(&args.location, args.context)?;

    if !path.exists() {
        return Err(Error::IoError {
            message: format!("File not found: {}", path.display()),
        });
    }

    let file = File::open(&path).await.map_err(|e| Error::IoError {
        message: format!("Failed to open file: {}", e),
    })?;

    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut current_line = 0;

    println!("Reading {}:{}-{}", path.display(), start, end);
    println!("---");

    while let Some(line) = lines.next_line().await.map_err(|e| Error::IoError {
        message: format!("Failed to read line: {}", e),
    })? {
        current_line += 1;

        if current_line >= start && current_line <= end {
            println!("{:4} | {}", current_line, line);
        }

        if current_line > end {
            break;
        }
    }
    println!("---");

    Ok(())
}

fn parse_location(location: &str, context: usize) -> Result<(PathBuf, usize, usize)> {
    // Pattern 1: file:start-end
    let range_re = Regex::new(r"^(.*):(\d+)-(\d+)$").unwrap();
    if let Some(caps) = range_re.captures(location) {
        let path = PathBuf::from(&caps[1]);
        let start: usize = caps[2].parse().unwrap_or(1);
        let end: usize = caps[3].parse().unwrap_or(start);
        return Ok((path, start, end));
    }

    // Pattern 2: file:line
    let line_re = Regex::new(r"^(.*):(\d+)$").unwrap();
    if let Some(caps) = line_re.captures(location) {
        let path = PathBuf::from(&caps[1]);
        let line: usize = caps[2].parse().unwrap_or(1);
        let start = line.saturating_sub(context).max(1);
        let end = line + context;
        return Ok((path, start, end));
    }

    // Pattern 3: file (read whole file, or first N lines? Let's default to first 100 for safety, or all?)
    // The python version reads first 50 lines. Let's do first 100.
    let path = PathBuf::from(location);
    Ok((path, 1, 100))
}
