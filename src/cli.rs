use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "greppy")]
#[command(author, version, about = "Sub-millisecond local semantic code search")]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Run in daemon mode (internal use)
    #[arg(long, hide = true)]
    pub daemon_mode: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the greppy daemon
    Start,

    /// Stop the greppy daemon
    Stop,

    /// Show daemon status
    Status,

    /// Search for code in the current project
    Search {
        /// Search query
        query: String,

        /// Maximum number of results
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Project path (defaults to current directory)
        #[arg(short, long)]
        project: Option<PathBuf>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Index the current project
    Index {
        /// Project path (defaults to current directory)
        #[arg(short, long)]
        project: Option<PathBuf>,

        /// Force full re-index
        #[arg(short, long)]
        force: bool,

        /// Watch for changes after indexing
        #[arg(short, long)]
        watch: bool,
    },

    /// List indexed projects
    List,

    /// Remove a project from the index
    Forget {
        /// Project path (defaults to current directory)
        path: Option<PathBuf>,
    },

    /// Ping the daemon (health check)
    Ping,
}

impl Cli {
    pub fn parse_args() -> Self {
        Cli::parse()
    }
}
