use clap::Parser;
use greppy::cli::{print_logo, Cli, Commands};
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
        Some(Commands::Search { query, limit, project, json }) => {
            cmd_search(query, limit, project, json).await
        }
        Some(Commands::Index { project, force, watch }) => {
            cmd_index(project, force, watch).await
        }
        Some(Commands::List) => cmd_list().await,
        Some(Commands::Forget { path }) => cmd_forget(path).await,
        Some(Commands::Ping) => cmd_ping().await,
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

async fn cmd_search(query: String, limit: usize, project: Option<PathBuf>, json: bool) -> Result<()> {
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

    let mut client = DaemonClient::connect()?;
    let request = Request::search(query, project_path, limit);
    let response = client.send(request)?;

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
