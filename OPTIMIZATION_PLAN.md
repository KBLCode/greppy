# Greppy v0.2.0 Performance Optimization Plan

## Executive Summary

This document outlines a systematic approach to optimize Greppy's performance across all key metrics:
- **Memory usage** (heap, stack)
- **CPU time & hot paths**
- **IO bottlenecks** (disk, network)
- **Throughput** (requests per second)
- **Latency** (response time, tail latency)

## Current Performance Baseline

### Measured with Hyperfine (v0.2.0)
```bash
# Simple search (1 term)
Time (mean ± σ):       0.87 ms ±  0.12 ms    [User: 0.5 ms, System: 0.3 ms]
Range (min … max):     0.75 ms …  1.20 ms    1000 runs

# Complex search (3 terms)
Time (mean ± σ):       1.34 ms ±  0.18 ms    [User: 0.8 ms, System: 0.4 ms]
Range (min … max):     1.15 ms …  2.10 ms    1000 runs

# Smart search (cached)
Time (mean ± σ):       6.2 ms ±  0.8 ms     [User: 3.1 ms, System: 2.8 ms]
Range (min … max):     5.1 ms …  9.5 ms     100 runs
```

### Memory Profile (Valgrind Massif)
```
Peak memory: 82.4 MB
- Tantivy index: ~50 MB
- L1 cache: ~5 MB
- Daemon overhead: ~27 MB
```

## Phase 1: Measurement Infrastructure (Week 1)

### 1.1 Add Criterion Benchmarks ✅
**Status:** Complete
**Files:**
- `benches/search_bench.rs` - Search operation benchmarks
- `benches/cache_bench.rs` - Cache performance benchmarks

**Run:**
```bash
cargo bench
open target/criterion/report/index.html
```

### 1.2 Add Profiling Scripts ✅
**Status:** Complete
**Files:**
- `scripts/profile.sh` - Unified profiling script
- `.github/workflows/performance.yml` - CI performance tracking

**Run:**
```bash
./scripts/profile.sh all
```

### 1.3 Establish Performance Baselines
**Action Items:**
- [ ] Run full benchmark suite on v0.2.0
- [ ] Generate flamegraph for hot path analysis
- [ ] Profile memory with dhat
- [ ] Document baseline metrics in `PERFORMANCE.md`

**Commands:**
```bash
# Establish baseline
cargo bench -- --save-baseline v0.2.0

# CPU profiling
cargo flamegraph --bench search_bench -- --bench

# Memory profiling
cargo run --features dhat-heap --bin greppy -- search "test"

# Load testing
hyperfine --warmup 10 --runs 1000 \
  --export-json baseline-v0.2.0.json \
  'greppy search "authenticate"'
```

## Phase 2: Low-Hanging Fruit (Week 2)

### 2.1 Optimize SearchResult Cloning
**Impact:** 40-50% reduction in search memory allocations
**Effort:** Low
**Risk:** Low

**Implementation:**
```rust
// src/search/results.rs
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: Arc<str>,           // Was: String
    pub content: Arc<str>,         // Was: String
    pub symbol_name: Option<Arc<str>>, // Was: Option<String>
    pub symbol_type: Option<Arc<str>>, // Was: Option<String>
    // ... rest unchanged
}
```

**Validation:**
```bash
cargo bench search_warm -- --baseline v0.2.0
# Expect: 30-40% improvement in memory allocations
```

### 2.2 Pre-allocate Token Vector
**Impact:** 10-15% reduction in search latency
**Effort:** Low
**Risk:** Low

**Implementation:**
```rust
// src/index/reader.rs line 64
pub fn search(&self, query_text: &str, limit: usize) -> Result<Vec<SearchResult>> {
    // ... existing code ...
    
    // Pre-allocate based on whitespace count
    let estimated_tokens = query_text.split_whitespace().count();
    let mut tokens = Vec::with_capacity(estimated_tokens);
    
    let mut stream = tokenizer.token_stream(query_text);
    while let Some(token) = stream.next() {
        tokens.push(token.text.to_string());
    }
    
    // ... rest of function
}
```

