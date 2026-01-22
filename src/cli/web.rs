//! Web command implementation
//!
//! Launches the greppy web UI for visual codebase exploration.
//!
//! @module cli/web

use clap::Args;
use std::env;
use std::path::PathBuf;

use crate::core::error::Result;

/// Arguments for the web command
#[derive(Args, Debug)]
#[command(after_help = "EXAMPLES:
    greppy web                    Start web UI on localhost:3000
    greppy web --port 8080        Use custom port
    greppy web --open             Auto-open browser
    greppy web -p ~/project       Specify project path")]
pub struct WebArgs {
    /// Project path (default: current directory)
    #[arg(short, long)]
    pub project: Option<PathBuf>,

    /// Port to serve on (default: 3000)
    #[arg(long, default_value = "3000")]
    pub port: u16,

    /// Auto-open browser
    #[arg(long)]
    pub open: bool,
}

/// Run the web command
pub async fn run(args: WebArgs) -> Result<()> {
    let project_path = args
        .project
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    crate::web::server::run(project_path, args.port, args.open).await
}
