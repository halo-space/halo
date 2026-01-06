use criterion::{Criterion, criterion_group, criterion_main};
use halo_core::context::{Background, WithValue};

const DEPTHS: &[usize] = &[1, 4, 16, 64, 256];

fn bench_value_lookup(c: &mut Criterion) {
    c.bench_function("value_build_and_lookup", |b| {
        b.iter(|| {
            for &depth in DEPTHS {
                let mut ctx = Background();
                for i in 0..depth {
                    ctx = WithValue(ctx, i, i);
                }
                let key = depth.saturating_sub(1);
                let val = ctx.value(&key).unwrap();
                let _ = val.downcast::<usize>().unwrap();
            }
        })
    });
}

criterion_group!(benches, bench_value_lookup);
criterion_main!(benches);
