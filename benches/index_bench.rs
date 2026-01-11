//! Indexing benchmarks

use criterion::{criterion_group, criterion_main, Criterion};

fn index_benchmark(_c: &mut Criterion) {
    // TODO: Implement indexing benchmarks
}

criterion_group!(benches, index_benchmark);
criterion_main!(benches);
