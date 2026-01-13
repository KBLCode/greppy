//! Greppy CLI entry point

use clap::Parser;
use greppy::cli::{Cli, Commands};
use greppy::core::error::Result;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Check for hidden daemon mode (spawned by `greppy start`)
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && args[1] == "__daemon" {
        return run_daemon_server().await;
    }

    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("GREPPY_LOG"))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Search(args) => greppy::cli::search::run(args).await,
        Commands::Index(args) => greppy::cli::index::run(args),
        Commands::Start => greppy::cli::daemon::start(),
        Commands::Stop => greppy::cli::daemon::stop(),
        Commands::Status => greppy::cli::daemon::status(),
        Commands::Login => greppy::cli::login::run().await,
        Commands::Logout => greppy::cli::login::logout(),
    }
}

/// Run the daemon server (called when spawned with __daemon arg)
async fn run_daemon_server() -> Result<()> {
    // Initialize logging for daemon
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("GREPPY_LOG").add_directive("greppy=info".parse().unwrap()))
        .init();

    greppy::daemon::server::run_server().await
}
