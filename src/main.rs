use clap::Parser;
use colored::Colorize;
use greppy::auth;
use greppy::cli::{print_logo, AuthCommands, Cli, Commands};
use greppy::daemon::{
    is_daemon_running, start_daemon, stop_daemon, DaemonClient, DaemonServer, Request,
};
use greppy::error::Result;
use greppy::output::{format_human, format_json};
use greppy::project::detect_project_root;
use std::path::PathBuf;
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    // Check if help is requested - print logo first
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 || args.iter().any(|a| a == "-h" || a == "--help") {
        print_logo();
    }

    let cli = Cli::parse();

    // Special case: daemon mode
    if cli.daemon_mode {
        return run_daemon().await;
    }

    // Handle commands
    let result = match cli.command {
        Some(Commands::Start) => cmd_start().await,
        Some(Commands::Stop) => cmd_stop().await,
        Some(Commands::Status) => cmd_status().await,
        Some(Commands::Search { query, limit, project, json, smart }) => {
            cmd_search(query, limit, project, json, smart).await
        }
        Some(Commands::Index { project, force, watch }) => {
            cmd_index(project, force, watch).await
        }
        Some(Commands::List) => cmd_list().await,
        Some(Commands::Forget { path }) => cmd_forget(path).await,
        Some(Commands::Ping) => cmd_ping().await,
        Some(Commands::Auth { command }) => cmd_auth(command).await,
        None => {
            // No command - show help
            eprintln!("Usage: greppy <command>");
            eprintln!("Run 'greppy --help' for more information.");
            return ExitCode::FAILURE;
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

async fn run_daemon() -> ExitCode {
    let server = DaemonServer::new();
    match server.run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Daemon error: {}", e);
            ExitCode::FAILURE
        }
    }
}

async fn cmd_start() -> Result<()> {
    if is_daemon_running() {
        println!("Daemon is already running");
        return Ok(());
    }

    let pid = start_daemon()?;
    println!("Daemon started (pid {})", pid);
    Ok(())
}

async fn cmd_stop() -> Result<()> {
    if !is_daemon_running() {
        println!("Daemon is not running");
        return Ok(());
    }

    // Try graceful shutdown first
    if let Ok(mut client) = DaemonClient::connect() {
        let _ = client.send(Request::shutdown());
    }

    // Then force stop if needed
    stop_daemon()?;
    println!("Daemon stopped");
    Ok(())
}

async fn cmd_status() -> Result<()> {
    if !is_daemon_running() {
        println!("Daemon is not running");
        return Ok(());
    }

    let mut client = DaemonClient::connect()?;
    let response = client.send(Request::status())?;
    println!("{}", format_human(&response));
    Ok(())
}

async fn cmd_search(query: String, limit: usize, project: Option<PathBuf>, json: bool, smart: bool) -> Result<()> {
    // Ensure daemon is running
    if !is_daemon_running() {
        eprintln!("Daemon is not running. Start it with: greppy start");
        return Err(greppy::GreppyError::DaemonNotRunning);
    }

    // Detect project
    let project_path = match project {
        Some(p) => p,
        None => detect_project_root(&std::env::current_dir()?)?,
    };

    // Handle smart search mode with speculative execution
    let response = if smart {
        // Check if authenticated
        if !auth::is_authenticated().await {
            eprintln!("{}", "Warning: Not authenticated. Using regular search.".yellow());
            eprintln!("Run 'greppy auth login' to enable smart search.");
            let mut client = DaemonClient::connect()?;
            client.send(Request::search(query, project_path, limit))?
        } else {
            // SPECULATIVE EXECUTION: Start both searches in parallel
            // 1. Immediate search with original query (fast)
            // 2. LLM enhancement (may be cached = instant, or API = slower)
            eprintln!("{}", "Enhancing query with AI...".cyan());
            
            let query_clone = query.clone();
            let project_clone = project_path.clone();
            
            // Start LLM enhancement
            let enhancement = greppy::llm::try_enhance_query(&query).await;
            
            if enhancement.intent != "general" {
                eprintln!(
                    "{}",
                    format!("Intent: {} | Expanded: {}", enhancement.intent, &enhancement.expanded_query).dimmed()
                );
            }
            
            // If LLM returned same query, just search once
            let search_query = if enhancement.expanded_query == query_clone 
                || enhancement.expanded_query.is_empty() {
                query_clone
            } else {
                enhancement.expanded_query
            };
            
            let mut client = DaemonClient::connect()?;
            client.send(Request::search(search_query, project_clone, limit))?
        }
    } else {
        let mut client = DaemonClient::connect()?;
        client.send(Request::search(query, project_path, limit))?
    };

    if json {
        println!("{}", format_json(&response));
    } else {
        println!("{}", format_human(&response));
    }

    Ok(())
}

