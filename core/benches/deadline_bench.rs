use criterion::{Criterion, criterion_group, criterion_main};
use halo_core::context::{Background, WithTimeout};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Builder;

const TIMEOUTS: &[u64] = &[10, 50, 100];

fn bench_deadline_timeout(c: &mut Criterion) {
    let rt = Arc::new(
        Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("rt"),
    );
    for &ms in TIMEOUTS {
        let name = format!("halo_timeout_{}ms", ms);
        let rt = rt.clone();
        c.bench_function(&name, |b| {
            let rt = rt.clone();
            b.iter(|| {
                rt.block_on(async {
                    let (_ctx, cancel) = WithTimeout(Background(), Duration::from_millis(ms));
                    cancel(); // cancel early
                })
            })
        });
    }
}

criterion_group!(benches, bench_deadline_timeout);
criterion_main!(benches);
