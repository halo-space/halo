//! RollingWindow：滚动窗口（按时间片分桶）。
//!
//! 目标：
//! - 通用：桶类型由调用方定义（只需实现 `Bucket<V>` 的 `add/reset`）。
//! - 高性能：`add` 用写锁，`reduce` 用读锁。
//! - 可选避免半桶：`ignore_current_bucket` 让 `reduce` 跳过当前桶（当前桶可能只有部分数据）。

use std::time::{Duration, Instant};

use parking_lot::RwLock;

fn duration_from_nanos_u128(nanos: u128) -> Duration {
    // 拆分成 (secs, nanos) 以避免 Duration::from_nanos(u64) 的上限问题。
    let secs = (nanos / 1_000_000_000) as u64;
    let sub = (nanos % 1_000_000_000) as u32;
    Duration::new(secs, sub)
}

/// Bucket 抽象：只需支持 Add + Reset。
pub trait Bucket<V>: Send + Sync + 'static {
    fn add(&mut self, v: V);
    fn reset(&mut self);
}

/// RollingWindow 的自定义选项。
pub type RollingWindowOption<V, B> = fn(&mut RollingWindow<V, B>);

/// 忽略当前桶，避免“半桶数据”。
pub fn ignore_current_bucket<V, B>() -> RollingWindowOption<V, B> {
    |w: &mut RollingWindow<V, B>| w.ignore_current = true
}

#[derive(Debug, thiserror::Error)]
pub enum RollingWindowConfigError {
    #[error("size must be > 0")]
    InvalidSize,
    #[error("interval must be > 0")]
    InvalidInterval,
}

#[derive(Debug)]
struct State<B> {
    buckets: Vec<B>,
    offset: usize,
    last_time: Instant, // 当前桶的起始时间（对齐到 interval 边界）
}

/// 滚动窗口：把时间按 interval 切成 size 个桶循环使用。
#[derive(Debug)]
pub struct RollingWindow<V, B> {
    size: usize,
    interval: Duration,
    ignore_current: bool,
    state: RwLock<State<B>>,
    _phantom: std::marker::PhantomData<V>,
}

impl<V, B> RollingWindow<V, B>
where
    B: Bucket<V>,
{
    fn reset_bucket(&self, st: &mut State<B>, offset: usize) {
        st.buckets[offset % self.size].reset();
    }

    pub fn try_new(
        mut new_bucket: impl FnMut() -> B,
        size: usize,
        interval: Duration,
        opts: &[RollingWindowOption<V, B>],
    ) -> Result<Self, RollingWindowConfigError> {
        if size == 0 {
            return Err(RollingWindowConfigError::InvalidSize);
        }
        if interval.is_zero() {
            return Err(RollingWindowConfigError::InvalidInterval);
        }

        let mut buckets = Vec::with_capacity(size);
        for _ in 0..size {
            buckets.push(new_bucket());
        }

        let mut w = Self {
            size,
            interval,
            ignore_current: false,
            state: RwLock::new(State {
                buckets,
                offset: 0,
                last_time: Instant::now(),
            }),
            _phantom: std::marker::PhantomData,
        };
        for opt in opts {
            opt(&mut w);
        }
        Ok(w)
    }

    pub fn size(&self) -> usize {
        self.size
    }

    #[inline]
    fn span(&self, now: Instant, last_time: Instant) -> usize {
        let offset = (now.duration_since(last_time).as_nanos() / self.interval.as_nanos()) as usize;
        if offset < self.size {
            offset
        } else {
            self.size
        }
    }

    fn update_offset(&self, now: Instant, st: &mut State<B>) {
        let span = self.span(now, st.last_time);
        if span == 0 {
            return;
        }

        let offset = st.offset;
        // reset expired buckets
        for i in 0..span {
            let idx = (offset + i + 1) % self.size;
            self.reset_bucket(st, idx);
        }

        st.offset = (offset + span) % self.size;

        // align to interval boundary: last_time = now - (now-last_time)%interval
        let elapsed = now.duration_since(st.last_time);
        let rem_nanos = elapsed.as_nanos() % self.interval.as_nanos();
        let rem = duration_from_nanos_u128(rem_nanos);
        st.last_time = now - rem;
    }

    /// 写入：加到“当前桶”。
    pub fn add(&self, now: Instant, v: V) {
        let mut st = self.state.write();
        self.update_offset(now, &mut st);
        let idx = st.offset;
        st.buckets[idx].add(v);
    }

    /// 聚合：遍历窗口内桶，执行 fn。
    ///
    /// - 若设置 `ignore_current`，且当前时间仍在当前桶（span==0），则跳过当前桶（避免半桶）。
    pub fn reduce(&self, now: Instant, mut f: impl FnMut(&B)) {
        let st = self.state.read();
        let span = self.span(now, st.last_time);

        let diff = if span == 0 && self.ignore_current {
            self.size.saturating_sub(1)
        } else {
            self.size.saturating_sub(span)
        };
        if diff == 0 {
            return;
        }

        let start = (st.offset + span + 1) % self.size;
        for i in 0..diff {
            f(&st.buckets[(start + i) % self.size]);
        }
    }
}
