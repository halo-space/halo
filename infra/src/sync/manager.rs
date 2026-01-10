use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use anyhow::Error;
use parking_lot::RwLock;

use crate::sync::singleflight::SingleFlight;

/// 受管资源需要实现的关闭能力。
pub trait ResourceHandle: Send + Sync + 'static {
    fn close(&self) -> Result<(), Error>;
}

/// 资源：按 key 懒加载资源，避免并发重复构建，并统一关闭。
pub struct Resource<K, R>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    R: ResourceHandle,
{
    resources: RwLock<HashMap<K, Arc<R>>>,
    flight: SingleFlight<K>,
}

impl<K, R> Resource<K, R>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    R: ResourceHandle,
{
    pub fn new() -> Self {
        Self {
            resources: RwLock::new(HashMap::new()),
            flight: SingleFlight::new(),
        }
    }

    /// 获取或创建资源（并发下单次创建），返回 Arc。
    pub async fn get_or_try_init<Fut, Make>(
        &self,
        ctx: &crate::context::Context,
        key: K,
        make: Make,
    ) -> Result<Arc<R>, Error>
    where
        Make: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<R, Error>> + Send + 'static,
    {
        // 先尝试读缓存。
        if let Some(existing) = self.resources.read().get(&key) {
            return Ok(existing.clone());
        }

        let res = self.flight.done(ctx, key.clone(), move || async move {
            let created = make().await?;
            Ok::<Arc<R>, Error>(Arc::new(created))
        });

        let res = res.await.map_err(Error::from)?;

        let arc: Arc<R> = res
            .value
            .map_err(|e| Error::msg(e.to_string()))?
            .as_ref()
            .clone();
        // 回填缓存
        let mut guard = self.resources.write();
        guard.entry(key).or_insert_with(|| arc.clone());

        Ok(arc)
    }

    /// 主动插入资源。
    pub fn insert(&self, key: K, resource: R) {
        self.resources.write().insert(key, Arc::new(resource));
    }

    /// 关闭并清空已缓存资源。
    pub fn close(&self) -> Result<(), Error> {
        let mut errors = Vec::new();
        let mut guard = self.resources.write();
        for (_k, res) in guard.drain() {
            if let Err(e) = res.close() {
                errors.push(e);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::msg(
                errors
                    .into_iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; "),
            ))
        }
    }
}

impl<K, R> Default for Resource<K, R>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    R: ResourceHandle,
{
    fn default() -> Self {
        Self::new()
    }
}
