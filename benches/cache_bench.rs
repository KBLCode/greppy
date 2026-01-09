use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use greppy::cache::QueryCache;
use greppy::search::{SearchResponse, SearchResult};
use std::sync::Arc;
use string_cache::DefaultAtom as Atom;

fn create_mock_results(count: usize) -> Vec<SearchResult> {
    (0..count)
        .map(|i| SearchResult {
            path: Atom::from(format!("src/file{}.rs", i)),
            content: Arc::from(format!("fn test_function_{}() {{}}", i).as_str()),
            symbol_name: Some(Arc::from(format!("test_function_{}", i).as_str())),
            symbol_type: Some(Arc::from("function")),
            start_line: i * 10,
            end_line: i * 10 + 5,
            language: Arc::from("rust"),
            score: 10.0 - (i as f32 * 0.1),
            signature: Some(Arc::from(format!("fn test_function_{}()", i).as_str())),
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
                let mut cache = QueryCache::new();
                let results = create_mock_results(count);
                let response = SearchResponse {
                    query: "test query".to_string(),
                    project: "/test/project".to_string(),
                    results: results.clone(),
                    elapsed_ms: 1.0,
                    cached: false,
                };

                b.iter(|| {
                    cache.put(
                        black_box("/test/project:test query".to_string()),
                        black_box(response.clone()),
                    );
                });
            },
        );
    }
    group.finish();
}

fn bench_cache_lookup_hit(c: &mut Criterion) {
    let mut cache = QueryCache::new();
    let results = create_mock_results(20);
    let response = SearchResponse {
        query: "test query".to_string(),
        project: "/test/project".to_string(),
        results,
        elapsed_ms: 1.0,
        cached: false,
    };
    cache.put("/test/project:test query".to_string(), response);

    c.bench_function("cache_lookup_hit", |b| {
        b.iter(|| {
            let result = cache.get(black_box("/test/project:test query"));
            black_box(result);
        });
    });
}

fn bench_cache_lookup_miss(c: &mut Criterion) {
    let cache = QueryCache::new();

    c.bench_function("cache_lookup_miss", |b| {
        b.iter(|| {
            let result = cache.get(black_box("/test/project:nonexistent query"));
            black_box(result);
        });
    });
}

fn bench_cache_eviction(c: &mut Criterion) {
    c.bench_function("cache_eviction", |b| {
        b.iter(|| {
            let mut cache = QueryCache::new();
            let results = create_mock_results(20);
            let response = SearchResponse {
                query: "test".to_string(),
                project: "/test/project".to_string(),
                results: results.clone(),
                elapsed_ms: 1.0,
                cached: false,
            };

            // Fill cache beyond capacity to trigger eviction
            for i in 0..1500 {
                cache.put(format!("/test/project:query_{}", i), response.clone());
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
