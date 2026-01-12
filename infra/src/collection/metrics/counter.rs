use std::time::Instant;

use parking_lot::Mutex;

use crate::collection::metrics::rolling_window::Bucket;

/// Counter：只累计（不随时间衰减）。
#[derive(Debug)]
pub struct Counter<V, B>
where
    B: Bucket<V>,
{
    inner: Mutex<B>,
    _phantom: std::marker::PhantomData<V>,
}

impl<V, B> Counter<V, B>
where
    B: Bucket<V>,
{
    pub fn new(mut new_bucket: impl FnMut() -> B) -> Self {
        Self {
            inner: Mutex::new(new_bucket()),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn add(&self, _now: Instant, v: V) {
        self.inner.lock().add(v);
    }

    pub fn reduce(&self, _now: Instant, mut f: impl FnMut(&B)) {
        let b = self.inner.lock();
        f(&b);
    }
}