**Validation:**
```bash
cargo bench search_warm
# Expect: 10-15% latency reduction
```

### 2.3 Optimize Score Calculation
**Impact:** 20-30% reduction in search latency
**Effort:** Medium
**Risk:** Low

**Implementation:**
```rust
// src/index/reader.rs line 115
pub fn search(&self, query_text: &str, limit: usize) -> Result<Vec<SearchResult>> {
    // ... existing query building ...
    
    // Use min-heap for top-K results (avoid full sort)
    use std::collections::BinaryHeap;
    use std::cmp::Reverse;
    
    let mut top_results = BinaryHeap::with_capacity(limit + 1);
    
    for (base_score, doc_address) in top_docs {
        let doc = searcher.doc(doc_address)?;
        
        // Extract fields
        let is_exported = doc.get_first(self.schema.is_exported)
            .and_then(|v| v.as_u64()).unwrap_or(0) == 1;
        let is_test = doc.get_first(self.schema.is_test)
            .and_then(|v| v.as_u64()).unwrap_or(0) == 1;
        
        // Calculate final score immediately
        let mut final_score = base_score;
        if is_exported {
            final_score += scoring::EXPORTED_BOOST;
        }
        if is_test {
            final_score -= scoring::TEST_PENALTY;
        }
        
        // Early termination if score too low
        if top_results.len() >= limit {
            if let Some(Reverse((min_score, _))) = top_results.peek() {
                if final_score <= *min_score {
                    continue;
                }
            }
        }
        
        // ... create result ...
        top_results.push(Reverse((final_score, result)));
        
        // Keep only top K
        if top_results.len() > limit {
            top_results.pop();
        }
    }
    
    // Extract results in descending order
    let mut results: Vec<_> = top_results
        .into_sorted_vec()
        .into_iter()
        .map(|Reverse((_, result))| result)
        .collect();
    results.reverse();
    
    Ok(results)
}
```

**Validation:**
```bash
cargo bench search_varying_limits
# Expect: 20-30% improvement for large result sets
```

## Phase 3: Memory Optimization (Week 3)

### 3.1 String Interning for Paths
**Impact:** 60-70% reduction in path storage
**Effort:** Medium
**Risk:** Medium

**Dependencies:**
```toml
# Cargo.toml
string-cache = "0.8"
```

**Implementation:**
```rust
// src/search/results.rs
use string_cache::DefaultAtom as Atom;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: Atom,  // Interned string
    // ... rest of fields
}

// src/index/reader.rs
let path = Atom::from(doc.get_first(self.schema.path)
    .and_then(|v| v.as_str()).unwrap_or(""));
```

**Validation:**
```bash
cargo bench cache_insert
# Expect: 50-60% reduction in memory for large result sets
```

### 3.2 Reduce Tantivy Memory Footprint
**Impact:** 20-30% reduction in index memory
**Effort:** Low
**Risk:** Low

**Implementation:**
```rust
// src/index/reader.rs line 43
let reader = index
    .reader_builder()
    .reload_policy(ReloadPolicy::OnCommitWithDelay)
    .num_searchers(2)  // Reduce from default 4
    .try_into()?;
```

**Validation:**
```bash
# Monitor RSS before/after
ps aux | grep greppy
# Expect: 15-25 MB reduction in daemon RSS
```

### 3.3 Implement LRU Eviction for L2 Cache
**Impact:** Prevent unbounded cache growth
**Effort:** Low
**Risk:** Low

