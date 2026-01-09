use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use greppy::cache::QueryCache;
use greppy::search::SearchResult;

fn create_mock_results(count: usize) -> Vec<SearchResult> {
    (0..count)
        .map(|i| SearchResult {
            path: format!("src/file{}.rs", i),
            content: format!("fn test_function_{}() {{}}", i),
            symbol_name: Some(format!("test_function_{}", i)),
            symbol_type: Some("function".to_string()),
            start_line: i * 10,
            end_line: i * 10 + 5,
            language: "rust".to_string(),
            score: 10.0 - (i as f32 * 0.1),
            signature: Some(format!("fn test_function_{}()", i)),
            parent_symbol: None,
            doc_comment: None,
            is_exported: true,
            is_test: false,
        })
        .collect()
}

fn bench_cache_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_insert");

    for result_count in [10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(result_count),
            result_count,
            |b, &count| {
                let cache = QueryCache::new();
                let results = create_mock_results(count);

                b.iter(|| {
                    cache.set(
                        black_box("test query"),
                        black_box("/test/project"),
                        black_box(results.clone()),
                    );
                });
            },
        );
    }
    group.finish();
}

fn bench_cache_lookup_hit(c: &mut Criterion) {
    let cache = QueryCache::new();
    let results = create_mock_results(20);
    cache.set("test query", "/test/project", results);

    c.bench_function("cache_lookup_hit", |b| {
        b.iter(|| {
            let result = cache.get(black_box("test query"), black_box("/test/project"));
            black_box(result);
        });
    });
}

fn bench_cache_lookup_miss(c: &mut Criterion) {
    let cache = QueryCache::new();

    c.bench_function("cache_lookup_miss", |b| {
        b.iter(|| {
            let result = cache.get(black_box("nonexistent query"), black_box("/test/project"));
            black_box(result);
        });
    });
}

fn bench_cache_eviction(c: &mut Criterion) {
    c.bench_function("cache_eviction", |b| {
        b.iter(|| {
            let cache = QueryCache::new();
            let results = create_mock_results(20);

            // Fill cache beyond capacity to trigger eviction
            for i in 0..1500 {
                cache.set(&format!("query_{}", i), "/test/project", results.clone());
            }

            black_box(cache);
        });
    });
}

criterion_group!(
    benches,
    bench_cache_insert,
    bench_cache_lookup_hit,
    bench_cache_lookup_miss,
    bench_cache_eviction
);
criterion_main!(benches);
