# Greppy Performance Optimization Guide

## Measurement Tools

### 1. Hyperfine (CLI Benchmarking)
```bash
# Install
brew install hyperfine

# Benchmark search operations
hyperfine --warmup 3 'greppy search "authenticate"'
hyperfine --warmup 3 --runs 100 'greppy search "user database"'

# Compare different query types
hyperfine --warmup 3 \
  'greppy search "auth"' \
  'greppy search "authentication flow"' \
  'greppy search --smart "how does auth work"'
```

### 2. Flamegraph (CPU Profiling)
```bash
# Install
cargo install flamegraph

# Profile search operations
cargo flamegraph --bench search_bench -- --bench

# Profile daemon server
cargo flamegraph --bin greppy -- start

# View flamegraph.svg in browser
open flamegraph.svg
```

### 3. Dhat (Memory Profiling)
```bash
# Add to Cargo.toml dev-dependencies:
# dhat = "0.3"

# Run with dhat
cargo run --features dhat-heap --bin greppy -- search "test"

# View dhat-heap.json with dh_view.html
```

### 4. Criterion (Microbenchmarks)
```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench search_warm

# Generate detailed reports
cargo bench -- --save-baseline main
# After changes:
cargo bench -- --baseline main
```

## Current Performance Baseline (v0.2.0)

### Search Performance
| Metric | Cold Start | Warm (Cached) | Target |
|--------|-----------|---------------|--------|
| Simple query (1 term) | 0.87ms | 0.01ms | <1ms |
| Complex query (3+ terms) | 1.34ms | 0.04ms | <2ms |
| Smart search (API) | ~2-3s | 6ms | <3s |
| Smart search (cached) | - | <10ms | <10ms |

### Memory Usage
| Component | Current | Target | Notes |
|-----------|---------|--------|-------|
| Index (10K chunks) | ~50MB | <100MB | Tantivy in-memory |
| L1 Cache (500 entries) | ~5MB | <10MB | LRU cache |
| L2 Cache (5K entries) | ~2MB disk | <5MB | JSON file |
| Daemon RSS | ~80MB | <150MB | Idle state |

### Throughput
| Operation | Current | Target |
|-----------|---------|--------|
| Searches/sec (single client) | ~1000 | >1000 |
| Concurrent clients | 10 | 50+ |
| Index writes/sec | ~500 chunks | >1000 |

## Optimization Opportunities

### 1. Memory Optimization

#### A. Reduce Index Memory Footprint
**Current Issue:** Tantivy loads entire index into memory.

**Optimization:**
```rust
// src/index/reader.rs
use tantivy::ReloadPolicy;

// Current: Manual reload (keeps everything in memory)
let reader = index
    .reader_builder()
    .reload_policy(ReloadPolicy::Manual)
    .try_into()?;

// Optimized: Use OnCommit with smaller cache
let reader = index
    .reader_builder()
    .reload_policy(ReloadPolicy::OnCommitWithDelay)
    .num_searchers(2) // Reduce from default 4
    .try_into()?;
```

**Expected Impact:** 20-30% memory reduction for large indexes.

#### B. Optimize SearchResult Cloning
**Current Issue:** Results are cloned multiple times during scoring.

**Optimization:**
```rust
// src/search/results.rs
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SearchResult {
    // Wrap large strings in Arc to avoid cloning
    pub path: Arc<str>,
    pub content: Arc<str>,
    pub symbol_name: Option<Arc<str>>,
    // ... rest of fields
}
```

**Expected Impact:** 40-50% reduction in search memory allocations.

#### C. Implement String Interning for Paths
**Current Issue:** File paths are duplicated across many results.

**Optimization:**
```rust
// src/index/schema.rs
use string_cache::DefaultAtom as Atom;

pub struct SearchResult {
    pub path: Atom, // Interned string
    // ...
}
```

**Expected Impact:** 60-70% reduction in path storage for large result sets.

### 2. CPU Optimization

#### A. Parallelize Index Search
**Current Issue:** Search is single-threaded.

**Optimization:**
```rust
// src/index/reader.rs
use rayon::prelude::*;

pub fn search(&self, query_text: &str, limit: usize) -> Result<Vec<SearchResult>> {
    // ... existing query building ...
    
    // Parallel document processing
    let results: Vec<_> = top_docs
        .par_iter()
        .map(|(score, doc_address)| {
            let doc = searcher.doc(*doc_address)?;
            // ... process document ...
            Ok(result)
        })
        .collect::<Result<Vec<_>>>()?;
    
    // ... rest of function ...
}
```

