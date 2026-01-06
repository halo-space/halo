use criterion::{Criterion, criterion_group, criterion_main};
use halo_core::context::{Background, WithCancel, WithoutCancel};
use std::thread;
use std::time::Duration;

fn bench_without_cancel(c: &mut Criterion) {
    c.bench_function("halo_without_cancel_detach", |b| {
        b.iter(|| {
            let (ctx, cancel) = WithCancel(Background());
            let detached = WithoutCancel(ctx.clone());
            // 让父取消
            cancel();
            // 模拟子仍然工作一小段时间
            thread::sleep(Duration::from_millis(1));
            assert!(detached.err().is_none());
        })
    });
}

criterion_group!(benches, bench_without_cancel);
criterion_main!(benches);
