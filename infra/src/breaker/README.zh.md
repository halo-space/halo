## Breaker（熔断 / 过载保护）

本模块提供熔断/过载保护能力，并结合 Google SRE《Handling Overload》中“客户端请求拒绝概率”思想实现过载保护算法（SRE eq2101）。

- **Google SRE 章节**：`https://landing.google.com/sre/sre-book/chapters/handling-overload/#eq2101`
- **熔断器模式原理（状态/策略/降级）**：[`揭秘微服务架构：熔断器模式如何保障系统稳定运行`](https://www.oryoy.com/news/jie-mi-wei-fu-wu-jia-gou-rong-duan-qi-mo-shi-ru-he-bao-zhang-xi-tong-wen-ding-yun-xing.html)

### 熔断器模式原理（概念对齐）

参考上面的文章（“二、熔断器模式原理”），熔断器通常有三种状态：

- **Closed（关闭）**：正常放行，请求可访问依赖
- **Half-Open（半开）**：探测恢复，放行少量请求验证依赖是否恢复
- **Open（开启）**：直接拒绝请求，避免故障扩散

常见熔断策略：

- **固定时间窗口**：窗口内失败率达到阈值则触发
- **滑动时间窗口**：滑动窗口内失败率达到阈值则触发
- **计数**：一段时间内失败次数达到阈值则触发

常见降级策略：

- **返回默认值**：例如返回缓存/兜底响应
- **调用备用服务**：或返回明确错误信息

> 当前本 crate 的设计是：
> - **采集层**：`BreakerConfig::{RollingWindow|FixedWindow|Counter}`（内部使用 `crate::collection::metrics` 的通用采集器）
> - **判定层**：统一使用 Google SRE(eq2101) 计算是否拒绝（open）

### 核心入口

对外统一入口是：

- `Breaker::new(name, config)`：创建一个以 `name` 为维度的 breaker 实例
- `BreakerConfig`：选择算法以及算法参数

### 四个常用方法

在 Rust 里因为 `do` 是关键字，我们使用 `execute*` 命名：

- **`execute`**
- **`execute_with_acceptable`**
- **`execute_with_fallback`**
- **`execute_with_fallback_acceptable`**

### 使用示例

完整可运行示例见：`infra/examples/breaker_usage.rs`

#### 1）execute（最常用）

```rust
use infra::breaker::{Breaker, BreakerConfig};
use infra::context::Background;

let brk = Breaker::new("demo", BreakerConfig::default()).unwrap();
let ctx = Background();

let v: i32 = brk.execute(&ctx, || Ok(42)).unwrap();
assert_eq!(v, 42);
```

#### 2）execute_with_acceptable（自定义可接受错误）

当 `req()` 返回错误，但你希望“这类错误不计失败/不影响熔断统计”时使用。

```rust
use infra::breaker::{Breaker, BreakerConfig, BreakerPolicy};
use infra::context::Background;

let brk = Breaker::new("demo", BreakerConfig::default()).unwrap();
let ctx = Background();

let r = brk.execute_with_acceptable(&ctx, || Err(anyhow::anyhow!("logical_err")), |_e| true);
assert!(r.is_err()); // 注意：只是“统计上算成功”，返回值仍然是 Err
```

#### 3）execute_with_fallback（拒绝时降级）

当 breaker 因过载/熔断等原因拒绝时执行 fallback，常用于返回缓存/默认值等“降级响应”。

```rust
use infra::breaker::{Breaker, BreakerConfig, BreakerPolicy, ExecuteError, Reject};
use infra::context::Background;

let brk = Breaker::new("demo", BreakerConfig::FixedWindow { window: std::time::Duration::from_secs(10), google: None }).unwrap();
let ctx = Background();

let _hold = brk.allow(&ctx).unwrap(); // 占住一个 in-flight（演示拒绝）

let v = brk.execute_with_fallback(
    &ctx,
    || Ok::<_, anyhow::Error>(1),
    |rej: Reject| Ok::<_, ExecuteError<anyhow::Error>>(match rej {
        Reject::Open { .. } => 0,
        _ => -1,
    }),
).unwrap();

assert_eq!(v, 0);
```

#### 4）execute_with_fallback_acceptable（组合）

同时需要“拒绝时降级”与“可接受错误”时使用。

### 错误类型说明

- **`Reject`**：表示 **breaker 拒绝**（未执行 `req`）
  - `CtxDone / Open / OutOfQuota`
- **`ExecuteError<E>`**：表示一次 `execute*` 的总体结果
  - `Rejected(Reject)`：准入失败
  - `Call(E)`：调用失败
  - `Panic`：调用发生 panic（已计入 fail，并转换为可处理错误；若编译为 `panic=abort` 则无法捕获）

### 参考

- **Google SRE（eq2101）**：`https://landing.google.com/sre/sre-book/chapters/handling-overload/#eq2101`
- **熔断器模式原理（状态/策略/降级）**：[`揭秘微服务架构：熔断器模式如何保障系统稳定运行`](https://www.oryoy.com/news/jie-mi-wei-fu-wu-jia-gou-rong-duan-qi-mo-shi-ru-he-bao-zhang-xi-tong-wen-ding-yun-xing.html)
- **Hystrix 雪崩/线程池隔离/熔断概念**：[`防雪崩利器：熔断器 Hystrix 的原理与使用`](https://segmentfault.com/a/1190000005988895)