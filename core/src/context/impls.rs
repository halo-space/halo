#![allow(non_snake_case)]
use crate::context::error::{CANCELLED, ContextError, ContextErrorKind, DEADLINE_EXCEEDED};
use crate::context::state::{CancelKind, CancelState, DoneHandle, StopFunc};
use crate::context::value::ValueKey;
use std::any::Any;
use std::error::Error;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::OnceLock;
use std::task::{Context as TaskContext, Poll};
use std::thread;
use std::time::{Duration, Instant};
use tokio::runtime::Handle;

/// 与 Go `context` 包一致的取消函数。
pub type CancelFunc = Box<dyn FnOnce() + Send + 'static>;
/// 与 Go `context` 包一致的带 cause 取消函数。
pub type CancelCauseFunc = Box<dyn FnOnce(Option<Arc<dyn Error + Send + Sync>>) + Send + 'static>;

#[derive(Clone)]
pub struct Context {
    inner: Arc<ContextInner>,
}

enum ContextInner {
    Empty,
    Cancelable(CancelCtx),
    Deadline(DeadlineCtx),
    Value(ValueCtx),
    WithoutCancel(WithoutCancelCtx),
}

#[derive(Clone)]
struct CancelCtx {
    parent: Context,
    state: Arc<CancelState>,
}

#[derive(Clone)]
struct DeadlineCtx {
    parent: Context,
    state: Arc<CancelState>,
    deadline: Instant,
}

#[derive(Clone)]
struct WithoutCancelCtx {
    parent: Context,
}

struct ValueCtx {
    parent: Context,
    key: Arc<dyn ValueKey>,
    value: Arc<dyn Any + Send + Sync>,
}

impl Debug for Context {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context").finish_non_exhaustive()
    }
}

impl Context {
    fn empty() -> Self {
        Self {
            inner: Arc::new(ContextInner::Empty),
        }
    }

    fn cancelable(parent: Context, state: Arc<CancelState>) -> Self {
        Self {
            inner: Arc::new(ContextInner::Cancelable(CancelCtx { parent, state })),
        }
    }

    fn new_deadline(parent: Context, state: Arc<CancelState>, deadline: Instant) -> Self {
        Self {
            inner: Arc::new(ContextInner::Deadline(DeadlineCtx {
                parent,
                state,
                deadline,
            })),
        }
    }

    fn new_value(
        parent: Context,
        key: Arc<dyn ValueKey>,
        value: Arc<dyn Any + Send + Sync>,
    ) -> Self {
        Self {
            inner: Arc::new(ContextInner::Value(ValueCtx { parent, key, value })),
        }
    }

    fn without_cancel(parent: Context) -> Self {
        Self {
            inner: Arc::new(ContextInner::WithoutCancel(WithoutCancelCtx { parent })),
        }
    }

    pub fn deadline(&self) -> Option<Instant> {
        match self.inner.as_ref() {
            ContextInner::Empty => None,
            ContextInner::Cancelable(ctx) => ctx.parent.deadline(),
            ContextInner::Deadline(ctx) => Some(ctx.deadline),
            ContextInner::Value(ctx) => ctx.parent.deadline(),
            ContextInner::WithoutCancel(ctx) => ctx.parent.deadline(),
        }
    }

    pub fn done(&self) -> DoneHandle {
        match self.inner.as_ref() {
            ContextInner::Empty => DoneHandle::never(),
            ContextInner::Cancelable(ctx) => CancelState::done_handle(&ctx.state),
            ContextInner::Deadline(ctx) => CancelState::done_handle(&ctx.state),
            ContextInner::Value(ctx) => ctx.parent.done(),
            ContextInner::WithoutCancel(_) => DoneHandle::never(),
        }
    }

    /// 异步版 Done：返回一个 Future，在 Context 完成时立即 ready；Never 时永 pending。
    pub fn done_async(&self) -> DoneFuture {
        match self.done() {
            DoneHandle::Never => DoneFuture::Never,
            DoneHandle::Active(state) => {
                if state.is_done() {
                    return DoneFuture::Ready;
                }
                let notify = state.notify();
                DoneFuture::Wait(Box::pin(async move {
                    notify.notified().await;
                }))
            }
        }
    }