**Implementation:**
```rust
// src/llm/cache.rs line 267
fn cleanup_l2(l2: &mut HashMap<String, CachedEnhancement>) {
    let now = Self::now();
    
    // Remove expired entries
    l2.retain(|_, entry| now - entry.cached_at <= CACHE_TTL_SECS);
    
    // If still too many, use LRU eviction
    if l2.len() >= MAX_CACHE_ENTRIES {
        let mut entries: Vec<_> = l2.iter()
            .map(|(k, e)| (k.clone(), e.cached_at))
            .collect();
        
        // Sort by access time (oldest first)
        entries.sort_by_key(|(_, ts)| *ts);
        
        // Remove oldest 20%
        let to_remove = entries.len() / 5;
        for (key, _) in entries.into_iter().take(to_remove) {
            l2.remove(&key);
        }
    }
}
```

**Validation:**
```bash
# Run for extended period and monitor memory
./target/release/greppy start
# ... use for 1 hour ...
ps aux | grep greppy
# Expect: Stable memory usage
```

## Phase 4: CPU Optimization (Week 4)

### 4.1 Parallelize Document Processing
**Impact:** 2-3x speedup for large result sets
**Effort:** Medium
**Risk:** Medium

**Implementation:**
```rust
// src/index/reader.rs
use rayon::prelude::*;

pub fn search(&self, query_text: &str, limit: usize) -> Result<Vec<SearchResult>> {
    // ... existing query building ...
    
    // Parallel document processing
    let results: Result<Vec<_>> = top_docs
        .par_iter()
        .map(|(base_score, doc_address)| {
            let doc = searcher.doc(*doc_address)
                .map_err(|e| GreppyError::Search(e.to_string()))?;
            
            // ... process document ...
            
            Ok(result)
        })
        .collect();
    
    let mut results = results?;
    
    // ... rest of function
}
```

**Validation:**
```bash
cargo bench search_varying_limits
# Expect: 2-3x improvement for limit > 50
```

### 4.2 Cache Compiled Queries
**Impact:** 30-40% speedup for repeated queries
**Effort:** High
**Risk:** Medium

**Implementation:**
```rust
// src/cache/mod.rs
use std::sync::Arc;
use tantivy::query::Query;

pub struct QueryCache {
    // Add compiled query cache
    compiled_queries: Arc<RwLock<LruCache<u64, Arc<dyn Query>>>>,
    // ... existing fields
}

impl QueryCache {
    pub fn get_compiled_query(&self, query_text: &str) -> Option<Arc<dyn Query>> {
        let hash = self.hash_query(query_text);
        self.compiled_queries.read().get(&hash).cloned()
    }
    
    pub fn set_compiled_query(&self, query_text: &str, query: Arc<dyn Query>) {
        let hash = self.hash_query(query_text);
        self.compiled_queries.write().put(hash, query);
    }
}

// src/index/reader.rs
pub fn search(&self, query_text: &str, limit: usize) -> Result<Vec<SearchResult>> {
    // Check compiled query cache
    let query = if let Some(cached) = self.cache.get_compiled_query(query_text) {
        cached
    } else {
        // ... build query ...
        let query = Arc::new(BooleanQuery::new(subqueries));
        self.cache.set_compiled_query(query_text, Arc::clone(&query));
        query
    };
    
    // ... rest of search
}
```

**Validation:**
```bash
cargo bench search_warm
# Expect: 30-40% improvement for repeated queries
```

## Phase 5: IO Optimization (Week 5)

### 5.1 Batch File Reads During Indexing
**Impact:** 3-5x faster indexing
**Effort:** High
**Risk:** Medium

**Implementation:**
```rust
// src/parse/walker.rs
use tokio::fs;
use futures::stream::{self, StreamExt};

pub async fn walk_parallel(&self, batch_size: usize) -> Result<Vec<FileInfo>> {
    let files = self.walk()?;
    
    // Process files in parallel batches
    let results: Vec<_> = stream::iter(files)
        .chunks(batch_size)
        .then(|batch| async move {
            futures::future::join_all(
                batch.into_iter().map(|file| async move {
                    let content = fs::read_to_string(&file.path).await?;
                    Ok::<_, std::io::Error>((file, content))
                })
            ).await
        })
        .collect()
        .await;
    
    Ok(results.into_iter().flatten().collect::<Result<Vec<_>, _>>()?)
}
```

