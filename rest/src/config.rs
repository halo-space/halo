use core::service::ServiceConf;
use serde::{Deserialize, Serialize};

/// MiddlewaresConf with defaults set to true.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewaresConf {
    #[serde(default = "default_true")]
    pub trace: bool,
    #[serde(default = "default_true")]
    pub log: bool,
    #[serde(default = "default_true")]
    pub prometheus: bool,
    #[serde(default = "default_true")]
    pub max_connections: bool,
    #[serde(default = "default_true")]
    pub breaker: bool,
    #[serde(default = "default_true")]
    pub shedding: bool,
    #[serde(default = "default_true")]
    pub timeout: bool,
    #[serde(default = "default_true")]
    pub recover: bool,
    #[serde(default = "default_true")]
    pub metrics: bool,
    #[serde(default = "default_true")]
    pub max_bytes: bool,
    #[serde(default = "default_true")]
    pub gzip: bool,
}

fn default_true() -> bool {
    true
}

impl Default for MiddlewaresConf {
    fn default() -> Self {
        Self {
            trace: true,
            log: true,
            prometheus: true,
            max_connections: true,
            breaker: true,
            shedding: true,
            timeout: true,
            recover: true,
            metrics: true,
            max_bytes: true,
            gzip: true,
        }
    }
}

/// Private key configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrivateKeyConf {
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default)]
    pub key_file: String,
}

fn default_signature_strict() -> bool {
    false
}

/// Default expiry is 1h; stored as seconds (3600s) to avoid extra Duration deps.
fn default_signature_expiry_secs() -> u64 {
    3600
}

/// Signature configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureConf {
    #[serde(default = "default_signature_strict")]
    pub strict: bool,
    /// Expiry in seconds (default 1h).
    #[serde(default = "default_signature_expiry_secs")]
    pub expiry_secs: u64,
    #[serde(default)]
    pub private_keys: Vec<PrivateKeyConf>,
}

impl Default for SignatureConf {
    fn default() -> Self {
        Self {
            strict: false,
            expiry_secs: 3600,
            private_keys: Vec::new(),
        }
    }
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}
fn default_port() -> u16 {
    8888
}
fn default_max_connections() -> i64 {
    10_000
}
fn default_max_bytes() -> i64 {
    16 * 1_048_576 // 16 MiB default limit
}
fn default_timeout_ms() -> Option<u64> {
    Some(3_000)
}
fn default_cpu_threshold() -> i64 {
    900
}
fn default_reuse_port() -> bool {
    false
}
fn default_workers() -> Option<usize> {
    None
}
fn default_cpu_affinity() -> Option<Vec<usize>> {
    None
}
fn default_http2() -> bool {
    false
}
fn default_http2_h2c() -> bool {
    false
}
fn default_http1_keep_alive() -> bool {
    true
}
fn default_http1_max_buf_size() -> Option<usize> {
    None
}
fn default_tcp_keepalive_secs() -> Option<u64> {
    None
}
fn default_rate_limit() -> Option<RateLimitConf> {
    None
}
fn default_concurrency_limit() -> Option<usize> {
    None
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConf {
    /// tokens per second
    pub permits_per_second: u64,
    /// burst size
    pub burst: u64,
}

/// RestConf definition (rest package).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestConf {
    /// Equivalent to embedding `service.ServiceConf`
    #[serde(flatten, default)]
    pub service: ServiceConf,

    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cert_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_file: Option<String>,

    #[serde(default = "default_max_connections")]
    pub max_connections: i64,
    #[serde(default = "default_max_bytes")]
    pub max_bytes: i64,

    /// Per-request timeout in milliseconds. None disables it. Default 3000ms.
    #[serde(
        default = "default_timeout_ms",
        alias = "RequestTimeoutMs",
        skip_serializing_if = "Option::is_none"
    )]
    pub timeout: Option<u64>,

    /// range: [0, 1000)
    #[serde(default = "default_cpu_threshold")]
    pub cpu_threshold: i64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<SignatureConf>,

    /// There are default values for all the items in Middlewares.
    #[serde(default)]
    pub middlewares: MiddlewaresConf,

    /// TraceIgnorePaths is paths blacklist for trace middleware.
    #[serde(default)]
    pub trace_ignore_paths: Vec<String>,

    /// Tokio worker threads (None => tokio default).
    #[serde(default = "default_workers", skip_serializing_if = "Option::is_none")]
    pub workers: Option<usize>,

    /// Enable SO_REUSEPORT multi-listener (Linux recommended for multi-core).
    #[serde(default = "default_reuse_port")]
    pub reuse_port: bool,

    /// CPU affinity (Linux only). Ignored on unsupported platforms.
    #[serde(
        default = "default_cpu_affinity",
        skip_serializing_if = "Option::is_none"
    )]
    pub cpu_affinity: Option<Vec<usize>>,

    /// Enable HTTP/2 (TLS or h2c). Default off (HTTP/1.1).
    #[serde(default = "default_http2")]
    pub http2: bool,
    /// Enable h2c (HTTP/2 without TLS). Default off.
    #[serde(default = "default_http2_h2c")]
    pub h2c: bool,
    /// http1 keep-alive toggle.
    #[serde(default = "default_http1_keep_alive")]
    pub http1_keep_alive: bool,
    /// http1 max buffer size.
    #[serde(
        default = "default_http1_max_buf_size",
        skip_serializing_if = "Option::is_none"
    )]
    pub http1_max_buf_size: Option<usize>,
    /// TCP keepalive seconds (None => OS default).
    #[serde(
        default = "default_tcp_keepalive_secs",
        skip_serializing_if = "Option::is_none"
    )]
    pub tcp_keepalive_secs: Option<u64>,

    /// Global rate limit config (tokens/s, burst). None => disabled.
    #[serde(
        default = "default_rate_limit",
        skip_serializing_if = "Option::is_none"
    )]
    pub rate_limit: Option<RateLimitConf>,

    /// Global concurrent in-flight request limit. None => disabled.
    #[serde(
        default = "default_concurrency_limit",
        skip_serializing_if = "Option::is_none"
    )]
    pub concurrency_limit: Option<usize>,
}

