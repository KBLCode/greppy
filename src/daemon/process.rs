use crate::config::Config;
use crate::error::{GreppyError, Result};
use std::process::{Command, Stdio};

/// Check if daemon is running
pub fn is_running() -> Result<bool> {
    let pid_path = Config::pid_path()?;
    if !pid_path.exists() {
        return Ok(false);
    }

    let pid_str = std::fs::read_to_string(&pid_path)?;
    let pid: u32 = pid_str.trim().parse().unwrap_or(0);

    if pid == 0 {
        return Ok(false);
    }

    // Check if process exists
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let result = unsafe { libc::kill(pid as i32, 0) };
        Ok(result == 0)
    }

    #[cfg(not(unix))]
    {
        Ok(false)
    }
}

/// Get daemon PID
pub fn get_pid() -> Result<Option<u32>> {
    let pid_path = Config::pid_path()?;
    if !pid_path.exists() {
        return Ok(None);
    }

    let pid_str = std::fs::read_to_string(&pid_path)?;
    let pid: u32 = pid_str.trim().parse().unwrap_or(0);

    if pid == 0 {
        return Ok(None);
    }

    Ok(Some(pid))
}

/// Start daemon in background
pub fn start_daemon() -> Result<u32> {
    if is_running()? {
        if let Some(pid) = get_pid()? {
            return Err(GreppyError::DaemonAlreadyRunning(pid));
        }
    }

    Config::ensure_home()?;

    // Get current executable path
    let exe = std::env::current_exe()?;

    // Spawn daemon process
    let child = Command::new(&exe)
        .arg("__daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    let pid = child.id();

    // Write PID file
    let pid_path = Config::pid_path()?;
    std::fs::write(&pid_path, pid.to_string())?;

    // Wait a moment for daemon to start
    std::thread::sleep(std::time::Duration::from_millis(100));

    Ok(pid)
}

/// Stop daemon
pub fn stop_daemon() -> Result<bool> {
    let pid = match get_pid()? {
        Some(p) => p,
        None => return Ok(false),
    };

    #[cfg(unix)]
    {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }

    // Clean up files
    let pid_path = Config::pid_path()?;
    let socket_path = Config::socket_path()?;

    if pid_path.exists() {
        let _ = std::fs::remove_file(&pid_path);
    }
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    Ok(true)
}