async fn cmd_index(project: Option<PathBuf>, force: bool, _watch: bool) -> Result<()> {
    // Ensure daemon is running
    if !is_daemon_running() {
        eprintln!("Daemon is not running. Start it with: greppy start");
        return Err(greppy::GreppyError::DaemonNotRunning);
    }

    // Detect project
    let project_path = match project {
        Some(p) => p,
        None => detect_project_root(&std::env::current_dir()?)?,
    };

    println!("Indexing {}...", project_path.display());

    let mut client = DaemonClient::connect()?;
    let request = Request::index(project_path, force);
    let response = client.send(request)?;

    println!("{}", format_human(&response));

    // TODO: Implement watch mode
    // if watch {
    //     println!("Watching for changes...");
    // }

    Ok(())
}

async fn cmd_list() -> Result<()> {
    if !is_daemon_running() {
        eprintln!("Daemon is not running. Start it with: greppy start");
        return Err(greppy::GreppyError::DaemonNotRunning);
    }

    let mut client = DaemonClient::connect()?;
    let response = client.send(Request::list_projects())?;
    println!("{}", format_human(&response));
    Ok(())
}

async fn cmd_forget(path: Option<PathBuf>) -> Result<()> {
    if !is_daemon_running() {
        eprintln!("Daemon is not running. Start it with: greppy start");
        return Err(greppy::GreppyError::DaemonNotRunning);
    }

    let project_path = match path {
        Some(p) => p,
        None => detect_project_root(&std::env::current_dir()?)?,
    };

    let mut client = DaemonClient::connect()?;
    let response = client.send(Request::forget_project(project_path))?;
    println!("{}", format_human(&response));
    Ok(())
}

async fn cmd_ping() -> Result<()> {
    if !is_daemon_running() {
        println!("Daemon is not running");
        return Ok(());
    }

    let mut client = DaemonClient::connect()?;
    let response = client.send(Request::ping())?;
    println!("{}", format_human(&response));
    Ok(())
}

async fn cmd_auth(command: AuthCommands) -> Result<()> {
    match command {
        AuthCommands::Login => cmd_auth_login().await,
        AuthCommands::Logout => cmd_auth_logout().await,
        AuthCommands::Status => cmd_auth_status().await,
    }
}

async fn cmd_auth_login() -> Result<()> {
    // Check if already authenticated
    if auth::is_authenticated().await {
        println!("{}", "Already authenticated!".green());
        println!("Run 'greppy auth logout' to sign out first.");
        return Ok(());
    }

    // Start OAuth flow
    let auth_request = auth::authorize();
    
    println!("{}", "Opening browser for authentication...".cyan());
    println!();
    println!("If the browser doesn't open, visit this URL:");
    println!("{}", auth_request.url.bright_blue());
    println!();
    
    // Try to open browser
    if let Err(_) = open_browser(&auth_request.url) {
        println!("{}", "Could not open browser automatically.".yellow());
    }
    
    println!("After authorizing, you'll see a code. Paste it here:");
    print!("> ");
    use std::io::{self, Write};
    io::stdout().flush().unwrap();
    
    let mut code = String::new();
    io::stdin().read_line(&mut code).unwrap();
    let code = code.trim();
    
    if code.is_empty() {
        println!("{}", "No code provided. Login cancelled.".red());
        return Ok(());
    }
    
    // Exchange code for tokens
    println!("Exchanging code for tokens...");
    match auth::exchange(code, &auth_request.verifier).await {
        Ok(tokens) => {
            auth::store_tokens(&tokens)?;
            println!("{}", "Successfully authenticated!".green());
            println!("Smart search is now available with: greppy search --smart \"query\"");
        }
        Err(e) => {
            println!("{}", format!("Authentication failed: {}", e).red());
        }
    }
    
    Ok(())
}

async fn cmd_auth_logout() -> Result<()> {
    auth::clear_tokens()?;
    println!("{}", "Logged out successfully.".green());
    Ok(())
}

async fn cmd_auth_status() -> Result<()> {
    if auth::is_authenticated().await {
        println!("{}", "Authenticated".green());
        println!("Smart search is available with: greppy search --smart \"query\"");
    } else {
        println!("{}", "Not authenticated".yellow());
        println!("Run 'greppy auth login' to enable smart search features.");
    }
    Ok(())
}

/// Try to open a URL in the default browser
fn open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd").args(["/C", "start", url]).spawn()?;
    }
    Ok(())
}
