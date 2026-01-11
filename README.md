# Greppy

**Sub-millisecond local semantic code search for AI coding tools.**

No cloud. No config. Just `greppy search "query"`.

## Why Greppy?

AI coding tools (Claude Code, OpenCode, Cursor, Aider) need fast code search. Existing solutions are either:
- **Too slow** (mgrep: 100-200ms)
- **Cloud-dependent** (Sourcegraph)
- **Not semantic** (ripgrep)

Greppy gives you **<10ms semantic search** that runs entirely on your machine.

## New in v0.4.0
- **Google OAuth**: Secure authentication for AI features.
- **Semantic Search**: Local vector embeddings for understanding intent.
- **Ask Command**: Ask natural language questions about your codebase (powered by Gemini Flash).
- **Parallel Indexing**: Blazing fast indexing using all CPU cores.

## Installation

```bash
# Homebrew (macOS/Linux)
brew install greppy

# curl installer
curl -fsSL https://greppy.dev/install.sh | sh

# Cargo
cargo install greppy
```

## Quick Start

```bash
# 1. Authenticate (Optional, for AI features)
greppy login

# 2. Index your project
cd your-project
greppy index

# 3. Search
greppy search "authentication middleware"
greppy search "database connection" --limit 10

# 4. Ask Questions
greppy ask "How does the authentication flow work?"
```

## Usage

### Search

```bash
greppy search <query> [options]

Options:
  -l, --limit <N>      Maximum results (default: 20)
  -f, --format <FMT>   Output format: human, json (default: human)
  -p, --project <PATH> Project path (default: current directory)
  --path <PATH>        Filter to specific paths (can repeat)
  --include-tests      Include test files in results
```

### Ask (AI)

```bash
greppy ask <question> [options]

Options:
  -p, --project <PATH> Project path (default: current directory)
```

### Index

```bash
greppy index [options]

Options:
  -p, --project <PATH> Project path (default: current directory)
  -w, --watch          Watch for changes (daemon mode)
  --force              Force full re-index
```

### Auth

```bash
greppy login          # Authenticate with Google
greppy logout         # Log out
```

### Daemon (Optional)

For even faster queries (<1ms), use daemon mode:

```bash
greppy daemon start   # Start background daemon
greppy daemon stop    # Stop daemon
greppy daemon status  # Check status
```

### Other Commands

```bash
greppy list           # List indexed projects
greppy forget <path>  # Remove a project's index
```

## How It Works

1. **Indexing**: Greppy parses your code into semantic chunks (functions, classes, etc.) using tree-sitter. It also generates vector embeddings locally using `fastembed-rs`.
2. **Search**: A hybrid approach combining BM25 (keyword) and Vector Similarity (semantic) finds the most relevant code.
3. **Speed**: Tantivy (Rust search engine) + memory-mapped indexes + parallel processing = sub-millisecond queries.

## Configuration

Optional config at `~/.greppy/config.toml`:

```toml
[general]
default_limit = 20

[ignore]
patterns = ["node_modules", ".git", "dist"]

[index]
max_file_size = 1048576  # 1MB
```

## Performance

| Mode | Latency |
|------|---------|
| Daemon (warm) | <1ms |
| Direct (warm) | <10ms |
| Direct (cold) | <100ms |

## Supported Languages

TypeScript, JavaScript, Python, Rust, Go, Java, Ruby, PHP, C/C++, and more.

## License

MIT
