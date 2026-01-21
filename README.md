# Greppy

```
 ██████╗ ██████╗ ███████╗██████╗ ██████╗ ██╗   ██╗
██╔════╝ ██╔══██╗██╔════╝██╔══██╗██╔══██╗╚██╗ ██╔╝
██║  ███╗██████╔╝█████╗  ██████╔╝██████╔╝ ╚████╔╝ 
██║   ██║██╔══██╗██╔══╝  ██╔═══╝ ██╔═══╝   ╚██╔╝  
╚██████╔╝██║  ██║███████╗██║     ██║        ██║   
 ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚═╝        ╚═╝   
```

**Sub-millisecond semantic code search and invocation tracing with AI-powered reranking.**

No cloud indexing. Works with **Ollama (local)**, Claude, or Gemini. Just `greppy search "query"` or `greppy trace symbol`.

---

## What is Greppy?

Greppy is a local code search tool that combines:

- **BM25 full-text search** via [Tantivy](https://github.com/quickwit-oss/tantivy) for sub-millisecond queries
- **AI reranking** via Ollama (local), Claude, or Gemini to surface the most relevant results
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

When configured with an AI provider, Greppy:
1. Runs a fast BM25 search to find candidate results
2. Sends candidates to AI (Ollama local, Claude, or Gemini) for reranking
3. Returns results ordered by semantic relevance

Without AI configured, automatically falls back to direct BM25 mode.

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

## Trace (Invocation Mapping)

Greppy Trace provides complete codebase invocation mapping - like Sentry's stack traces, but for your entire codebase without running code.

### Basic Trace

```bash
# Find all invocation paths for a symbol
greppy trace validateUser
```

Output:
```
╔══════════════════════════════════════════════════════════════════════════════╗
║  TRACE: validateUser                                                         ║
║  Defined: utils/validation.ts:8                                              ║
║  Found: 47 invocation paths from 12 entry points                             ║
╚══════════════════════════════════════════════════════════════════════════════╝

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Path 1/47                                              POST /api/auth/login
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  routes.ts:15          →  POST /api/auth/login
       │
  auth.controller.ts:8  →  loginController.handle(req, res)
       │
  auth.service.ts:42    →  authService.login(credentials)
       │
  validation.ts:8       →  validateUser(user)  ← TARGET
```

### Trace Commands

```bash
# Call graph trace (who calls this function)
greppy trace <symbol>

# Direct mode (no AI, sub-millisecond)
greppy trace <symbol> -d

# Reference tracing with code context
greppy trace --refs userId              # All references
greppy trace --refs userId -c 2         # With 2 lines of context
greppy trace --refs userId --in src/    # Limit to src/ directory
greppy trace --refs userId --count      # Just show count
greppy trace --reads userId             # Reads only
greppy trace --writes userId            # Writes only

# Call graph analysis
greppy trace --callers fetchData        # What calls this symbol
greppy trace --callees fetchData        # What this symbol calls

# Type tracing (where does this type flow)
greppy trace --type User

# Module tracing (import/export relationships)
greppy trace --module utils/auth
greppy trace --cycles                   # Find circular dependencies

# Pattern tracing (find any pattern with regex)
greppy trace --pattern "TODO:.*"
greppy trace --pattern "async function" -c 2

# Data flow analysis
greppy trace --flow password            # Track data from source to sink

# Impact analysis (what breaks if I change this)
greppy trace --impact validateUser

# Dead code detection
greppy trace --dead
greppy trace --dead --xref             # With potential callers

# Codebase statistics
greppy trace --stats

# Scope analysis
greppy trace --scope src/api.ts:42      # What's visible at location

# Output formats
greppy trace <symbol> --json            # JSON for tooling
greppy trace <symbol> --plain           # No colors (for pipes)
greppy trace <symbol> --csv             # CSV for spreadsheets
greppy trace <symbol> --dot             # DOT for graph visualization
greppy trace <symbol> --markdown        # Markdown for documentation
```

### Composable Operations

Run multiple analyses in a single command:

```bash
# Run dead code + stats + cycles together
greppy trace --dead --stats --cycles

# Filter all operations to a path
greppy trace --dead --stats --in src/auth

# Summary mode: one-line output per operation
greppy trace --dead --stats --cycles --summary

# Combined JSON output for tooling
greppy trace --dead --stats --json
```

**Summary mode output:**
```
DEAD CODE ANALYSIS
  Dead symbols: 61  (unknown=4, function=16, struct=41)

CODEBASE STATISTICS
  Files: 5  Symbols: 84  Refs: 1711  Edges: 1688

CIRCULAR DEPENDENCIES
  Circular deps: 2
```

### Cross-Reference Dead Code

The `--xref` flag shows potential callers for dead symbols:

```bash
greppy trace --dead --xref -n 5
```

Output:
```
MessageRequest  src/ai/claude.rs:17  No references or calls found
    Potential callers:
      → new  src/ai/claude.rs:66  Same file - could call this
      → get_access_token  src/ai/claude.rs:75  Same file - could call this
      → MessageRequest  src/ai/claude.rs:143  Token match - name appears here
```

This helps you understand *why* code is dead - is it truly unused, or is there a missing call?

### What grep/ripgrep CAN'T do (but greppy can)

| Feature | grep/ripgrep | greppy |
|---------|--------------|--------|
| Impact analysis | No | `--impact` shows callers & affected entry points |
| Dead code detection | No | `--dead` finds unused symbols |
| Dead code cross-reference | No | `--dead --xref` shows potential callers |
| Call chain visualization | No | Shows full invocation paths |
| Semantic reference filtering | No | `--reads` vs `--writes` vs `--kind call` |
| Codebase statistics | No | `--stats` shows symbols, call depth, etc. |
| Circular dependency detection | No | `--cycles` finds import loops |
| Composable operations | No | `--dead --stats --cycles` runs all at once |
| Summary mode | No | `--summary` for condensed output |

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

### Search Performance

| Mode | Latency | Notes |
|------|---------|-------|
| Daemon (warm) | <1ms | Index in memory |
| Direct (warm) | 1-10ms | Index on disk |
| Direct (cold) | 50-100ms | First query loads index |
| Semantic (AI) | 500-2000ms | Includes AI reranking |

### Benchmark: greppy vs grep vs ripgrep

Tested on a 75k file, 13.7M line TypeScript codebase:

| Query: "userId" | Results | Time | Notes |
|-----------------|---------|------|-------|
| grep            | 2,648   | ~2.5s | Text matching (scans all files) |
| ripgrep         | 1,296   | ~0.04s | Text matching (parallel, faster) |
| **greppy**      | 990     | **~0.07s** | **Semantic refs** (knows symbol context) |

| Query: "useState" | Results | Time | Notes |
|-------------------|---------|------|-------|
| grep              | 1,449   | ~2.6s | Includes comments, strings |
| ripgrep           | 1,292   | ~0.04s | Includes comments, strings |
| **greppy**        | 1,258   | **~0.08s** | **Only actual symbol references** |

**Key difference:** grep/ripgrep find text matches. Greppy finds **semantic symbol references** - it knows when `userId` is a variable vs a string vs a comment.

### Trace Performance

| Query Type | Time | Notes |
|------------|------|-------|
| Symbol references | ~70ms | All usages of a symbol |
| Impact analysis | ~75ms | What breaks if you change this |
| Dead code detection | ~78ms | Find unused symbols |
| Codebase statistics | ~600ms | Full analysis |
| Call chain trace | <1ms | Pre-computed call graph |

### Token Usage: greppy vs AI Reading Files

When AI tools search code, they typically read entire files. Greppy returns only semantic references with targeted context, dramatically reducing token usage.

**Real test on 75k file codebase:**

| Query: "userId" (262 files contain it) | Tokens | Savings |
|----------------------------------------|--------|---------|
| AI reads 20 matching files | 43,493 | baseline |
| greppy --refs -c 2 (50 refs + context) | 3,100 | **93% less** |

| Query: "validateFounderAccess" | Tokens | Savings |
|--------------------------------|--------|---------|
| AI reads 4 matching files | 7,659 | baseline |
| greppy --refs -c 2 | 532 | **93% less** |
| greppy --impact | 170 | **98% less** |

**Cost savings at $3/1M tokens (Claude):**
- Reading 20 files: $0.13 per query
- Using greppy: $0.009 per query
- **14x cost reduction**

### System Performance

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
