//! `infra`：halo 的基础能力（配置加载、ServiceConf 等）。

pub mod conf;
pub mod context;
pub mod service;
pub mod storage;
pub mod sync;

// 为示例与外部引用提供稳定别名。
extern crate self as halo_infra;