    pub fn err(&self) -> Option<ContextError> {
        match self.inner.as_ref() {
            ContextInner::Empty => None,
            ContextInner::Cancelable(ctx) => ctx.state.err(),
            ContextInner::Deadline(ctx) => ctx.state.err(),
            ContextInner::Value(ctx) => ctx.parent.err(),
            ContextInner::WithoutCancel(_) => None,
        }
    }

    pub fn cause(&self) -> Option<Arc<dyn Error + Send + Sync>> {
        match self.inner.as_ref() {
            ContextInner::Empty => None,
            ContextInner::Cancelable(ctx) => ctx.state.cause(),
            ContextInner::Deadline(ctx) => ctx.state.cause(),
            ContextInner::Value(ctx) => ctx.parent.cause(),
            ContextInner::WithoutCancel(_) => None,
        }
    }

    pub fn value(&self, key: &dyn ValueKey) -> Option<Arc<dyn Any + Send + Sync>> {
        match self.inner.as_ref() {
            ContextInner::Value(ctx) => {
                if ctx.key.equals(key) {
                    Some(ctx.value.clone())
                } else {
                    ctx.parent.value(key)
                }
            }
            ContextInner::Empty => None,
            ContextInner::Cancelable(ctx) => ctx.parent.value(key),
            ContextInner::Deadline(ctx) => ctx.parent.value(key),
            ContextInner::WithoutCancel(ctx) => ctx.parent.value(key),
        }
    }
}

