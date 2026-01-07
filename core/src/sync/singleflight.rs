use std::any::Any;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use crate::context::{Context, ContextError};
use tokio::sync::{Mutex, Notify};

/// singleflight 管理器：并发相同 key 的调用合并为一次执行，共享结果。
#[derive(Clone)]
pub struct SingleFlight<K> {
    flights: Arc<Mutex<HashMap<K, Arc<Flight>>>>,
}

impl<K> SingleFlight<K>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
{
    /// 创建空的 singleflight。
    pub fn new() -> Self {
        Self {
            flights: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 执行或复用某个 key 的异步任务，返回结果与是否复用（shared）。
    /// `ctx` 仅用于等待方的取消/超时，不会中断正在执行的任务。
    pub async fn done<Fut, V, E>(
        &self,
        ctx: &Context,
        key: K,
        make: impl FnOnce() -> Fut,
    ) -> Result<SharedResult<V, E>, ContextError>
    where
        Fut: std::future::Future<Output = Result<V, E>> + Send + 'static,
        V: Any + Send + Sync + 'static,
        E: Any + Send + Sync + 'static,
    {
        if let Some(err) = ctx.err() {
            return Err(err);
        }
        let fut = self.inner_done(key, make);
        tokio::select! {
            res = fut => Ok(res),
            _ = ctx.done_async() => Err(ctx.err().unwrap_or(crate::context::error::CANCELLED)),
        }
    }

    async fn inner_done<Fut, V, E>(&self, key: K, make: impl FnOnce() -> Fut) -> SharedResult<V, E>
    where
        Fut: std::future::Future<Output = Result<V, E>> + Send + 'static,
        V: Any + Send + Sync + 'static,
        E: Any + Send + Sync + 'static,
    {
        // 原子化检查+插入，避免窗口期并发重复插入。
        let (flight, shared) = {
            let mut guard = self.flights.lock().await;
            if let Some(entry) = guard.get(&key) {
                (entry.clone(), true)
            } else {
                let entry = Arc::new(Flight::new());
                guard.insert(key.clone(), entry.clone());
                (entry, false)
            }
        };

        if shared {
            let value = flight.wait::<V, E>().await;
            return SharedResult { value, shared };
        }

        let value = run_and_finish(flight.clone(), make).await;

        // 清理：只移除当前 flight，避免 race。
        let mut guard = self.flights.lock().await;
        if let Some(existing) = guard.get(&key) {
            if Arc::ptr_eq(existing, &flight) {
                guard.remove(&key);
            }
        }

        SharedResult { value, shared }
    }

    /// DoChan：返回一个 oneshot 通道，结果就绪后发送。
    pub fn do_chan<Fut, V, E>(
        &self,
        ctx: Context,
        key: K,
        make: impl FnOnce() -> Fut + Send + 'static,
    ) -> tokio::sync::oneshot::Receiver<Result<SharedResult<V, E>, ContextError>>
    where
        Fut: std::future::Future<Output = Result<V, E>> + Send + 'static,
        V: Any + Send + Sync + 'static,
        E: Any + Send + Sync + 'static,
        K: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let this = self.clone();
        tokio::spawn(async move {
            let res = this.done(&ctx, key, make).await;
            let _ = tx.send(res);
        });
        rx
    }

    /// 主动遗忘某个 key，方便下次重新发起；不依赖 ctx。
    pub async fn forget(&self, key: &K) {
        let mut guard = self.flights.lock().await;
        guard.remove(key);
    }
}

/// 共享结果，shared=true 表示复用了正在进行的调用。
#[derive(Debug)]
pub struct SharedResult<V, E> {
    pub value: Result<Arc<V>, Arc<E>>,
    pub shared: bool,
}

struct Flight {
    notify: Notify,
    result: Mutex<Option<Result<Arc<dyn Any + Send + Sync>, Arc<dyn Any + Send + Sync>>>>,
}

impl Flight {
    fn new() -> Self {
        Self {
            notify: Notify::new(),
            result: Mutex::new(None),
        }
    }

    async fn finish<V, E>(&self, value: Result<Arc<V>, Arc<E>>)
    where
        V: Any + Send + Sync + 'static,
        E: Any + Send + Sync + 'static,
    {
        let mut guard = self.result.lock().await;
        if guard.is_none() {
            let erased: Result<Arc<dyn Any + Send + Sync>, Arc<dyn Any + Send + Sync>> = match value
            {
                Ok(v) => Ok(v as Arc<dyn Any + Send + Sync>),
                Err(e) => Err(e as Arc<dyn Any + Send + Sync>),
            };
            *guard = Some(erased);
        }
        drop(guard);
        self.notify.notify_waiters();
    }

    async fn wait<V, E>(&self) -> Result<Arc<V>, Arc<E>>
    where
        V: Any + Send + Sync + 'static,
        E: Any + Send + Sync + 'static,
    {
        loop {
            if let Some(res) = self.try_get::<V, E>().await {
                return res;
            }
            self.notify.notified().await;
        }
    }

