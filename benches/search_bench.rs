//! Search benchmarks

use criterion::{criterion_group, criterion_main, Criterion};

fn search_benchmark(_c: &mut Criterion) {
    // TODO: Implement search benchmarks
}

criterion_group!(benches, search_benchmark);
criterion_main!(benches);
