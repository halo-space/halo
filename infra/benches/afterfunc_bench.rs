use criterion::{Criterion, criterion_group, criterion_main};
use infra::context::{AfterFunc, Background, WithCancel};
use std::sync::Arc;
use tokio::runtime::Builder;

const CALLBACKS: &[usize] = &[10, 100, 500, 1000];

fn bench_afterfunc(c: &mut Criterion) {
    let rt = Arc::new(
        Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("rt"),
    );
    for &n in CALLBACKS {
        let name = format!("halo_afterfunc_callbacks_{}", n);
        let rt = rt.clone();
        c.bench_function(&name, |b| {
            let rt = rt.clone();
            b.iter(|| {
                rt.block_on(async {
                    let (ctx, cancel) = WithCancel(Background());
                    let mut stops = Vec::with_capacity(n);
                    for _ in 0..n {
                        stops.push(AfterFunc(&ctx, || {}));
                    }
                    cancel();
                    // ensure Stop can still be called safely after cancel
                    for stop in stops {
                        let _ = stop.Stop();
                    }
                })
            })
        });
    }
}

criterion_group!(benches, bench_afterfunc);
criterion_main!(benches);
