# Greppy Enhancement Plan: True Semantic Search & Performance

## 1. Objective
Transform `greppy` from a keyword-based search tool into a **true semantic code search engine** with local embeddings and high-performance parallel indexing.

## 2. Core Enhancements

### 2.1 Local Embeddings (Hybrid Search)
Currently, `greppy` uses Tantivy for keyword search (BM25). We will add vector search to understand intent (e.g., "auth logic" finds `login` function).

-   **Library**: `fastembed-rs` (ONNX Runtime wrapper, supports `bge-small-en-v1.5` - lightweight and effective).
-   **Schema**: Add `embedding` field (vector of `f32`) to `IndexSchema`.
-   **Indexing**: Generate embeddings for each code chunk during indexing.
-   **Search**:
    1.  Generate embedding for user query.
    2.  Perform vector search (ANN) in Tantivy.
    3.  Combine with keyword search (Hybrid RRF or simple boosting).

### 2.2 Parallel Indexing Pipeline
The current indexing is sequential. We will move to a **Producer-Consumer** model to saturate CPU and I/O.

-   **Architecture**:
    1.  **Walker (Main Thread)**: Discovers files.
    2.  **Workers (Rayon/Tokio)**: Read file -> Chunk -> **Generate Embedding** (CPU intensive) -> Build Document.
    3.  **Writer (Single Thread)**: Receives Documents via Channel -> `IndexWriter.add_document`.
-   **Benefit**: Massive speedup, especially with embedding generation which is CPU heavy.

### 2.3 "Ask" Command (RAG)
Leverage the new Google OAuth integration to answer questions.

-   **Command**: `greppy ask "How does auth work?"`
-   **Flow**:
    1.  **Search**: Retrieve top 10 relevant chunks using Hybrid Search.
    2.  **Prompt**: Construct context-rich prompt.
    3.  **LLM**: Call Gemini Flash (`gemini-2.5-flash`) via Google API.
    4.  **Output**: Stream answer to terminal.

## 3. Implementation Steps

### Phase 1: Dependencies & Schema
-   Add `fastembed`, `ort`.
-   Enable `tantivy` vector features.
-   Update `IndexSchema` with `vector` field.

### Phase 2: Parallel Indexing with Embeddings
-   Refactor `src/cli/index.rs`.
-   Implement `EmbeddingModel` struct in `src/ai/embedding.rs`.
-   Wire up the channel pipeline.

### Phase 3: Hybrid Search
-   Update `SearchQuery` to support vector queries.
-   Implement hybrid ranking.

### Phase 4: Ask Command
-   Implement `src/cli/ask.rs`.
-   Connect to `src/auth` and Google API.

## 4. Performance Targets
-   **Indexing**: < 10ms per file (amortized).
-   **Search**: < 50ms latency.
-   **Memory**: Keep embedding model loaded only when needed (or daemonize).
