use crate::context::error::{CANCELLED, ContextError, DEADLINE_EXCEEDED, Error};
use std::error::Error;
use std::fmt::{Debug, Formatter};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use tokio::sync::Notify;

pub type CancelKind = Error;

/// 取消状态核心。
pub struct CancelState {
    status: AtomicU8, // 0 active, 1 canceled, 2 deadline
    cause: Mutex<Option<Arc<dyn Error + Send + Sync>>>,
    notify: Arc<Notify>,
    waiters: Waiters,
}

struct Waiters {
    inner: Mutex<WaitersInner>,
    cvar: Condvar,
}

struct WaitersInner {
    done: bool,
    next_id: usize,
    callbacks: Vec<(usize, Option<Box<dyn FnOnce() + Send + 'static>>)>,
}

impl Waiters {
    fn new() -> Self {
        Self {
            inner: Mutex::new(WaitersInner {
                done: false,
                next_id: 0,
                callbacks: Vec::new(),
            }),
            cvar: Condvar::new(),
        }
    }

    fn register(
        &self,
        owner: Arc<CancelState>,
        callback: Box<dyn FnOnce() + Send + 'static>,
    ) -> StopFunc {
        let mut guard = self.inner.lock().unwrap();
        if guard.done {
            drop(guard);
            // 已完成时异步执行，保持与 Go AfterFunc 语义一致。
            spawn_callback(callback);
            return StopFunc::noop();
        }
        let id = guard.next_id;
        guard.next_id += 1;
        guard.callbacks.push((id, Some(callback)));
        StopFunc::new(owner, id)
    }

    fn complete(&self) {
        let callbacks = {
            let mut guard = self.inner.lock().unwrap();
            if guard.done {
                return;
            }
            guard.done = true;
            let mut cbs = Vec::new();
            for (_, callback) in guard.callbacks.drain(..) {
                if let Some(callback) = callback {
                    cbs.push(callback);
                }
            }
            self.cvar.notify_all();
            cbs
        };
        for callback in callbacks {
            spawn_callback(callback);
        }
    }

    fn remove(&self, id: usize) -> bool {
        let mut guard = self.inner.lock().unwrap();
        if guard.done {
            return false;
        }
        if let Some((idx, _)) = guard
            .callbacks
            .iter()
            .enumerate()
            .find(|(_, (cid, _))| *cid == id)
        {
            let (_, cb) = guard.callbacks.swap_remove(idx);
            return cb.is_some();
        }
        false
    }

    fn wait(&self) {
        let mut guard = self.inner.lock().unwrap();
        while !guard.done {
            guard = self.cvar.wait(guard).unwrap();
        }
    }

    fn wait_timeout(&self, dur: Duration) -> bool {
        let mut guard = self.inner.lock().unwrap();
        let deadline = Instant::now() + dur;
        while !guard.done {
            let now = Instant::now();
            if now >= deadline {
                return guard.done;
            }
            let remaining = deadline.saturating_duration_since(now);
            let (g, timeout_res) = self.cvar.wait_timeout(guard, remaining).unwrap();
            guard = g;
            if timeout_res.timed_out() {
                return guard.done;
            }
        }
        true
    }
}

impl CancelState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            status: AtomicU8::new(0),
            cause: Mutex::new(None),
            notify: Arc::new(Notify::new()),
            waiters: Waiters::new(),
        })
    }

    pub fn done_handle(this: &Arc<Self>) -> DoneHandle {
        DoneHandle::Active(this.clone())
    }

    pub fn err(&self) -> Option<ContextError> {
        match self.status.load(Ordering::Acquire) {
            0 => None,
            1 => Some(ContextError::with_cause(
                Error::Canceled,
                self.cause.lock().unwrap().clone(),
            )),
            _ => Some(ContextError::with_cause(
                Error::DeadlineExceeded,
                self.cause.lock().unwrap().clone(),
            )),
        }
    }

    pub fn cause(&self) -> Option<Arc<dyn Error + Send + Sync>> {
        self.cause.lock().unwrap().clone()
    }

    pub fn is_done(&self) -> bool {
        self.status.load(Ordering::Acquire) != 0
    }

    pub fn cancel(
        self: &Arc<Self>,
        kind: CancelKind,
        cause: Option<Arc<dyn Error + Send + Sync>>,
    ) {
        let new = match kind {
            CancelKind::Canceled => 1,
            CancelKind::DeadlineExceeded => 2,
        };
        if self
            .status
            .compare_exchange(0, new, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let mut guard = self.cause.lock().unwrap();
        if guard.is_none() {
            *guard = cause.or_else(|| {
                let err = match kind {
                    CancelKind::Canceled => CANCELLED,
                    CancelKind::DeadlineExceeded => DEADLINE_EXCEEDED,
                };
                Some(Arc::new(err) as Arc<dyn Error + Send + Sync>)
            });
        }
        drop(guard);
        self.notify.notify_waiters();
        self.waiters.complete();
    }

    pub fn notify(&self) -> Arc<Notify> {
        self.notify.clone()
    }
}

impl Debug for CancelState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancelState")
            .field("status", &self.status.load(Ordering::Acquire))
            .finish()
    }
}

fn spawn_callback(callback: impl FnOnce() + Send + 'static) {
    if let Ok(handle) = Handle::try_current() {
        handle.spawn(async move {
            callback();
        });
    } else {
        thread::spawn(move || callback());
    }
}

/// `Done()` 的返回句柄。
#[derive(Clone, Debug)]
pub enum DoneHandle {
    Never,
    Active(Arc<CancelState>),
}

impl DoneHandle {
    pub const fn never() -> Self {
        Self::Never
    }

    pub fn is_done(&self) -> bool {
        match self {
            DoneHandle::Never => false,
            DoneHandle::Active(state) => state.is_done(),
        }
    }

    pub fn wait(&self) {
        if let DoneHandle::Active(state) = self {
            state.waiters.wait();
        }
    }

    pub fn wait_timeout(&self, dur: Duration) -> bool {
        match self {
            DoneHandle::Never => false,
            DoneHandle::Active(state) => state.waiters.wait_timeout(dur),
        }
    }

    pub fn register(&self, cb: impl FnOnce() + Send + 'static) -> StopFunc {
        match self {
            DoneHandle::Never => StopFunc::noop(),
            DoneHandle::Active(state) => state.waiters.register(state.clone(), Box::new(cb)),
        }
    }
}

/// 与 Go AfterFunc 返回的 StopFunc 对齐。
pub struct StopFunc {
    inner: Option<Box<dyn FnOnce() -> bool + Send + 'static>>,
}

impl StopFunc {
    fn new(state: Arc<CancelState>, id: usize) -> Self {
        Self {
            inner: Some(Box::new(move || state.waiters.remove(id))),
        }
    }

    pub fn noop() -> Self {
        Self {
            inner: Some(Box::new(|| false)),
        }
    }

    #[allow(non_snake_case)]
    pub fn Stop(mut self) -> bool {
        if let Some(f) = self.inner.take() {
            f()
        } else {
            false
        }
    }
}

impl Clone for StopFunc {
    fn clone(&self) -> Self {
        // StopFunc 在 Go 中允许多次调用，我们在这里提供一次性的克隆，返回的闭包在重复调用时返回 false。
        Self {
            inner: Some(Box::new(|| false)),
        }
    }
}
