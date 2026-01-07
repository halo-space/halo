use crate::context::error::{CANCELLED, ContextError, DEADLINE_EXCEEDED, Error};
use std::fmt::{Debug, Formatter};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Active,
    Canceled,
    DeadlineExceeded,
}

/// CancelState 内部数据。
/// 不变量与锁序：
/// - 锁顺序：始终先锁父再锁子，避免死锁。
/// - parent_idx：在父 children 中的位置，detach 时用 O(1) 位置移除并更新被交换节点。
/// - handle_count：Context 克隆/Drop 维护；handle_count 为 0 且 children 为空且 done 时向上裁剪。
/// - callbacks_head：侵入式回调链表头，只能在持有 self.inner 锁时读写。
struct Inner {
    status: Status,
    cause: Option<Arc<dyn std::error::Error + Send + Sync>>,
    parent: Option<Weak<CancelState>>,
    children: Vec<Weak<CancelState>>,
    parent_idx: Option<usize>,
    done: bool,
    handle_count: AtomicUsize,
    next_id: usize,
    callbacks_head: *mut CallbackNode,
}

unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

/// 侵入式回调节点，仿 tokio-util 链表。
pub(crate) struct CallbackNode {
    #[allow(dead_code)]
    id: usize,
    callback: Option<Box<dyn FnOnce() + Send + 'static>>,
    next: *mut CallbackNode,
}

unsafe impl Send for CallbackNode {}
unsafe impl Sync for CallbackNode {}

/// 取消状态核心（树形结构，紧贴 tokio-util 设计）。
pub struct CancelState {
    inner: Mutex<Inner>,
    cvar: Condvar,
    notify: Arc<Notify>,
}

