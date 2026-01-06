## Go 风格 Context（halo-core）

本模块等价 Go `context`：取消传递、截止/超时、值链、AfterFunc 回调。

### 特性
- 背景根：`Background()/TODO()`
- 取消：`WithCancel/WithCancelCause`
- 截止/超时：`WithDeadline/WithDeadlineCause/WithTimeout/WithTimeoutCause`
- 值传递：`WithValue`（键需实现 `ValueKey`，推荐用新类型包装）
- 回调：`AfterFunc` 返回 `StopFunc::Stop()`
- 异步等待：`done_async()/Done(ctx)/ContextAware(ctx, fut)`
- 脱离取消：`WithoutCancel`

### 快速上手
```rust
use core::context::{
    Background, WithTimeout, WithValue, AfterFunc, ContextAware, ContextError,
};
use std::time::Duration;

async fn work() -> Result<(), ContextError> {
    let base = WithValue(Background(), "req_id", "123");
    let (ctx, cancel) = WithTimeout(base, Duration::from_secs(3));
    let _cb = AfterFunc(&ctx, || println!("canceled!"));

    let job = async {
        // 业务逻辑（可用 ctx.value 获取值），无需显式检查取消
        Ok(())
    };

    let res = ContextAware(ctx.clone(), job).await;
    cancel(); // 始终安全
    res
}
```

### 同步场景
```rust
use core::context::{Background, WithCancel, AfterFunc};

let (ctx, cancel) = WithCancel(Background());
let cb = AfterFunc(&ctx, || println!("canceled"));
// 线程内可轮询 ctx.err() 感知取消
cancel();
cb.stop();
```

### 关键 API
- 根：`Background()`, `TODO()`
- 取消：`WithCancel`, `WithCancelCause`
- 截止/超时：`WithDeadline`, `WithDeadlineCause`, `WithTimeout`, `WithTimeoutCause`
- 值：`WithValue`
- 脱钩：`WithoutCancel`
- 完成等待：
  - 同步：`done() -> DoneHandle`, `wait()`, `register(cb)`
  - 异步：`done_async() -> DoneFuture`, `Done(ctx)`, `ContextAware(ctx, fut)`
- 回调：`AfterFunc` / `StopFunc::Stop()`

### 分特性示例
取消与 cause
```rust
use core::context::{Background, WithCancel, WithCancelCause};
use std::sync::Arc;

fn cancel_demo() {
    let (ctx, cancel) = WithCancel(Background());
    let (_child, cancel_child) = WithCancelCause(ctx.clone());
    cancel(); // context canceled
    cancel_child(Some(Arc::new(anyhow::anyhow!("why"))));
}
```

截止 / 超时
```rust
use core::context::{Background, WithDeadline};
use std::time::{Duration, Instant};

async fn deadline_demo() {
    let (ctx, _cancel) =
        WithDeadline(Background(), Instant::now() + Duration::from_millis(50));
    tokio::select! { _ = ctx.done_async() => { assert!(ctx.err().is_some()); } }
}
```

值传递
```rust
use core::context::{Background, WithValue};

fn value_demo() {
    let ctx = WithValue(Background(), "user_id", 42u64);
    let val = ctx.value(&"user_id").unwrap().downcast::<u64>().unwrap();
    assert_eq!(*val, 42);
}
```

AfterFunc + Stop
```rust
use core::context::{AfterFunc, Background, WithCancel};

fn after_func_demo() {
    let (ctx, cancel) = WithCancel(Background());
    let guard = AfterFunc(&ctx, || println!("canceled"));
    cancel();
    guard.stop(); // 如需在触发前移除
}
```

异步等待 / ContextAware
```rust
async fn work(ctx: Context) -> Result<(), ContextError> {
    tokio::time::sleep(Duration::from_secs(5)).await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), ContextError> {
    let (ctx, cancel) = WithTimeout(Background(), Duration::from_secs(1));
    let res = ContextAware(ctx.clone(), work(ctx)).await; // 返回超时错误
    cancel();
    res
}
```

脱离取消
```rust
use core::context::{Background, WithCancel, WithoutCancel};

fn without_cancel_demo() {
    let (ctx, cancel) = WithCancel(Background());
    let detached = WithoutCancel(ctx.clone());
    cancel(); // detached 不受影响
    assert!(detached.err().is_none());
}
```

### 仓库内示例
- `examples/context_timeout.rs`：线程版，轮询感知取消。
- `examples/context_timeout_thread_nosense.rs`：线程版，无感知，外部 watcher 抢占。
- `examples/context_timeout_async_select.rs`：异步版，`tokio::select!` 感知取消。
- `examples/context_timeout_async_nosense.rs`：异步版，无感知，用 `ContextAware` 抢占。

