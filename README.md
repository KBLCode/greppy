# Greppy

```
 ██████╗ ██████╗ ███████╗██████╗ ██████╗ ██╗   ██╗
██╔════╝ ██╔══██╗██╔════╝██╔══██╗██╔══██╗╚██╗ ██╔╝
██║  ███╗██████╔╝█████╗  ██████╔╝██████╔╝ ╚████╔╝ 
██║   ██║██╔══██╗██╔══╝  ██╔═══╝ ██╔═══╝   ╚██╔╝  
╚██████╔╝██║  ██║███████╗██║     ██║        ██║   
 ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚═╝        ╚═╝   
```

**Sub-millisecond semantic code search with AI-powered reranking.**

No cloud indexing. No API keys. Just `greppy search "query"`.

---

## What is Greppy?

Greppy is a local code search tool that combines:

- **BM25 full-text search** via [Tantivy](https://github.com/quickwit-oss/tantivy) for sub-millisecond queries
- **AI reranking** via Claude or Gemini to surface the most relevant results
- **Background daemon** with file watching for instant, always-up-to-date searches

### Why Greppy?

AI coding tools (Claude Code, Cursor, Aider, OpenCode) need fast code search. Existing solutions are either:

- **Too slow** - grep/ripgrep scan files on every query
- **Cloud-dependent** - Sourcegraph, GitHub search require network
- **Not semantic** - keyword matching misses context

Greppy gives you **<1ms semantic search** that runs entirely on your machine.

---

## Installation

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/KBLCode/greppy/main/install.sh | bash
```

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/KBLCode/greppy/main/install.ps1 | iex
```

### Cargo

```bash
cargo install greppy-cli
```

### From Source

```bash
git clone https://github.com/KBLCode/greppy
cd greppy
cargo install --path .
```

---

## Quick Start

```bash
# 1. Index your project (one-time setup)
cd your-project
greppy index

# 2. (Optional) Authenticate for AI-powered reranking
greppy login

# 3. Search!
greppy search "authentication middleware"
```

That's it! Greppy works immediately after indexing. Authentication is optional but recommended for better results.

---

## Search Modes

### Semantic Search (Default)

```bash
greppy search "error handling"
```

When authenticated, Greppy:
1. Runs a fast BM25 search to find candidate results
2. Sends candidates to Claude or Gemini for reranking
3. Returns results ordered by semantic relevance

If not authenticated, automatically falls back to direct mode.

### Direct Search (BM25 Only)

```bash
greppy search -d "TODO"
greppy search --direct "FIXME"
```

Pure BM25 search without AI. Faster, but results are ranked by keyword frequency rather than semantic relevance.

### Search Options

```
Usage: greppy search [OPTIONS] <QUERY>

Options:
  -d, --direct             Direct mode (BM25 only, no AI)
  -n, --limit <N>          Maximum results (default: 20)
      --json               JSON output for scripting
  -p, --project <PATH>     Project path (default: current directory)
```

### Examples

```bash
# Find authentication code
greppy search "user authentication"

# Find all TODOs (direct mode, faster)
greppy search -d "TODO" -n 50

# JSON output for scripting
greppy search "database" --json | jq '.results[0].path'

# Search a specific project
greppy search "config" -p ~/projects/myapp
```

---

## Authentication

Greppy uses OAuth to authenticate with AI providers. **No API keys needed!**

### Login

```bash
greppy login
```

1. Select your provider using arrow keys:
   - **Claude (Anthropic)** - Uses your Claude.ai account
   - **Gemini (Google)** - Uses your Google account

2. Complete the OAuth flow in your browser

3. You're ready to use semantic search!

### Logout

```bash
greppy logout
```

Removes all stored credentials from your system keychain.

### How It Works

- Tokens are stored securely in your system keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- Uses OAuth free tier - no API billing
- Without authentication, searches fall back to direct BM25 mode automatically

---

## Daemon

The background daemon provides sub-millisecond queries and automatic index updates.

### Commands

```bash
greppy start    # Start the daemon
greppy stop     # Stop the daemon
greppy status   # Check if daemon is running
```

### Features

- **In-memory indexes** - Queries return in <1ms
- **File watching** - Automatically updates indexes when files change
- **Query caching** - Repeated queries are instant

### Platform Support

| Platform | IPC Method |
|----------|------------|
| macOS    | Unix socket (`~/.greppy/daemon.sock`) |
| Linux    | Unix socket (`~/.greppy/daemon.sock`) |
| Windows  | TCP localhost (port in `~/.greppy/daemon.port`) |

---

## Indexing

### Basic Usage

```bash
# Index current directory
greppy index

# Index specific project
greppy index -p ~/projects/myapp

# Force full re-index
greppy index --force
```

### What Gets Indexed

Greppy automatically:
- Respects `.gitignore` patterns
- Chunks code into semantic units (functions, classes, methods)
- Extracts symbol names for boosted matching
- Skips binary files and common non-code directories

### Supported Languages

TypeScript, JavaScript, Python, Rust, Go, Java, Kotlin, Ruby, PHP, C, C++, C#, Swift, Elixir, Haskell, Lua, Shell, SQL, Vue, Svelte, HTML, CSS, JSON, YAML, Markdown, and more.

---

## Performance

| Mode | Latency | Notes |
|------|---------|-------|
| Daemon (warm) | <1ms | Index in memory |
| Direct (warm) | 1-10ms | Index on disk |
| Direct (cold) | 50-100ms | First query loads index |
| Semantic (AI) | 500-2000ms | Includes AI reranking |

**Indexing speed:** ~17,000 chunks/second

**Memory usage:** ~55MB during indexing

---

## Configuration

Optional config at `~/.greppy/config.toml`:

```toml
[general]
default_limit = 20

[ignore]
patterns = ["node_modules", ".git", "dist", "build", "__pycache__"]

[index]
max_file_size = 1048576  # 1MB
max_files = 100000

[cache]
query_ttl = 60
max_queries = 1000
```

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `GREPPY_HOME` | Override config/data directory (default: `~/.greppy`) |
| `GREPPY_LOG` | Log level: `debug`, `info`, `warn`, `error` |

---

## How It Works

1. **Indexing** - Greppy walks your project, respecting `.gitignore`, and chunks code into semantic units (functions, classes, methods)

2. **Storage** - Chunks are stored in a [Tantivy](https://github.com/quickwit-oss/tantivy) index with BM25 ranking

3. **Search** - Queries are parsed and matched against the index with symbol name boosting

4. **AI Reranking** - When authenticated, top BM25 results are sent to Claude or Gemini for semantic reranking

5. **Watching** - The daemon monitors file changes and incrementally updates indexes

---

## Integration with AI Tools

Greppy works great with AI coding assistants:

- **Claude Code** - Use as a code search tool
- **OpenCode** - Integrate via CLI
- **Cursor** - Call from terminal
- **Aider** - Use for codebase exploration
- **Custom MCP servers** - JSON output for easy parsing

### JSON Output

```bash
greppy search "auth" --json
```

```json
{
  "results": [
    {
      "path": "src/auth/login.rs",
      "content": "pub async fn login() -> Result<()> { ... }",
      "symbol_name": "login",
      "symbol_type": "method",
      "start_line": 1,
      "end_line": 50,
      "language": "rust",
      "score": 4.23
    }
  ],
  "query": "auth",
  "elapsed_ms": 0.8,
  "project": "/path/to/project"
}
```

---

## Troubleshooting

### "Not logged in" message

This is informational, not an error. Without authentication, Greppy uses direct BM25 search which still works great for most queries.

To enable AI reranking:
```bash
greppy login
```

### Daemon won't start

Check if another instance is running:
```bash
greppy status
greppy stop
greppy start
```

### Index seems outdated

Force a full re-index:
```bash
greppy index --force
```

Or start the daemon for automatic updates:
```bash
greppy start
```

### OAuth login fails

1. Make sure you have a browser available
2. Check your internet connection
3. Try logging out and back in:
   ```bash
   greppy logout
   greppy login
   ```

---

## License

MIT

---

## Links

- **Repository:** https://github.com/KBLCode/greppy
- **Issues:** https://github.com/KBLCode/greppy/issues
- **Releases:** https://github.com/KBLCode/greppy/releases
