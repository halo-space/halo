//! Breaker 核心抽象（Rust 风格：RAII `Promise` + 语义化拒绝）。
//!
//! 设计目标：
//! - **简洁**：策略只实现 `name/allow/success/fail`，其余 `execute*` 由 trait 默认实现提供。
//! - **不易误用**：`Promise` 采用 RAII，若调用方忘记结算，`Drop` 会自动记为失败（避免漏统计）。
//! - **可扩展**：通过 `Reject`/`RetryHint` 表达过载/熔断/配额等拒绝语义，便于上层做降级与重试控制。

use std::fmt::Formatter;
use std::panic::catch_unwind;

use crate::context::Context;

use std::time::Instant;

use crate::breaker::google_sre::{GoogleSreJudge, GoogleSreParams, Snapshot};
use crate::collection::metrics::{
    Counter, FixedWindow, FixedWindowConfigError, RollingWindow, RollingWindowConfigError,
    ignore_current_bucket,
};

/// 允许票据的统一能力：成功/失败结算。
pub trait Permit {
    fn success(self);
    fn fail(self, reason: &str);
}

/// 默认：任何调用错误都不接受（即 Err 计失败）。
pub const fn default_acceptable<E>(_err: &E) -> bool {
    false
}

/// 重试提示（用于避免重试风暴，参见 SRE overload 建议）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryHint {
    /// 不要重试（例如：全链路过载、已到重试预算等）。
    DontRetry,
    /// 可重试，但建议等待一段时间（可选）。
    RetryAfter(std::time::Duration),
}

/// 语义化拒绝原因。
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Reject {
    /// ctx 已结束（取消/超时）。
    #[error("context done")]
    CtxDone,

    /// 熔断器打开。
    #[error("open")]
    Open { retry: RetryHint },

    /// 系统过载（入口准入失败/自适应节流等）。
    #[error("overloaded")]
    Overloaded { retry: RetryHint },

    /// 配额不足（per-customer limits）。
    #[error("out of quota: {key}")]
    OutOfQuota { key: String, retry: RetryHint },
}

/// `execute*` 系列的错误类型：语义化区分“被拒绝 / 调用失败 / panic”。
///
/// 注意：这里不强约束 `E: std::error::Error`，以适配部分上层错误类型（如 `anyhow::Error`）。
#[derive(Debug, thiserror::Error)]
pub enum ExecuteError<E: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static> {
    /// `allow()` 未通过（过载/熔断/配额/ctx done 等）。
    #[error("rejected: {0}")]
    Rejected(Reject),

    /// 被保护的调用本身返回 Err。
    #[error("call error: {0}")]
    Call(E),

    /// 被保护的调用发生 panic（已计入 fail），但这里不再 re-panic。
    #[error("panic in request")]
    Panic,
}

/// 一次“被允许执行”的票据（RAII）。
///
/// - 调用方（或 `execute*` 默认实现）需要在结束时显式调用 `success()` 或 `fail(...)`。
/// - 若未显式结算，`Drop` 会自动视为失败（reason = "dropped"）。
pub struct Promise<'a, B: BreakerPolicy + ?Sized> {
    breaker: &'a B,
    done: bool,
}

impl<B: BreakerPolicy + ?Sized> std::fmt::Debug for Promise<'_, B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Promise").finish()
    }
}

impl<'a, B: BreakerPolicy + ?Sized> Promise<'a, B> {
    pub(crate) fn new(breaker: &'a B) -> Self {
        Self {
            breaker,
            done: false,
        }
    }

    pub fn success(mut self) {
        if !self.done {
            self.breaker.success();
            self.done = true;
        }
    }

    pub fn fail(mut self, reason: &str) {
        if !self.done {
            self.breaker.fail(reason);
            self.done = true;
        }
    }
}

impl<B: BreakerPolicy + ?Sized> Permit for Promise<'_, B> {
    fn success(self) {
        Promise::success(self);
    }

    fn fail(self, reason: &str) {
        Promise::fail(self, reason);
    }
}

impl<B: BreakerPolicy + ?Sized> Drop for Promise<'_, B> {
    fn drop(&mut self) {
        if !self.done {
            self.breaker.fail("dropped");
            self.done = true;
        }
    }
}

