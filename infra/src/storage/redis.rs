//! Redis 存储模块。
//! 配置在 `config.rs`，客户端与连接管理在 `manager.rs`。

pub mod config;
pub mod manager;

pub use config::{Conf, ConfigError, RedisType};
pub use manager::{Redis, RedisClientError, new_redis};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RedisStore;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redis_store_is_constructible() {
        let store = RedisStore::default();
        assert_eq!(store, RedisStore);
    }
}
