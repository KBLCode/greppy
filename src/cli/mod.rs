//! CLI command definitions and handlers

pub mod daemon;
pub mod index;
pub mod login;
pub mod search;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

const LONG_ABOUT: &str = r#"
 ██████╗ ██████╗ ███████╗██████╗ ██████╗ ██╗   ██╗
██╔════╝ ██╔══██╗██╔════╝██╔══██╗██╔══██╗╚██╗ ██╔╝
██║  ███╗██████╔╝█████╗  ██████╔╝██████╔╝ ╚████╔╝ 
██║   ██║██╔══██╗██╔══╝  ██╔═══╝ ██╔═══╝   ╚██╔╝  
╚██████╔╝██║  ██║███████╗██║     ██║        ██║   
 ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚═╝        ╚═╝   

Sub-millisecond semantic code search powered by BM25 + AI reranking.

QUICK START:
    1. greppy index           Index your codebase (one-time setup)
    2. greppy login           Authenticate with Claude or Gemini (optional)
    3. greppy search <query>  Search your code!

SEARCH MODES:
    greppy search "query"     Semantic search - AI reranks BM25 results
    greppy search -d "query"  Direct search - BM25 only (no AI, faster)

DAEMON (optional, for faster searches):
    greppy start              Start background daemon with file watcher
    greppy stop               Stop the daemon
    greppy status             Check if daemon is running

AUTHENTICATION:
    greppy login              Authenticate with Claude or Gemini via OAuth
    greppy logout             Remove stored credentials

    Semantic search uses AI to rerank results by relevance. Without login,
    searches fall back to direct BM25 mode automatically.

EXAMPLES:
    greppy index                      Index current directory
    greppy search "error handling"    Find error handling code
    greppy search -d "TODO" -n 50     Find all TODOs (direct mode)
    greppy search "auth" --json       JSON output for scripting
"#;

/// Sub-millisecond semantic code search
#[derive(Parser, Debug)]
#[command(name = "greppy")]
#[command(author, version)]
#[command(about = "Sub-millisecond semantic code search")]
#[command(long_about = LONG_ABOUT)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Search for code (semantic by default, -d for direct BM25)
    #[command(visible_alias = "s")]
    Search(SearchArgs),

    /// Index a project for searching
    #[command(visible_alias = "i")]
    Index(IndexArgs),

    /// Start the background daemon
    Start,

    /// Stop the background daemon
    Stop,

    /// Check if the daemon is running
    Status,

    /// Authenticate with Claude or Gemini for AI-powered semantic search
    #[command(after_help = "AUTHENTICATION:
    Greppy uses OAuth to authenticate with AI providers. No API keys needed!
    
    1. Run 'greppy login'
    2. Select your provider (Claude or Gemini) using arrow keys
    3. Complete the OAuth flow in your browser
    4. You're ready to use semantic search!

PROVIDERS:
    Claude (Anthropic) - Uses your Claude.ai account
    Gemini (Google)    - Uses your Google account

NOTES:
    - Tokens are stored securely in your system keychain
    - Free tier usage through OAuth (no API billing)
    - Run 'greppy logout' to remove stored credentials")]
    Login,

    /// Remove stored credentials and log out from all providers
    #[command(
        after_help = "This removes all stored OAuth tokens from your system keychain.
After logging out, semantic search will fall back to direct BM25 search.
Run 'greppy login' to authenticate again."
    )]
    Logout,
}

/// Arguments for the search command
#[derive(Parser, Debug)]
#[command(after_help = "EXAMPLES:
    greppy search \"authentication\"       Semantic search (AI)
    greppy search -d \"authentication\"    Direct BM25 search
    greppy search \"error\" -n 10          Limit results
    greppy search \"query\" --json         JSON output")]
pub struct SearchArgs {
    /// Search query
    pub query: String,

    /// Direct mode (BM25 only, no AI)
    #[arg(short = 'd', long)]
    pub direct: bool,

    /// Max results
    #[arg(short = 'n', long, default_value = "20")]
    pub limit: usize,

    /// JSON output
    #[arg(long)]
    pub json: bool,

    /// Project path (default: current directory)
    #[arg(short, long)]
    pub project: Option<PathBuf>,
}

/// Arguments for the index command
#[derive(Parser, Debug)]
#[command(after_help = "EXAMPLES:
    greppy index              Index current directory
    greppy index -p ~/code    Index specific directory
    greppy index --force      Force full re-index")]
pub struct IndexArgs {
    /// Project path (default: current directory)
    #[arg(short, long)]
    pub project: Option<PathBuf>,

    /// Force full re-index
    #[arg(short, long)]
    pub force: bool,
}

/// Output format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}
