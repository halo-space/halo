use crate::config::RestConf;
use crate::http::HandlerFunc;
use crate::middleware::Middleware;
use crate::router::{Route, Router};
use anyhow::Context;
use core::service::Mode;
use hyper::Server as HyperServer;
use hyper::service::{make_service_fn, service_fn};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpSocket};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Lightweight web server built on hyper.
#[derive(Clone)]
pub struct Server {
    conf: RestConf,
    routes: Vec<Route>,
    // For debug printing (mode dev/test): (group_prefix, method, path)
    debug_routes: Vec<(String, String, String)>,
    debug_group: Option<String>,
    middlewares: Vec<Middleware>,
    not_found: Option<HandlerFunc>,
    not_allowed: Option<HandlerFunc>,
    prefix_chain: Vec<String>,
}

impl Server {
    pub fn new(conf: RestConf) -> Self {
        Self {
            conf,
            routes: Vec::new(),
            debug_routes: Vec::new(),
            debug_group: None,
            middlewares: Vec::new(),
            not_found: None,
            not_allowed: None,
            prefix_chain: Vec::new(),
        }
    }

    /// Get server config (read-only).
    pub fn conf(&self) -> &RestConf {
        &self.conf
    }

    /// Debug print all registered routes (after prefixes applied) when mode is dev/test.
    fn debug_print_routes(&self) {
        let is_verbose = matches!(self.conf.service.mode, Mode::Dev | Mode::Test);
        if !is_verbose {
            return;
        }
        // Flatten to rows, compute column widths, print as table.
        let mut rows: Vec<(String, String, String)> = self.debug_routes.clone();
        // Sort by group, then path, then method for stable output.
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)).then(a.1.cmp(&b.1)));
        let mut rows_dedup = Vec::new();
        let mut last: Option<(String, String, String)> = None;
        for r in rows {
            if last.as_ref() == Some(&r) {
                continue;
            }
            rows_dedup.push(r.clone());
            last = Some(r);
        }
        let mut group_w = "Group".len();
        let mut method_w = "Method".len();
        let mut path_w = "Path".len();
        for (g, m, p) in &rows_dedup {
            group_w = group_w.max(g.len());
            method_w = method_w.max(m.len());
            path_w = path_w.max(p.len());
        }
        let header = format!(
            "{:<group_w$} | {:<method_w$} | {:<path_w$}",
            "Group",
            "Method",
            "Path",
            group_w = group_w,
            method_w = method_w,
            path_w = path_w
        );
        println!("Registered routes (mode {:?}):", self.conf.service.mode);
        println!("{header}");
        println!(
            "{}-+-{}-+-{}",
            "-".repeat(group_w),
            "-".repeat(method_w),
            "-".repeat(path_w)
        );
        for (g, m, p) in rows_dedup {
            let label = if g == "-" { "-" } else { g.as_str() };
            println!(
                "{:<group_w$} | {:<method_w$} | {:<path_w$}",
                label,
                m,
                p,
                group_w = group_w,
                method_w = method_w,
                path_w = path_w
            );
        }
    }

    /// Register routes (in-place, chain-friendly).
    pub fn add_routes<I>(&mut self, routes: I) -> &mut Self
    where
        I: IntoIterator<Item = Route>,
    {
        let routes_vec: Vec<Route> = routes.into_iter().collect();
        let group_prefix = self.current_group_label();
        let routes = self.apply_prefixes(routes_vec);
        for r in &routes {
            self.debug_routes
                .push((group_prefix.clone(), r.method.to_string(), r.path.clone()));
        }
        self.routes.extend(routes);
        self.reset_to_root();
        self
    }

    /// Register a single route (in-place).
    pub fn add_route(&mut self, route: Route) -> &mut Self {
        let group_prefix = self.current_group_label();
        let routes = self.apply_prefixes(vec![route]);
        for r in &routes {
            self.debug_routes
                .push((group_prefix.clone(), r.method.to_string(), r.path.clone()));
        }
        self.routes.extend(routes);
        self.reset_to_root();
        self
    }

    fn current_group_label(&self) -> String {
        if let Some(g) = &self.debug_group {
            return g.clone();
        }
        if self.prefix_chain.is_empty() {
            return "-".to_string();
        }
        let mut joined = self.prefix_chain.join("");
        if !joined.starts_with('/') {
            joined.insert(0, '/');
        }
        while joined.contains("//") {
            joined = joined.replace("//", "/");
        }
        if joined == "/" {
            "-".to_string()
        } else {
            joined
        }
    }

    fn reset_to_root(&mut self) {
        if self.prefix_chain.is_empty() {
            return;
        }
        let root = self.prefix_chain.first().cloned();
        self.prefix_chain.clear();
        if let Some(r) = root {
            self.prefix_chain.push(r);
        }
    }

    pub fn set_debug_group(&mut self, name: impl Into<String>) {
        self.debug_group = Some(name.into());
    }

    pub fn clear_debug_group(&mut self) {
        self.debug_group = None;
    }

    /// Register global middleware applied to all routes.
    pub fn use_middleware(&mut self, middleware: Middleware) -> &mut Self {
        self.middlewares.push(middleware);
        self
    }

    /// Chain-friendly append for global middlewares.
    pub fn with_middlewares<I>(&mut self, middlewares: I) -> &mut Self
    where
        I: IntoIterator<Item = Middleware>,
    {
        self.middlewares.extend(middlewares);
        self
    }

    /// Chain-friendly append for a single global middleware.
    pub fn with_middleware(&mut self, middleware: Middleware) -> &mut Self {
        self.with_middlewares(std::iter::once(middleware))
    }

    /// Set custom 404 handler.
    pub fn set_not_found_handler(&mut self, handler: HandlerFunc) {
        self.not_found = Some(handler);
    }

    /// Set custom 405 handler.
    pub fn set_not_allowed_handler(&mut self, handler: HandlerFunc) {
        self.not_allowed = Some(handler);
    }

    /// Set root prefix (replaces previous prefix chain).
    pub fn with_root_prefix(&mut self, prefix: impl Into<String>) -> &mut Self {
        self.prefix_chain.clear();
        self.prefix_chain.push(prefix.into());
        self
    }

    /// Append prefix (can be chained).
    pub fn with_prefix(&mut self, prefix: impl Into<String>) -> &mut Self {
        self.prefix_chain.push(prefix.into());
        self
    }

    /// Deprecated aliases removed to keep pure Rust naming.
    /// Start HTTP server and return controllable handle.
    pub async fn start(self) -> anyhow::Result<ServerHandle> {
        self.debug_print_routes();
        let listen_addr: SocketAddr = self
            .conf
            .addr_string()
            .parse()
            .context("parse listen addr")?;
        let router = Arc::new(self.build_router()?);

        if self.conf.reuse_port {
            start_with_reuse_port(listen_addr, router, &self.conf).await
        } else {
            let socket = TcpSocket::new_v4()?;
            socket.set_reuseaddr(true)?;
            if self.conf.tcp_keepalive_secs.is_some() {
                socket.set_keepalive(true)?;
            }
            socket.bind(listen_addr)?;
            let listener = socket.listen(1024)?;
            start_single(listener, router, &self.conf).await
        }
    }

    fn build_router(&self) -> anyhow::Result<Router> {
        let mut router = Router::new();
        if let Some(h) = &self.not_found {
            router.set_not_found_handler(h.clone());
        }
        if let Some(h) = &self.not_allowed {
            router.set_not_allowed_handler(h.clone());
        }
        // Auto middlewares: max_bytes -> rate/concurrency/timeout -> gzip (decode/encode) -> user
        let mut auto_mws: Vec<Middleware> = Vec::new();
        if self.conf.middlewares.max_bytes {
            auto_mws.push(crate::middleware::max_bytes(self.conf.max_bytes as u64));
        }
        if let Some(rl) = &self.conf.rate_limit {
            auto_mws.push(crate::middleware::rate_limit(
                rl.permits_per_second,
                rl.burst,
            ));
        }
        if let Some(c) = self.conf.concurrency_limit {
            auto_mws.push(crate::middleware::concurrency_limit(c));
        }
        if let Some(ms) = self.conf.timeout {
            auto_mws.push(crate::middleware::timeout(
                std::time::Duration::from_millis(ms),
            ));
        }
        if self.conf.middlewares.gzip {
            auto_mws.push(crate::middleware::gzip());
        }

        for route in &self.routes {
            let mut mws = auto_mws.clone();
            mws.extend(self.middlewares.clone());
            let route = route.clone().with_middlewares(&mws);
            router.add_route(route)?;
        }
        Ok(router)
    }

    fn apply_prefixes(&self, routes: Vec<Route>) -> Vec<Route> {
        // Add shorter suffixes first, then prepend root prefix to ensure `/api` + `/v1` + `/hello`.
        let mut acc = routes;
        for p in self.prefix_chain.iter().rev() {
            acc = crate::with_prefix(p, acc);
        }
        acc
    }
}

