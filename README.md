# Greppy

**Sub-millisecond local semantic code search for AI coding tools.**

No cloud. No config. Just `greppy search "query"`.

## Why Greppy?

AI coding tools (Claude Code, OpenCode, Cursor, Aider) need fast code search. Existing solutions are either:
- **Too slow** (mgrep: 100-200ms)
- **Cloud-dependent** (Sourcegraph)
- **Not semantic** (ripgrep)

Greppy gives you **<10ms semantic search** that runs entirely on your machine.

## New in v0.5.0
- **Precision Parsing**: Tree-sitter integration for Rust, Python, Go, Java, and TypeScript/JavaScript.
- **Read Command**: Precise file reading for agents (`greppy read file:line`).
- **Google OAuth**: Secure authentication for AI features.
- **Semantic Search**: Local vector embeddings for understanding intent.
- **Ask Command**: Ask natural language questions about your codebase (powered by Gemini Flash).
- **Parallel Indexing**: Blazing fast indexing using all CPU cores.

## Installation

### Option 1: Pre-built Binaries (macOS & Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/KBLCode/greppy/main/install.sh | sh
```

### Option 2: From Source (Cargo)

```bash
cargo install --git https://github.com/KBLCode/greppy.git
```

### Option 3: Manual Download

Download the latest release for your platform from [GitHub Releases](https://github.com/KBLCode/greppy/releases).

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

### Read (Agent Tool)

```bash
greppy read <location> [options]

# Examples:
greppy read src/main.rs          # Read first 100 lines
greppy read src/main.rs:50       # Read around line 50
greppy read src/main.rs:10-20    # Read lines 10 to 20

Options:
  -c, --context <N>    Context lines around single line (default: 20)
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

1. **Indexing**: Greppy parses your code into semantic chunks using **Tree-sitter** (for supported languages) or heuristics. It also generates vector embeddings locally using `fastembed-rs`.
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
