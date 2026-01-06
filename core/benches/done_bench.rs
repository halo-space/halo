use criterion::{criterion_group, criterion_main, Criterion};
use halo_core::context::{Background, WithCancel};
use std::sync::Arc;
use tokio::runtime::Builder;

const WAITERS: &[usize] = &[1, 10, 100, 500, 1000];

fn bench_done_sync_async(c: &mut Criterion) {
    let rt = Arc::new(
        Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("rt"),
    );
    for &n in WAITERS {
        let name = format!("halo_done_waiters_sync_async_{}", n);
        let rt = rt.clone();
        c.bench_function(&name, |b| {
            let rt = rt.clone();
            b.iter(|| {
                rt.block_on(async {
                    let (ctx, cancel) = WithCancel(Background());
                    let mut async_tasks = Vec::with_capacity(n);
                    for _ in 0..n {
                        let c = ctx.clone();
                        async_tasks.push(tokio::spawn(async move {
                            c.done_async().await;
                        }));
                    }
                    std::thread::spawn({
                        let c = ctx.clone();
                        move || {
                            c.done().wait();
                        }
                    })
                    .join()
                    .unwrap();
                    cancel();
                    for t in async_tasks {
                        let _ = t.await;
                    }
                })
            })
        });
    }
}

criterion_group!(benches, bench_done_sync_async);
criterion_main!(benches);
