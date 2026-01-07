//! Redis 配置，参考 go-zero `core/stores/redis/conf.go`。
//! 仅定义配置与校验逻辑，实际客户端连接可按需扩展。

use std::time::Duration;
use thiserror::Error;

/// Redis 部署类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedisType {
    Node,
    Cluster,
}

impl Default for RedisType {
    fn default() -> Self {
        RedisType::Node
    }
}

/// Redis 基础配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedisConfig {
    pub host: String,
    pub kind: RedisType,
    pub user: Option<String>,
    pub pass: Option<String>,
    pub tls: bool,
    /// 是否启用 RESP3 协议（默认 RESP2）。当前 redis-rs 1.x 未公开通用配置入口，启用将返回错误。
    pub resp3: bool,
    /// go-zero 默认 true。
    pub non_block: bool,
    /// go-zero 默认 1s。
    pub ping_timeout: Duration,
    /// 连接超时，用于 pool 和 sync connect。
    pub connect_timeout: Option<Duration>,
    /// 命令超时占位（未使用），后续可用于自定义调用。
    pub command_timeout: Option<Duration>,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            kind: RedisType::Node,
            user: None,
            pass: None,
            tls: false,
            resp3: false,
            non_block: true,
            ping_timeout: Duration::from_secs(1),
            connect_timeout: None,
            command_timeout: None,
        }
    }
}

pub enum ClientKind {
    Single(redis::Client),
    Cluster(redis::cluster::ClusterClient),
}

impl RedisConfig {
    /// 校验基础配置，确保 host 非空。
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.host.is_empty() {
            return Err(ConfigError::EmptyHost);
        }
        Ok(())
    }

    /// 构造 redis 客户端（不建立连接）。
    pub fn new_client(&self) -> Result<ClientKind, ConfigError> {
        self.validate()?;
        match self.kind {
            RedisType::Node => {
                if self.resp3 {
                    return Err(ConfigError::UnsupportedResp3);
                }
                let url = build_redis_url(
                    &self.host,
                    self.user.as_deref(),
                    self.pass.as_deref(),
                    self.tls,
                );
                let client = redis::Client::open(url).map_err(ConfigError::from)?;
                Ok(ClientKind::Single(client))
            }
            RedisType::Cluster => {
                if self.resp3 {
                    return Err(ConfigError::UnsupportedResp3);
                }
                let nodes = build_cluster_urls(
                    &self.host,
                    self.user.as_deref(),
                    self.pass.as_deref(),
                    self.tls,
                )?;
                let client =
                    redis::cluster::ClusterClient::new(nodes).map_err(ConfigError::from)?;
                Ok(ClientKind::Cluster(client))
            }
        }
    }

    /// 获取异步多路复用连接（单节点）。redis 官方文档指出 async 场景通常无需连接池。
    pub async fn multiplexed_connection(
        &self,
    ) -> Result<redis::aio::MultiplexedConnection, ConfigError> {
        match self.new_client()? {
            ClientKind::Single(client) => client
                .get_multiplexed_async_connection()
                .await
                .map_err(ConfigError::from),
            ClientKind::Cluster(_) => Err(ConfigError::UnsupportedClusterConnection),
        }
    }

    /// 同步获取连接（单节点）；Cluster 返回 UnsupportedClusterConnection。
    pub fn get_connection(&self) -> Result<redis::Connection, ConfigError> {
        match self.new_client()? {
            ClientKind::Single(client) => {
                if let Some(timeout) = self.connect_timeout {
                    client
                        .get_connection_with_timeout(timeout)
                        .map_err(ConfigError::from)
                } else {
                    client.get_connection().map_err(ConfigError::from)
                }
            }
            ClientKind::Cluster(_) => Err(ConfigError::UnsupportedClusterConnection),
        }
    }

    /// 构建异步 ConnectionManager（单节点）。
    pub async fn connection_manager(&self) -> Result<redis::aio::ConnectionManager, ConfigError> {
        match self.new_client()? {
            ClientKind::Single(client) => {
                let manager = redis::aio::ConnectionManager::new(client)
                    .await
                    .map_err(ConfigError::from)?;
                Ok(manager)
            }
            ClientKind::Cluster(_) => Err(ConfigError::UnsupportedClusterConnection),
        }
    }

    /// 构建同步 r2d2 Pool（单节点）。
    pub fn pool(&self, max_size: Option<u32>) -> Result<r2d2::Pool<redis::Client>, ConfigError> {
        match self.new_client()? {
            ClientKind::Single(client) => {
                let mut builder = r2d2::Pool::builder();
                if let Some(sz) = max_size {
                    builder = builder.max_size(sz);
                }
                if let Some(timeout) = self.connect_timeout {
                    builder = builder.connection_timeout(timeout);
                }
                builder.build(client).map_err(ConfigError::from)
            }
            ClientKind::Cluster(_) => Err(ConfigError::UnsupportedClusterPool),
        }
    }
}

/// 携带 key 的 Redis 配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedisKeyConfig {
    pub config: RedisConfig,
    pub key: String,
}

