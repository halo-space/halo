## Halo REST Usage (English)

Chinese version: [README.zh.md](README.zh.md)

### Quick Start
- Run example: `cargo run -p halo-rest --example hello --release`
- Structure: build a `Server` with config → set prefixes → attach middlewares → register routes → start.

```rust
let mut conf = RestConf::default();
// Optional: tweak built-ins
// conf.middlewares.gzip = false;
// conf.max_bytes = 8 * 1024 * 1024;

let mut server = Server::new(conf)
    .expect("new server")
    .with_root_prefix("/ai/v1/")
    .with_prefix("/v1/api/square");

// attach global middleware(s)
server = server.with_middleware(add_header_middleware());

// register routes
server
    .add_routes(vec![Route::new(
        Method::GET,
        "/",
        square_list_handler(AppContext::default()),
    )])
    .expect("add routes");

let handle = server.start().await.expect("start server");
// block until Ctrl+C, then stop gracefully
handle.stop().await.expect("stop server");
```

### Handlers & Middleware
- Use attribute macros for zero-boilerplate definitions:
  - Handler: `#[rest::handler] async fn foo(ctx: AppCtx, req: Request<Body>) -> Response<Body> { ... }`
  - Middleware: `#[rest::middleware] async fn bar(req: Request<Body>, next: HandlerFunc) -> Response<Body> { ... }`
- The helper `with_middleware` / `with_middlewares` chain global middleware; route-scoped middleware use `with_handlers(mws, routes)` when building route lists.

### Built-in Middlewares (default state)
- `max_bytes`: ON, limit `conf.max_bytes` (default 16 MiB). Disable via `conf.middlewares.max_bytes = false`.
- `gzip` (decode/encode): ON. Disable via `conf.middlewares.gzip = false`.
- `rate_limit`: OFF by default. Enable with `conf.rate_limit = Some(RateLimitConf { permits_per_second, burst })`.
- `concurrency_limit`: OFF by default. Enable with `conf.concurrency_limit = Some(limit)`.
- `timeout`: ON by default (`conf.timeout = Some(3000)` ms). Disable with `conf.timeout = None`.

Built-ins run in order: max_bytes → rate_limit → concurrency_limit → timeout → gzip → user middlewares.

### Configuration (TOML example)
```toml
[Middlewares]
MaxBytes = true
Gzip = true

MaxBytes = 16777216          # 16 MiB
Timeout = 3000               # ms, None to disable

[RateLimit]
PermitsPerSecond = 50000
Burst = 100000

ConcurrencyLimit = 10000
```

### Running Benchmarks
- Start three services (halo-rest, axum, gin) as described in `examples/bench.sh`.
- Run `./bench.sh` to execute wrk benchmarks; results are written to `examples/bench_out/*.txt`.