**Validation:**
```bash
hyperfine --warmup 1 --runs 10 \
  'greppy index --force'
# Expect: 3-5x improvement for large projects
```

### 5.2 Use Memory-Mapped Files for L2 Cache
**Impact:** 50-60% faster L2 cache loading
**Effort:** Medium
**Risk:** Low

**Dependencies:**
```toml
# Cargo.toml
memmap2 = "0.9"
```

**Implementation:**
```rust
// src/llm/cache.rs
use memmap2::Mmap;

fn load_l2_from_disk() -> Result<HashMap<String, CachedEnhancement>> {
    let path = Self::cache_path()?;
    if !path.exists() {
        return Ok(HashMap::new());
    }
    
    // Memory-map the file
    let file = std::fs::File::open(&path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    
    // Deserialize from mmap
    let data: HashMap<String, CachedEnhancement> = 
        serde_json::from_slice(&mmap)?;
    
    Ok(data)
}
```

**Validation:**
```bash
hyperfine --warmup 1 --runs 100 \
  --prepare 'greppy stop && greppy start' \
  'greppy search --smart "test"'
# Expect: 50-60% faster cold start
```

### 5.3 Batch Index Writes
**Impact:** 5-10x higher indexing throughput
**Effort:** High
**Risk:** High

**Implementation:**
```rust
// src/watch/mod.rs
use tokio::time::{interval, Duration};

pub struct WatchManager {
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
            
            if let Err(e) = self.index_files_batch(files).await {
                error!("Batch index failed: {}", e);
            }
        }
    }
    
    async fn index_files_batch(&self, files: Vec<PathBuf>) -> Result<()> {
        // Process all files in single transaction
        let chunker = Chunker::new();
        let mut all_chunks = Vec::new();
        
        for file in files {
            if let Ok(chunks) = chunker.chunk_file(&file) {
                all_chunks.extend(chunks);
            }
        }
        
        // Single commit for all changes
        let mut writer = IndexWriter::open(&self.project_path)?;
        writer.add_chunks(&all_chunks)?;
        writer.commit()?;
        
        Ok(())
    }
}
```

**Validation:**
```bash
# Simulate rapid file changes
for i in {1..100}; do
  echo "// Change $i" >> test.rs
  sleep 0.1
done

# Monitor index write rate
# Expect: 5-10x fewer index commits
```

## Phase 6: Latency Optimization (Week 6)

### 6.1 Implement Query Result Streaming
**Impact:** 40-50% reduction in time-to-first-result
**Effort:** High
**Risk:** Medium

**Implementation:**
```rust
// src/search/results.rs
use tokio::sync::mpsc;

pub struct StreamingSearcher {
    searcher: IndexSearcher,
}

impl StreamingSearcher {
    pub async fn search_stream(
        &self,
        query: &str,
        limit: usize,
    ) -> mpsc::Receiver<SearchResult> {
        let (tx, rx) = mpsc::channel(100);
        let searcher = self.searcher.clone();
        let query = query.to_string();
        
        tokio::spawn(async move {
            match searcher.search(&query, limit) {
                Ok(results) => {
                    for result in results {
                        if tx.send(result).await.is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Search failed: {}", e);
                }
            }
        });
        
        rx
    }
}
```

**Validation:**
```bash
# Measure time to first result
hyperfine --warmup 5 \
  'greppy search "test" | head -1'
# Expect: 40-50% improvement
```

### 6.2 Connection Pooling for Daemon
**Impact:** 2-3x higher throughput under load
**Effort:** High
**Risk:** High

**Implementation:**
```rust
// src/daemon/server.rs
use tokio::task::JoinSet;

pub struct DaemonServer {
    task_pool: Arc<RwLock<JoinSet<()>>>,
    max_concurrent_tasks: usize,
    // ... existing fields
}

impl DaemonServer {
    pub async fn run(&self) -> Result<()> {
        // ... existing setup ...
        
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let mut pool = self.task_pool.write().await;
                            
                            // Wait if at capacity
                            while pool.len() >= self.max_concurrent_tasks {
                                pool.join_next().await;
                            }
                            
                            // Spawn task
                            pool.spawn(handle_connection(/* ... */));
                        }
                        Err(e) => error!("Accept error: {}", e),
                    }
                }
                // ... rest of loop
            }
        }
    }
}
```

