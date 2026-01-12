use crate::core::config::Config;
use crate::core::error::{Error, Result};
#[cfg(unix)]
use libc;
use std::process::{Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Windows: CREATE_NO_WINDOW flag to prevent console window from appearing
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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
        let result = unsafe { libc::kill(pid as i32, 0) };
        Ok(result == 0)
    }

    #[cfg(windows)]
    {
        // Use tasklist to check if PID exists
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH", "/FO", "CSV"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // tasklist returns the process info if found, or "INFO: No tasks" if not
                Ok(stdout.contains(&pid.to_string()) && !stdout.contains("No tasks"))
            }
            Err(_) => Ok(false),
        }
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
            return Err(Error::DaemonAlreadyRunning(pid));
        }
    }

    Config::ensure_home()?;

    // Get current executable path
    let exe = std::env::current_exe()?;

    // Spawn daemon process
    // We use a hidden subcommand "__daemon" to start the server
    #[cfg(unix)]
    let child = Command::new(&exe)
        .arg("__daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    #[cfg(windows)]
    let child = Command::new(&exe)
        .arg("__daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
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

    #[cfg(windows)]
    {
        // Use taskkill to terminate process
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }

    // Clean up files
    let pid_path = Config::pid_path()?;

    if pid_path.exists() {
        let _ = std::fs::remove_file(&pid_path);
    }

    // Platform-specific cleanup
    #[cfg(unix)]
    {
        let socket_path = Config::socket_path()?;
        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }
    }

    #[cfg(windows)]
    {
        let port_path = Config::port_path()?;
        if port_path.exists() {
            let _ = std::fs::remove_file(&port_path);
        }
    }

    Ok(true)
}
