//! metrics：统计/指标采集器（事件采集、滚动/固定窗口、计数器等）。
//!
//! 当前包含三类采集策略：
//! - `rolling_window`：滚动窗口（按时间片分桶，支持 Reduce 聚合，可配置忽略当前桶避免“半桶数据”）
//! - `fixed_window`：固定窗口（本质是 rolling window 的特化，默认避免半桶）
//! - `counter`：纯计数（不随时间衰减）

pub mod counter;
pub mod fixed_window;
pub mod rolling_window;

pub use counter::Counter;
pub use fixed_window::{FixedWindow, FixedWindowConfigError};
pub use rolling_window::{
    Bucket, RollingWindow, RollingWindowConfigError, RollingWindowOption, ignore_current_bucket,
};
