# Greppy - Product Requirements Document

> Sub-millisecond local semantic code search for AI coding tools.

**Version:** 1.0.0-draft  
**Last Updated:** January 7, 2026  
**Status:** Design Phase

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Problem Statement](#2-problem-statement)
3. [Solution Overview](#3-solution-overview)
4. [User Personas](#4-user-personas)
5. [User Stories](#5-user-stories)
6. [Functional Requirements](#6-functional-requirements)
7. [Technical Architecture](#7-technical-architecture)
8. [Data Model](#8-data-model)
9. [API Specification](#9-api-specification)
10. [CLI Specification](#10-cli-specification)
11. [Performance Requirements](#11-performance-requirements)
12. [Security Requirements](#12-security-requirements)
13. [Error Handling](#13-error-handling)
14. [Caching Strategy](#14-caching-strategy)
15. [File System Watching](#15-file-system-watching)
16. [Multi-Project Support](#16-multi-project-support)
17. [Language Support](#17-language-support)
18. [Installation & Distribution](#18-installation--distribution)
19. [Configuration](#19-configuration)
20. [Observability](#20-observability)
21. [Testing Strategy](#21-testing-strategy)
22. [Release Plan](#22-release-plan)
23. [Success Metrics](#23-success-metrics)
24. [Open Questions](#24-open-questions)
25. [Appendix](#25-appendix)

---

## 1. Executive Summary

### What is Greppy?

Greppy is a local, open-source code search engine that provides sub-millisecond semantic search for AI coding tools. It's a single Rust binary that indexes codebases using Tantivy (BM25) and tree-sitter (AST parsing), exposing results via a simple CLI.

### Why Greppy?

| Problem | mgrep (Current) | Greppy (Solution) |
|---------|-----------------|-------------------|
| Speed | 100-200ms (network) | **0.3-10ms (local)** |
| Cost | $20/mo | **Free** |
| Privacy | Code uploaded to cloud | **100% local** |
| Offline | No | **Yes** |
| Setup | Account + API key + config | **One command** |
| Works with | MCP clients only | **Any tool** |

### Key Insight

> The LLM is already the semantic layer. We don't need embeddings because the agent LLM reads and understands the results. We just need fast, accurate retrieval.

---

## 2. Problem Statement

### Current State

AI coding tools (Claude Code, OpenCode, Cursor, Aider) need to search codebases to answer questions like "where is authentication handled?" Current solutions:

1. **grep/ripgrep**: Fast but literal matching only. No semantic understanding.
2. **mgrep**: Semantic search via cloud embeddings. Slow (100-200ms), expensive ($20/mo), privacy concerns.
3. **Built-in search**: Each tool implements its own, inconsistent quality.

### Pain Points

1. **Latency**: 100-200ms per search adds up. 50 searches = 5-10 seconds of waiting.
2. **Cost**: $20/mo for mgrep, per user.
3. **Privacy**: Code uploaded to third-party cloud for embedding.
4. **Offline**: Cloud solutions don't work offline.
5. **Setup friction**: Account creation, API keys, MCP configuration.

### Opportunity

A local search engine that:
- Returns results in <10ms (100x faster than mgrep)
- Costs nothing (open source)
- Never uploads code (100% local)
- Works offline
- Requires zero configuration
- Works with ANY tool that can run shell commands

---

## 3. Solution Overview

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Greppy                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ANY AI TOOL (shells out)                                                  │
│   ├── Claude Code                                                           │
│   ├── OpenCode                                                              │
│   ├── Cursor                                                                │
│   ├── Aider                                                                 │
│   ├── Cline                                                                 │
│   └── Any tool that can run commands                                        │
│                              │                                              │
│                              ▼                                              │
│                    ┌─────────────────┐                                      │
│                    │  greppy search  │  ← Single binary                     │
│                    └────────┬────────┘                                      │
│                             │                                               │
│              ┌──────────────┴──────────────┐                                │
│              ▼                             ▼                                │
│     ┌─────────────────┐          ┌─────────────────┐                        │
│     │  Direct Mode    │          │  Daemon Mode    │                        │
│     │  ~5-10ms        │          │  ~0.3-0.5ms     │                        │
│     └─────────────────┘          └─────────────────┘                        │
│                                          │                                  │
│                                          ▼                                  │
│                                 ┌─────────────────┐                         │
│                                 │  Tantivy Index  │                         │
│                                 │  (memory-mapped)│                         │
│                                 └─────────────────┘                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Core Components

| Component | Technology | Purpose |
|-----------|------------|---------|
| CLI | Rust + clap | User interface |
| Search Engine | Tantivy | BM25 full-text search |
| AST Parser | tree-sitter | Semantic chunking |
| IPC | Unix socket | Daemon communication |
| Index Storage | Memory-mapped files | Fast access |
| File Watcher | notify-rs | Live updates |

### Why Not MCP?

MCP adds overhead and complexity:

| | MCP | Shell |
|-|-----|-------|
| Setup | Config JSON files | **None** |
| Latency | +1-2ms protocol overhead | **0ms** |
| Compatibility | MCP clients only | **Any tool** |
| Complexity | JSON-RPC, stdio, parsing | **Just run command** |

Shell is universal. Every AI tool can run `greppy search "query"`.

---

## 4. User Personas

### Persona 1: AI-Assisted Developer

**Name:** Alex  
**Role:** Full-stack developer using Claude Code  
**Goals:**
- Find relevant code quickly when asking Claude questions
- Not wait for slow search results
- Not pay for another subscription

**Pain Points:**
- mgrep is slow and costs money
- grep doesn't understand semantic queries
- Switching between tools breaks flow

**How Greppy Helps:**
- Sub-10ms search results
- Free forever
- Works seamlessly with Claude Code

### Persona 2: Privacy-Conscious Developer

**Name:** Jordan  
**Role:** Developer at security-focused company  
**Goals:**
- Never upload code to third parties
- Work offline on planes/trains
- Comply with company security policies

**Pain Points:**
- mgrep uploads code to cloud
- Can't use cloud tools due to policy
- No good local alternatives

**How Greppy Helps:**
- 100% local, code never leaves machine
- Works completely offline
- Open source, auditable

### Persona 3: Multi-Tool User

**Name:** Sam  
**Role:** Developer using multiple AI tools  
**Goals:**
- Same search experience across tools
- Not configure each tool separately
- Switch tools without friction

**Pain Points:**
- Each tool has different search capabilities
- MCP config is tool-specific
- Inconsistent results across tools

**How Greppy Helps:**
- Works with any tool via shell
- One install, works everywhere
- Consistent results

---

## 5. User Stories

### Epic 1: Basic Search

| ID | Story | Priority | Acceptance Criteria |
|----|-------|----------|---------------------|
| US-1.1 | As a developer, I want to search my codebase with a query so I can find relevant code | P0 | `greppy search "auth"` returns matching code chunks |
| US-1.2 | As a developer, I want search results to show file path and line numbers so I can navigate to the code | P0 | Results include `path:line` format |
| US-1.3 | As a developer, I want to limit the number of results so I don't get overwhelmed | P0 | `--limit 10` flag works |
| US-1.4 | As a developer, I want JSON output so my tools can parse results | P0 | `--json` flag outputs valid JSON |
| US-1.5 | As a developer, I want to search only in specific directories so I can scope my search | P1 | `--path src/` flag works |

### Epic 2: Indexing

| ID | Story | Priority | Acceptance Criteria |
|----|-------|----------|---------------------|
| US-2.1 | As a developer, I want my project indexed automatically on first search so I don't have to run a separate command | P0 | First search triggers indexing |
| US-2.2 | As a developer, I want to manually trigger indexing so I can refresh the index | P1 | `greppy index` command works |
| US-2.3 | As a developer, I want the index to update when files change so search results are current | P1 | Watch mode detects changes |
| US-2.4 | As a developer, I want to see indexing progress so I know it's working | P2 | Progress bar during indexing |

### Epic 3: Multi-Project

| ID | Story | Priority | Acceptance Criteria |
|----|-------|----------|---------------------|
| US-3.1 | As a developer, I want Greppy to auto-detect my project root so I don't have to specify it | P0 | Finds `.git`, `package.json`, etc. |
| US-3.2 | As a developer, I want separate indexes per project so they don't interfere | P0 | Each project has own index |
| US-3.3 | As a developer, I want to list my indexed projects so I can see what's cached | P1 | `greppy list` shows projects |
| US-3.4 | As a developer, I want to remove a project's index so I can free space | P2 | `greppy forget <path>` works |

### Epic 4: Daemon Mode

| ID | Story | Priority | Acceptance Criteria |
|----|-------|----------|---------------------|
| US-4.1 | As a developer, I want a daemon mode so searches are faster | P1 | `greppy daemon start` keeps index warm |
| US-4.2 | As a developer, I want the daemon to auto-start on boot so I don't have to remember | P2 | `--install` adds to startup |
| US-4.3 | As a developer, I want to check daemon status so I know if it's running | P1 | `greppy daemon status` works |

### Epic 5: Installation

| ID | Story | Priority | Acceptance Criteria |
|----|-------|----------|---------------------|
| US-5.1 | As a developer, I want to install via Homebrew so it's easy on macOS | P0 | `brew install greppy` works |
| US-5.2 | As a developer, I want to install via curl so it works on any Unix | P0 | curl installer works |
| US-5.3 | As a developer, I want to install via Cargo so I can build from source | P1 | `cargo install greppy` works |

---

## 6. Functional Requirements

### FR-1: Search

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1.1 | MUST support free-text search queries | P0 |
| FR-1.2 | MUST return results ranked by relevance (BM25) | P0 |
| FR-1.3 | MUST include file path, line numbers, and code snippet in results | P0 |
| FR-1.4 | MUST support `--limit N` to cap results (default: 20, max: 100) | P0 |
| FR-1.5 | MUST support `--json` for machine-readable output | P0 |
| FR-1.6 | MUST support `--path <dir>` to scope search | P1 |
| FR-1.7 | SHOULD support `--include <glob>` for file patterns | P2 |
| FR-1.8 | SHOULD support `--exclude <glob>` for exclusions | P2 |
| FR-1.9 | SHOULD boost matches in symbol names (functions, classes) | P1 |
| FR-1.10 | SHOULD deprioritize test files and generated code | P1 |

### FR-2: Indexing

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-2.1 | MUST index on first search if no index exists | P0 |
| FR-2.2 | MUST support manual indexing via `greppy index` | P0 |
| FR-2.3 | MUST respect `.gitignore` patterns | P0 |
| FR-2.4 | MUST store index in `~/.greppy/indexes/<hash>/` | P0 |
| FR-2.5 | SHOULD support incremental indexing (only changed files) | P1 |
| FR-2.6 | SHOULD support watch mode for live updates | P1 |
| FR-2.7 | SHOULD use tree-sitter for AST-aware chunking | P1 |
| FR-2.8 | SHOULD show progress during indexing | P2 |

### FR-3: Project Detection

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-3.1 | MUST auto-detect project root by walking up from cwd | P0 |
| FR-3.2 | MUST recognize: `.git`, `package.json`, `Cargo.toml`, `pyproject.toml`, `go.mod`, `.greppy` | P0 |
| FR-3.3 | MUST support explicit project path via `--project <path>` | P0 |
| FR-3.4 | MUST maintain separate indexes per project | P0 |
| FR-3.5 | SHOULD support `greppy list` to show indexed projects | P1 |
| FR-3.6 | SHOULD support `greppy forget <path>` to remove index | P2 |

### FR-4: Daemon

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-4.1 | MUST support daemon mode via `greppy daemon start` | P1 |
| FR-4.2 | MUST communicate via Unix socket at `~/.greppy/daemon.sock` | P1 |
| FR-4.3 | MUST fall back to direct mode if daemon not running | P0 |
| FR-4.4 | SHOULD support `greppy daemon stop` | P1 |
| FR-4.5 | SHOULD support `greppy daemon status` | P1 |
| FR-4.6 | SHOULD support `--install` to add to system startup | P2 |
| FR-4.7 | SHOULD watch all configured projects for changes | P2 |

### FR-5: Output

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-5.1 | MUST output human-readable format by default | P0 |
| FR-5.2 | MUST support `--json` for structured output | P0 |
| FR-5.3 | MUST include in each result: path, start_line, end_line, content, score | P0 |
| FR-5.4 | SHOULD syntax-highlight code in terminal output | P2 |
| FR-5.5 | SHOULD truncate long snippets with `...` | P1 |

---

## 7. Technical Architecture

### 7.1 Component Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                 CLI Layer                                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │   search    │  │    index    │  │   daemon    │  │    list     │        │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘        │
└─────────┼────────────────┼────────────────┼────────────────┼────────────────┘
          │                │                │                │
          ▼                ▼                ▼                ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Core Layer                                     │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        Project Manager                               │   │
│  │  - Detect project root                                               │   │
│  │  - Manage index paths                                                │   │
│  │  - Track indexed projects                                            │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│          ┌─────────────────────────┼─────────────────────────┐             │
│          ▼                         ▼                         ▼             │
│  ┌───────────────┐        ┌───────────────┐        ┌───────────────┐       │
│  │    Indexer    │        │   Searcher    │        │    Watcher    │       │
│  │  - Parse AST  │        │  - BM25 query │        │  - fs events  │       │
│  │  - Chunk code │        │  - Rank/boost │        │  - Incremental│       │
│  │  - Write idx  │        │  - Format out │        │  - Debounce   │       │
│  └───────┬───────┘        └───────┬───────┘        └───────┬───────┘       │
│          │                        │                        │               │
└──────────┼────────────────────────┼────────────────────────┼───────────────┘
           │                        │                        │
           ▼                        ▼                        ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Storage Layer                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         Tantivy Index                                │   │
│  │  ~/.greppy/indexes/<project-hash>/                                   │   │
│  │  - Memory-mapped                                                     │   │
│  │  - BM25 scoring                                                      │   │
│  │  - Compressed postings                                               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         Metadata Store                               │   │
│  │  ~/.greppy/projects.json                                             │   │
│  │  - Project paths                                                     │   │
│  │  - Last indexed time                                                 │   │
│  │  - File counts                                                       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Daemon Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Daemon Process                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────┐                                                        │
│  │  Unix Socket    │ ← ~/.greppy/daemon.sock                               │
│  │  Listener       │                                                        │
│  └────────┬────────┘                                                        │
│           │                                                                 │
│           ▼                                                                 │
│  ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐       │
│  │ Request Handler │────►│  Index Manager  │────►│  File Watcher   │       │
│  │ (async, pooled) │     │  (warm indexes) │     │  (all projects) │       │
│  └─────────────────┘     └─────────────────┘     └─────────────────┘       │
│           │                      │                       │                  │
│           │                      ▼                       │                  │
│           │              ┌─────────────────┐             │                  │
│           │              │  LRU Cache      │             │                  │
│           │              │  (recent queries)│            │                  │
│           │              └─────────────────┘             │                  │
│           │                      │                       │                  │
│           └──────────────────────┼───────────────────────┘                  │
│                                  ▼                                          │
│                         ┌─────────────────┐                                 │
│                         │  Tantivy Indexes│                                 │
│                         │  (memory-mapped)│                                 │
│                         └─────────────────┘                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 7.3 Technology Stack

| Layer | Technology | Version | Justification |
|-------|------------|---------|---------------|
| Language | Rust | 1.75+ | Performance, safety, single binary |
| CLI | clap | 4.x | Best Rust CLI framework |
| Search | tantivy | 0.22+ | 2x faster than Lucene, memory-mapped |
| AST | tree-sitter | 0.22+ | Incremental, multi-language |
| Async | tokio | 1.x | Async runtime for daemon |
| IPC | Unix sockets | - | Low latency, simple |
| Serialization | serde + JSON | - | Universal compatibility |
| File watching | notify | 6.x | Cross-platform fs events |
| Hashing | xxhash | - | Fast path hashing |

### 7.4 Crate Dependencies

```toml
[dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# Search
tantivy = "0.22"

# AST parsing
tree-sitter = "0.22"
tree-sitter-typescript = "0.21"
tree-sitter-python = "0.21"
tree-sitter-rust = "0.21"
tree-sitter-go = "0.21"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# File system
notify = "6"
ignore = "0.4"  # gitignore support
walkdir = "2"

# Utilities
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
xxhash-rust = { version = "0.8", features = ["xxh3"] }
directories = "5"  # XDG paths
```

---

## 8. Data Model

### 8.1 Index Schema (Tantivy)

```rust
/// Document stored in Tantivy index
struct IndexedChunk {
    /// Unique identifier: "{file_path}:{start_line}:{end_line}"
    id: String,
    
    /// Relative file path from project root
    path: String,
    
    /// Starting line number (1-indexed)
    start_line: u64,
    
    /// Ending line number (1-indexed)
    end_line: u64,
    
    /// The actual code content
    content: String,
    
    /// Symbol name if this chunk is a function/class/method
    symbol_name: Option<String>,
    
    /// Symbol kind: "function", "class", "method", "module", etc.
    symbol_kind: Option<String>,
    
    /// Language identifier: "typescript", "python", "rust", etc.
    language: String,
    
    /// File modification timestamp (for staleness detection)
    modified_at: u64,
    
    /// Boost factors (stored for ranking)
    is_test: bool,
    is_generated: bool,
}
```

### 8.2 Project Metadata

```rust
/// Stored in ~/.greppy/projects.json
struct ProjectsMetadata {
    version: u32,
    projects: Vec<ProjectInfo>,
}

struct ProjectInfo {
    /// Absolute path to project root
    path: String,
    
    /// Hash of path (used for index directory name)
    hash: String,
    
    /// Last time index was updated
    last_indexed: u64,
    
    /// Number of files in index
    file_count: u64,
    
    /// Number of chunks in index
    chunk_count: u64,
    
    /// Total index size in bytes
    index_size: u64,
}
```

### 8.3 Search Result

```rust
/// Returned from search
struct SearchResult {
    /// Relative file path
    path: String,
    
    /// Starting line (1-indexed)
    start_line: u64,
    
    /// Ending line (1-indexed)
    end_line: u64,
    
    /// Code content
    content: String,
    
    /// Symbol name if applicable
    symbol_name: Option<String>,
    
    /// Symbol kind if applicable
    symbol_kind: Option<String>,
    
    /// BM25 relevance score
    score: f32,
}
```

### 8.4 Daemon Protocol

```rust
/// Request sent to daemon via Unix socket
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum DaemonRequest {
    Search {
        project_path: String,
        query: String,
        limit: u32,
        path_filter: Option<String>,
    },
    Index {
        project_path: String,
    },
    Status,
    Shutdown,
}

/// Response from daemon
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum DaemonResponse {
    SearchResults {
        results: Vec<SearchResult>,
        took_ms: f64,
    },
    IndexComplete {
        file_count: u64,
        chunk_count: u64,
        took_ms: f64,
    },
    Status {
        running: bool,
        projects: Vec<ProjectInfo>,
        uptime_seconds: u64,
    },
    Error {
        code: String,
        message: String,
    },
}
```

---

## 9. API Specification

### 9.1 CLI Commands

#### `greppy search <query>`

Search the codebase for relevant code.

**Arguments:**
| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `query` | string | Yes | Search query |

**Flags:**
| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--limit` | `-l` | u32 | 20 | Max results (1-100) |
| `--project` | `-p` | path | auto | Project path |
| `--path` | | path | | Scope to subdirectory |
| `--json` | `-j` | bool | false | JSON output |
| `--include` | | glob | | Include file patterns |
| `--exclude` | | glob | | Exclude file patterns |

**Exit Codes:**
| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | No results found |
| 2 | Project not found |
| 3 | Index error |
| 4 | Invalid arguments |

**Example:**
```bash
$ greppy search "authentication middleware"

src/auth/middleware.ts:15-42
│ export function authMiddleware(req: Request) {
│   const token = req.headers.authorization;
│   if (!token) return unauthorized();
│   const user = validateToken(token);
│   ...

src/auth/jwt.ts:8-25
│ export function validateToken(token: string): User | null {
│   try {
│     return jwt.verify(token, SECRET);
│   ...

Found 12 results in 3ms
```

**JSON Output:**
```json
{
  "results": [
    {
      "path": "src/auth/middleware.ts",
      "start_line": 15,
      "end_line": 42,
      "content": "export function authMiddleware...",
      "symbol_name": "authMiddleware",
      "symbol_kind": "function",
      "score": 12.45
    }
  ],
  "total": 12,
  "took_ms": 3.2
}
```

#### `greppy index [path]`

Index a project.

**Arguments:**
| Argument | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `path` | path | No | `.` | Project path |

**Flags:**
| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--watch` | `-w` | bool | false | Watch for changes |
| `--force` | `-f` | bool | false | Rebuild from scratch |
| `--recursive` | `-r` | bool | false | Find sub-projects |

**Example:**
```bash
$ greppy index

Indexing /Users/you/project...
  Scanning files... 2,847 files
  Parsing AST... done
  Building index... done

Indexed 2,847 files (15,234 chunks) in 1.2s
```

#### `greppy daemon <command>`

Manage the background daemon.

**Subcommands:**
| Command | Description |
|---------|-------------|
| `start` | Start daemon |
| `stop` | Stop daemon |
| `status` | Show status |
| `restart` | Restart daemon |

**Flags (start):**
| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--install` | bool | false | Add to system startup |

**Example:**
```bash
$ greppy daemon start
Daemon started (pid 12345)

$ greppy daemon status
Daemon running (pid 12345, uptime 2h 15m)
Watching 3 projects:
  ~/Dev/project-a     2,847 files   Updated 2 min ago
  ~/Dev/project-b     1,203 files   Updated 5 min ago
  ~/work/client       8,421 files   Updated 1 hour ago
```

#### `greppy list`

List indexed projects.

**Example:**
```bash
$ greppy list

Indexed projects:
  ~/Dev/project-a     2,847 files   15.2 MB   Updated 2 min ago
  ~/Dev/project-b     1,203 files    6.8 MB   Updated 5 min ago
  ~/work/client       8,421 files   42.1 MB   Updated 1 hour ago

Total: 3 projects, 64.1 MB
```

#### `greppy forget <path>`

Remove a project's index.

**Example:**
```bash
$ greppy forget ~/Dev/old-project
Removed index for ~/Dev/old-project (freed 12.3 MB)
```

---

## 10. CLI Specification

### 10.1 Output Formats

#### Human-Readable (Default)

```
<path>:<start_line>-<end_line>
│ <code line 1>
│ <code line 2>
│ ...

<path>:<start_line>-<end_line>
│ <code line 1>
│ ...

Found <N> results in <time>ms
```

#### JSON (`--json`)

```json
{
  "results": [
    {
      "path": "string",
      "start_line": 0,
      "end_line": 0,
      "content": "string",
      "symbol_name": "string | null",
      "symbol_kind": "string | null",
      "score": 0.0
    }
  ],
  "total": 0,
  "took_ms": 0.0
}
```

### 10.2 Error Output

#### Human-Readable

```
Error: <message>

<suggestion if applicable>
```

#### JSON

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable message"
  }
}
```

### 10.3 Error Codes

| Code | HTTP Equiv | Description |
|------|------------|-------------|
| `NO_PROJECT` | 404 | No project found at path |
| `NO_INDEX` | 404 | Project not indexed |
| `INDEX_ERROR` | 500 | Failed to read/write index |
| `PARSE_ERROR` | 400 | Invalid query or arguments |
| `DAEMON_ERROR` | 503 | Daemon communication failed |
| `IO_ERROR` | 500 | File system error |

---

## 11. Performance Requirements

### 11.1 Latency Targets

| Operation | Target | Max Acceptable |
|-----------|--------|----------------|
| Search (daemon, warm) | <1ms | 5ms |
| Search (direct, warm) | <10ms | 50ms |
| Search (direct, cold) | <100ms | 500ms |
| Index (1k files) | <1s | 3s |
| Index (10k files) | <5s | 15s |
| Index (50k files) | <20s | 60s |
| Incremental update (1 file) | <100ms | 500ms |

### 11.2 Memory Targets

| Scenario | Target | Max Acceptable |
|----------|--------|----------------|
| CLI (no daemon) | <50MB | 100MB |
| Daemon (idle) | <100MB | 200MB |
| Daemon (10 projects) | <500MB | 1GB |
| Index size (per 1k files) | <5MB | 10MB |

### 11.3 Throughput Targets

| Metric | Target |
|--------|--------|
| Queries per second (daemon) | >1000 |
| Files indexed per second | >500 |
| Concurrent searches | >100 |

### 11.4 Benchmarks

Must pass these benchmarks before release:

```rust
#[bench]
fn search_small_project_1k_files() {
    // Target: <1ms (daemon), <10ms (direct)
}

#[bench]
fn search_medium_project_10k_files() {
    // Target: <2ms (daemon), <15ms (direct)
}

#[bench]
fn search_large_project_50k_files() {
    // Target: <5ms (daemon), <25ms (direct)
}

#[bench]
fn index_1k_files() {
    // Target: <1s
}

#[bench]
fn index_10k_files() {
    // Target: <5s
}

#[bench]
fn incremental_update_single_file() {
    // Target: <100ms
}
```

---

## 12. Security Requirements

### 12.1 Data Security

| ID | Requirement | Priority |
|----|-------------|----------|
| SEC-1.1 | MUST NOT transmit any code or data over network | P0 |
| SEC-1.2 | MUST store indexes with user-only permissions (0600) | P0 |
| SEC-1.3 | MUST NOT log file contents | P0 |
| SEC-1.4 | MUST sanitize paths to prevent directory traversal | P0 |
| SEC-1.5 | SHOULD support encrypted index storage | P2 |

### 12.2 Input Validation

| ID | Requirement | Priority |
|----|-------------|----------|
| SEC-2.1 | MUST validate all CLI arguments | P0 |
| SEC-2.2 | MUST validate daemon socket messages | P0 |
| SEC-2.3 | MUST reject paths outside project root | P0 |
| SEC-2.4 | MUST limit query length (max 1000 chars) | P0 |
| SEC-2.5 | MUST limit result count (max 100) | P0 |

### 12.3 Process Security

| ID | Requirement | Priority |
|----|-------------|----------|
| SEC-3.1 | Daemon MUST run as user, not root | P0 |
| SEC-3.2 | Unix socket MUST have user-only permissions | P0 |
| SEC-3.3 | MUST NOT execute any code from indexed files | P0 |
| SEC-3.4 | SHOULD drop privileges after startup | P1 |

### 12.4 Path Validation

```rust
/// Validate path is within project and safe
fn validate_path(project_root: &Path, requested: &Path) -> Result<PathBuf> {
    let canonical = requested.canonicalize()?;
    let root_canonical = project_root.canonicalize()?;
    
    if !canonical.starts_with(&root_canonical) {
        return Err(Error::PathTraversal);
    }
    
    // Check for suspicious patterns
    let path_str = canonical.to_string_lossy();
    if path_str.contains("..") || path_str.contains('\0') {
        return Err(Error::InvalidPath);
    }
    
    Ok(canonical)
}
```

---

## 13. Error Handling

### 13.1 Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum GreppyError {
    #[error("Project not found: {path}")]
    ProjectNotFound { path: PathBuf },
    
    #[error("No index for project: {path}. Run 'greppy index' first.")]
    NoIndex { path: PathBuf },
    
    #[error("Index corrupted: {path}. Run 'greppy index --force' to rebuild.")]
    IndexCorrupted { path: PathBuf },
    
    #[error("Failed to parse query: {message}")]
    QueryParseError { message: String },
    
    #[error("Path traversal attempt blocked: {path}")]
    PathTraversal { path: PathBuf },
    
    #[error("Daemon not running. Start with 'greppy daemon start'.")]
    DaemonNotRunning,
    
    #[error("Daemon communication failed: {message}")]
    DaemonError { message: String },
    
    #[error("IO error: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },
    
    #[error("Index error: {source}")]
    TantivyError {
        #[from]
        source: tantivy::TantivyError,
    },
}
```

### 13.2 Error Recovery

| Error | Recovery Strategy |
|-------|-------------------|
| `ProjectNotFound` | Show helpful message with detection markers |
| `NoIndex` | Auto-index on search, or prompt user |
| `IndexCorrupted` | Suggest `--force` rebuild |
| `DaemonNotRunning` | Fall back to direct mode |
| `DaemonError` | Fall back to direct mode, log warning |
| `IoError` | Log, return error to user |

### 13.3 Graceful Degradation

```rust
async fn search(query: &str, project: &Path) -> Result<Vec<SearchResult>> {
    // Try daemon first (fastest)
    match daemon_search(query, project).await {
        Ok(results) => return Ok(results),
        Err(GreppyError::DaemonNotRunning) => {
            // Expected - fall through to direct mode
        }
        Err(GreppyError::DaemonError { message }) => {
            tracing::warn!("Daemon error, falling back to direct: {}", message);
        }
        Err(e) => return Err(e),
    }
    
    // Fall back to direct mode
    direct_search(query, project).await
}
```

---

## 14. Caching Strategy

### 14.1 Index Caching

| Cache | Location | TTL | Eviction |
|-------|----------|-----|----------|
| Tantivy index | `~/.greppy/indexes/<hash>/` | Permanent | Manual (`forget`) |
| Project metadata | `~/.greppy/projects.json` | Permanent | On project removal |
| Query cache (daemon) | In-memory | 60s | LRU (1000 entries) |

### 14.2 Query Cache (Daemon)

```rust
struct QueryCache {
    cache: LruCache<QueryKey, CachedResult>,
    max_size: usize,
    ttl: Duration,
}

#[derive(Hash, Eq, PartialEq)]
struct QueryKey {
    project_hash: String,
    query: String,
    limit: u32,
    path_filter: Option<String>,
}

struct CachedResult {
    results: Vec<SearchResult>,
    cached_at: Instant,
}

impl QueryCache {
    fn get(&mut self, key: &QueryKey) -> Option<Vec<SearchResult>> {
        let entry = self.cache.get(key)?;
        
        // Check TTL
        if entry.cached_at.elapsed() > self.ttl {
            self.cache.pop(key);
            return None;
        }
        
        Some(entry.results.clone())
    }
    
    fn set(&mut self, key: QueryKey, results: Vec<SearchResult>) {
        self.cache.put(key, CachedResult {
            results,
            cached_at: Instant::now(),
        });
    }
    
    fn invalidate_project(&mut self, project_hash: &str) {
        // Remove all entries for this project
        self.cache.retain(|k, _| k.project_hash != project_hash);
    }
}
```

### 14.3 Cache Invalidation

| Event | Invalidation |
|-------|--------------|
| File changed | Invalidate project's query cache |
| File deleted | Invalidate project's query cache |
| Index rebuilt | Invalidate project's query cache |
| Project removed | Remove all caches for project |

---

## 15. File System Watching

### 15.1 Watch Strategy

```rust
struct FileWatcher {
    watcher: RecommendedWatcher,
    debounce: Duration,
    pending_changes: HashMap<PathBuf, Instant>,
}

impl FileWatcher {
    fn new(debounce_ms: u64) -> Self {
        Self {
            watcher: notify::recommended_watcher(|res| {
                // Handle events
            }).unwrap(),
            debounce: Duration::from_millis(debounce_ms),
            pending_changes: HashMap::new(),
        }
    }
    
    fn watch_project(&mut self, path: &Path) -> Result<()> {
        self.watcher.watch(path, RecursiveMode::Recursive)?;
        Ok(())
    }
    
    fn handle_event(&mut self, event: Event) {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                for path in event.paths {
                    // Skip ignored files
                    if self.should_ignore(&path) {
                        continue;
                    }
                    
                    // Debounce: only process after quiet period
                    self.pending_changes.insert(path, Instant::now());
                }
            }
            _ => {}
        }
    }
    
    fn process_pending(&mut self) -> Vec<PathBuf> {
        let now = Instant::now();
        let ready: Vec<_> = self.pending_changes
            .iter()
            .filter(|(_, time)| now.duration_since(**time) > self.debounce)
            .map(|(path, _)| path.clone())
            .collect();
        
        for path in &ready {
            self.pending_changes.remove(path);
        }
        
        ready
    }
    
    fn should_ignore(&self, path: &Path) -> bool {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        
        // Common ignores
        name.starts_with('.') ||
        name == "node_modules" ||
        name == "target" ||
        name == "__pycache__" ||
        name == "dist" ||
        name == "build"
    }
}
```

### 15.2 Debouncing

| Scenario | Debounce Time |
|----------|---------------|
| Single file save | 100ms |
| Batch file operations | 500ms |
| Git checkout | 1000ms |

### 15.3 Incremental Updates

```rust
async fn update_index_incrementally(
    index: &mut Index,
    changed_files: Vec<PathBuf>,
) -> Result<()> {
    for path in changed_files {
        if path.exists() {
            // File created or modified
            let chunks = parse_file(&path)?;
            index.delete_by_path(&path)?;
            index.add_chunks(chunks)?;
        } else {
            // File deleted
            index.delete_by_path(&path)?;
        }
    }
    
    index.commit()?;
    Ok(())
}
```

---

## 16. Multi-Project Support

### 16.1 Project Detection

```rust
const PROJECT_MARKERS: &[&str] = &[
    ".git",
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    ".greppy",
];

fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    
    loop {
        for marker in PROJECT_MARKERS {
            if current.join(marker).exists() {
                return Some(current);
            }
        }
        
        if !current.pop() {
            return None;
        }
    }
}
```

### 16.2 Index Path Calculation

```rust
fn get_index_path(project_root: &Path) -> PathBuf {
    let hash = xxh3_64(project_root.to_string_lossy().as_bytes());
    let hash_str = format!("{:016x}", hash);
    
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("~/.greppy"))
        .join("greppy")
        .join("indexes")
        .join(hash_str)
}
```

### 16.3 Recursive Project Discovery

```rust
fn discover_projects(root: &Path) -> Vec<PathBuf> {
    let mut projects = Vec::new();
    
    for entry in WalkDir::new(root)
        .max_depth(5)  // Don't go too deep
        .into_iter()
        .filter_entry(|e| !is_ignored(e))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        
        if entry.file_type().is_dir() {
            for marker in PROJECT_MARKERS {
                if entry.path().join(marker).exists() {
                    projects.push(entry.path().to_path_buf());
                    break;
                }
            }
        }
    }
    
    projects
}
```

---

## 17. Language Support

### 17.1 Supported Languages

| Language | tree-sitter Grammar | Priority | Chunking |
|----------|---------------------|----------|----------|
| TypeScript | tree-sitter-typescript | P0 | Function, class, method |
| JavaScript | tree-sitter-typescript | P0 | Function, class, method |
| Python | tree-sitter-python | P0 | Function, class, method |
| Rust | tree-sitter-rust | P0 | Function, impl, struct |
| Go | tree-sitter-go | P0 | Function, type, method |
| Java | tree-sitter-java | P1 | Class, method |
| C# | tree-sitter-c-sharp | P1 | Class, method |
| Ruby | tree-sitter-ruby | P1 | Class, method, module |
| PHP | tree-sitter-php | P2 | Class, function |
| C/C++ | tree-sitter-c/cpp | P2 | Function, struct |
| Swift | tree-sitter-swift | P2 | Function, class |
| Kotlin | tree-sitter-kotlin | P2 | Function, class |
| Other | N/A | P0 | Line-based (fallback) |

### 17.2 Language Detection

```rust
fn detect_language(path: &Path) -> Language {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    
    match ext {
        "ts" | "tsx" => Language::TypeScript,
        "js" | "jsx" | "mjs" | "cjs" => Language::JavaScript,
        "py" | "pyi" => Language::Python,
        "rs" => Language::Rust,
        "go" => Language::Go,
        "java" => Language::Java,
        "cs" => Language::CSharp,
        "rb" => Language::Ruby,
        "php" => Language::Php,
        "c" | "h" => Language::C,
        "cpp" | "cc" | "cxx" | "hpp" => Language::Cpp,
        "swift" => Language::Swift,
        "kt" | "kts" => Language::Kotlin,
        _ => Language::Unknown,
    }
}
```

### 17.3 AST Chunking Strategy

```rust
/// Chunk a file into semantic units
fn chunk_file(path: &Path, content: &str) -> Vec<Chunk> {
    let language = detect_language(path);
    
    match language {
        Language::Unknown => chunk_by_lines(content, 25),
        _ => chunk_by_ast(content, language),
    }
}

fn chunk_by_ast(content: &str, language: Language) -> Vec<Chunk> {
    let parser = get_parser(language);
    let tree = parser.parse(content, None)?;
    
    let mut chunks = Vec::new();
    let query = get_chunk_query(language);
    
    for capture in query.captures(&tree, content) {
        let node = capture.node;
        let start = node.start_position();
        let end = node.end_position();
        
        chunks.push(Chunk {
            start_line: start.row + 1,
            end_line: end.row + 1,
            content: content[node.byte_range()].to_string(),
            symbol_name: extract_symbol_name(&node, content),
            symbol_kind: capture.name.to_string(),
        });
    }
    
    chunks
}
```

### 17.4 tree-sitter Queries

**TypeScript/JavaScript:**
```scheme
; queries/typescript.scm
(function_declaration
  name: (identifier) @name) @function

(arrow_function) @function

(class_declaration
  name: (type_identifier) @name) @class

(method_definition
  name: (property_identifier) @name) @method

(export_statement
  declaration: (_) @export)
```

**Python:**
```scheme
; queries/python.scm
(function_definition
  name: (identifier) @name) @function

(class_definition
  name: (identifier) @name) @class

(decorated_definition) @decorated
```

**Rust:**
```scheme
; queries/rust.scm
(function_item
  name: (identifier) @name) @function

(impl_item) @impl

(struct_item
  name: (type_identifier) @name) @struct

(enum_item
  name: (type_identifier) @name) @enum
```

---

## 18. Installation & Distribution

### 18.1 Distribution Channels

| Channel | Command | Priority |
|---------|---------|----------|
| Homebrew | `brew install greppy` | P0 |
| curl installer | `curl -fsSL https://greppy.dev/install.sh \| sh` | P0 |
| Cargo | `cargo install greppy` | P1 |
| GitHub Releases | Direct download | P0 |
| npm | `npm install -g greppy` | P2 |

### 18.2 Pre-built Binaries

| Platform | Architecture | Filename |
|----------|--------------|----------|
| macOS | arm64 | `greppy-darwin-arm64` |
| macOS | x64 | `greppy-darwin-x64` |
| Linux | arm64 | `greppy-linux-arm64` |
| Linux | x64 | `greppy-linux-x64` |
| Windows | x64 | `greppy-windows-x64.exe` |

### 18.3 curl Installer

```bash
#!/bin/sh
set -e

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
    x86_64) ARCH="x64" ;;
    aarch64|arm64) ARCH="arm64" ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Download binary
BINARY="greppy-${OS}-${ARCH}"
URL="https://github.com/greppy/greppy/releases/latest/download/${BINARY}"

echo "Downloading Greppy..."
curl -fsSL "$URL" -o /tmp/greppy

# Install
chmod +x /tmp/greppy
sudo mv /tmp/greppy /usr/local/bin/greppy

echo "Greppy installed successfully!"
echo "Run 'greppy search \"your query\"' to get started."
```

### 18.4 Homebrew Formula

```ruby
class Greppy < Formula
  desc "Sub-millisecond local semantic code search"
  homepage "https://greppy.dev"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/greppy/greppy/releases/download/v#{version}/greppy-darwin-arm64"
      sha256 "..."
    else
      url "https://github.com/greppy/greppy/releases/download/v#{version}/greppy-darwin-x64"
      sha256 "..."
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/greppy/greppy/releases/download/v#{version}/greppy-linux-arm64"
      sha256 "..."
    else
      url "https://github.com/greppy/greppy/releases/download/v#{version}/greppy-linux-x64"
      sha256 "..."
    end
  end

  def install
    bin.install "greppy-#{OS}-#{Hardware::CPU.arch}" => "greppy"
  end

  test do
    system "#{bin}/greppy", "--version"
  end
end
```

---

## 19. Configuration

### 19.1 Configuration File

Location: `~/.greppy/config.toml`

```toml
# Greppy Configuration

[general]
# Default result limit
default_limit = 20

# Enable daemon auto-start
daemon_autostart = false

[watch]
# Directories to watch (daemon mode)
paths = [
    "~/Dev",
    "~/work"
]

# Recursively discover projects
recursive = true

# Debounce time in milliseconds
debounce_ms = 100

[ignore]
# Global ignore patterns (in addition to .gitignore)
patterns = [
    "node_modules",
    ".git",
    "dist",
    "build",
    "__pycache__",
    "*.min.js",
    "*.map"
]

[index]
# Maximum file size to index (bytes)
max_file_size = 1048576  # 1MB

# Maximum files per project
max_files = 100000

[cache]
# Query cache TTL (seconds)
query_ttl = 60

# Maximum cached queries
max_queries = 1000

[projects."~/Dev/special-project"]
# Project-specific overrides
ignore = ["generated/", "vendor/"]
```

### 19.2 Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `GREPPY_HOME` | Config/data directory | `~/.greppy` |
| `GREPPY_LOG` | Log level (trace, debug, info, warn, error) | `info` |
| `GREPPY_NO_COLOR` | Disable colored output | `false` |
| `GREPPY_DAEMON_SOCKET` | Custom socket path | `~/.greppy/daemon.sock` |

### 19.3 Configuration Loading

```rust
fn load_config() -> Config {
    let config_path = get_config_path();
    
    // Load from file if exists
    let file_config = if config_path.exists() {
        toml::from_str(&fs::read_to_string(&config_path)?)?
    } else {
        Config::default()
    };
    
    // Override with environment variables
    Config {
        log_level: env::var("GREPPY_LOG")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(file_config.log_level),
        home: env::var("GREPPY_HOME")
            .map(PathBuf::from)
            .unwrap_or(file_config.home),
        ..file_config
    }
}
```

---

## 20. Observability

### 20.1 Logging

```rust
use tracing::{info, warn, error, debug, trace, instrument};

#[instrument(skip(query), fields(project = %project.display()))]
async fn search(query: &str, project: &Path) -> Result<Vec<SearchResult>> {
    let start = Instant::now();
    
    debug!("Starting search");
    
    let results = do_search(query, project).await?;
    
    info!(
        results = results.len(),
        took_ms = start.elapsed().as_millis(),
        "Search completed"
    );
    
    Ok(results)
}
```

### 20.2 Log Format

```
2026-01-07T10:30:45.123Z INFO  greppy::search project=/Users/you/project results=12 took_ms=3 "Search completed"
2026-01-07T10:30:46.456Z DEBUG greppy::index file=src/auth.ts chunks=5 "File indexed"
2026-01-07T10:30:47.789Z WARN  greppy::daemon "Daemon connection failed, falling back to direct mode"
```

### 20.3 Metrics (Future)

| Metric | Type | Description |
|--------|------|-------------|
| `greppy_search_duration_ms` | Histogram | Search latency |
| `greppy_search_results` | Histogram | Results per search |
| `greppy_index_files` | Gauge | Files in index |
| `greppy_index_chunks` | Gauge | Chunks in index |
| `greppy_cache_hits` | Counter | Query cache hits |
| `greppy_cache_misses` | Counter | Query cache misses |

---

## 21. Testing Strategy

### 21.1 Test Categories

| Category | Coverage Target | Tools |
|----------|-----------------|-------|
| Unit tests | 80% | `cargo test` |
| Integration tests | Key paths | `cargo test --test '*'` |
| Benchmark tests | All perf targets | `cargo bench` |
| End-to-end tests | CLI commands | Shell scripts |

### 21.2 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_find_project_root() {
        let temp = tempdir().unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("package.json"), "{}").unwrap();
        
        let found = find_project_root(&project.join("src/deep/nested"));
        assert_eq!(found, Some(project));
    }
    
    #[test]
    fn test_chunk_by_lines() {
        let content = "line1\nline2\nline3\nline4\nline5";
        let chunks = chunk_by_lines(content, 2);
        
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].content, "line1\nline2");
        assert_eq!(chunks[1].content, "line3\nline4");
        assert_eq!(chunks[2].content, "line5");
    }
    
    #[test]
    fn test_path_validation_blocks_traversal() {
        let project = PathBuf::from("/home/user/project");
        let malicious = PathBuf::from("/home/user/project/../../../etc/passwd");
        
        let result = validate_path(&project, &malicious);
        assert!(matches!(result, Err(GreppyError::PathTraversal { .. })));
    }
}
```

### 21.3 Integration Tests

```rust
#[tokio::test]
async fn test_index_and_search() {
    let temp = tempdir().unwrap();
    let project = temp.path();
    
    // Create test files
    fs::write(project.join("auth.ts"), r#"
        export function authenticate(user: User) {
            return validateToken(user.token);
        }
    "#).unwrap();
    
    // Index
    let index = Index::create(project).await.unwrap();
    
    // Search
    let results = index.search("authenticate", 10).await.unwrap();
    
    assert_eq!(results.len(), 1);
    assert!(results[0].content.contains("authenticate"));
}
```

### 21.4 Benchmark Tests

```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_search(c: &mut Criterion) {
    let index = setup_test_index(10_000); // 10k files
    
    c.bench_function("search_10k_files", |b| {
        b.iter(|| {
            index.search("authentication", 20).unwrap()
        })
    });
}

fn bench_index(c: &mut Criterion) {
    let project = setup_test_project(1_000); // 1k files
    
    c.bench_function("index_1k_files", |b| {
        b.iter(|| {
            Index::create(&project).unwrap()
        })
    });
}

criterion_group!(benches, bench_search, bench_index);
criterion_main!(benches);
```

### 21.5 End-to-End Tests

```bash
#!/bin/bash
# test/e2e.sh

set -e

# Setup
TEMP=$(mktemp -d)
cd "$TEMP"
mkdir -p project/src
echo 'function auth() { return true; }' > project/src/auth.js

# Test: Search auto-indexes
OUTPUT=$(greppy search "auth" --project project)
echo "$OUTPUT" | grep -q "auth.js" || { echo "FAIL: search"; exit 1; }

# Test: JSON output
OUTPUT=$(greppy search "auth" --project project --json)
echo "$OUTPUT" | jq -e '.results[0].path' || { echo "FAIL: json"; exit 1; }

# Test: Limit
OUTPUT=$(greppy search "auth" --project project --limit 1 --json)
COUNT=$(echo "$OUTPUT" | jq '.results | length')
[ "$COUNT" -eq 1 ] || { echo "FAIL: limit"; exit 1; }

# Cleanup
rm -rf "$TEMP"

echo "All E2E tests passed!"
```

---

## 22. Release Plan

### 22.1 Milestones

| Version | Target Date | Features |
|---------|-------------|----------|
| v0.1.0-alpha | Week 1 | Basic search, line-based chunking, CLI |
| v0.1.0-beta | Week 2 | AST chunking (TS, Python), smart ranking |
| v0.1.0 | Week 3 | Daemon mode, multi-project, Homebrew |
| v0.2.0 | Week 5 | All P0 languages, watch mode, incremental |
| v0.3.0 | Week 7 | P1 languages, query cache, performance |
| v1.0.0 | Week 10 | Production-ready, all features |

### 22.2 v0.1.0-alpha Scope

**Must Have:**
- [ ] `greppy search <query>` with BM25 ranking
- [ ] `greppy index [path]` manual indexing
- [ ] Auto-detect project root
- [ ] Line-based chunking (fallback)
- [ ] JSON output (`--json`)
- [ ] Limit results (`--limit`)
- [ ] Basic error handling

**Nice to Have:**
- [ ] Progress bar during indexing
- [ ] Colored output

### 22.3 v0.1.0-beta Scope

**Must Have:**
- [ ] tree-sitter AST chunking (TypeScript, Python)
- [ ] Smart ranking (symbol name boost)
- [ ] Deprioritize test files
- [ ] Path scoping (`--path`)

### 22.4 v0.1.0 Scope

**Must Have:**
- [ ] Daemon mode (`greppy daemon start/stop/status`)
- [ ] Multi-project support
- [ ] `greppy list` command
- [ ] Homebrew formula
- [ ] curl installer
- [ ] GitHub releases with binaries

### 22.5 Release Checklist

```markdown
## Release v0.x.0

### Pre-release
- [ ] All tests passing (`cargo test`)
- [ ] Benchmarks meet targets (`cargo bench`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Version bumped in Cargo.toml

### Build
- [ ] Build all binaries (CI)
- [ ] Sign binaries (if applicable)
- [ ] Generate checksums

### Release
- [ ] Create GitHub release
- [ ] Upload binaries
- [ ] Update Homebrew formula
- [ ] Update install.sh
- [ ] Announce on social media

### Post-release
- [ ] Monitor for issues
- [ ] Respond to feedback
```

---

## 23. Success Metrics

### 23.1 Performance Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Search latency (daemon) | <1ms p50, <5ms p99 | Benchmark suite |
| Search latency (direct) | <10ms p50, <50ms p99 | Benchmark suite |
| Index time (10k files) | <5s | Benchmark suite |
| Memory usage (daemon) | <500MB | Profiling |
| Binary size | <20MB | Build output |

### 23.2 Adoption Metrics

| Metric | Target (6 months) | Measurement |
|--------|-------------------|-------------|
| GitHub stars | 1,000 | GitHub |
| Homebrew installs | 500/month | Homebrew analytics |
| Active users | 1,000 | Opt-in telemetry |
| Issues resolved | 90% within 1 week | GitHub |

### 23.3 Quality Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Test coverage | >80% | `cargo tarpaulin` |
| Bug reports | <10/month | GitHub issues |
| Crash rate | <0.1% | Error tracking |
| User satisfaction | >4.5/5 | Survey |

---

## 24. Open Questions

### 24.1 Technical

| ID | Question | Options | Decision |
|----|----------|---------|----------|
| OQ-1 | Should we support regex search? | Yes (Tantivy supports), No (BM25 only) | TBD |
| OQ-2 | Should we support fuzzy matching? | Yes (typo tolerance), No (exact) | TBD |
| OQ-3 | How to handle very large files (>1MB)? | Skip, Truncate, Chunk | Skip |
| OQ-4 | Should daemon auto-start on first search? | Yes, No (explicit) | No |
| OQ-5 | Should we support Windows? | Yes (v0.2), No | Yes (v0.2) |

### 24.2 Product

| ID | Question | Options | Decision |
|----|----------|---------|----------|
| OQ-6 | Final name? | Greppy, Seekr, Hound, Scout | Greppy |
| OQ-7 | Should we have a website? | Yes (greppy.dev), No | Yes |
| OQ-8 | Should we have opt-in telemetry? | Yes (usage stats), No | TBD |
| OQ-9 | Should we support IDE plugins? | Yes (VS Code), No | Future |

### 24.3 Business

| ID | Question | Options | Decision |
|----|----------|---------|----------|
| OQ-10 | License? | MIT, Apache 2.0, GPL | MIT |
| OQ-11 | Monetization? | None, Sponsorship, Pro tier | None (open source) |
| OQ-12 | Organization? | Personal, New org | TBD |

---

## 25. Appendix

### 25.1 Glossary

| Term | Definition |
|------|------------|
| BM25 | Best Matching 25, a ranking function for text search |
| Chunk | A semantic unit of code (function, class, or lines) |
| Daemon | Background process that keeps indexes warm |
| Index | Tantivy search index for a project |
| MCP | Model Context Protocol, Anthropic's tool protocol |
| Project | A directory with a recognized marker (.git, package.json, etc.) |
| tree-sitter | Incremental parsing library for AST extraction |

### 25.2 References

- [Tantivy Documentation](https://docs.rs/tantivy)
- [tree-sitter Documentation](https://tree-sitter.github.io/tree-sitter/)
- [BM25 Algorithm](https://en.wikipedia.org/wiki/Okapi_BM25)
- [Zoekt (Google's code search)](https://github.com/sourcegraph/zoekt)
- [ripgrep](https://github.com/BurntSushi/ripgrep)

### 25.3 Competitive Analysis

| Feature | Greppy | mgrep | ripgrep | Zoekt |
|---------|--------|-------|---------|-------|
| Semantic search | BM25+AST | Embeddings | No | Trigram |
| Latency | <10ms | 100-200ms | 10-100ms | 1-10ms |
| Cost | Free | $20/mo | Free | Free |
| Privacy | Local | Cloud | Local | Local |
| Setup | 1 command | Account+config | 1 command | Complex |
| AI tool integration | Shell | MCP | Shell | API |

### 25.4 File Structure

```
greppy/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── CHANGELOG.md
├── PRD.md                    # This document
├── IDEAS.md                  # Original design notes
├── src/
│   ├── main.rs               # Entry point
│   ├── lib.rs                # Library root
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── search.rs
│   │   ├── index.rs
│   │   ├── daemon.rs
│   │   └── list.rs
│   ├── core/
│   │   ├── mod.rs
│   │   ├── project.rs        # Project detection
│   │   ├── config.rs         # Configuration
│   │   └── error.rs          # Error types
│   ├── index/
│   │   ├── mod.rs
│   │   ├── tantivy.rs        # Tantivy wrapper
│   │   ├── schema.rs         # Index schema
│   │   └── writer.rs         # Index writer
│   ├── search/
│   │   ├── mod.rs
│   │   ├── query.rs          # Query parsing
│   │   ├── ranking.rs        # BM25 + boosters
│   │   └── results.rs        # Result formatting
│   ├── parse/
│   │   ├── mod.rs
│   │   ├── chunker.rs        # AST chunking
│   │   └── languages/
│   │       ├── mod.rs
│   │       ├── typescript.rs
│   │       ├── python.rs
│   │       ├── rust.rs
│   │       └── go.rs
│   ├── daemon/
│   │   ├── mod.rs
│   │   ├── server.rs         # Unix socket server
│   │   ├── client.rs         # Socket client
│   │   ├── cache.rs          # Query cache
│   │   └── watcher.rs        # File watcher
│   └── output/
│       ├── mod.rs
│       ├── human.rs          # Human-readable
│       └── json.rs           # JSON output
├── queries/                   # tree-sitter queries
│   ├── typescript.scm
│   ├── python.scm
│   ├── rust.scm
│   └── go.scm
├── tests/
│   ├── integration/
│   │   ├── search_test.rs
│   │   └── index_test.rs
│   └── e2e/
│       └── cli_test.sh
├── benches/
│   ├── search_bench.rs
│   └── index_bench.rs
├── scripts/
│   ├── install.sh
│   └── release.sh
└── Formula/
    └── greppy.rb
```

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0-draft | 2026-01-07 | Claude | Initial PRD |

---

*End of PRD*
