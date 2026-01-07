# Greppy

```
┌──────────────────────────────────────────────────┐
│ ██████╗ ██████╗ ███████╗██████╗ ██████╗ ██╗   ██╗│
│██╔════╝ ██╔══██╗██╔════╝██╔══██╗██╔══██╗╚██╗ ██╔╝│
│██║  ███╗██████╔╝█████╗  ██████╔╝██████╔╝ ╚████╔╝ │
│██║   ██║██╔══██╗██╔══╝  ██╔═══╝ ██╔═══╝   ╚██╔╝  │
│╚██████╔╝██║  ██║███████╗██║     ██║        ██║   │
│ ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚═╝        ╚═╝   │
└──────────────────────────────────────────────────┘
```

**Sub-millisecond local code search for AI coding tools.**

<p align="center">
  <img src="Greppy.gif" alt="Greppy demo" width="600">
</p>

```
$ greppy search "authentication middleware"
Found 12 results for "authentication middleware" (0.89ms)

1. src/middleware/auth.ts (lines 1-45)
   symbol authMiddleware
   Score: 12.34
   │ export const authMiddleware = async (req, res, next) => {
   │   const token = req.headers.authorization?.split(' ')[1];
   │   if (!token) return res.status(401).json({ error: 'Unauthorized' });
   │ ...
```

## Why Greppy?

AI coding assistants (Claude Code, Cursor, Aider, Copilot) need to search your codebase constantly. Every search means:

- **Grep/ripgrep**: Fast but no semantic understanding. Misses `authenticate` when you search `auth`.
- **Embeddings APIs**: Smart but slow (100-500ms) and expensive ($0.0001/search adds up).
- **Local LLMs**: Smart but requires GPU, complex setup, still 50-200ms.

**Greppy** gives you the best of both worlds:

| Tool | Speed | Semantic | Setup | Cost |
|------|-------|----------|-------|------|
| grep | 10ms | ❌ | None | Free |
| ripgrep | 5ms | ❌ | None | Free |
| Embeddings API | 200ms | ✅ | API key | $$$ |
| Local embeddings | 100ms | ✅ | GPU + models | Free |
| **Greppy** | **<1ms** | ✅ | `curl \| sh` | **Free** |

### How?

Greppy uses [Tantivy](https://github.com/quickwit-oss/tantivy) (Rust's Lucene) with BM25 ranking + symbol boosting. A background daemon keeps indexes hot in memory. **Auto-watches for file changes** and reindexes automatically.

## Installation

### Quick Install (macOS/Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/KBLCode/greppy/main/install.sh | bash
```

### Cargo (from source)

```bash
cargo install --git https://github.com/KBLCode/greppy
```

### Manual Download

Download the latest binary from [Releases](https://github.com/KBLCode/greppy/releases).

## Quick Start

```bash
# Start the daemon (runs in background)
greppy start

# Index your project (auto-watches for changes!)
cd /path/to/your/project
greppy index

# Search!
greppy search "database connection"
greppy search "handleSubmit" -l 5      # Limit to 5 results
greppy search "auth" --json            # JSON output for tools
```

## Features

- **Sub-millisecond search** - BM25 ranking with symbol boosting
- **Auto-watch** - Automatically reindexes when files change
- **Cache** - Repeated queries return in <0.1ms
- **JSON output** - Easy integration with AI tools
- **Multi-project** - Index and search multiple projects

## Usage

### Commands

| Command | Description |
|---------|-------------|
| `greppy start` | Start the background daemon |
| `greppy stop` | Stop the daemon |
| `greppy status` | Show daemon status and stats |
| `greppy index` | Index current project (+ auto-watch) |
| `greppy index --force` | Force full re-index |
| `greppy search <query>` | Search indexed code |
| `greppy search <query> -l 10` | Limit results |
| `greppy search <query> --json` | JSON output |
| `greppy search <query> -p /path` | Search specific project |
| `greppy list` | List indexed projects |
| `greppy forget` | Remove current project from index |

### JSON Output

For integration with AI tools, use `--json`:

```bash
greppy search "error handling" --json
```

```json
{
  "status": "ok",
  "data": {
    "type": "Search",
    "data": {
      "query": "error handling",
      "project": "/path/to/project",
      "results": [
        {
          "path": "src/lib/errors.ts",
          "content": "export class AppError extends Error {...}",
          "symbol_name": "AppError",
          "start_line": 1,
          "end_line": 45,
          "language": "typescript",
          "score": 8.5
        }
      ],
      "elapsed_ms": 0.87,
      "cached": false
    }
  }
}
```

## Architecture

```
┌─────────────────┐     ┌──────────────────────────────────┐
│ greppy search   │────▶│ Unix Socket (~/.greppy/daemon.sock)│
│ greppy index    │     └──────────────────────────────────┘
│ AI tools        │                    │
│ Scripts         │                    ▼
└─────────────────┘     ┌──────────────────────────────────┐
                        │         Daemon Process           │
                        │  ┌─────────┐  ┌───────────────┐  │
                        │  │ Tantivy │  │ LRU Cache     │  │
                        │  │ Indexes │  │ (1000 queries)│  │
                        │  └─────────┘  └───────────────┘  │
                        │  ┌─────────────────────────────┐ │
                        │  │ File Watcher (auto-reindex) │ │
                        │  └─────────────────────────────┘ │
                        └──────────────────────────────────┘
```

## Performance

| Project | Files | Chunks | Index Time | Search (cold) | Search (cached) |
|---------|-------|--------|------------|---------------|-----------------|
| Small CLI tool | 26 | 64 | 215ms | 0.87ms | 0.01ms |
| Medium Next.js app | 1,236 | 10,516 | 493ms | 1.34ms | 0.04ms |
| Large monorepo | 50,000+ | 500,000+ | ~30s | <5ms | <0.1ms |

## Supported Languages

Greppy indexes and extracts symbols from:

**Rust** • **TypeScript** • **JavaScript** • **Python** • **Go** • **Java** • **Kotlin** • **C/C++** • **C#** • **Ruby** • **PHP** • **Swift** • **Scala** • **Shell** • **HTML** • **CSS** • **SQL** • **Elixir** • **Haskell** • **Lua** • **Zig** • **Dart** • **Vue** • **Svelte** • and more...

## Configuration

Greppy stores data in:

- **macOS**: `~/Library/Application Support/dev.greppy.greppy/`
- **Linux**: `~/.local/share/greppy/`

Override with `GREPPY_HOME`:

```bash
export GREPPY_HOME=/custom/path
```

## Building from Source

```bash
git clone https://github.com/KBLCode/greppy.git
cd greppy
cargo build --release
./target/release/greppy --help
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Credits

Built with [Tantivy](https://github.com/quickwit-oss/tantivy), [Tokio](https://tokio.rs/), and [Clap](https://github.com/clap-rs/clap).

---

**Made for developers who want their AI tools to be fast.**