/// Handle to control server lifecycle (graceful stop).
pub struct ServerHandle {
    addr: SocketAddr,
    shutdowns: Vec<oneshot::Sender<()>>,
    joins: Vec<JoinHandle<anyhow::Result<()>>>,
}

impl ServerHandle {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Send shutdown signal and wait for graceful exit.
    pub async fn stop(mut self) -> anyhow::Result<()> {
        let shutdowns = std::mem::take(&mut self.shutdowns);
        for tx in shutdowns {
            let _ = tx.send(());
        }
        let joins = std::mem::take(&mut self.joins);
        for j in joins {
            j.await.context("join server task")?.context("server run")?;
        }
        Ok(())
    }
}

async fn start_single(
    listener: TcpListener,
    router: Arc<Router>,
    conf: &RestConf,
) -> anyhow::Result<ServerHandle> {
    let local_addr = listener.local_addr().context("get local addr")?;
    let (shutdown_tx, shutdown) = oneshot::channel::<()>();
    let mut builder = HyperServer::from_tcp(listener.into_std()?)?;
    if conf.http2 {
        builder = builder.http2_only(true);
    } else {
        builder = builder.http1_only(true);
        builder = builder.http1_keepalive(conf.http1_keep_alive);
        if let Some(sz) = conf.http1_max_buf_size {
            builder = builder.http1_max_buf_size(sz);
        }
    }

    let svc = make_service_fn(move |_conn| {
        let router = router.clone();
        async move {
            Ok::<_, std::convert::Infallible>(service_fn(move |req| {
                let router = router.clone();
                async move { Ok::<_, std::convert::Infallible>(router.dispatch(req).await) }
            }))
        }
    });

    let server = builder.serve(svc).with_graceful_shutdown(async move {
        let _ = shutdown.await;
    });

    let join: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
        server
            .await
            .map_err(|e| anyhow::anyhow!("hyper server error: {e}"))
    });

    Ok(ServerHandle {
        addr: local_addr,
        shutdowns: vec![shutdown_tx],
        joins: vec![join],
    })
}

