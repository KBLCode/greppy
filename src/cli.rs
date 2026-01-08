use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

/// Print the colored logo banner
pub fn print_logo() {
    let logo = r#"
┌──────────────────────────────────────────────────┐
│ ██████╗ ██████╗ ███████╗██████╗ ██████╗ ██╗   ██╗│
│██╔════╝ ██╔══██╗██╔════╝██╔══██╗██╔══██╗╚██╗ ██╔╝│
│██║  ███╗██████╔╝█████╗  ██████╔╝██████╔╝ ╚████╔╝ │
│██║   ██║██╔══██╗██╔══╝  ██╔═══╝ ██╔═══╝   ╚██╔╝  │
│╚██████╔╝██║  ██║███████╗██║     ██║        ██║   │
│ ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚═╝        ╚═╝   │
└──────────────────────────────────────────────────┘
"#;
    println!("{}", logo.bright_cyan());
}

const HELP_TEMPLATE: &str = r#"
{about}

{usage-heading} {usage}

{all-args}

{after-help}"#;

#[derive(Parser)]
#[command(name = "greppy")]
#[command(author, version)]
#[command(about = "Sub-millisecond local semantic code search for AI coding tools")]
#[command(after_help = "Examples:
  greppy start              Start the background daemon
  greppy index              Index the current project
  greppy search \"auth\"      Search for 'auth' in code
  greppy search -l 5 \"fn\"   Get top 5 results for 'fn'
  greppy search --json \"x\"  Output results as JSON

For AI tools (Claude Code, Cursor, Aider):
  greppy search --json \"authenticate user\" | head -50")]
#[command(help_template = HELP_TEMPLATE)]
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

        /// Use LLM-powered smart search (requires authentication)
        #[arg(short, long)]
        smart: bool,
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

    /// Manage authentication for LLM features
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
}

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Login with your Anthropic account
    Login,

    /// Logout and clear stored credentials
    Logout,

    /// Show authentication status
    Status,
}

impl Cli {
    pub fn parse_args() -> Self {
        Cli::parse()
    }
}