/// Breaker 基础约束（简洁：策略只实现 name/allow/success/fail，其余 Do* 默认实现）。
///
/// - `allow(ctx)`：放行则返回 `Promise`；拒绝则返回 `Reject`
/// - `success/fail`：策略回写（窗口统计/状态机/并发计数等）
pub trait BreakerPolicy: Send + Sync + 'static {
    type Error: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static;
    type Promise<'a>: Permit
    where
        Self: 'a;

    fn name(&self) -> &str;

    fn allow(&self, ctx: &Context) -> Result<Self::Promise<'_>, Reject>;

    fn success(&self);
    fn fail(&self, reason: &str);

    fn execute<T>(
        &self,
        ctx: &Context,
        req: impl FnOnce() -> Result<T, Self::Error>,
    ) -> Result<T, ExecuteError<Self::Error>>
    where
        Self: Sized,
    {
        self.execute_with_fallback_acceptable(
            ctx,
            req,
            None::<fn(Reject) -> Result<T, ExecuteError<Self::Error>>>,
            default_acceptable,
        )
    }

    fn execute_with_acceptable<T>(
        &self,
        ctx: &Context,
        req: impl FnOnce() -> Result<T, Self::Error>,
        acceptable: fn(&Self::Error) -> bool,
    ) -> Result<T, ExecuteError<Self::Error>>
    where
        Self: Sized,
    {
        self.execute_with_fallback_acceptable(
            ctx,
            req,
            None::<fn(Reject) -> Result<T, ExecuteError<Self::Error>>>,
            acceptable,
        )
    }

    fn execute_with_fallback<T>(
        &self,
        ctx: &Context,
        req: impl FnOnce() -> Result<T, Self::Error>,
        fallback: fn(Reject) -> Result<T, ExecuteError<Self::Error>>,
    ) -> Result<T, ExecuteError<Self::Error>>
    where
        Self: Sized,
    {
        self.execute_with_fallback_acceptable(ctx, req, Some(fallback), default_acceptable)
    }

    fn execute_with_fallback_acceptable<T>(
        &self,
        ctx: &Context,
        req: impl FnOnce() -> Result<T, Self::Error>,
        fallback: Option<fn(Reject) -> Result<T, ExecuteError<Self::Error>>>,
        acceptable: fn(&Self::Error) -> bool,
    ) -> Result<T, ExecuteError<Self::Error>>
    where
        Self: Sized,
    {
        if ctx.done().is_done() {
            return Err(ExecuteError::Rejected(Reject::CtxDone));
        }

        let promise = match self.allow(ctx) {
            Ok(p) => p,
            Err(reject) => {
                if let Some(fb) = fallback {
                    return fb(reject);
                }
                return Err(ExecuteError::Rejected(reject));
            }
        };

        let res = catch_unwind(std::panic::AssertUnwindSafe(req));
        match res {
            Ok(Ok(v)) => {
                promise.success();
                Ok(v)
            }
            Ok(Err(e)) => {
                if acceptable(&e) {
                    promise.success();
                    Err(ExecuteError::Call(e))
                } else {
                    promise.fail("error");
                    Err(ExecuteError::Call(e))
                }
            }
            Err(panic) => {
                promise.fail("panic");
                let _ = panic;
                Err(ExecuteError::Panic)
            }
        }
    }
}

/// breaker 构造错误（语义化，便于调用方处理）。
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error(transparent)]
    RollingWindowConfig(#[from] RollingWindowConfigError),

    #[error(transparent)]
    FixedWindowConfig(#[from] FixedWindowConfigError),
}

/// 对外配置：只有三种采集方式（滑动时间窗口 / 固定时间窗口 / 计数）。
///
/// - 采集到的窗口统计统一交给 Google SRE(eq2101) 判定是否拒绝（是否 open）。
#[derive(Debug, Clone)]
pub enum BreakerConfig {
    RollingWindow {
        window: std::time::Duration,
        buckets: usize,
        google: Option<GoogleSreParams>,
    },
    FixedWindow {
        window: std::time::Duration,
        google: Option<GoogleSreParams>,
    },
    Counter {
        google: Option<GoogleSreParams>,
    },
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self::RollingWindow {
            window: std::time::Duration::from_secs(10),
            buckets: 40,
            google: None,
        }
    }
}

#[derive(Debug)]
enum Event {
    Success,
    Failure,
    Drop,
}

#[derive(Debug, Default, Clone, Copy)]
struct BreakerBucket {
    sum: i64,
    success: i64,
    failure: i64,
    drop: i64,
}

impl crate::collection::metrics::Bucket<Event> for BreakerBucket {
    fn add(&mut self, v: Event) {
        self.sum += 1;
        match v {
            Event::Success => self.success += 1,
            Event::Failure => self.failure += 1,
            Event::Drop => self.drop += 1,
        }
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug)]
enum MetricsImpl {
    Rolling {
        buckets: usize,                             // 逻辑 buckets（对齐 breaker config）
        inner: RollingWindow<Event, BreakerBucket>, // 内部会额外 +1 桶用于忽略 current
    },
    Fixed {
        inner: FixedWindow<Event, BreakerBucket>,
    },
    Counter {
        inner: Counter<Event, BreakerBucket>,
    },
}

