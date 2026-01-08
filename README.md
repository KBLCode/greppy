<h1 align="center">Greppy</h1>

<p align="center">
  <strong>Sub-millisecond semantic code search for AI coding tools</strong>
</p>

<p align="center">
  <a href="#installation">Installation</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#smart-search">Smart Search</a> •
  <a href="#performance">Performance</a> •
  <a href="#privacy">Privacy</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-0.2.0-blue" alt="Version">
  <img src="https://img.shields.io/badge/rust-1.70+-orange" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="License">
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey" alt="Platform">
  <img src="https://img.shields.io/badge/100%25-local--first-brightgreen" alt="Local First">
</p>

---

<p align="center">
  <img src="Greppy.gif" alt="Greppy demo" width="700">
</p>

---

## 🔒 100% Local-First

**Your code never leaves your machine.** Greppy runs entirely locally:

- ✅ **All indexing is local** — Your code is parsed and indexed on your machine
- ✅ **All searching is local** — BM25 search runs against local Tantivy indexes
- ✅ **No telemetry** — Zero data collection, no analytics, no tracking
- ✅ **No cloud storage** — Indexes stored in your local filesystem only

**The only network connection** is optional OAuth authentication for Smart Search, which sends only your search query (not your code) to Claude for query expansion.

---

## Greppy vs grep/ripgrep

| Feature | grep/ripgrep | Greppy |
|---------|--------------|--------|
| **Speed** | 5-50ms (file I/O) | **<1ms** (in-memory index) |
| **Semantic understanding** | ❌ Literal text only | ✅ Understands code structure |
| **Symbol awareness** | ❌ No concept of functions/classes | ✅ Boosts functions, classes, exports |
| **Ranking** | ❌ No relevance scoring | ✅ BM25 + multi-factor scoring |
| **Natural language** | ❌ Must know exact text | ✅ "how does auth work" → finds auth code |
| **AI-ready output** | ❌ Raw text dump | ✅ Structured JSON with metadata |
| **Repeated queries** | Same speed every time | **<0.1ms** cached |

### Real-World Comparison

```bash
# grep: Returns 500+ matches, no ranking, you scroll forever
grep -r "auth" ./src
# 500 lines of output, mostly noise...

# ripgrep: Faster, but same problem
rg "auth" ./src
# Still 500 lines, still no ranking...

# Greppy: Top 20 ranked results in <1ms
greppy search "auth"
# [1] src/auth/oauth.rs  L45-89  function authenticate  score:34.2
# [2] src/auth/session.rs  L12-34  struct Session  score:28.1
# ...focused, ranked results

# Greppy Smart: Understands intent, expands query
greppy search --smart "how does authentication work"
# Intent: understand_flow
# Expanded: auth authenticate login session token verify credentials
# [1] src/auth/flow.rs  L1-120  module  score:45.8
# ...finds the actual auth flow, not just files containing "auth"
```

---

## The Problem

AI coding assistants search your codebase **hundreds of times per session**. Each search is slow and expensive:

```
You: "How does authentication work?"

Traditional AI Tool Approach:
├── grep "auth" → 500 files match
├── Stuff ALL into context → 50,000 tokens
├── LLM processes everything → $0.15, 3-5 seconds
└── Result: Slow, expensive, often misses the best code
```

## The Solution

```
You: "How does authentication work?"

Greppy Smart Search:
├── Check cache → HIT! → 6ms total ⚡
│   OR
├── Query → Claude Haiku → "auth login token session verify" (~2s, cached forever)
├── BM25 search with expanded terms → 0.87ms
├── Return TOP 20 ranked results → 2,000 tokens
└── Result: 24x fewer tokens, 200x faster, 5000x cheaper
```

---

## Why Greppy?

| Approach | Speed | Semantic | Tokens Used | Cost/Search |
|----------|-------|----------|-------------|-------------|
| grep/ripgrep | 5-50ms | ❌ | 50,000+ (dumps everything) | Free but wasteful |
| Embeddings API | 200-500ms | ✅ | 10,000+ | $0.001+ |
| Context stuffing | 2-5s | ❌ | 50,000-100,000 | $0.05-0.15 |
| **Greppy** | **<1ms** | **✅** | **~2,000** | **$0.00003** |
| **Greppy (cached)** | **<10ms** | **✅** | **~2,000** | **FREE** |

