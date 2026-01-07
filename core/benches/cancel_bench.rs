use core::context::{Background, WithCancel};
use criterion::{Criterion, criterion_group, criterion_main};
use std::sync::Arc;
use tokio::runtime::Builder;
use tokio_util::sync::CancellationToken;

const WAITERS: &[usize] = &[100, 500, 1_000, 5_000, 10_000];

fn bench_halo_cancel_many_waiters(c: &mut Criterion) {
    let rt = Arc::new(
        Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("rt"),
    );
    for &waiter_count in WAITERS {
        let name = format!("halo_cancel_waiters_{}", waiter_count);
        let rt = rt.clone();
        c.bench_function(&name, |b| {
            let rt = rt.clone();
            b.iter(|| {
                let rt = rt.clone();
                rt.block_on(async {
                    let (ctx, cancel) = WithCancel(Background());
                    let mut tasks = Vec::with_capacity(waiter_count);
                    for _ in 0..waiter_count {
                        let c = ctx.clone();
                        tasks.push(tokio::spawn(async move {
                            c.done_async().await;
                        }));
                    }
                    cancel();
                    for t in tasks {
                        let _ = t.await;
                    }
                })
            })
        });
    }
}

fn bench_tokio_util_cancel_many_waiters(c: &mut Criterion) {
    let rt = Arc::new(
        Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("rt"),
    );
    for &waiter_count in WAITERS {
        let name = format!("tokio_util_cancel_waiters_{}", waiter_count);
        let rt = rt.clone();
        c.bench_function(&name, |b| {
            let rt = rt.clone();
            b.iter(|| {
                let rt = rt.clone();
                rt.block_on(async {
                    let token = CancellationToken::new();
                    let mut tasks = Vec::with_capacity(waiter_count);
                    for _ in 0..waiter_count {
                        let child = token.child_token();
                        tasks.push(tokio::spawn(async move {
                            child.cancelled().await;
                        }));
                    }
                    token.cancel();
                    for t in tasks {
                        let _ = t.await;
                    }
                })
            })
        });
    }
}

criterion_group!(
    benches,
    bench_halo_cancel_many_waiters,
    bench_tokio_util_cancel_many_waiters
);
criterion_main!(benches);
