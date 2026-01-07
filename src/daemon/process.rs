use crate::config::Config;
use crate::error::{GreppyError, Result};
use std::fs;
use std::process::{Command, Stdio};

/// Start the daemon in the background
pub fn start_daemon() -> Result<u32> {
    // Check if already running
    if let Some(pid) = get_daemon_pid() {
        if is_process_running(pid) {
            return Err(GreppyError::DaemonAlreadyRunning(pid));
        }
        // Stale PID file, remove it
        let _ = fs::remove_file(Config::pid_path()?);
    }

    // Ensure home directory exists
    Config::ensure_home()?;

    // Get the path to our own executable
    let exe = std::env::current_exe()?;

    // Fork the daemon process
    // We use a special --daemon-mode flag to indicate we're running as daemon
    let child = Command::new(&exe)
        .arg("--daemon-mode")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    let pid = child.id();

    // Write PID file
    fs::write(Config::pid_path()?, pid.to_string())?;

    // Give the daemon a moment to start
    std::thread::sleep(std::time::Duration::from_millis(100));

    Ok(pid)
}

/// Stop the running daemon
pub fn stop_daemon() -> Result<()> {
    let pid = get_daemon_pid().ok_or(GreppyError::DaemonNotRunning)?;

    if !is_process_running(pid) {
        // Clean up stale files
        let _ = fs::remove_file(Config::pid_path()?);
        let _ = fs::remove_file(Config::socket_path()?);
        return Err(GreppyError::DaemonNotRunning);
    }

    // Send SIGTERM
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }

    // Wait for process to exit (with timeout)
    for _ in 0..50 {
        if !is_process_running(pid) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Clean up files
    let _ = fs::remove_file(Config::pid_path()?);
    let _ = fs::remove_file(Config::socket_path()?);

    Ok(())
}

/// Check if the daemon is running
pub fn is_daemon_running() -> bool {
    if let Some(pid) = get_daemon_pid() {
        is_process_running(pid)
    } else {
        false
    }
}

/// Get the daemon's PID if it's running
pub fn get_daemon_pid() -> Option<u32> {
    let pid_path = Config::pid_path().ok()?;
    let content = fs::read_to_string(pid_path).ok()?;
    content.trim().parse().ok()
}

/// Check if a process with the given PID is running
fn is_process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // kill with signal 0 checks if process exists
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}
