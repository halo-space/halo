use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

fn default_non_block() -> bool {
    true
}

fn default_ping_timeout() -> Duration {
    Duration::from_secs(1)
}

fn default_type_str() -> String {
    "node".to_string()
}

/// Redis 拓扑类型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RedisType {
    Node,
    Cluster,
}

impl Default for RedisType {
    fn default() -> Self {
        Self::Node
    }
}

/// Redis 连接配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conf {
    /// 节点地址，单节点为 `host:port`，集群用逗号分隔。
    pub host: String,
    /// 拓扑类型：`node` 或 `cluster`，默认 `node`。
    ///
    /// 上游直接使用字符串传入（字段名使用 `type`），在构建客户端时解析。
    #[serde(rename = "type", default = "default_type_str")]
    pub kind: String,
    /// 用户名，可选。
    #[serde(default)]
    pub user: Option<String>,
    /// 密码，可选。
    #[serde(default)]
    pub pass: Option<String>,
    /// 是否启用 TLS，对应 `rediss://`。
    #[serde(default)]
    pub tls: bool,
    /// 是否非阻塞创建客户端：`true` 时跳过启动时 PING。
    #[serde(default = "default_non_block")]
    pub non_block: bool,
    /// PING 超时时间，默认 1s。
    #[serde(default = "default_ping_timeout", with = "humantime_serde")]
    pub ping_timeout: Duration,
}

impl Default for Conf {
    fn default() -> Self {
        Self {
            host: String::new(),
            kind: default_type_str(),
            user: None,
            pass: None,
            tls: false,
            non_block: default_non_block(),
            ping_timeout: default_ping_timeout(),
        }
    }
}

/// 配置错误。
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("redis host cannot be empty")]
    EmptyHost,
    #[error("redis host list is empty after parsing")]
    NoValidHost,
    #[error("invalid redis type: {0}, expect node|cluster")]
    InvalidType(String),
}

impl Conf {
    fn parse_type(&self) -> Result<RedisType, ConfigError> {
        match self.kind.to_ascii_lowercase().as_str() {
            "node" => Ok(RedisType::Node),
            "cluster" => Ok(RedisType::Cluster),
            other => Err(ConfigError::InvalidType(other.to_string())),
        }
    }

    /// 将配置转换为可供驱动使用的 URL 列表。
    ///
    /// - 当 `host` 已含协议（`redis://` / `rediss://`）时原样返回。
    /// - 否则根据 `tls/user/pass` 拼接。
    /// - 集群场景按逗号分隔多个地址。
    pub fn to_urls(&self) -> Result<Vec<String>, ConfigError> {
        if self.host.trim().is_empty() {
            return Err(ConfigError::EmptyHost);
        }

        let auth_prefix = match (&self.user, &self.pass) {
            (Some(u), Some(p)) => format!("{u}:{p}@"),
            (Some(u), None) => format!("{u}@"),
            (None, Some(p)) => format!(":{p}@"),
            (None, None) => String::new(),
        };

        let scheme = if self.tls { "rediss" } else { "redis" };
        let mut urls = Vec::new();
        for raw in self.host.split(',') {
            let addr = raw.trim();
            if addr.is_empty() {
                continue;
            }
            let url = if addr.contains("://") {
                addr.to_string()
            } else {
                format!("{scheme}://{auth_prefix}{addr}")
            };
            urls.push(url);
        }

        if urls.is_empty() {
            return Err(ConfigError::NoValidHost);
        }

        Ok(urls)
    }

    /// 解析拓扑类型（node/cluster）。
    pub fn redis_type(&self) -> Result<RedisType, ConfigError> {
        self.parse_type()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values_should_match_expected_defaults() {
        let cfg = Conf::default();
        assert!(cfg.host.is_empty());
        assert_eq!(cfg.kind, "node");
        assert!(cfg.non_block);
        assert_eq!(cfg.ping_timeout, Duration::from_secs(1));
        assert!(!cfg.tls);
    }

    #[test]
    fn redis_type_should_parse_case_insensitive() {
        let mut cfg = Conf {
            host: "127.0.0.1:6379".into(),
            kind: "Cluster".into(),
            ..Default::default()
        };
        assert_eq!(cfg.redis_type().unwrap(), RedisType::Cluster);

        cfg.kind = "NODE".into();
        assert_eq!(cfg.redis_type().unwrap(), RedisType::Node);
    }

    #[test]
    fn redis_type_should_reject_invalid() {
        let cfg = Conf {
            host: "127.0.0.1:6379".into(),
            kind: "sharded".into(),
            ..Default::default()
        };
        let err = cfg.redis_type().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidType(t) if t == "sharded"));
    }

    #[test]
    fn to_urls_should_infer_scheme_and_auth() {
        let cfg = Conf {
            host: "127.0.0.1:6379".into(),
            user: Some("u".into()),
            pass: Some("p".into()),
            tls: false,
            ..Default::default()
        };
        let urls = cfg.to_urls().unwrap();
        assert_eq!(urls, vec!["redis://u:p@127.0.0.1:6379"]);
    }

    #[test]
    fn to_urls_should_respect_tls() {
        let cfg = Conf {
            host: "127.0.0.1:6380".into(),
            tls: true,
            ..Default::default()
        };
        let urls = cfg.to_urls().unwrap();
        assert_eq!(urls, vec!["rediss://127.0.0.1:6380"]);
    }

    #[test]
    fn to_urls_should_split_cluster_hosts() {
        let cfg = Conf {
            host: "10.0.0.1:6379,10.0.0.2:6379".into(),
            kind: "cluster".into(),
            ..Default::default()
        };
        let urls = cfg.to_urls().unwrap();
        assert_eq!(urls, vec!["redis://10.0.0.1:6379", "redis://10.0.0.2:6379"]);
    }

    #[test]
    fn to_urls_should_error_on_empty_host() {
        let cfg = Conf::default();
        let err = cfg.to_urls().unwrap_err();
        assert!(matches!(err, ConfigError::EmptyHost));
    }

    #[test]
    fn to_urls_should_skip_empty_parts() {
        let cfg = Conf {
            host: " , redis://127.0.0.1:6379 , ".into(),
            ..Default::default()
        };
        let urls = cfg.to_urls().unwrap();
        assert_eq!(urls, vec!["redis://127.0.0.1:6379"]);
    }
}