**Expected Impact:** 2-3x speedup for large result sets (>100 docs).

#### B. Optimize Token Processing
**Current Issue:** Tokenization allocates for each token.

**Optimization:**
```rust
// src/index/reader.rs
pub fn search(&self, query_text: &str, limit: usize) -> Result<Vec<SearchResult>> {
    // Pre-allocate token vector
    let mut tokens = Vec::with_capacity(query_text.split_whitespace().count());
    
    let mut stream = tokenizer.token_stream(query_text);
    while let Some(token) = stream.next() {
        tokens.push(token.text.to_string());
    }
    
    // ... rest of function ...
}
```

**Expected Impact:** 10-15% reduction in search latency.

#### C. Cache Compiled Queries
**Current Issue:** Query compilation happens on every search.

**Optimization:**
```rust
// src/cache/mod.rs
use std::sync::Arc;
use tantivy::query::Query;

pub struct QueryCache {
    // Add compiled query cache
    compiled_queries: LruCache<u64, Arc<dyn Query>>,
    // ... existing fields ...
}
```

**Expected Impact:** 30-40% speedup for repeated queries.

### 3. IO Optimization

#### A. Batch File Reads During Indexing
**Current Issue:** Files are read one at a time.

**Optimization:**
```rust
// src/parse/walker.rs
use tokio::fs;
use futures::stream::{self, StreamExt};

pub async fn walk_parallel(&self) -> Result<Vec<FileInfo>> {
    let files = self.walk()?;
    
    // Read files in parallel batches
    let chunks = stream::iter(files)
        .chunks(10) // Process 10 files at a time
        .map(|batch| async move {
            futures::future::join_all(
                batch.into_iter().map(|file| async move {
                    fs::read_to_string(&file.path).await
                })
            ).await
        })
        .collect::<Vec<_>>()
        .await;
    
    Ok(chunks.into_iter().flatten().collect())
}
```

**Expected Impact:** 3-5x faster indexing for large projects.

#### B. Use Memory-Mapped Files for L2 Cache
**Current Issue:** L2 cache loads entire JSON file into memory.

**Optimization:**
```rust
// src/llm/cache.rs
use memmap2::Mmap;

impl LlmCache {
    fn load_l2_from_disk() -> Result<HashMap<String, CachedEnhancement>> {
        let path = Self::cache_path()?;
        if !path.exists() {
            return Ok(HashMap::new());
        }
        
        // Memory-map the file instead of reading
        let file = std::fs::File::open(&path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let data: HashMap<String, CachedEnhancement> = 
            serde_json::from_slice(&mmap)?;
        
        Ok(data)
    }
}
```

**Expected Impact:** 50-60% faster L2 cache loading.

### 4. Latency Optimization

#### A. Implement Query Result Streaming
**Current Issue:** All results collected before returning.

**Optimization:**
```rust
// src/search/results.rs
use tokio::sync::mpsc;

pub async fn search_stream(
    &self,
    query: &str,
    limit: usize,
) -> mpsc::Receiver<SearchResult> {
    let (tx, rx) = mpsc::channel(100);
    
    // Stream results as they're found
    tokio::spawn(async move {
        for result in self.search(query, limit)? {
            if tx.send(result).await.is_err() {
                break;
            }
        }
    });
    
    rx
}
```

**Expected Impact:** 40-50% reduction in time-to-first-result.

#### B. Optimize Score Calculation
**Current Issue:** Score adjustments require full result materialization.

**Optimization:**
```rust
// src/index/reader.rs
impl IndexSearcher {
    pub fn search(&self, query_text: &str, limit: usize) -> Result<Vec<SearchResult>> {
        // ... existing code ...
        
        // Apply score adjustments during iteration, not after
        let mut results = Vec::with_capacity(limit);
        for (base_score, doc_address) in top_docs {
            let doc = searcher.doc(doc_address)?;
            
            // Extract fields
            let is_exported = /* ... */;
            let is_test = /* ... */;
            
            // Calculate final score immediately
            let mut final_score = base_score;
            if is_exported {
                final_score += scoring::EXPORTED_BOOST;
            }
            if is_test {
                final_score -= scoring::TEST_PENALTY;
            }
            
            // Early termination if score too low
            if results.len() >= limit && final_score < results[limit-1].score {
                continue;
            }
            
            // ... create result ...
            results.push(result);
            
            // Keep sorted and truncated
            if results.len() > limit {
                results.sort_unstable_by(|a, b| 
                    b.score.partial_cmp(&a.score).unwrap()
                );
                results.truncate(limit);
            }
        }
        
        Ok(results)
    }
}
```

