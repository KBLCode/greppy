//! Greppy CLI entry point

use clap::Parser;
use greppy::cli::{Cli, Commands};
use greppy::core::error::Result;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("GREPPY_LOG"))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Search(args) => greppy::cli::search::run(args),
        Commands::Index(args) => greppy::cli::index::run(args),
        Commands::Daemon(args) => greppy::cli::daemon::run(args),
        Commands::List(args) => greppy::cli::list::run(args),
        Commands::Forget(args) => greppy::cli::forget::run(args),
        Commands::Login(args) => greppy::cli::login::run(args).await,
        Commands::Logout(args) => greppy::cli::logout::run(args),
        Commands::Ask(args) => greppy::cli::ask::run(args).await,
        Commands::Read(args) => greppy::cli::read::run(args).await,
    }
}
