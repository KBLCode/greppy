//! Daemon command implementation

use crate::cli::{DaemonAction, DaemonArgs};
use crate::core::error::Result;
use crate::daemon::process;

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
    match process::start_daemon() {
        Ok(pid) => {
            println!("Daemon started (PID: {})", pid);
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to start daemon: {}", e);
            Err(e)
        }
    }
}

fn stop_daemon() -> Result<()> {
    match process::stop_daemon() {
        Ok(true) => {
            println!("Daemon stopped.");
            Ok(())
        }
        Ok(false) => {
            println!("Daemon is not running.");
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to stop daemon: {}", e);
            Err(e)
        }
    }
}

fn check_status() -> Result<()> {
    match process::is_running() {
        Ok(true) => {
            if let Ok(Some(pid)) = process::get_pid() {
                println!("Daemon is running (PID: {}).", pid);
            } else {
                println!("Daemon is running.");
            }
        }
        Ok(false) => {
            println!("Daemon is not running.");
        }
        Err(e) => {
            eprintln!("Error checking status: {}", e);
            return Err(e);
        }
    }
    Ok(())
}
