# Greppy Sprint Plan

## Overview

Build complete Greppy CLI in one shot. Single Rust binary. Daemon runs in background. Any tool can search.

```
Target: 2-3 days
Lines of code: ~2500
Result: brew install greppy && greppy start && greppy search "auth"
```

---

## Architecture

```
+------------------+     +------------------+     +------------------+
|   Terminal 1     |     |   Terminal 2     |     |   Claude Code    |
|   greppy start   |     |   greppy search  |     |   shells out     |
+--------+---------+     +--------+---------+     +--------+---------+
         |                        |                        |
         |                        +------------------------+
         |                                   |
         v                                   v
+--------+-----------------------------------+---------+
|                  Unix Socket                         |
|              ~/.greppy/daemon.sock                   |
+--------+---------------------------------------------+
         |
         v
+--------+---------------------------------------------+
|                  Greppy Daemon                       |
|                                                      |
|  +-------------+  +-------------+  +-------------+   |
|  |   Tantivy   |  |   Watcher   |  |    Cache    |   |
|  |   Indexes   |  |   (notify)  |  |   (LRU)     |   |
|  +-------------+  +-------------+  +-------------+   |
|                                                      |
|  +-----------------------------------------------+   |
|  |              Project Registry                 |   |
|  |  ~/project-a -> index-a                       |   |
|  |  ~/project-b -> index-b                       |   |
|  +-----------------------------------------------+   |
+------------------------------------------------------+
```

---

## File Structure

```
greppy/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, CLI parsing
│   ├── lib.rs               # Library root
│   ├── cli.rs               # Command definitions (clap)
│   ├── daemon/
│   │   ├── mod.rs
│   │   ├── server.rs        # Unix socket server
│   │   ├── client.rs        # Unix socket client
│   │   ├── protocol.rs      # Request/Response types
│   │   └── process.rs       # Fork/spawn daemon
│   ├── index/
│   │   ├── mod.rs
│   │   ├── schema.rs        # Tantivy schema
│   │   ├── writer.rs        # Index writer
│   │   └── reader.rs        # Index reader/searcher
│   ├── search/
│   │   ├── mod.rs
│   │   ├── query.rs         # Query parsing
│   │   └── results.rs       # Result types
│   ├── parse/
│   │   ├── mod.rs
│   │   ├── chunker.rs       # Code chunking
│   │   └── walker.rs        # File walking
│   ├── project/
│   │   ├── mod.rs
│   │   ├── detect.rs        # Project root detection
│   │   └── registry.rs      # Multi-project management
│   ├── watch/
│   │   ├── mod.rs
│   │   └── watcher.rs       # File system watcher
│   ├── cache/
│   │   ├── mod.rs
│   │   └── lru.rs           # Query cache
│   ├── config.rs            # Configuration
│   ├── error.rs             # Error types
│   └── output.rs            # Human/JSON output
└── tests/
    └── integration.rs
```

---

## Sprint Phases

### Phase 1: Core Infrastructure [~400 lines]

```
[ ] 1.1 Cargo.toml with all dependencies
[ ] 1.2 Error types (error.rs)
[ ] 1.3 Configuration (config.rs)
[ ] 1.4 Project detection (project/detect.rs)
[ ] 1.5 Project registry (project/registry.rs)
```

### Phase 2: Indexing [~500 lines]

```
[ ] 2.1 Tantivy schema (index/schema.rs)
[ ] 2.2 File walker with gitignore (parse/walker.rs)
[ ] 2.3 Code chunker (parse/chunker.rs)
[ ] 2.4 Index writer (index/writer.rs)
[ ] 2.5 Index reader (index/reader.rs)
```

### Phase 3: Search [~300 lines]

```
[ ] 3.1 Query parsing (search/query.rs)
[ ] 3.2 BM25 search with boosting (search/mod.rs)
[ ] 3.3 Result types and ranking (search/results.rs)
[ ] 3.4 Output formatting (output.rs)
```

### Phase 4: Daemon [~600 lines]

