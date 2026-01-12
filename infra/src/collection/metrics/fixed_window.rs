use std::time::{Duration, Instant};

use crate::collection::metrics::rolling_window::{
    Bucket, RollingWindow, RollingWindowConfigError, RollingWindowOption, ignore_current_bucket,
};

/// 固定时间窗口：每个 window 周期滚动一次。
///
/// 为了避免“半桶数据”，这里内部使用 2 个桶，并默认 ignore current：
/// - 当前桶：正在累计（可能是半桶）
/// - 上一个桶：完整窗口，可用于统计
#[derive(Debug)]
pub struct FixedWindow<V, B>
where
    B: Bucket<V>,
{
    inner: RollingWindow<V, B>,
}

#[derive(Debug, thiserror::Error)]
pub enum FixedWindowConfigError {
    #[error(transparent)]
    Rolling(#[from] RollingWindowConfigError),
}

impl<V, B> FixedWindow<V, B>
where
    B: Bucket<V>,
{
    pub fn try_new(
        new_bucket: impl FnMut() -> B,
        window: Duration,
        opts: &[RollingWindowOption<V, B>],
    ) -> Result<Self, FixedWindowConfigError> {
        // 默认 ignore current（避免半桶），调用方可自行覆盖（再次设置也无副作用）。
        let mut merged: Vec<RollingWindowOption<V, B>> = Vec::with_capacity(opts.len() + 1);
        merged.push(ignore_current_bucket::<V, B>());
        merged.extend_from_slice(opts);

        Ok(Self {
            inner: RollingWindow::try_new(new_bucket, 2, window, &merged)?,
        })
    }

    pub fn add(&self, now: Instant, v: V) {
        self.inner.add(now, v);
    }

    pub fn reduce(&self, now: Instant, f: impl FnMut(&B)) {
        self.inner.reduce(now, f);
    }
}
