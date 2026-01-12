//! 熔断/过载保护相关抽象与实现。
//! - `breaker`：核心抽象（RAII `Promise` + `Reject/RetryHint`）
//! - 采集层：通用采集器在 `crate::collection::metrics`（RollingWindow/FixedWindow/Counter）

pub mod breaker;
pub mod google_sre;

pub use breaker::*;
pub use google_sre::Snapshot;
pub use google_sre::{GoogleSreJudge, GoogleSreParams};
