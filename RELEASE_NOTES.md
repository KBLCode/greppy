# Release Notes v0.6.0

## 🚀 Major Architecture Upgrade: Daemon Mode

This release introduces a fully functional **Daemon Mode**, transforming Greppy from a simple CLI tool into a high-performance search server.

### ✨ Highlights

*   **Sub-millisecond Search**: By keeping indexes loaded in memory, search queries now complete in **< 1ms** (down from ~15-20ms).
*   **Background Indexing**: Indexing operations are now offloaded to the daemon, freeing up your terminal immediately.
*   **Robust Process Management**: New `start`, `stop`, and `status` commands with proper PID handling and error reporting.
*   **Seamless Fallback**: The CLI automatically detects if the daemon is running. If not, it falls back to direct mode or warns the user.

### 🛠 Fixes & Improvements

*   **Fixed**: Daemon start/stop commands were previously stubbed; they are now fully implemented.
*   **Fixed**: `process.rs` was disconnected from the build; it is now integrated and modernized.
*   **Improved**: Search CLI now accepts a `--use-daemon` flag (default: true).
*   **Improved**: Error handling for daemon connection failures.

### 📦 Installation

```bash
curl -fsSL https://raw.githubusercontent.com/greppy/greppy/v0.6.0/install.sh | bash
```

### ⚠️ Breaking Changes

*   Internal IPC protocol has changed. Ensure you restart the daemon after upgrading:
    ```bash
    greppy daemon restart
    ```