impl RedisKeyConfig {
    pub fn new(config: RedisConfig, key: impl Into<String>) -> Self {
        Self {
            config,
            key: key.into(),
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        self.config.validate()?;
        if self.key.is_empty() {
            return Err(ConfigError::EmptyKey);
        }
        Ok(())
    }
}

/// 配置校验错误。
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("empty redis host")]
    EmptyHost,
    #[error("empty redis key")]
    EmptyKey,
    #[error("redis client error: {0}")]
    Client(#[from] redis::RedisError),
    #[error("resp3 is not configurable with current redis-rs version")]
    UnsupportedResp3,
    #[error("cluster sync connection/manager not supported yet")]
    UnsupportedClusterConnection,
    #[error("cluster r2d2 pool not supported yet")]
    UnsupportedClusterPool,
    #[error("r2d2 pool error: {0}")]
    Pool(#[from] r2d2::Error),
}

impl PartialEq for ConfigError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ConfigError::EmptyHost, ConfigError::EmptyHost)
            | (ConfigError::EmptyKey, ConfigError::EmptyKey)
            | (
                ConfigError::UnsupportedClusterConnection,
                ConfigError::UnsupportedClusterConnection,
            )
            | (ConfigError::UnsupportedClusterPool, ConfigError::UnsupportedClusterPool) => true,
            (ConfigError::Client(a), ConfigError::Client(b)) => a.to_string() == b.to_string(),
            (ConfigError::Pool(a), ConfigError::Pool(b)) => a.to_string() == b.to_string(),
            _ => false,
        }
    }
}

fn build_redis_url(host: &str, user: Option<&str>, pass: Option<&str>, tls: bool) -> String {
    let mut url = host.trim().to_string();
    let scheme = if tls { "rediss://" } else { "redis://" };
    if !url.starts_with("redis://") && !url.starts_with("rediss://") {
        url = format!("{scheme}{url}");
    }
    if user.is_none() && pass.is_none() {
        return url;
    }
    // inject credentials
    let credentials = match (user, pass) {
        (Some(u), Some(p)) => format!("{u}:{p}@"),
        (Some(u), None) => format!("{u}@"),
        (None, Some(p)) => format!(":{p}@"),
        (None, None) => String::new(),
    };
    if let Some(rest) = url.strip_prefix("redis://") {
        format!("redis://{credentials}{rest}")
    } else if let Some(rest) = url.strip_prefix("rediss://") {
        format!("rediss://{credentials}{rest}")
    } else {
        url
    }
}

fn build_cluster_urls(
    hosts: &str,
    user: Option<&str>,
    pass: Option<&str>,
    tls: bool,
) -> Result<Vec<String>, ConfigError> {
    let mut urls = Vec::new();
    for host in hosts.split(',') {
        let trimmed = host.trim();
        if trimmed.is_empty() {
            continue;
        }
        urls.push(build_redis_url(trimmed, user, pass, tls));
    }
    if urls.is_empty() {
        return Err(ConfigError::EmptyHost);
    }
    Ok(urls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_ok() {
        let cfg = RedisConfig {
            host: "127.0.0.1:6379".into(),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());

        let key_cfg = RedisKeyConfig::new(cfg, "k");
        assert!(key_cfg.validate().is_ok());
    }

    #[test]
    fn validate_empty_host() {
        let cfg = RedisConfig::default();
        let err = cfg.validate().expect_err("should fail");
        assert_eq!(err, ConfigError::EmptyHost);
    }

    #[test]
    fn validate_empty_key() {
        let cfg = RedisConfig {
            host: "127.0.0.1:6379".into(),
            ..Default::default()
        };
        let key_cfg = RedisKeyConfig::new(cfg, "");
        let err = key_cfg.validate().expect_err("should fail");
        assert_eq!(err, ConfigError::EmptyKey);
    }

    #[test]
    fn build_client_url_should_inject_scheme_and_creds() {
        let cfg = RedisConfig {
            host: "127.0.0.1:6379".into(),
            user: Some("u".into()),
            pass: Some("p".into()),
            ..Default::default()
        };
        let url = build_redis_url(&cfg.host, cfg.user.as_deref(), cfg.pass.as_deref(), cfg.tls);
        assert!(url.starts_with("redis://u:p@127.0.0.1:6379"));
        // constructing client should not connect, just parse URL
        let client = cfg.new_client().expect("client build");
        matches!(client, ClientKind::Single(_));
    }

    #[test]
    fn cluster_supported_builds_client() {
        let cfg = RedisConfig {
            host: "127.0.0.1:7001,127.0.0.1:7002".into(),
            kind: RedisType::Cluster,
            ..Default::default()
        };
        let client = cfg.new_client().expect("cluster client");
        matches!(client, ClientKind::Cluster(_));
        // pool unsupported for cluster
        let err = cfg.pool(None).expect_err("cluster pool unsupported");
        assert_eq!(err, ConfigError::UnsupportedClusterPool);
    }

    #[test]
    fn rediss_scheme_for_tls() {
        let cfg = RedisConfig {
            host: "127.0.0.1:6380".into(),
            tls: true,
            ..Default::default()
        };
        let url = build_redis_url(&cfg.host, None, None, cfg.tls);
        assert!(url.starts_with("rediss://"));
    }
}