    async fn try_get<V, E>(&self) -> Option<Result<Arc<V>, Arc<E>>>
    where
        V: Any + Send + Sync + 'static,
        E: Any + Send + Sync + 'static,
    {
        let guard = self.result.lock().await;
        guard.as_ref().map(|res| match res {
            Ok(v) => {
                let v = v
                    .clone()
                    .downcast::<V>()
                    .unwrap_or_else(|_| panic!("type mismatch when downcasting shared value"));
                Ok(v)
            }
            Err(e) => {
                let e = e
                    .clone()
                    .downcast::<E>()
                    .unwrap_or_else(|_| panic!("type mismatch when downcasting shared error"));
                Err(e)
            }
        })
    }
}

async fn run_and_finish<Fut, V, E>(
    flight: Arc<Flight>,
    make: impl FnOnce() -> Fut,
) -> Result<Arc<V>, Arc<E>>
where
    Fut: std::future::Future<Output = Result<V, E>> + Send + 'static,
    V: Any + Send + Sync + 'static,
    E: Any + Send + Sync + 'static,
{
    let output = make().await;
    let mapped = output.map(Arc::new).map_err(Arc::new);
    flight.finish(mapped.clone()).await;
    mapped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::error::Error as CtxErr;
    use crate::context::{Background, WithTimeout};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn shared_success() {
        let group = SingleFlight::<&'static str>::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let t1 = {
            let g = group.clone();
            let c = counter.clone();
            tokio::spawn(async move {
                let ctx = Background();
                g.done(&ctx, "k", move || {
                    let c = c.clone();
                    async move {
                        // 保持 flight 挂起一段时间，等待共享者加入。
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        c.fetch_add(1, Ordering::SeqCst);
                        Ok::<_, ()>(1u32)
                    }
                })
                .await
            })
        };

        // 确保 t1 已插入 flight。
        tokio::time::sleep(Duration::from_millis(10)).await;

        let t2 = {
            let g = group.clone();
            let ctx = Background();
            tokio::spawn(async move { g.done(&ctx, "k", || async { Ok::<_, ()>(2u32) }).await })
        };

        let r1 = t1.await.unwrap().unwrap();
        let r2 = t2.await.unwrap().unwrap();

        assert_eq!(*r1.value.unwrap(), 1);
        assert_eq!(*r2.value.unwrap(), 1);
        assert_eq!(r1.shared, false);
        assert_eq!(r2.shared, true);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn shared_error() {
        let group = SingleFlight::<&'static str>::new();

        let t1 = {
            let g = group.clone();
            tokio::spawn(async move {
                let ctx = Background();
                g.done(&ctx, "k", move || async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Err::<(), &str>("boom")
                })
                .await
            })
        };

        tokio::time::sleep(Duration::from_millis(10)).await;

        let t2 = {
            let g = group.clone();
            let ctx = Background();
            tokio::spawn(async move { g.done(&ctx, "k", || async { Ok::<(), &str>(()) }).await })
        };

        let r1: SharedResult<(), &str> = t1.await.unwrap().unwrap();
        let r2: SharedResult<(), &str> = t2.await.unwrap().unwrap();

        assert_eq!(r1.shared, false);
        assert_eq!(r2.shared, true);
        assert_eq!(r1.value.unwrap_err().as_ref(), &"boom");
        assert_eq!(r2.value.unwrap_err().as_ref(), &"boom");
    }

    #[tokio::test]
    async fn different_keys_not_shared() {
        let group = SingleFlight::<&'static str>::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let t1 = {
            let g = group.clone();
            let c = counter.clone();
            tokio::spawn(async move {
                let ctx = Background();
                g.done(&ctx, "k1", move || {
                    let c = c.clone();
                    async move {
                        c.fetch_add(1, Ordering::SeqCst);
                        Ok::<_, ()>(1u32)
                    }
                })
                .await
            })
        };

        let t2 = {
            let g = group.clone();
            let c = counter.clone();
            tokio::spawn(async move {
                let ctx = Background();
                g.done(&ctx, "k2", move || {
                    let c = c.clone();
                    async move {
                        c.fetch_add(1, Ordering::SeqCst);
                        Ok::<_, ()>(2u32)
                    }
                })
                .await
            })
        };

        let r1 = t1.await.unwrap().unwrap();
        let r2 = t2.await.unwrap().unwrap();

        assert_eq!(r1.shared, false);
        assert_eq!(r2.shared, false);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn ctx_timeout_returns_ctx_error() {
        let group = SingleFlight::<&'static str>::new();
        let (ctx, _) = WithTimeout(Background(), Duration::from_millis(50));

        let res = group
            .done(&ctx, "k", || async {
                tokio::time::sleep(Duration::from_millis(200)).await;
                Ok::<_, ()>(1u32)
            })
            .await;

        let err = res.expect_err("should timeout");
        assert_eq!(err.kind(), CtxErr::DeadlineExceeded);
    }
}
