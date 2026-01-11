# Greppy

**Sub-millisecond local semantic code search for AI coding tools.**

No cloud. No config. Just `greppy search "query"`.

## Why Greppy?

AI coding tools (Claude Code, OpenCode, Cursor, Aider) need fast code search. Existing solutions are either:
- **Too slow** (mgrep: 100-200ms)
- **Cloud-dependent** (Sourcegraph)
- **Not semantic** (ripgrep)

Greppy gives you **<10ms semantic search** that runs entirely on your machine.

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
# Index your project
cd your-project
greppy index

# Search
greppy search "authentication middleware"
greppy search "database connection" --limit 10
greppy search "error handling" --json
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

### Index

```bash
greppy index [options]

Options:
  -p, --project <PATH> Project path (default: current directory)
  -w, --watch          Watch for changes (daemon mode)
  --force              Force full re-index
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

1. **Indexing**: Greppy parses your code into semantic chunks (functions, classes, etc.) using tree-sitter
2. **Search**: BM25 ranking with symbol name boosting finds the most relevant code
3. **Speed**: Tantivy (Rust search engine) + memory-mapped indexes = sub-millisecond queries

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
