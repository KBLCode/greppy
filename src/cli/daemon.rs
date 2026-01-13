//! Daemon command implementations (start, stop, status)

use crate::core::error::Result;
use crate::daemon::process;

/// Start the daemon
pub fn start() -> Result<()> {
    if process::is_running()? {
        if let Some(pid) = process::get_pid()? {
            println!("Daemon already running (PID: {})", pid);
            return Ok(());
        }
    }

    match process::start_daemon() {
        Ok(pid) => {
            println!("Daemon started (PID: {})", pid);
            println!("File watcher active for incremental indexing.");
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to start daemon: {}", e);
            Err(e)
        }
    }
}

/// Stop the daemon
pub fn stop() -> Result<()> {
    if !process::is_running()? {
        println!("Daemon is not running.");
        return Ok(());
    }

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

/// Check daemon status
pub fn status() -> Result<()> {
    if process::is_running()? {
        if let Some(pid) = process::get_pid()? {
            println!("Daemon is running (PID: {})", pid);
        } else {
            println!("Daemon is running.");
        }
    } else {
        println!("Daemon is not running.");
    }
    Ok(())
}
