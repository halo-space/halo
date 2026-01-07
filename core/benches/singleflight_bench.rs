use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::sync::Arc;
use tokio::runtime::Runtime;

use core::sync::singleflight::SingleFlight;

fn bench_singleflight(c: &mut Criterion) {
    let rt = Runtime::new().expect("rt");

    let mut g = c.benchmark_group("singleflight");
    for &n in &[1usize, 2, 4, 8, 16, 32, 64] {
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::new("shared_hit", n), &n, |b, &n| {
            b.iter(|| {
                rt.block_on(async {
                    let group = SingleFlight::<Arc<str>>::new();
                    let mut handles = Vec::with_capacity(n);
                    for _ in 0..n {
                        let g = group.clone();
                        let key: Arc<str> = Arc::from("k");
                        handles.push(tokio::spawn(async move {
                            g.done(&core::context::Background(), key, || async {
                                Ok::<_, ()>(1u32)
                            })
                            .await
                            .unwrap()
                        }));
                    }
                    for h in handles {
                        let _ = h.await.unwrap();
                    }
                });
            });
        });

        g.bench_with_input(BenchmarkId::new("distinct_keys", n), &n, |b, &n| {
            b.iter(|| {
                rt.block_on(async {
                    let group = SingleFlight::<Arc<str>>::new();
                    let mut handles = Vec::with_capacity(n);
                    for i in 0..n {
                        let g = group.clone();
                        handles.push(tokio::spawn(async move {
                            let key: Arc<str> = Arc::from(format!("k-{i}"));
                            g.done(&core::context::Background(), key, || async {
                                Ok::<_, ()>(1u32)
                            })
                            .await
                            .unwrap()
                        }));
                    }
                    for h in handles {
                        let _ = h.await.unwrap();
                    }
                });
            });
        });
    }
    g.finish();
}

criterion_group!(benches, bench_singleflight);
criterion_main!(benches);
