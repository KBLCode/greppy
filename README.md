# Greppy

<div align="center">

```text
-- ┌──────────────────────────────────────────────────┐
-- │ ██████╗ ██████╗ ███████╗██████╗ ██████╗ ██╗   ██╗│
-- │██╔════╝ ██╔══██╗██╔════╝██╔══██╗██╔══██╗╚██╗ ██╔╝│
-- │██║  ███╗██████╔╝█████╗  ██████╔╝██████╔╝ ╚████╔╝ │
-- │██║   ██║██╔══██╗██╔══╝  ██╔═══╝ ██╔═══╝   ╚██╔╝  │
-- │╚██████╔╝██║  ██║███████╗██║     ██║        ██║   │
-- │ ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚═╝        ╚═╝   │
-- └──────────────────────────────────────────────────┘
```

**Sub-millisecond local semantic code search.**

[![Crates.io](https://img.shields.io/crates/v/greppy.svg)](https://crates.io/crates/greppy)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Downloads](https://img.shields.io/crates/d/greppy.svg)](https://crates.io/crates/greppy)

</div>

---

## ⚡️ What is Greppy?

Greppy is a **local-first semantic code search engine** designed for AI coding agents and developers who need instant answers. Unlike `grep` or `ripgrep` which match exact text, Greppy understands the **meaning** of your code using vector embeddings and hybrid search.

It runs a background daemon that keeps your codebase indexed in memory, allowing for **sub-millisecond** query times.

### Why Greppy?

| Feature | Greppy | Ripgrep (rg) | GitHub Search |
|---------|--------|--------------|---------------|
| **Search Type** | Semantic + Keyword | Exact Keyword | Keyword |
| **Speed** | **< 1ms** (Hot) | < 20ms | ~500ms |
| **Understanding** | "Auth logic" finds `login()` | Finds "Auth logic" only | Limited |
| **Privacy** | **100% Local** | 100% Local | Cloud |
| **Index** | Persistent Vector Index | No Index (Scan) | Cloud Index |

## 🚀 Features

- **🧠 Hybrid Search**: Combines BM25 (keyword) and embedding-based (semantic) search for best-of-both-worlds accuracy.
- **🔥 Daemon Mode**: Keeps indexes hot in memory for instant results (<1ms).
- **🔒 Local & Private**: No code leaves your machine. Embeddings are generated locally using `FastEmbed`.
- **🤖 AI-Ready**: Outputs machine-readable formats (JSON) perfect for LLM context windows.
- **⚡️ Blazing Fast**: Built in Rust, using `Tantivy` for indexing and `Tokio` for async I/O.
- **🔄 Auto-Indexing**: Watches your project for changes and updates the index in real-time (coming soon).

## 📦 Installation

### One-line Installer (macOS/Linux)
```bash
curl -fsSL https://raw.githubusercontent.com/greppy/greppy/main/install.sh | bash
```

### From Source (Rust)
```bash
cargo install greppy
```

## 🛠 Usage

### 1. Start the Daemon (Recommended)
The daemon loads indexes into memory for maximum speed.
```bash
greppy daemon start
```

### 2. Index Your Project
Run this in your project root. It parses code, generates embeddings, and builds the index.
```bash
greppy index
```

### 3. Search
Search using natural language or code snippets.
```bash
# Semantic search (finds relevant code even if words don't match)
greppy search "how do we handle authentication?"

# Keyword search (works like grep but ranked by relevance)
greppy search "struct User"

# JSON output for tools
greppy search "database connection" --format json
```

### 4. Ask AI (Experimental)
Ask questions about your codebase using a local or cloud LLM (requires API key).
```bash
greppy ask "Explain the authentication flow"
```

## 📊 Performance

Tested on a MacBook Pro M3 Max with the Linux Kernel source tree (~70k files).

| Operation | Time |
|-----------|------|
| **Cold Search** (Direct) | ~15ms |
| **Hot Search** (Daemon) | **0.8ms** |
| **Indexing** (10k files) | ~45s |

*Note: First-time indexing requires embedding generation which is compute-intensive. Subsequent updates are incremental.*

## 🏗 Architecture

Greppy uses a modern search stack:

1.  **Parser**: `Tree-sitter` parses code into structural chunks (functions, classes).
2.  **Embedder**: `FastEmbed` (ONNX Runtime) generates 384-dimensional vectors locally.
3.  **Indexer**: `Tantivy` (Rust's Lucene alternative) stores vectors and text.
4.  **Daemon**: A `Tokio`-based server holds `IndexReaders` in memory and handles IPC via Unix sockets.

## 🤝 Contributing

We love contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.

1.  Fork the repo
2.  Create a feature branch
3.  Submit a Pull Request

## 📄 License

MIT © [Greppy Contributors](https://github.com/greppy)
