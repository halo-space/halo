# sync

同步能力集合，位于 `core/src` 下与 `context` 平级。

## singleflight

### 功能
并发相同 key 的调用只执行一次，结果（成功/失败）在等待者之间共享，类似 Go `singleflight.Group`：
- API 对标：
  - Go `Group.Do` -> `done(ctx, key, make) -> Result<SharedResult<V, E>, ContextError>`
  - Go `Group.DoChan` -> `do_chan(ctx, key, make) -> oneshot::Receiver<Result<SharedResult<V, E>, ContextError>>`
  - Go `Group.Forget` -> `forget(key)`
- `forget(key)`：主动清理某个 key，下一次会重新执行。

### 依赖与环境
- 需要 Tokio 运行时（示例使用 `#[tokio::main]`）。
- 泛型约束：`K: Eq + Hash + Clone + Send + Sync + 'static`，`V/E: Any + Send + Sync + 'static`。

### 使用示例
```rust
use core::sync::singleflight::SingleFlight;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use core::context::Background;

#[tokio::main]
async fn main() {
    let group = SingleFlight::<Arc<str>>::new();
    let counter = Arc::new(AtomicUsize::new(0));

    // 第一个调用：实际执行业务逻辑
    let g1 = group.clone();
    let c1 = counter.clone();
    let t1 = tokio::spawn(async move {
        let ctx = Background();
        g1.done(&ctx, Arc::from("user:42"), move || {
            let c = c1.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst); // 只应执行一次
                Ok::<_, &'static str>(format!("profile_of_42"))
            }
        })
        .await
    });

    // 第二个调用：相同 key，会共享结果，不重复执行业务逻辑
    let g2 = group.clone();
    let t2 = tokio::spawn(async move {
        let ctx = Background();
        g2.done(&ctx, Arc::from("user:42"), || async {
            // 即便这里写不同的返回值，也不会执行到
            Ok::<_, &'static str>("should not run".to_string())
        })
        .await
    });

    let r1 = t1.await.unwrap();
    let r2 = t2.await.unwrap();

    assert_eq!(r1.shared, false); // 首次
    assert_eq!(r2.shared, true);  // 复用
    assert_eq!(r1.value.as_ref().unwrap(), "profile_of_42");
    assert_eq!(r2.value.as_ref().unwrap(), "profile_of_42");
    assert_eq!(counter.load(Ordering::SeqCst), 1); // 只执行过一次

    // 如需重新执行，可主动 forget
    group.forget(&"user:42").await;

    // 搭配 Context 取消/超时（仅影响等待方，不中断执行任务）
    use core::context::{Background, WithTimeout};
    let (ctx, _) = WithTimeout(Background(), std::time::Duration::from_millis(50));
    let res = group
        .done(&ctx, Arc::from("user:cancel"), || async {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            Ok::<_, &'static str>("slow")
        })
        .await;
    assert!(res.is_err()); // Context 先结束，等待方提前返回（执行者继续跑完）
}
```

### 行为说明
- shared=true 表示当前结果来自复用的执行（不是本次 make）。
- 错误也会共享（与 Go 行为一致）；如需仅缓存成功，可在外层加策略。
- forget 仅移除 map 中的 flight，不会取消正在执行的任务。
- 若需要等待者超时/取消，可在调用方使用 `tokio::select!` 搭配自身超时，不影响主执行任务。
 
### 基准
- `core/benches/singleflight_bench.rs`（criterion，n∈{1,2,4,8}，每次迭代新建 group，避免状态堆积）。
- 运行：`cargo bench -p halo-core --bench singleflight_bench`