async fn start_with_reuse_port(
    addr: SocketAddr,
    router: Arc<Router>,
    conf: &RestConf,
) -> anyhow::Result<ServerHandle> {
    let workers = conf.workers.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    });
    let mut joins = Vec::with_capacity(workers);
    let mut shutdowns = Vec::with_capacity(workers);
    let mut bound_addr = None;

    for _ in 0..workers {
        let socket = TcpSocket::new_v4()?;
        socket.set_reuseaddr(true)?;
        if conf.tcp_keepalive_secs.is_some() {
            socket.set_keepalive(true)?;
        }
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        socket.set_reuseport(true)?;

        socket.bind(addr)?;
        let listener = socket.listen(1024)?;
        let local = listener.local_addr().context("get local addr")?;
        if bound_addr.is_none() {
            bound_addr = Some(local);
        }

        let router_clone = router.clone();
        let (tx, shutdown) = oneshot::channel::<()>();
        let mut builder = HyperServer::from_tcp(listener.into_std()?)?;
        if conf.http2 {
            builder = builder.http2_only(true);
        } else {
            builder = builder.http1_only(true);
            builder = builder.http1_keepalive(conf.http1_keep_alive);
            if let Some(sz) = conf.http1_max_buf_size {
                builder = builder.http1_max_buf_size(sz);
            }
        }
        let server = builder
            .serve(make_service_fn(move |_conn| {
                let router = router_clone.clone();
                async move {
                    Ok::<_, std::convert::Infallible>(service_fn(move |req| {
                        let router = router.clone();
                        async move { Ok::<_, std::convert::Infallible>(router.dispatch(req).await) }
                    }))
                }
            }))
            .with_graceful_shutdown(async move {
                let _ = shutdown.await;
            });

        let join: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
            server
                .await
                .map_err(|e| anyhow::anyhow!("hyper server error: {e}"))
        });
        joins.push(join);
        shutdowns.push(tx);
    }

    Ok(ServerHandle {
        addr: bound_addr.unwrap_or(addr),
        shutdowns,
        joins,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{Method, StatusCode};
    use hyper::body::to_bytes;
    use hyper::{Body, Client};
    use tokio::runtime::Runtime;

    fn runtime() -> Runtime {
        Runtime::new().unwrap()
    }

    fn ok_route(path: &str) -> Route {
        Route::new(Method::GET, path, |_: http::Request<Body>| async {
            http::Response::builder()
                .status(StatusCode::OK)
                .body(Body::from("ok"))
                .unwrap()
        })
    }

    #[test]
    fn add_routes_should_store() {
        runtime().block_on(async {
            let mut server = Server::new(RestConf::default());
            server.add_route(ok_route("/hello"));
            assert_eq!(server.routes.len(), 1);
        });
    }

    #[test]
    fn start_should_serve_requests() {
        runtime().block_on(async {
            let mut conf = RestConf::default();
            // pick free port
            let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            conf.host = "127.0.0.1".to_string();
            conf.port = probe.local_addr().unwrap().port();
            drop(probe);

            let mut server = Server::new(conf);
            server.add_route(ok_route("/ping"));
            let handle = server.start().await.unwrap();
            let client = Client::new();
            let uri = format!("http://{}{}", handle.addr(), "/ping")
                .parse()
                .unwrap();
            let resp = client.get(uri).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
            let body = to_bytes(resp.into_body()).await.unwrap();
            assert_eq!(&body[..], b"ok");

            handle.stop().await.unwrap();
        });
    }

    #[test]
    fn demo_service_with_middleware_and_prefix() {
        runtime().block_on(async {
            let mut conf = RestConf::default();
            let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            conf.host = "127.0.0.1".to_string();
            conf.port = probe.local_addr().unwrap().port();
            drop(probe);

            let routes = vec![Route::new(
                Method::GET,
                "/hello",
                |_: http::Request<Body>| async {
                    http::Response::builder()
                        .status(StatusCode::OK)
                        .body(Body::from("hi"))
                        .unwrap()
                },
            )];

            // Global middleware: add response header
            let mw = crate::middleware(|req, next| async move {
                let mut resp = next.call(req).await;
                resp.headers_mut()
                    .insert("X-Demo", http::HeaderValue::from_static("1"));
                resp
            });

            let mut server = Server::new(conf);
            server.with_root_prefix("/api").with_prefix("/session");
            server.use_middleware(mw);
            server.add_routes(routes);
            let handle = server.start().await.unwrap();

            let client = Client::new();
            let uri = format!("http://{}{}", handle.addr(), "/api/session/hello")
                .parse()
                .unwrap();
            let resp = client.get(uri).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
            assert_eq!(resp.headers().get("X-Demo").unwrap(), "1");

            handle.stop().await.unwrap();
        });
    }
}