impl MetricsImpl {
    fn record(&self, now: Instant, event: Event) {
        match self {
            MetricsImpl::Rolling { inner, .. } => inner.add(now, event),
            MetricsImpl::Fixed { inner } => inner.add(now, event),
            MetricsImpl::Counter { inner } => inner.add(now, event),
        }
    }

    fn snapshot(&self, now: Instant) -> Snapshot {
        match self {
            MetricsImpl::Rolling { buckets, inner } => snapshot_from_reduce(*buckets, now, |f| {
                inner.reduce(now, f);
            }),
            MetricsImpl::Fixed { inner } => snapshot_from_reduce(1, now, |f| {
                inner.reduce(now, f);
            }),
            MetricsImpl::Counter { inner } => snapshot_from_reduce(1, now, |f| {
                inner.reduce(now, f);
            }),
        }
    }
}

fn snapshot_from_reduce(
    buckets: usize,
    _now: Instant,
    reduce: impl FnOnce(&mut dyn FnMut(&BreakerBucket)),
) -> Snapshot {
    let mut r = Snapshot {
        buckets: buckets.max(1),
        ..Default::default()
    };

    let mut f = |b: &BreakerBucket| {
        r.accepts += b.success;
        r.total += b.sum;

        // 连续桶计数（从旧到新）
        if b.failure > 0 {
            r.working_buckets = 0;
        } else if b.success > 0 {
            r.working_buckets += 1;
        }

        if b.success > 0 {
            r.failing_buckets = 0;
        } else if b.failure > 0 {
            r.failing_buckets += 1;
        }
    };

    reduce(&mut f);
    r
}

/// 对外统一 breaker 实例：入口 `Breaker::new("demo", config)`。
#[derive(Debug)]
pub struct Breaker {
    name: String,
    window: MetricsImpl,
    judge: GoogleSreJudge,
}

impl Breaker {
    pub fn new(name: impl Into<String>, config: BreakerConfig) -> Result<Self, BuildError> {
        let name = name.into();
        let (window, google) = match config {
            BreakerConfig::RollingWindow {
                window,
                buckets,
                google,
            } => (
                MetricsImpl::Rolling {
                    buckets,
                    inner: RollingWindow::try_new(
                        || BreakerBucket::default(),
                        buckets.saturating_add(1),
                        if buckets == 0 {
                            window
                        } else {
                            std::time::Duration::from_nanos(
                                (window.as_nanos() / buckets as u128) as u64,
                            )
                        },
                        &[ignore_current_bucket::<Event, BreakerBucket>()],
                    )?,
                },
                google,
            ),
            BreakerConfig::FixedWindow { window, google } => (
                MetricsImpl::Fixed {
                    inner: FixedWindow::try_new(|| BreakerBucket::default(), window, &[])?,
                },
                google,
            ),
            BreakerConfig::Counter { google } => (
                MetricsImpl::Counter {
                    inner: Counter::new(|| BreakerBucket::default()),
                },
                google,
            ),
        };

        Ok(Self {
            name,
            window,
            judge: GoogleSreJudge::new(google.unwrap_or_default()),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn execute<T>(
        &self,
        ctx: &Context,
        req: impl FnOnce() -> Result<T, anyhow::Error>,
    ) -> Result<T, ExecuteError<anyhow::Error>> {
        BreakerPolicy::execute(self, ctx, req)
    }
}

impl BreakerPolicy for Breaker {
    type Error = anyhow::Error;
    type Promise<'a>
        = Promise<'a, Breaker>
    where
        Self: 'a;

    fn name(&self) -> &str {
        &self.name
    }

    fn allow(&self, ctx: &Context) -> Result<Self::Promise<'_>, Reject> {
        if ctx.done().is_done() {
            return Err(Reject::CtxDone);
        }

        let now = Instant::now();
        let snap = self.window.snapshot(now);
        if self.judge.should_reject(now, snap) {
            self.window.record(now, Event::Drop);
            return Err(Reject::Open {
                retry: RetryHint::DontRetry,
            });
        }

        Ok(Promise::new(self))
    }

    fn success(&self) {
        self.window.record(Instant::now(), Event::Success);
    }

    fn fail(&self, _reason: &str) {
        self.window.record(Instant::now(), Event::Failure);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Background;

    #[test]
    fn breaker_build_default_is_rolling_window() {
        let b = Breaker::new("x", BreakerConfig::default()).unwrap();
        assert_eq!(b.name(), "x");
        let ctx = Background();
        let _ = b.execute(&ctx, || Ok::<_, anyhow::Error>(1)).unwrap();
    }
}
