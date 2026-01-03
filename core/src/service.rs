//! 基础服务配置（与 go-zero 的 `service.ServiceConf` 对齐的占位实现）。

use serde::{Deserialize, Serialize};

/// 与 go-zero `ServiceConf.Mode` 对齐：`dev|test|rt|pre|pro`，默认 `pro`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Dev,
    Test,
    Rt,
    Pre,
    #[default]
    Pro,
}

/// 轻量日志配置：先对齐结构入口，字段后续可逐步细化。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogConf {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

/// Deprecated: please use DevServer（与 go-zero 注释一致）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrometheusConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// 链路追踪/Telemetry 配置（占位对齐）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampler: Option<String>,
}

/// 开发服务配置（占位对齐）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DevServerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

/// 优雅关停配置（占位对齐）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShutdownConf {
    /// 是否开启优雅关停。
    #[serde(default)]
    pub enabled: bool,
    /// 最大等待毫秒数（占位字段）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<i64>,
}

/// 与 go-zero `service.ServiceConf` 对齐的基础服务配置（应位于 core/service）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceConf {
    /// 服务名称（Go 版无默认；这里给 default 以便最小配置也能跑起来）。
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub log: LogConf,
    /// 运行模式：dev|test|rt|pre|pro（大小写均可，默认 pro）。
    #[serde(default)]
    pub mode: Mode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics_url: Option<String>,
    /// Deprecated: please use DevServer
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prometheus: Option<PrometheusConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry: Option<TraceConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dev_server: Option<DevServerConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shutdown: Option<ShutdownConf>,
}
