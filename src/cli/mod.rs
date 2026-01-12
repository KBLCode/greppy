//! CLI command definitions and handlers

pub mod ask;
pub mod daemon;
pub mod forget;
pub mod index;
pub mod list;
pub mod login;
pub mod logout;
pub mod read;
pub mod search;

use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .usage(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .literal(AnsiColor::Blue.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Green.on_default())
}

/// Sub-millisecond local semantic code search
#[derive(Parser, Debug)]
#[command(name = "greppy")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
#[command(styles = styles())]
#[command(help_template = "\
{before-help}{name} {version}
{author-with-newline}{about-with-newline}
{usage-heading}
    {usage}

{all-args}{after-help}")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Search for code semantically
    Search(SearchArgs),

    /// Index a project
    Index(IndexArgs),

    /// Manage the background daemon
    Daemon(DaemonArgs),

    /// List indexed projects
    List(ListArgs),

    /// Remove a project's index
    Forget(ForgetArgs),

    /// Authenticate with Google
    Login(login::LoginArgs),

    /// Log out
    Logout(logout::LogoutArgs),

    /// Ask a question about the codebase
    Ask(ask::AskArgs),

    /// Read a file or specific lines
    Read(read::ReadArgs),
}

/// Arguments for the search command
#[derive(Parser, Debug)]
pub struct SearchArgs {
    /// The search query
    pub query: String,

    /// Maximum number of results
    #[arg(short, long, default_value = "20")]
    pub limit: usize,

    /// Output format
    #[arg(short = 'f', long, default_value = "human")]
    pub format: OutputFormat,

    /// Project path (defaults to current directory)
    #[arg(short, long)]
    pub project: Option<PathBuf>,

    /// Search only in specific paths (can be repeated)
    #[arg(long = "path")]
    pub paths: Vec<PathBuf>,

    /// Include test files in results
    #[arg(long)]
    pub include_tests: bool,

    /// Use daemon if available (faster)
    #[arg(long, default_value = "true")]
    pub use_daemon: bool,
}

/// Arguments for the index command
#[derive(Parser, Debug)]
pub struct IndexArgs {
    /// Project path (defaults to current directory)
    #[arg(short, long)]
    pub project: Option<PathBuf>,

    /// Watch for changes and re-index
    #[arg(short, long)]
    pub watch: bool,

    /// Force full re-index
    #[arg(long)]
    pub force: bool,
}

/// Arguments for the daemon command
#[derive(Parser, Debug)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub action: DaemonAction,
}

#[derive(Subcommand, Debug)]
pub enum DaemonAction {
    /// Start the daemon
    Start,
    /// Stop the daemon
    Stop,
    /// Check daemon status
    Status,
    /// Restart the daemon
    Restart,
}

/// Arguments for the list command
#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Output format
    #[arg(short = 'f', long, default_value = "human")]
    pub format: OutputFormat,
}

/// Arguments for the forget command
#[derive(Parser, Debug)]
pub struct ForgetArgs {
    /// Project path to forget
    pub project: PathBuf,
}

/// Output format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable output
    Human,
    /// JSON output
    Json,
}
