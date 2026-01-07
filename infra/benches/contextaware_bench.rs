use criterion::{Criterion, criterion_group, criterion_main};
use infra::context::{Background, ContextAware, WithTimeout};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Builder;

fn bench_contextaware(c: &mut Criterion) {
    let rt = Arc::new(
        Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("rt"),
    );
    let rt_clone = rt.clone();
    c.bench_function("halo_contextaware_timeout_1s", |b| {
        let rt = rt_clone.clone();
        b.iter(|| {
            rt.block_on(async {
                let (ctx, _cancel) = WithTimeout(Background(), Duration::from_millis(50));
                let fut = async {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    Ok::<(), infra::context::ContextError>(())
                };
                let _ = ContextAware(ctx, fut).await;
            })
        })
    });
}

criterion_group!(benches, bench_contextaware);
criterion_main!(benches);