### Key Metrics

| Metric | Traditional | Greppy | Improvement |
|--------|-------------|--------|-------------|
| **Speed** | 2-5 seconds | <10ms | **200-500x faster** |
| **Tokens** | 50,000+ | ~2,000 | **25x fewer** |
| **Cost** | $0.05-0.15 | $0.00003 | **1,600-5,000x cheaper** |
| **Relevance** | Dumps everything | Ranked by relevance | **Far better results** |

---

## Installation

### Quick Install (macOS/Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/KBLCode/greppy/main/install.sh | bash
```

### Cargo

```bash
cargo install --git https://github.com/KBLCode/greppy
```

### Manual Download

Download from [Releases](https://github.com/KBLCode/greppy/releases).

---

## Quick Start

```bash
# 1. Start the daemon (runs in background)
greppy start

# 2. Index your project (auto-watches for changes)
cd /path/to/your/project
greppy index

# 3. Search!
greppy search "database connection"
```

**Output:**
```
Found 12 results for "database connection" (0.89ms)

[1] src/db/connection.ts  L1-45  class DatabaseConnection  score:34.2
    │ export class DatabaseConnection {
    │   private pool: Pool;
    │   async connect(config: DbConfig): Promise<void> {
    │ ... +42 more lines

[2] src/lib/postgres.ts  L23-67  function createPool  score:28.7
    │ export async function createPool(options: PoolOptions) {
    │ ...
```

---

## Smart Search

**NEW in v0.2.0** — Use Claude AI to understand your intent and find the right code:

```bash
# One-time authentication (optional - enables smart search)
greppy auth login

# Natural language queries
greppy search --smart "how does the authentication flow work"
# → Intent: understand_flow
# → Expanded: auth authenticate login session token verify credentials OAuth
# → Found 20 results in 6ms ⚡

greppy search --smart "find where errors are handled"
# → Intent: find_implementation  
# → Expanded: error catch try exception handle throw Result Err unwrap
# → Found 15 results in 5ms ⚡
```

### Instant Smart Search with Caching

Smart Search results are **cached locally** for instant repeat queries:

| Query Type | First Search | Repeat Searches |
|------------|--------------|-----------------|
| Regular search | <1ms | <0.1ms |
| Smart search (API) | ~2-3s | — |
| Smart search (cached) | — | **<10ms** ⚡ |

The cache uses:
- **L1: In-memory LRU** — 500 most recent queries, sub-millisecond
- **L2: Persistent file** — 5,000 queries, 7-day TTL, survives restarts
- **Fuzzy matching** — Similar queries hit cache (75% word match)

```bash
# First time: hits Claude API (~2s)
greppy search --smart "how does auth work"

# Second time: instant from cache (6ms)
greppy search --smart "how does auth work"

# Similar query: also hits cache! (6ms)
greppy search --smart "how does authentication work"
```

### How Smart Search Works

```
┌─────────────────────────────────────────────────────────────────┐
│  "how does authentication work?"                                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Check Local Cache                                              │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ L1 Memory Cache → L2 File Cache → Fuzzy Match             │  │
│  │ HIT? → Return instantly (<10ms)                           │  │
│  │ MISS? → Continue to Claude API                            │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │ (cache miss only)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Claude Haiku (~2s, cached forever after)                       │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ Intent: understand_flow                                    │  │
│  │ Expanded: auth authenticate login session token verify    │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Greppy BM25 Search (<1ms)                                      │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ • Symbol boosting (functions, classes ranked higher)      │  │
│  │ • Export bonus (public APIs prioritized)                  │  │
│  │ • Test penalty (test files ranked lower)                  │  │
│  │ • Recency weighting                                       │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Top 20 Results (~2,000 tokens)                                 │
│  Ready for AI context                                           │
└─────────────────────────────────────────────────────────────────┘
```

### Authentication Options

**Option 1: OAuth (Recommended)**
```bash
greppy auth login    # Opens browser for Anthropic OAuth
greppy auth status   # Check authentication status
greppy auth logout   # Clear stored credentials
```

**Option 2: API Key**
```bash
export ANTHROPIC_API_KEY=sk-ant-...
greppy search --smart "your query"
```

---

## Privacy

### What Stays Local (Everything Important)

| Data | Location | Shared? |
|------|----------|---------|
| Your source code | Never leaves machine | ❌ Never |
| Code indexes | `~/Library/Application Support/dev.greppy.greppy/` | ❌ Never |
| Search results | Local memory only | ❌ Never |
| Query cache | `~/Library/Application Support/dev.greppy.greppy/llm_cache.json` | ❌ Never |
| Auth tokens | `~/.config/greppy/auth.json` | ❌ Never |

### What's Sent to Claude (Smart Search Only)

When you use `--smart`, **only your search query** is sent to Claude:

```
Sent: "how does authentication work"
NOT sent: Any of your actual code
```

The response (expanded query terms) is cached locally, so repeated queries never hit the network.

### No Smart Search? No Network.

Regular `greppy search` is **100% offline**. No network calls, ever.

---

## Performance

### Search Speed

| Project Size | Files | Chunks | Index Time | Search (cold) | Search (cached) |
|--------------|-------|--------|------------|---------------|-----------------|
| Small CLI | 26 | 64 | 215ms | 0.87ms | 0.01ms |
| Medium webapp | 1,236 | 10,516 | 493ms | 1.34ms | 0.04ms |
| Large monorepo | 50,000+ | 500,000+ | ~30s | <5ms | <0.1ms |

### Smart Search Performance

| Scenario | Time | Notes |
|----------|------|-------|
| Cache hit (L1 memory) | **<1ms** | Most common after warmup |
| Cache hit (L2 file) | **<5ms** | After restart |
| Cache hit (fuzzy) | **<10ms** | Similar queries |
| Cache miss (API) | **~2-3s** | First time only, then cached |

### vs Traditional Approaches

| Approach | Latency | Why |
|----------|---------|-----|
| grep + context stuff | 3-5s | File I/O + LLM processing |
| Embeddings search | 200-500ms | API round-trip |
| **Greppy regular** | **<1ms** | In-memory index |
| **Greppy smart (cached)** | **<10ms** | Local cache + in-memory index |

---

## Features

### Core Search
- **Sub-millisecond BM25 search** — Tantivy-powered full-text search
- **Symbol boosting** — Functions, classes, methods ranked higher
- **AST-aware chunking** — Tree-sitter parsing for 25+ languages
- **Multi-factor scoring** — Export bonus, test penalty, recency

### Smart Search (v0.2.0)
- **Intent detection** — Understands what you're looking for
- **Query expansion** — Adds synonyms and related terms
- **Multi-tier caching** — L1 memory + L2 persistent + fuzzy matching
- **HTTP/2 optimized** — Connection pooling for faster API calls
- **OAuth authentication** — Secure Anthropic integration
- **Graceful fallback** — Works without auth (regular search)

### Developer Experience
- **Auto-watch** — Reindexes on file changes
- **Query cache** — Repeated queries in <0.1ms
- **JSON output** — Easy AI tool integration
- **Multi-project** — Index multiple codebases

---

## Commands

| Command | Description |
|---------|-------------|
| `greppy start` | Start background daemon |
| `greppy stop` | Stop daemon |
| `greppy status` | Show daemon status |
| `greppy index` | Index current project |
| `greppy index --force` | Force full re-index |
| `greppy search <query>` | Search code |
| `greppy search --smart <query>` | AI-enhanced search |
| `greppy search -l 10` | Limit results |
| `greppy search --json` | JSON output |
| `greppy list` | List indexed projects |
| `greppy forget` | Remove project from index |
| `greppy auth login` | Authenticate with Anthropic |
| `greppy auth logout` | Clear credentials |
| `greppy auth status` | Check auth status |

---

## JSON Output

For AI tool integration:

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
          "symbol_type": "class",
          "start_line": 1,
          "end_line": 45,
          "language": "typescript",
          "score": 8.5,
          "is_exported": true,
          "is_test": false
        }
      ],
      "elapsed_ms": 0.87,
      "cached": false
    }
  }
}
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              Greppy                                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────────┐     ┌──────────────────────────────────────────────┐  │
│  │   CLI        │     │              Daemon Process                   │  │
│  │              │     │                                               │  │
│  │ greppy search│────▶│  ┌─────────────┐  ┌───────────────────────┐  │  │
│  │ greppy index │     │  │   Tantivy   │  │     Query Cache       │  │  │
│  │ greppy auth  │     │  │   Indexes   │  │   (1000 queries)      │  │  │
│  │              │     │  └─────────────┘  └───────────────────────┘  │  │
│  └──────────────┘     │                                               │  │
│         │             │  ┌─────────────┐  ┌───────────────────────┐  │  │
│         │             │  │ File Watcher│  │   Tree-sitter AST     │  │  │
│         ▼             │  │ (debounced) │  │   (25+ languages)     │  │  │
│  ┌──────────────┐     │  └─────────────┘  └───────────────────────┘  │  │
│  │ Smart Search │     │                                               │  │
│  │ ┌──────────┐ │     └──────────────────────────────────────────────┘  │
│  │ │LLM Cache │ │                        ▲                            │
│  │ │L1+L2     │ │                        │                            │
│  │ └──────────┘ │                        │                            │
│  │      │       │                        │                            │
│  │      ▼       │                        │                            │
│  │ ┌──────────┐ │                        │                            │
│  │ │Claude API│ │ (query only,           │                            │
│  │ │(optional)│ │  no code sent)         │                            │
│  │ └──────────┘ │                        │                            │
│  └──────────────┘────────────────────────┘                            │
│                    Unix Socket IPC                                       │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Supported Languages

Greppy uses Tree-sitter for AST-aware parsing:

**Tier 1** (full symbol extraction):
`Rust` • `TypeScript` • `JavaScript` • `Python` • `Go` • `Java` • `C` • `C++`

**Tier 2** (symbol extraction):
`C#` • `Ruby` • `PHP` • `Swift` • `Kotlin` • `Scala` • `Elixir` • `Haskell`

**Tier 3** (text indexing):
`Shell` • `HTML` • `CSS` • `SQL` • `Lua` • `Zig` • `Dart` • `Vue` • `Svelte` • and more...

---

## Configuration

### Data Location

- **macOS**: `~/Library/Application Support/dev.greppy.greppy/`
- **Linux**: `~/.local/share/greppy/`

Override with:
```bash
export GREPPY_HOME=/custom/path
```

### Auth Storage

Credentials stored in `~/.config/greppy/auth.json` (excluded from git).

---

## Building from Source

```bash
git clone https://github.com/KBLCode/greppy.git
cd greppy
cargo build --release
./target/release/greppy --help
```

---

## Roadmap

- [x] **v0.1.0** — BM25 search, daemon, file watching
- [x] **v0.2.0** — Smart search with Claude, OAuth, multi-tier caching
- [ ] **v0.3.0** — Token-aware output, incremental indexing
- [ ] **v0.4.0** — IDE plugins, cross-project search

---

## License

MIT License — see [LICENSE](LICENSE).

---

## Credits

Built with [Tantivy](https://github.com/quickwit-oss/tantivy), [Tree-sitter](https://tree-sitter.github.io/), [Tokio](https://tokio.rs/), and [Clap](https://github.com/clap-rs/clap).

---

<p align="center">
  <strong>🔒 Local-first. Lightning-fast. AI-ready.</strong>
</p>

<p align="center">
  <a href="https://github.com/KBLCode/greppy">GitHub</a> •
  <a href="https://github.com/KBLCode/greppy/issues">Issues</a> •
  <a href="https://github.com/KBLCode/greppy/releases">Releases</a>
</p>
