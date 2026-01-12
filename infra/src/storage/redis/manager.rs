use redis::RedisError as DriverError;
use redis::aio::MultiplexedConnection;
use redis::cluster::ClusterClient;
use redis::cluster_async::ClusterConnection;
use thiserror::Error;
use tokio::time::timeout;

use crate::storage::redis::config::{Conf, ConfigError, RedisType};

/// Redis 客户端错误。
#[derive(Debug, Error)]
pub enum RedisClientError {
    #[error("invalid redis config: {0}")]
    InvalidConfig(#[from] ConfigError),
    #[error(transparent)]
    Driver(#[from] DriverError),
    #[error("redis operation timed out")]
    Timeout(#[from] tokio::time::error::Elapsed),
}

pub type RedisResult<T> = Result<T, RedisClientError>;

enum Inner {
    Node(redis::Client),
    Cluster(ClusterClient),
}

/// 统一的异步连接类型：直接可用于 redis-rs 的 AsyncCommands/AsyncTypedCommands。
pub enum Redis {
    Node(MultiplexedConnection),
    Cluster(ClusterConnection),
}

impl std::fmt::Debug for Redis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Redis::Node(_) => f.write_str("Redis::Node"),
            Redis::Cluster(_) => f.write_str("Redis::Cluster"),
        }
    }
}

/// 创建统一的 Redis 连接（单节点或集群），可直接使用 redis-rs 的 AsyncCommands/AsyncTypedCommands。
///
/// - `non_block == false` 时会在创建阶段执行一次 PING（带超时）。
pub async fn new_redis(conf: Conf) -> RedisResult<Redis> {
    let urls = conf.to_urls()?;
    let redis_type = conf.redis_type()?;
    let inner = match redis_type {
        RedisType::Node => {
            let url = urls
                .first()
                .ok_or(ConfigError::NoValidHost)
                .map_err(RedisClientError::from)?;
            let client = redis::Client::open(url.as_str())?;
            Inner::Node(client)
        }
        RedisType::Cluster => {
            let client = ClusterClient::new(urls)?;
            Inner::Cluster(client)
        }
    };

    let conn = match &inner {
        Inner::Node(client) => {
            let conn = client.get_multiplexed_async_connection().await?;
            Redis::Node(conn)
        }
        Inner::Cluster(client) => {
            let conn = client.get_async_connection().await?;
            Redis::Cluster(conn)
        }
    };

    // 非阻塞=false 时做一次 PING 检测
    if !conf.non_block {
        let mut conn_for_ping = match &inner {
            Inner::Node(client) => {
                let c = client.get_multiplexed_async_connection().await?;
                Redis::Node(c)
            }
            Inner::Cluster(client) => {
                let c = client.get_async_connection().await?;
                Redis::Cluster(c)
            }
        };
        timeout(conf.ping_timeout, async {
            let _: String = redis::cmd("PING").query_async(&mut conn_for_ping).await?;
            Ok::<(), RedisClientError>(())
        })
        .await??;
    }

    Ok(conn)
}

impl redis::aio::ConnectionLike for Redis {
    fn req_packed_command<'a>(
        &'a mut self,
        cmd: &'a redis::Cmd,
    ) -> redis::RedisFuture<'a, redis::Value> {
        match self {
            Redis::Node(c) => c.req_packed_command(cmd),
            Redis::Cluster(c) => c.req_packed_command(cmd),
        }
    }

    fn req_packed_commands<'a>(
        &'a mut self,
        pipeline: &'a redis::Pipeline,
        offset: usize,
        count: usize,
    ) -> redis::RedisFuture<'a, Vec<redis::Value>> {
        match self {
            Redis::Node(c) => c.req_packed_commands(pipeline, offset, count),
            Redis::Cluster(c) => c.req_packed_commands(pipeline, offset, count),
        }
    }

    fn get_db(&self) -> i64 {
        match self {
            Redis::Node(c) => c.get_db(),
            // Cluster 语义上固定 0
            Redis::Cluster(_) => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use redis::AsyncCommands;
    use std::env;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn new_redis_should_reject_empty_host() {
        let conf = Conf::default();
        let err = new_redis(conf).await.unwrap_err();
        assert!(matches!(
            err,
            RedisClientError::InvalidConfig(ConfigError::EmptyHost)
        ));
    }

    #[tokio::test]
    async fn new_redis_should_fail_on_invalid_address() {
        let conf = Conf {
            host: "not-a-valid-host".into(),
            non_block: false,
            ..Default::default()
        };
        let err = new_redis(conf).await.unwrap_err();
        assert!(matches!(err, RedisClientError::Driver(_)));
    }

    #[tokio::test]
    async fn set_and_get_should_work_when_redis_available() {
        let host = match env::var("TEST_REDIS_URL") {
            Ok(v) => v,
            Err(_) => return,
        };

        let conf = Conf {
            host,
            non_block: false,
            ping_timeout: Duration::from_secs(2),
            ..Default::default()
        };

        let mut conn = new_redis(conf).await.unwrap();
        let key = format!(
            "halo:test:{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );

        let _: () = conn.set(&key, "hello").await.unwrap();
        let val: String = conn.get(&key).await.unwrap();
        assert_eq!(val, "hello".to_string());
    }
}
