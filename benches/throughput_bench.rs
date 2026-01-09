use criterion::{black_box, criterion_group, criterion_main, Criterion};
use greppy::daemon::{DaemonClient, Request};
use std::path::PathBuf;
use std::time::Instant;

fn throughput_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("ipc_throughput_100_requests", |b| {
        b.iter_custom(|iters| {
            let total = rt.block_on(async {
                let mut total_duration = std::time::Duration::ZERO;

                for _ in 0..iters {
                    let mut client = DaemonClient::connect().await.unwrap();

                    let start = Instant::now();
                    // Send 100 requests on the same connection
                    for _ in 0..100 {
                        let req = Request::search(
                            black_box("authentication".to_string()),
                            black_box(PathBuf::from("/Users/cw/Desktop/greppy/greppy-release")),
                            black_box(20),
                        );
                        let _ = client.send(req).await.unwrap();
                    }
                    total_duration += start.elapsed();
                }

                total_duration
            });
            total
        });
    });
}

criterion_group!(benches, throughput_benchmark);
criterion_main!(benches);
