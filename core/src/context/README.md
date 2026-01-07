## Go-style Context for Rust (halo-core)

Chinese guide: [README.zh.md](README.zh.md).

This module mirrors Go’s `context` package: cancellation propagation, deadlines/timeouts, values, and AfterFunc callbacks.

### Features
- Background/TODO roots.
- Cancel/CancelCause.
- Deadline/Timeout/TimeoutCause.
- Value chaining with typed keys.
- AfterFunc + Stop (Go-compatible).
- Async wait: `done_async`, `Done`, `ContextAware`.
- WithoutCancel to detach cancellation.

### Quickstart
```rust
use halo_micro::core::context::{
    Background, WithTimeout, WithValue, AfterFunc, ContextAware, ContextError,
};
use std::time::Duration;

async fn work() -> Result<(), ContextError> {
    let ctx = WithValue(Background(), "req_id", "123");
    let (ctx, cancel) = WithTimeout(ctx, Duration::from_secs(3));

    let _cb = AfterFunc(&ctx, || println!("canceled!"));

    // Business future, no cancel checks inside
    let job = async {
        // ... do work, can read ctx.value if needed
        Ok(())
    };

    let res = ContextAware(ctx.clone(), job).await;
    cancel(); // always safe
    res
}
```

### Blocking usage
```rust
use halo_micro::core::context::{Background, WithCancel, AfterFunc};

let (ctx, cancel) = WithCancel(Background());
let cb = AfterFunc(&ctx, || println!("canceled"));
std::thread::spawn(move || {
    // ... do work; poll ctx.err() if you need synchronous awareness
});
cancel();
cb.stop();
```

### API reference (high level)
- Roots: `Background()`, `TODO()`
- Cancellation: `WithCancel`, `WithCancelCause`
- Deadline/Timeout: `WithDeadline`, `WithDeadlineCause`, `WithTimeout`, `WithTimeoutCause`
- Values: `WithValue`
- Detach: `WithoutCancel`
- Completion:
  - Sync: `done() -> DoneHandle`, `DoneHandle::wait`, `DoneHandle::register` (AfterFunc uses this)
  - Async: `done_async() -> DoneFuture`, `Done(ctx)`, `ContextAware(ctx, fut)`
- Callbacks: `AfterFunc` returns `StopFunc` (call `Stop()` to remove)

### Feature-by-feature snippets
Cancellation & cause
```rust
use halo_micro::core::context::{Background, WithCancel, WithCancelCause};
use std::sync::Arc;

fn cancel_demo() {
    let (ctx, cancel) = WithCancel(Background());
    let (_child, cancel_child) = WithCancelCause(ctx.clone());

    cancel(); // parent canceled
    cancel_child(Some(Arc::new(anyhow::anyhow!("reason"))));
}
```

Deadline / Timeout
```rust
use halo_micro::core::context::{Background, WithDeadline};
use std::time::{Duration, Instant};

async fn deadline_demo() {
    let (ctx, _cancel) =
        WithDeadline(Background(), Instant::now() + Duration::from_millis(50));
    tokio::select! {
        _ = ctx.done_async() => { assert!(ctx.err().is_some()); }
    }
}
```

Values
```rust
use halo_micro::core::context::{Background, WithValue};

fn value_demo() {
    let ctx = WithValue(Background(), "user_id", 42u64);
    let val = ctx.value(&"user_id").unwrap().downcast::<u64>().unwrap();
    assert_eq!(*val, 42);
}
```

AfterFunc + Stop
```rust
use halo_micro::core::context::{AfterFunc, Background, WithCancel};

fn after_func_demo() {
    let (ctx, cancel) = WithCancel(Background());
    let guard = AfterFunc(&ctx, || println!("canceled"));
    cancel();
    guard.stop(); // remove if needed before firing
}
```

Async wait / ContextAware
```rust
async fn work(ctx: Context) -> Result<(), ContextError> {
    // simulate long job
    tokio::time::sleep(Duration::from_secs(5)).await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), ContextError> {
    let (ctx, cancel) = WithTimeout(Background(), Duration::from_secs(1));
    let res = ContextAware(ctx.clone(), work(ctx)).await; // returns timeout error
    cancel();
    res
}
```

WithoutCancel
```rust
use halo_micro::core::context::{Background, WithCancel, WithoutCancel};

fn without_cancel_demo() {
    let (ctx, cancel) = WithCancel(Background());
    let detached = WithoutCancel(ctx.clone());
    cancel(); // detached stays active
    assert!(detached.err().is_none());
}
```

### Examples in repo
- `examples/context_timeout.rs` — threaded, cancel-aware (polling).
- `examples/context_timeout_thread_nosense.rs` — threaded, cancel-unaware (external watcher).
- `examples/context_timeout_async_select.rs` — async with `tokio::select!`.
- `examples/context_timeout_async_nosense.rs` — async, cancel-unaware with `ContextAware`.