impl Default for RestConf {
    fn default() -> Self {
        Self::new()
    }
}

impl RestConf {
    pub fn new() -> Self {
        let conf = Self {
            service: ServiceConf::default(),
            host: default_host(),
            port: default_port(),
            cert_file: None,
            key_file: None,
            max_connections: default_max_connections(),
            max_bytes: default_max_bytes(),
            timeout: default_timeout_ms(),
            cpu_threshold: default_cpu_threshold(),
            signature: None,
            middlewares: MiddlewaresConf::default(),
            trace_ignore_paths: Vec::new(),
            workers: default_workers(),
            reuse_port: default_reuse_port(),
            cpu_affinity: default_cpu_affinity(),
            http2: default_http2(),
            h2c: default_http2_h2c(),
            http1_keep_alive: default_http1_keep_alive(),
            http1_max_buf_size: default_http1_max_buf_size(),
            tcp_keepalive_secs: default_tcp_keepalive_secs(),
            rate_limit: default_rate_limit(),
            concurrency_limit: default_concurrency_limit(),
        };
        conf.validate().expect("RestConf::new validation failed");
        conf
    }
}

impl RestConf {
    pub fn addr_string(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(0..1000).contains(&self.cpu_threshold) {
            return Err(format!(
                "CpuThreshold out of range [0,1000): {}",
                self.cpu_threshold
            ));
        }
        if self.port == 0 {
            return Err("Port must be > 0".to_string());
        }
        if let Some(w) = self.workers {
            if w == 0 {
                return Err("Workers must be >= 1".to_string());
            }
        }
        if let Some(aff) = &self.cpu_affinity {
            if aff.is_empty() {
                return Err("CpuAffinity cannot be empty when set".to_string());
            }
        }
        if self.h2c && !self.http2 {
            return Err("h2c requires http2=true".to_string());
        }
        if let Some(rl) = &self.rate_limit {
            if rl.permits_per_second == 0 || rl.burst == 0 {
                return Err("RateLimit permits_per_second and burst must be > 0".to_string());
            }
        }
        if let Some(c) = self.concurrency_limit {
            if c == 0 {
                return Err("ConcurrencyLimit must be > 0".to_string());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::service::Mode;

    #[test]
    fn defaults_should_work() {
        let c = RestConf::default();
        assert_eq!(c.host, "0.0.0.0");
        assert!(c.port > 0);
        assert_eq!(c.max_connections, 10_000);
        assert_eq!(c.max_bytes, 16 * 1_048_576);
        assert_eq!(c.timeout, Some(3_000));
        assert_eq!(c.cpu_threshold, 900);
        assert_eq!(c.service.mode, Mode::Pro);
        assert!(c.middlewares.trace);
        assert!(c.middlewares.gzip);
    }
}
