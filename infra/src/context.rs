//! 对齐 Go `context` 包的一站式实现：支持取消、截止时间和值传递。
//! 设计目标：
//! - API 与 go context 对齐：Background/TODO/WithCancel/WithDeadline/WithTimeout/WithValue 等。
//! - 采用零依赖、以原子/Condvar 为主的实现，追求最小运行时开销。
//! - 线程安全（Send + Sync），并发取消和回调调用安全。

pub mod error;
mod impls;
mod state;
mod value;

pub use error::{CANCELLED, ContextError, DEADLINE_EXCEEDED, Error};
pub use impls::{
    AfterFunc, Background, CancelCauseFunc, CancelFunc, Cause, Context, ContextAware, Done,
    DoneFuture, TODO, WithCancel, WithCancelCause, WithDeadline, WithDeadlineCause, WithTimeout,
    WithTimeoutCause, WithValue, WithoutCancel,
};
pub use state::{DoneHandle, StopFunc};
pub use value::ValueKey;
