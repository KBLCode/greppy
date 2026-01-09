use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use greppy::index::{IndexSearcher, IndexWriter};
use greppy::parse::{Chunker, FileWalker};
use std::path::PathBuf;
use tempfile::TempDir;

fn setup_test_index() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    // Create test files
    std::fs::create_dir_all(project_path.join("src")).unwrap();
    std::fs::write(
        project_path.join("src/lib.rs"),
        r#"
pub fn authenticate(username: &str, password: &str) -> bool {
    // Authentication logic
    true
}

pub struct User {
    pub id: u64,
    pub name: String,
}

impl User {
    pub fn new(id: u64, name: String) -> Self {
        Self { id, name }
    }
}
"#,
    )
    .unwrap();

    std::fs::write(
        project_path.join("src/database.rs"),
        r#"
use std::collections::HashMap;

pub struct Database {
    users: HashMap<u64, String>,
}

impl Database {
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
        }
    }
    
    pub fn insert(&mut self, id: u64, name: String) {
        self.users.insert(id, name);
    }
}
"#,
    )
    .unwrap();

    // Index the project
    let walker = FileWalker::new(&project_path);
    let files = walker.walk().unwrap();

    let mut writer = IndexWriter::create(&project_path).unwrap();

    for file in files {
        if let Ok(chunks) = Chunker::chunk_file(&file, &project_path) {
            for chunk in chunks {
                writer.add_chunk(&chunk).unwrap();
            }
        }
    }

    writer.commit().unwrap();

    (temp_dir, project_path)
}

fn bench_search_cold(c: &mut Criterion) {
    let (_temp, project_path) = setup_test_index();

    c.bench_function("search_cold_simple", |b| {
        b.iter(|| {
            let searcher = IndexSearcher::open(&project_path).unwrap();
            let results = searcher.search(black_box("authenticate"), 20).unwrap();
            black_box(results);
        });
    });
}

fn bench_search_warm(c: &mut Criterion) {
    let (_temp, project_path) = setup_test_index();
    let searcher = IndexSearcher::open(&project_path).unwrap();

    c.bench_function("search_warm_simple", |b| {
        b.iter(|| {
            let results = searcher.search(black_box("authenticate"), 20).unwrap();
            black_box(results);
        });
    });
}

fn bench_search_varying_limits(c: &mut Criterion) {
    let (_temp, project_path) = setup_test_index();
    let searcher = IndexSearcher::open(&project_path).unwrap();

    let mut group = c.benchmark_group("search_limits");
    for limit in [10, 20, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(limit), limit, |b, &limit| {
            b.iter(|| {
                let results = searcher.search(black_box("user"), limit).unwrap();
                black_box(results);
            });
        });
    }
    group.finish();
}

fn bench_search_query_complexity(c: &mut Criterion) {
    let (_temp, project_path) = setup_test_index();
    let searcher = IndexSearcher::open(&project_path).unwrap();

    let mut group = c.benchmark_group("query_complexity");

    group.bench_function("single_term", |b| {
        b.iter(|| {
            let results = searcher.search(black_box("user"), 20).unwrap();
            black_box(results);
        });
    });

    group.bench_function("two_terms", |b| {
        b.iter(|| {
            let results = searcher.search(black_box("user database"), 20).unwrap();
            black_box(results);
        });
    });

    group.bench_function("three_terms", |b| {
        b.iter(|| {
            let results = searcher
                .search(black_box("user database authenticate"), 20)
                .unwrap();
            black_box(results);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_search_cold,
    bench_search_warm,
    bench_search_varying_limits,
    bench_search_query_complexity
);
criterion_main!(benches);
