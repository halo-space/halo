use crate::IntoHandler;
use crate::config::RestConf;
use crate::http::HandlerFunc;
use crate::middleware::Middleware;
use crate::router::Route;
use crate::server::{Server, ServerHandle};

/// Engine that collects routes/middlewares and then starts the server.
#[derive(Clone)]
pub struct Engine {
    conf: RestConf,
    routes: Vec<Route>,
    middlewares: Vec<Middleware>,
    not_found: Option<HandlerFunc>,
    not_allowed: Option<HandlerFunc>,
}

impl Engine {
    /// Create engine with RestConf; validates config.
    /// Create engine with RestConf and validate config.
    pub fn new(conf: RestConf) -> anyhow::Result<Self> {
        Ok(Self {
            conf,
            routes: Vec::new(),
            middlewares: Vec::new(),
            not_found: None,
            not_allowed: None,
        })
    }

    /// Panic on invalid config (MustNew-style behavior).
    pub fn must_new(conf: RestConf) -> Self {
        match Self::new(conf) {
            Ok(engine) => engine,
            Err(err) => panic!("invalid RestConf: {err}"),
        }
    }

    /// Add multiple routes.
    /// Add multiple routes.
    pub fn add_routes<I>(&mut self, routes: I)
    where
        I: IntoIterator<Item = Route>,
    {
        self.routes.extend(routes);
    }

    /// Add single route.
    /// Add a single route.
    pub fn add_route(&mut self, route: Route) {
        self.routes.push(route);
    }

    /// Register global middleware.
    /// Register global middleware.
    pub fn use_middleware(&mut self, middleware: Middleware) {
        self.middlewares.push(middleware);
    }

    /// Set custom 404 handler.
    /// Set custom 404 handler.
    pub fn set_not_found_handler(&mut self, handler: impl IntoHandler) {
        self.not_found = Some(handler.into_handler());
    }

    /// Set custom 405 handler.
    /// Set custom 405 handler.
    pub fn set_not_allowed_handler(&mut self, handler: impl IntoHandler) {
        self.not_allowed = Some(handler.into_handler());
    }

    /// Start server with collected routes/middlewares.
    /// Start server with collected routes and middlewares.
    pub async fn start(self) -> anyhow::Result<ServerHandle> {
        let mut server = Server::new(self.conf.clone());

        if let Some(h) = self.not_found.clone() {
            server.set_not_found_handler(h);
        }
        if let Some(h) = self.not_allowed.clone() {
            server.set_not_allowed_handler(h);
        }
        for mw in &self.middlewares {
            server.use_middleware(mw.clone());
        }
        server.add_routes(self.routes.clone());
        server.start().await
    }

    /// Print registered routes (method path).
    /// Print registered routes (method + path).
    pub fn print_routes(&self) {
        let mut routes: Vec<String> = self
            .routes
            .iter()
            .map(|r| format!("{} {}", r.method, r.path))
            .collect();
        routes.sort();
        println!("Routes:");
        for r in routes {
            println!("  {r}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::Method;
    use hyper::{Body, Response};

    fn ok_route(path: &str) -> Route {
        Route::new(Method::GET, path, |_req: http::Request<Body>| async {
            Response::new(Body::from("ok"))
        })
    }

    #[test]
    fn add_routes_should_collect() {
        let mut eng = Engine::must_new(RestConf::default());
        eng.add_route(ok_route("/a"));
        eng.add_routes(vec![ok_route("/b"), ok_route("/c")]);
        assert_eq!(eng.routes.len(), 3);
    }
}