**Expected Impact:** 20-30% reduction in search latency.

### 5. Throughput Optimization

#### A. Connection Pooling for Daemon
**Current Issue:** Each request spawns a new task.

**Optimization:**
```rust
// src/daemon/server.rs
use tokio::task::JoinSet;

pub struct DaemonServer {
    // Add task pool
    task_pool: Arc<RwLock<JoinSet<()>>>,
    max_concurrent_tasks: usize,
    // ... existing fields ...
}

impl DaemonServer {
    pub async fn run(&self) -> Result<()> {
        // ... existing code ...
        
        loop {
            tokio::select! {
                result = listener.accept() => {
                    let mut pool = self.task_pool.write().await;
                    
                    // Limit concurrent tasks
                    while pool.len() >= self.max_concurrent_tasks {
                        pool.join_next().await;
                    }
                    
                    pool.spawn(handle_connection(/* ... */));
                }
                // ... rest of loop ...
            }
        }
    }
}
```

**Expected Impact:** 2-3x higher throughput under load.

#### B. Batch Index Writes
**Current Issue:** Each file change triggers immediate index write.

**Optimization:**
```rust
// src/watch/mod.rs
use tokio::time::{interval, Duration};

pub struct WatchManager {
    // Add write batching
    pending_writes: Arc<RwLock<Vec<PathBuf>>>,
    batch_interval: Duration,
}

impl WatchManager {
    pub async fn start_batch_writer(&self) {
        let mut ticker = interval(self.batch_interval);
        
        loop {
            ticker.tick().await;
            
            let mut pending = self.pending_writes.write().await;
            if pending.is_empty() {
                continue;
            }
            
            // Batch write all pending changes
            let files = std::mem::take(&mut *pending);
            drop(pending);
            
            self.index_files(files).await;
        }
    }
}
```

**Expected Impact:** 5-10x higher indexing throughput.

## Profiling Workflow

### Step 1: Establish Baseline
```bash
# Run benchmarks
cargo bench --bench search_bench -- --save-baseline v0.2.0

# Profile with hyperfine
hyperfine --warmup 5 --runs 100 \
  --export-json baseline.json \
  'greppy search "authenticate"'
```

### Step 2: Profile Hot Paths
```bash
# CPU profiling
cargo flamegraph --bench search_bench -- --bench

# Memory profiling
cargo run --features dhat-heap --bin greppy -- search "test"

# Identify bottlenecks in flamegraph.svg
```

### Step 3: Apply Optimizations
```bash
# Implement one optimization at a time
# Run benchmarks after each change
cargo bench --bench search_bench -- --baseline v0.2.0

# Compare results
critcmp v0.2.0 new
```

### Step 4: Validate in Production
```bash
# Load test with realistic workload
hyperfine --warmup 10 --runs 1000 \
  'greppy search "$(shuf -n 1 queries.txt)"'

# Monitor memory over time
while true; do
  ps aux | grep greppy | awk '{print $6}'
  sleep 1
done
```

## Performance Checklist

Before releasing optimizations:

- [ ] Run `cargo bench` and verify improvements
- [ ] Profile with `flamegraph` to confirm hot path changes
- [ ] Check memory usage with `dhat` or `heaptrack`
- [ ] Load test with `hyperfine` (1000+ runs)
- [ ] Verify no regressions in search accuracy
- [ ] Test with large projects (50K+ files)
- [ ] Monitor daemon memory over 24 hours
- [ ] Validate cache hit rates in production
- [ ] Check tail latency (p95, p99)
- [ ] Document performance characteristics

## Monitoring in Production

### Key Metrics to Track

1. **Search Latency**
   - p50, p95, p99 response times
   - Cache hit rate (L1, L2)
   - Query complexity distribution

2. **Memory Usage**
   - Daemon RSS over time
   - Index size growth
   - Cache memory consumption

3. **Throughput**
   - Searches per second
   - Concurrent client count
   - Index writes per second

4. **Resource Utilization**
   - CPU usage per search
   - Disk I/O during indexing
   - Network I/O for smart search

### Alerting Thresholds

- Search latency p99 > 10ms
- Memory growth > 10MB/hour
- Cache hit rate < 80%
- CPU usage > 50% sustained
- Disk I/O > 100MB/s during indexing

## References

- [Tantivy Performance Guide](https://github.com/quickwit-oss/tantivy/blob/main/PERFORMANCE.md)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Criterion User Guide](https://bheisler.github.io/criterion.rs/book/)
- [Flamegraph Guide](https://www.brendangregg.com/flamegraphs.html)