```
[ ] 4.1 Protocol types (daemon/protocol.rs)
[ ] 4.2 Daemon process management (daemon/process.rs)
[ ] 4.3 Unix socket server (daemon/server.rs)
[ ] 4.4 Unix socket client (daemon/client.rs)
[ ] 4.5 Request routing (daemon/mod.rs)
```

### Phase 5: File Watching [~200 lines]

```
[ ] 5.1 File watcher setup (watch/watcher.rs)
[ ] 5.2 Incremental re-indexing (watch/mod.rs)
```

### Phase 6: Cache [~150 lines]

```
[ ] 6.1 LRU cache implementation (cache/lru.rs)
[ ] 6.2 Cache integration (cache/mod.rs)
```

### Phase 7: CLI [~350 lines]

```
[ ] 7.1 Command definitions (cli.rs)
[ ] 7.2 Main entry point (main.rs)
[ ] 7.3 Library root (lib.rs)
```

### Phase 8: Polish [~100 lines]

```
[ ] 8.1 Integration tests
[ ] 8.2 Error messages
[ ] 8.3 Help text
```

---

## Commands (Final)

```
greppy start                    Start daemon in background
greppy stop                     Stop daemon
greppy status                   Show daemon status, indexed projects

greppy search <query>           Search current project
greppy search <query> -l 10     Limit results
greppy search <query> --json    JSON output
greppy search <query> -p <path> Search specific project

greppy index                    Index current project
greppy index --watch            Index and watch for changes
greppy index --force            Force full re-index

greppy list                     List indexed projects
greppy forget [path]            Remove project index
```

---

## Protocol (Unix Socket)

Request:
```json
{
  "id": "uuid",
  "method": "search",
  "params": {
    "query": "authentication",
    "project": "/Users/x/myproject",
    "limit": 20
  }
}
```

Response:
```json
{
  "id": "uuid",
  "result": {
    "results": [...],
    "elapsed_ms": 0.8
  }
}
```

Methods:
```
search      Search a project
index       Index a project
index_watch Start watching a project
status      Get daemon status
list        List indexed projects
forget      Remove project index
stop        Stop daemon
```

---

## Dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
tantivy = "0.22"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
notify = "6"
ignore = "0.4"
walkdir = "2"
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
directories = "5"
uuid = { version = "1", features = ["v4"] }
lru = "0.12"
```

---

## Execution Order

Build in this exact order. Each step compiles before moving on.

```
1. Cargo.toml + error.rs + config.rs           -> cargo check
2. project/detect.rs + project/registry.rs     -> cargo check
3. index/schema.rs                             -> cargo check
4. parse/walker.rs + parse/chunker.rs          -> cargo check
5. index/writer.rs + index/reader.rs           -> cargo check
6. search/query.rs + search/results.rs         -> cargo check
7. output.rs                                   -> cargo check
8. daemon/protocol.rs                          -> cargo check
9. daemon/process.rs                           -> cargo check
10. daemon/server.rs + daemon/client.rs        -> cargo check
11. watch/watcher.rs                           -> cargo check
12. cache/lru.rs                               -> cargo check
13. cli.rs + main.rs + lib.rs                  -> cargo check
14. cargo build --release                      -> binary
15. Test manually                              -> done
```

---

## Success Criteria

```bash
# Terminal 1
$ cargo build --release
$ ./target/release/greppy start
Daemon started (pid 12345)

# Terminal 2
$ cd ~/some-project
$ ./target/release/greppy search "auth"
Indexing project... done (1247 files, 3.2s)

1. src/auth/middleware.ts:15-42 (0.94)
   function authenticateRequest
   ...

2. src/auth/jwt.ts:8-25 (0.87)
   function verifyToken
   ...

Found 5 results in 0.8ms

$ ./target/release/greppy search "auth"
Found 5 results in 0.3ms   # Cached

# Terminal 1
$ ./target/release/greppy stop
Daemon stopped
```

---

## Start

Begin Phase 1.1: Cargo.toml
