//! Daemon command implementation

use crate::cli::{DaemonAction, DaemonArgs};
use crate::core::config::Config;
use crate::core::error::Result;
use crate::daemon::client;

/// Run the daemon command
pub fn run(args: DaemonArgs) -> Result<()> {
    match args.action {
        DaemonAction::Start => start_daemon(),
        DaemonAction::Stop => stop_daemon(),
        DaemonAction::Status => check_status(),
        DaemonAction::Restart => {
            stop_daemon()?;
            start_daemon()
        }
    }
}

fn start_daemon() -> Result<()> {
    // TODO: Implement daemon start
    println!("Daemon mode not yet implemented.");
    println!("Use direct mode: greppy search \"query\"");
    Ok(())
}

fn stop_daemon() -> Result<()> {
    let socket_path = Config::daemon_socket()?;
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
        println!("Daemon stopped.");
    } else {
        println!("Daemon is not running.");
    }
    Ok(())
}

fn check_status() -> Result<()> {
    if client::is_running()? {
        println!("Daemon is running.");
    } else {
        println!("Daemon is not running.");
    }
    Ok(())
}
