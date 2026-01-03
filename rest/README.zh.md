## Halo REST 使用指南（中文）

### 快速开始
- 运行示例：`cargo run -p halo-rest --example hello --release`
- 流程：构建 `Server` → 设置前缀 → 挂载中间件 → 注册路由 → 启动。

```rust
let mut conf = RestConf::default();
// 可选：调整内置中间件
// conf.middlewares.gzip = false;
// conf.max_bytes = 8 * 1024 * 1024;

let mut server = Server::new(conf)
    .expect("new server")
    .with_root_prefix("/ai/v1/")
    .with_prefix("/v1/api/square");

// 全局中间件
server = server.with_middleware(add_header_middleware());

// 路由
server
    .add_routes(vec![Route::new(
        Method::GET,
        "/",
        square_list_handler(AppContext::default()),
    )])
    .expect("add routes");

let handle = server.start().await.expect("start server");
// 等待 Ctrl+C，再优雅退出
handle.stop().await.expect("stop server");
```

### Handler 与 Middleware
- 使用属性宏消除样板代码：
  - Handler: `#[rest::handler] async fn foo(ctx: AppCtx, req: Request<Body>) -> Response<Body> { ... }`
  - Middleware: `#[rest::middleware] async fn bar(req: Request<Body>, next: HandlerFunc) -> Response<Body> { ... }`
- 全局中间件用 `with_middleware/with_middlewares` 链式追加；路由级中间件通过 `with_handlers(mws, routes)` 生成带中间件的路由列表后再 `add_routes(...)`。

### 内置中间件（默认状态）
- `max_bytes`：默认开启，阈值 `conf.max_bytes`（默认 16 MiB）。关闭：`conf.middlewares.max_bytes = false`。
- `gzip`（请求解压/响应压缩）：默认开启。关闭：`conf.middlewares.gzip = false`。
- `rate_limit`：默认关闭。开启：`conf.rate_limit = Some(RateLimitConf { permits_per_second, burst })`。
- `concurrency_limit`：默认关闭。开启：`conf.concurrency_limit = Some(limit)`。
- `timeout`：默认开启 `conf.timeout = Some(3000)` 毫秒。关闭：`conf.timeout = None`。

内置执行顺序：max_bytes → rate_limit → concurrency_limit → timeout → gzip → 用户中间件。

### 配置示例（TOML）
```toml
[Middlewares]
MaxBytes = true
Gzip = true

MaxBytes = 16777216          # 16 MiB
Timeout = 3000               # 毫秒；设为 None 可关闭

[RateLimit]
PermitsPerSecond = 50000
Burst = 100000

ConcurrencyLimit = 10000
```

### 压测
- 按 `examples/bench.sh` 描述先启动 halo-rest、axum、gin 三个服务。
- 运行 `./bench.sh`，结果输出到 `examples/bench_out/*.txt`。

