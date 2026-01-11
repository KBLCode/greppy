# PLAN: Greppy v0.5.0 - Precision & Agent UX

## 1. Problem Statement
While `greppy` v0.4.0 achieves high performance and hybrid search, it lags behind `dyoburon/greppy` (Python) in **parsing precision**. Our current heuristic chunker (regex-based) can split functions incorrectly or miss complex nested structures. Additionally, we lack a dedicated tool for Agents to "read around" a match, which is critical for understanding code context.

## 2. Core Objectives

### 2.1 Tree-sitter Integration (The "Gold Standard")
Replace heuristics with **Abstract Syntax Tree (AST)** parsing using `tree-sitter`.
-   **Goal**: 100% accurate function/class boundaries. Never split a function in half.
-   **Languages**: Rust, Python, TypeScript/JavaScript, Go, Java.
-   **Fallback**: Keep the v0.4.0 heuristic chunker for other languages (Lua, Ruby, etc.).

### 2.2 The `read` Command
Implement a command specifically designed for LLM Agents to retrieve context.
-   **Usage**: `greppy read src/main.rs:50-100` or `greppy read src/main.rs:50` (centered).
-   **Why**: Search gives a snippet. Agents often need to "expand" that snippet to understand the surrounding logic without reading the whole file.

## 3. Technical Architecture

### 3.1 Parsing Layer (`src/parse/`)
Refactor `chunker.rs` into a trait-based system:

```rust
trait CodeParser {
    fn chunk(&self, content: &str) -> Vec<Chunk>;
}

struct TreeSitterParser { language: Language }
struct HeuristicParser;
```

**Dependencies**:
-   `tree-sitter`
-   `tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-typescript`, etc.

**Strategy**:
1.  Detect language.
2.  If grammar available -> Parse AST -> Walk tree -> Extract nodes (FunctionDeclaration, ClassDeclaration).
3.  If node > `CHUNK_MAX_LINES`, split internally (smartly).
4.  If grammar unavailable -> Use `HeuristicParser`.

### 3.2 Read Command (`src/cli/read.rs`)
-   **Input**: File path + Line range (or center line).
-   **Output**: The exact lines, formatted for LLM consumption (e.g., with line numbers).
-   **Logic**: Simple file I/O, but robust range handling (clamping to file bounds).

## 4. Implementation Roadmap

### Phase 1: The `read` Command (Low Effort, High Value)
-   Implement `greppy read`.
-   Update `docs/USER_GUIDE.md`.

### Phase 2: Tree-sitter Infrastructure
-   Add `tree-sitter` dependencies.
-   Create the `CodeParser` trait.
-   Refactor `walker.rs` to select parsers.

### Phase 3: Language Support
-   Implement Rust parser.
-   Implement Python parser.
-   Implement TypeScript parser.
-   Benchmark indexing speed (Tree-sitter is slower than regex; optimize AST traversal).

## 5. Success Metrics
-   **Precision**: Zero "broken function" chunks in indexed data.
-   **Agent Success**: Agents can solve tasks with fewer steps using `read` + `search`.
-   **Performance**: Indexing time remains under 100ms/file (amortized).