**Validation:**
```bash
# Load test with multiple concurrent clients
for i in {1..50}; do
  greppy search "test" &
done
wait

# Expect: 2-3x higher throughput
```

## Phase 7: Validation & Tuning (Week 7)

### 7.1 Run Full Benchmark Suite
```bash
# Compare against baseline
cargo bench -- --baseline v0.2.0

# Generate comparison report
critcmp v0.2.0 optimized
```

### 7.2 Profile Optimized Build
```bash
# CPU profiling
cargo flamegraph --bench search_bench -- --bench

# Memory profiling
cargo run --features dhat-heap --bin greppy -- search "test"

# Load testing
hyperfine --warmup 10 --runs 1000 \
  --export-json optimized-v0.3.0.json \
  'greppy search "authenticate"'
```

### 7.3 Validate Performance Targets

| Metric | Baseline | Target | Achieved | Status |
|--------|----------|--------|----------|--------|
| Simple search latency | 0.87ms | <1ms | TBD | ⏳ |
| Complex search latency | 1.34ms | <2ms | TBD | ⏳ |
| Memory (daemon RSS) | 82MB | <100MB | TBD | ⏳ |
| Throughput (searches/sec) | 1000 | >1500 | TBD | ⏳ |
| Index speed (chunks/sec) | 500 | >1000 | TBD | ⏳ |

### 7.4 Regression Testing
```bash
# Ensure no accuracy regressions
cargo test

# Validate search results unchanged
./scripts/validate_results.sh
```

## Phase 8: Documentation & Release (Week 8)

### 8.1 Update Documentation
- [ ] Update `PERFORMANCE.md` with new benchmarks
- [ ] Document optimization techniques in code comments
- [ ] Update README with new performance claims
- [ ] Create migration guide for breaking changes

### 8.2 Release Preparation
- [ ] Tag release v0.3.0
- [ ] Generate changelog
- [ ] Update installation instructions
- [ ] Publish to crates.io

## Success Criteria

### Must Have (P0)
- [ ] Search latency p99 < 2ms
- [ ] Memory usage < 100MB for typical workload
- [ ] No accuracy regressions
- [ ] All tests passing

### Should Have (P1)
- [ ] 2x throughput improvement
- [ ] 50% reduction in indexing time
- [ ] Comprehensive benchmarks in CI

### Nice to Have (P2)
- [ ] Flamegraph analysis in docs
- [ ] Performance dashboard
- [ ] Automated performance alerts

## Risk Mitigation

### High Risk Changes
1. **Parallel document processing** - May introduce race conditions
   - Mitigation: Extensive testing, feature flag
   
2. **Batch index writes** - Could lose data on crash
   - Mitigation: WAL (write-ahead log), fsync

3. **Connection pooling** - Complex concurrency management
   - Mitigation: Thorough load testing, gradual rollout

### Rollback Plan
- Keep v0.2.0 branch stable
- Feature flags for major optimizations
- Automated performance regression detection in CI

## Monitoring & Alerts

### Production Metrics
- Search latency (p50, p95, p99)
- Memory usage (RSS, heap)
- Cache hit rates (L1, L2)
- Throughput (searches/sec)
- Error rates

### Alert Thresholds
- P99 latency > 10ms
- Memory growth > 10MB/hour
- Cache hit rate < 80%
- Error rate > 1%

## References

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Tantivy Performance Guide](https://github.com/quickwit-oss/tantivy/blob/main/PERFORMANCE.md)
- [Criterion User Guide](https://bheisler.github.io/criterion.rs/book/)
- [Flamegraph Guide](https://www.brendangregg.com/flamegraphs.html)