/// `Done()` 的异步版 Future。
pub enum DoneFuture {
    Ready,
    Wait(Pin<Box<dyn Future<Output = ()> + Send + 'static>>),
    Never,
}

impl Future for DoneFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        match this {
            DoneFuture::Ready => Poll::Ready(()),
            DoneFuture::Never => Poll::Pending,
            DoneFuture::Wait(rx) => match rx.as_mut().poll(cx) {
                Poll::Ready(_) => {
                    *this = DoneFuture::Ready;
                    Poll::Ready(())
                }
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

pub fn Background() -> Context {
    static BG: OnceLock<Context> = OnceLock::new();
    BG.get_or_init(Context::empty).clone()
}

pub fn TODO() -> Context {
    static TD: OnceLock<Context> = OnceLock::new();
    TD.get_or_init(Context::empty).clone()
}

pub fn WithoutCancel(parent: Context) -> Context {
    Context::without_cancel(parent)
}

pub fn WithValue<K, V>(parent: Context, key: K, value: V) -> Context
where
    K: ValueKey,
    V: Any + Send + Sync + 'static,
{
    Context::new_value(
        parent,
        Arc::new(key),
        Arc::new(value) as Arc<dyn Any + Send + Sync>,
    )
}

pub fn WithCancel(parent: Context) -> (Context, CancelFunc) {
    WithCancelCause(parent).map_cancel(|f| Box::new(move || f(None)))
}

pub fn WithCancelCause(parent: Context) -> (Context, CancelCauseFunc) {
    let state = CancelState::new();
    propagate_parent(parent.clone(), state.clone());
    let ctx = Context::cancelable(parent, state.clone());
    let cancel = Box::new(move |cause: Option<Arc<dyn Error + Send + Sync>>| {
        let final_cause = cause.or_else(default_canceled);
        state.cancel(CancelKind::Canceled, final_cause);
    });
    (ctx, cancel)
}

pub fn WithDeadline(parent: Context, deadline: Instant) -> (Context, CancelFunc) {
    WithDeadlineCause(parent, deadline, None).map_cancel(|f| Box::new(move || f()))
}

pub fn WithDeadlineCause(
    parent: Context,
    deadline: Instant,
    cause: Option<Arc<dyn Error + Send + Sync>>,
) -> (Context, CancelFunc) {
    let effective_deadline = match parent.deadline() {
        Some(parent_deadline) if parent_deadline <= deadline => parent_deadline,
        _ => deadline,
    };

    let state = CancelState::new();
    let ctx = Context::new_deadline(parent.clone(), state.clone(), effective_deadline);
    propagate_parent(parent, state.clone());
    start_deadline_timer(state.clone(), effective_deadline, cause.clone());

    let cancel = Box::new(move || {
        let cancel_cause = cause.clone().or_else(default_canceled);
        state.cancel(CancelKind::Canceled, cancel_cause);
    });
    (ctx, cancel)
}

pub fn WithTimeout(parent: Context, timeout: Duration) -> (Context, CancelFunc) {
    WithDeadline(parent, Instant::now() + timeout)
}

pub fn WithTimeoutCause(
    parent: Context,
    timeout: Duration,
    cause: Option<Arc<dyn Error + Send + Sync>>,
) -> (Context, CancelFunc) {
    WithDeadlineCause(parent, Instant::now() + timeout, cause)
}

pub fn AfterFunc(ctx: &Context, f: impl FnOnce() + Send + 'static) -> StopFunc {
    ctx.done().register(f)
}

pub fn Cause(ctx: &Context) -> Option<Arc<dyn Error + Send + Sync>> {
    ctx.cause()
}

/// 异步等待上下文完成，返回 Err 时即 ctx.err()。
pub async fn Done(ctx: Context) -> Option<ContextError> {
    ctx.done_async().await;
    ctx.err()
}

/// 并发等待业务 Future 与 ctx 完成（取消/超时）。ctx 完成时优先返回其错误。
pub async fn ContextAware<T, F>(ctx: Context, fut: F) -> Result<T, ContextError>
where
    F: Future<Output = Result<T, ContextError>>,
{
    let done = ctx.done_async();
    tokio::select! {
        res = fut => res,
        _ = done => Err(ctx.err().unwrap_or(CANCELLED)),
    }
}

fn start_deadline_timer(
    state: Arc<CancelState>,
    deadline: Instant,
    cause: Option<Arc<dyn Error + Send + Sync>>,
) {
    if deadline <= Instant::now() {
        let deadline_cause = cause.clone().or_else(default_deadline);
        state.cancel(CancelKind::Deadline, deadline_cause);
        return;
    }
    let sleep_dur = deadline.saturating_duration_since(Instant::now());
    if let Ok(handle) = Handle::try_current() {
        handle.spawn(async move {
            tokio::time::sleep(sleep_dur).await;
            let deadline_cause = cause.clone().or_else(default_deadline);
            state.cancel(CancelKind::Deadline, deadline_cause);
        });
    } else {
        thread::spawn(move || {
            thread::sleep(sleep_dur);
            let deadline_cause = cause.clone().or_else(default_deadline);
            state.cancel(CancelKind::Deadline, deadline_cause);
        });
    }
}

fn propagate_parent(parent: Context, state: Arc<CancelState>) {
    if state.is_done() {
        return;
    }
    if let Some(err) = parent.err() {
        let kind = map_error_kind(&err);
        let inherited = parent
            .cause()
            .or_else(|| Some(Arc::new(err) as Arc<dyn Error + Send + Sync>));
        state.cancel(kind, inherited);
        return;
    }
    let done = parent.done();
    done.register(move || {
        let err = parent.err();
        let kind = err
            .as_ref()
            .map(map_error_kind)
            .unwrap_or(CancelKind::Canceled);
        let inherited = parent
            .cause()
            .or_else(|| err.map(|e| Arc::new(e) as Arc<dyn Error + Send + Sync>));
        state.cancel(kind, inherited);
    });
}

fn map_error_kind(err: &ContextError) -> CancelKind {
    match err.kind() {
        ContextErrorKind::Canceled => CancelKind::Canceled,
        ContextErrorKind::DeadlineExceeded => CancelKind::Deadline,
    }
}

fn default_canceled() -> Option<Arc<dyn Error + Send + Sync>> {
    Some(Arc::new(CANCELLED) as Arc<dyn Error + Send + Sync>)
}

fn default_deadline() -> Option<Arc<dyn Error + Send + Sync>> {
    Some(Arc::new(DEADLINE_EXCEEDED) as Arc<dyn Error + Send + Sync>)
}

trait MapCancel<T> {
    fn map_cancel(self, f: impl FnOnce(T) -> CancelFunc) -> (Context, CancelFunc);
}

impl MapCancel<CancelCauseFunc> for (Context, CancelCauseFunc) {
    fn map_cancel(self, f: impl FnOnce(CancelCauseFunc) -> CancelFunc) -> (Context, CancelFunc) {
        let (ctx, c) = self;
        (ctx, f(c))
    }
}

impl MapCancel<CancelFunc> for (Context, CancelFunc) {
    fn map_cancel(self, f: impl FnOnce(CancelFunc) -> CancelFunc) -> (Context, CancelFunc) {
        let (ctx, c) = self;
        (ctx, f(c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread::sleep;

    fn assert_canceled(ctx: &Context) {
        let err = ctx.err().expect("expected canceled");
        assert_eq!(err.kind(), ContextErrorKind::Canceled);
    }

    #[test]
    fn background_never_cancels() {
        let ctx = Background();
        assert!(ctx.deadline().is_none());
        assert!(ctx.done().is_done() == false);
        assert!(ctx.err().is_none());
        assert!(ctx.cause().is_none());
        assert!(ctx.value(&"k").is_none());
    }

    #[test]
    fn cancel_func_cancels() {
        let (ctx, cancel) = WithCancel(Background());
        cancel();
        assert_canceled(&ctx);
        assert!(matches!(Cause(&ctx), Some(_)));
    }

    #[test]
    fn cancel_cause_propagates() {
        #[derive(Debug)]
        struct MyErr;
        impl std::fmt::Display for MyErr {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("mine")
            }
        }
        impl Error for MyErr {}

        let (ctx, cancel) = WithCancelCause(Background());
        let err = Arc::new(MyErr) as Arc<dyn Error + Send + Sync>;
        cancel(Some(err.clone()));
        assert_canceled(&ctx);
        let cause = Cause(&ctx).unwrap();
        assert!(cause.downcast_ref::<MyErr>().is_some());
    }

    #[test]
    fn parent_deadline_cancels_child() {
        let (parent, _) = WithTimeout(Background(), Duration::from_millis(50));
        let (child, _) = WithCancel(parent);
        sleep(Duration::from_millis(80));
        let err = child.err().expect("child canceled");
        assert_eq!(err.kind(), ContextErrorKind::DeadlineExceeded);
    }

    #[test]
    fn deadline_timer_triggers() {
        let (ctx, _) = WithDeadline(Background(), Instant::now() + Duration::from_millis(30));
        sleep(Duration::from_millis(60));
        let err = ctx.err().expect("deadline");
        assert_eq!(err.kind(), ContextErrorKind::DeadlineExceeded);
    }

    #[test]
    fn deadline_cause_used() {
        #[derive(Debug)]
        struct CauseErr;
        impl std::fmt::Display for CauseErr {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("cause")
            }
        }
        impl Error for CauseErr {}

        let (ctx, cancel) = WithDeadlineCause(
            Background(),
            Instant::now() + Duration::from_millis(100),
            Some(Arc::new(CauseErr)),
        );
        cancel();
        let cause = Cause(&ctx).unwrap();
        assert!(cause.downcast_ref::<CauseErr>().is_some());
    }

    #[test]
    fn value_lookup_respects_hierarchy() {
        let root = WithValue(Background(), "a", 1u32);
        let child = WithValue(root, "b", 2u32);
        let val_a = child.value(&"a").unwrap();
        let val_b = child.value(&"b").unwrap();
        assert_eq!(*val_a.downcast::<u32>().unwrap(), 1);
        assert_eq!(*val_b.downcast::<u32>().unwrap(), 2);
    }

    #[test]
    fn without_cancel_detaches() {
        let (parent, cancel) = WithCancel(Background());
        let child = WithoutCancel(parent);
        cancel();
        assert!(child.err().is_none());
        assert!(child.cause().is_none());
        assert!(child.done().is_done() == false);
    }

    #[test]
    fn after_func_runs_on_cancel() {
        let (ctx, cancel) = WithCancel(Background());
        let flag = Arc::new(AtomicBool::new(false));
        let mark = flag.clone();
        AfterFunc(&ctx, move || {
            mark.store(true, Ordering::SeqCst);
        });
        cancel();
        ctx.done().wait();
        std::thread::sleep(Duration::from_millis(10));
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn after_func_stop_on_never_done() {
        let stop = AfterFunc(&Background(), || panic!("should not run"));
        assert!(!stop.Stop());
    }
}
