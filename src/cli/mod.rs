//! CLI command definitions and handlers

pub mod daemon;
pub mod index;
pub mod login;
pub mod model;
pub mod search;
pub mod trace;
pub mod web;

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
    2. greppy login           Setup AI reranking (Ollama local or cloud)
    3. greppy search <query>  Search your code!

SEARCH MODES:
    greppy search "query"     Semantic search - AI reranks BM25 results
    greppy search -d "query"  Direct search - BM25 only (no AI, faster)

DAEMON (optional, for faster searches):
    greppy start              Start background daemon with file watcher
    greppy stop               Stop the daemon
    greppy status             Check if daemon is running

AI PROVIDERS:
    greppy login              Configure AI provider for semantic search
    greppy logout             Remove stored credentials

    Supports Ollama (local, free), Claude, or Gemini. Without login,
    searches fall back to direct BM25 mode automatically.

TRACE (symbol analysis):
    greppy trace <symbol>             Find all invocation paths
    greppy trace --refs <symbol>      Find all references  
    greppy trace --impact <symbol>    Analyze change impact
    greppy trace --dead               Find unused code
    greppy trace --stats              Codebase statistics

EXAMPLES:
    greppy index                      Index current directory
    greppy search "error handling"    Find error handling code
    greppy search -d "TODO" -n 50     Find all TODOs (direct mode)
    greppy search "auth" --json       JSON output for scripting
    greppy trace --refs createUser    Find all references to createUser
    greppy trace --impact auth        Analyze impact of changing auth
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

    /// Configure AI provider for semantic search (Ollama, Claude, or Gemini)
    #[command(after_help = "AI PROVIDERS:
    1. Run 'greppy login'
    2. Select your provider using arrow keys
    3. Complete setup (OAuth for cloud, auto-detect for Ollama)
    4. You're ready to use semantic search!

PROVIDERS:
    Ollama (Local)     - Free, private, runs on your machine
    Claude (Anthropic) - Uses your Claude.ai account via OAuth
    Gemini (Google)    - Uses your Google account via OAuth

NOTES:
    - Ollama: Install from ollama.com, run 'ollama pull <model>'
    - Cloud providers use OAuth (no API keys needed)
    - Credentials stored in ~/.config/greppy/config.toml
    - Run 'greppy logout' to remove stored credentials")]
    Login,

    /// Remove stored credentials and log out from all providers
    #[command(
        after_help = "This removes all stored credentials from your config file.
After logging out, semantic search will fall back to direct BM25 search.
Run 'greppy login' to authenticate again."
    )]
    Logout,

    /// Switch AI model/provider interactively
    #[command(visible_alias = "m")]
    #[command(after_help = "USAGE:
    greppy model              Interactive model switcher
    
Shows current AI configuration and lets you:
    - Switch between saved profiles (Claude, Gemini, Ollama models)
    - Add new Ollama models from your local installation
    - Set the active model for semantic search

EXAMPLES:
    greppy model              Open interactive switcher
    
Profiles are saved in ~/.config/greppy/config.toml")]
    Model,

    /// Trace symbol invocations across codebase
    #[command(visible_alias = "t")]
    Trace(trace::TraceArgs),

    /// Launch web UI for visual codebase exploration
    #[command(visible_alias = "w")]
    Web(web::WebArgs),
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
