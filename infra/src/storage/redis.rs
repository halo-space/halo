//! Redis 存储占位实现。
//! 仅定义结构体占位，后续按需补充实际功能。

pub mod config;

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