impl CancelState {
    pub fn new_root() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner {
                status: Status::Active,
                cause: None,
                parent: None,
                children: Vec::new(),
                parent_idx: None,
                done: false,
                handle_count: AtomicUsize::new(1),
                next_id: 0,
                callbacks_head: std::ptr::null_mut(),
            }),
            cvar: Condvar::new(),
            notify: Arc::new(Notify::new()),
        })
    }

    pub fn child_of(parent: &Arc<Self>) -> Arc<Self> {
        let child = Arc::new(Self {
            inner: Mutex::new(Inner {
                status: Status::Active,
                cause: None,
                parent: Some(Arc::downgrade(parent)),
                children: Vec::new(),
                parent_idx: None,
                done: false,
                handle_count: AtomicUsize::new(1),
                next_id: 0,
                callbacks_head: std::ptr::null_mut(),
            }),
            cvar: Condvar::new(),
            notify: Arc::new(Notify::new()),
        });

        // 挂载到父节点（锁父后推入）
        let weak_child = Arc::downgrade(&child);
        let mut guard = parent
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let idx = guard.children.len();
        guard.children.push(weak_child);
        drop(guard);
        child
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .parent_idx = Some(idx);

        child
    }

    pub fn done_handle(this: &Arc<Self>) -> DoneHandle {
        DoneHandle::Active(this.clone())
    }

    pub fn err(&self) -> Option<ContextError> {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match guard.status {
            Status::Active => None,
            Status::Canceled => Some(ContextError::with_cause(
                Error::Canceled,
                guard.cause.clone(),
            )),
            Status::DeadlineExceeded => Some(ContextError::with_cause(
                Error::DeadlineExceeded,
                guard.cause.clone(),
            )),
        }
    }

    pub fn cause(&self) -> Option<Arc<dyn std::error::Error + Send + Sync>> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).cause.clone()
    }

    pub fn is_done(&self) -> bool {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).done
    }

    pub fn add_handle(&self) {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.handle_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn release_handle(self: &Arc<Self>) {
        let mut current = self.clone();
        loop {
            let parent_opt = {
                let mut guard = current.inner.lock().unwrap_or_else(|e| e.into_inner());
                guard.handle_count.fetch_sub(1, Ordering::Relaxed);
                guard.children.retain(|w| w.upgrade().is_some());
                let has_handles = guard.handle_count.load(Ordering::Relaxed) > 0;
                let has_children = !guard.children.is_empty();
                let done = guard.done;
                let parent = guard.parent.clone();
                let parent_idx = guard.parent_idx;
                drop(guard);
                if has_handles || has_children || !done {
                    return;
                }
                parent.zip(parent_idx)
            };

            let Some((parent_weak, idx)) = parent_opt else {
                return;
            };
            let Some(parent) = parent_weak.upgrade() else {
                return;
            };

            if !Self::detach_from_parent_idx(&parent, &current, idx) {
                return;
            }
            current = parent;
        }
    }

    pub fn cancel(
        self: &Arc<Self>,
        kind: Error,
        cause: Option<Arc<dyn std::error::Error + Send + Sync>>,
    ) {
        // 尝试标记自身
        let cause_for_self = cause.clone();
        let (callbacks, children, notify_needed) = {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if guard.done {
                return;
            }

            guard.status = match kind {
                Error::Canceled => Status::Canceled,
                Error::DeadlineExceeded => Status::DeadlineExceeded,
                Error::Any => Status::Canceled,
            };
            if guard.cause.is_none() {
                guard.cause = cause_for_self.clone().or_else(|| {
                    let err = match kind {
                        Error::Canceled => CANCELLED,
                        Error::DeadlineExceeded => DEADLINE_EXCEEDED,
                        Error::Any => CANCELLED,
                    };
                    Some(Arc::new(err) as Arc<dyn std::error::Error + Send + Sync>)
                });
            }
            guard.done = true;

            let head = guard.callbacks_head;
            guard.callbacks_head = std::ptr::null_mut();
            let callbacks = if head.is_null() {
                Vec::new()
            } else {
                Self::drain_callbacks(head)
            };

            let children = guard
                .children
                .iter()
                .filter_map(|w| w.upgrade())
                .collect::<Vec<_>>();
            (callbacks, children, true)
        };

        if notify_needed {
            self.notify.notify_waiters();
            self.cvar.notify_all();
        }

        // 直接执行回调，避免额外 spawn
        for cb in callbacks {
            cb();
        }

        // 递归取消子节点
        for child in children {
            child.cancel(kind, cause.clone());
        }

        // 若已完成且无子节点，向上裁剪
        self.prune_if_detached();
    }

    fn prune_if_detached(self: &Arc<Self>) {
        let mut current = self.clone();
        loop {
            let parent_info = {
                let mut guard = current.inner.lock().unwrap_or_else(|e| e.into_inner());
                if !guard.done {
                    return;
                }
                guard.children.retain(|w| w.upgrade().is_some());
                if !guard.children.is_empty() {
                    return;
                }
                guard.parent.clone().zip(guard.parent_idx)
            };

            let Some((parent_weak, idx)) = parent_info else {
                return;
            };
            let Some(parent) = parent_weak.upgrade() else {
                return;
            };

            if !Self::detach_from_parent_idx(&parent, &current, idx) {
                return;
            }

            current = parent;
        }
    }

    pub fn notify(&self) -> Arc<Notify> {
        self.notify.clone()
    }

    fn drain_callbacks(mut head: *mut CallbackNode) -> Vec<Box<dyn FnOnce() + Send + 'static>> {
        let mut out = Vec::new();
        while !head.is_null() {
            let node = unsafe { Box::from_raw(head) };
            if let Some(callback) = node.callback {
                out.push(callback);
            }
            head = node.next;
        }
        out
    }

    pub fn register(
        &self,
        owner: Arc<CancelState>,
        cb: Box<dyn FnOnce() + Send + 'static>,
    ) -> StopFunc {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if guard.done {
            drop(guard);
            cb();
            return StopFunc::noop();
        }
        let id = guard.next_id;
        guard.next_id += 1;
        let node = Box::new(CallbackNode {
            id,
            callback: Some(cb),
            next: guard.callbacks_head,
        });
        let ptr = Box::into_raw(node);
        guard.callbacks_head = ptr;
        StopFunc::new(owner, ptr)
    }

    pub(crate) fn remove(&self, ptr: *mut CallbackNode) -> bool {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if guard.done {
            return false;
        }
        let mut current = guard.callbacks_head;
        let mut prev: *mut CallbackNode = std::ptr::null_mut();
        while !current.is_null() {
            if current == ptr {
                let next = unsafe { (*current).next };
                if prev.is_null() {
                    guard.callbacks_head = next;
                } else {
                    unsafe { (*prev).next = next };
                }
                let mut boxed = unsafe { Box::from_raw(current) };
                let existed = boxed.callback.take().is_some();
                drop(boxed);
                return existed;
            }
            prev = current;
            current = unsafe { (*current).next };
        }
        false
    }

    fn detach_from_parent_idx(
        parent: &Arc<CancelState>,
        child: &Arc<CancelState>,
        idx: usize,
    ) -> bool {
        let mut p_guard = parent.inner.lock().unwrap_or_else(|e| e.into_inner());
        let len_before = p_guard.children.len();
        if idx >= len_before {
            return false;
        }
        let Some(last) = p_guard.children.pop() else {
            return false;
        };
        let len_after = p_guard.children.len();
        if idx < len_after {
            p_guard.children[idx] = last;
            if let Some(last_child) = p_guard.children[idx].upgrade() {
                last_child.inner.lock().unwrap_or_else(|e| e.into_inner()).parent_idx = Some(idx);
            }
        }
        drop(p_guard);
        child
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .parent_idx = None;
        true
    }

    pub fn wait(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        while !guard.done {
            guard = self.cvar.wait(guard).unwrap_or_else(|e| e.into_inner());
        }
    }

    pub fn wait_timeout(&self, dur: Duration) -> bool {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let deadline = Instant::now() + dur;
        while !guard.done {
            let now = Instant::now();
            if now >= deadline {
                return guard.done;
            }
            let remaining = deadline.saturating_duration_since(now);
            let (g, timeout_res) = self
                .cvar
                .wait_timeout(guard, remaining)
                .unwrap_or_else(|e| e.into_inner());
            guard = g;
            if timeout_res.timed_out() {
                return guard.done;
            }
        }
        true
    }
}

impl Debug for CancelState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let status = self.inner.lock().unwrap_or_else(|e| e.into_inner()).status;
        f.debug_struct("CancelState")
            .field("status", &status)
            .finish()
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
            state.wait();
        }
    }

    pub fn wait_timeout(&self, dur: Duration) -> bool {
        match self {
            DoneHandle::Never => false,
            DoneHandle::Active(state) => state.wait_timeout(dur),
        }
    }

    pub fn register(&self, cb: impl FnOnce() + Send + 'static) -> StopFunc {
        match self {
            DoneHandle::Never => StopFunc::noop(),
            DoneHandle::Active(state) => state.register(state.clone(), Box::new(cb)),
        }
    }
}

/// 与 Go AfterFunc 返回的 StopFunc 对齐。
pub struct StopFunc {
    inner: Option<Box<dyn FnOnce() -> bool + Send + 'static>>,
}

impl StopFunc {
    fn new(state: Arc<CancelState>, ptr: *mut CallbackNode) -> Self {
        let ptr_usize = ptr as usize;
        Self {
            inner: Some(Box::new(move || {
                state.remove(ptr_usize as *mut CallbackNode)
            })),
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
