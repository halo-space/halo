//! Router module entry: routing descriptions (Route) and runtime router.

use crate::http::HandlerFunc;
use crate::middleware::{IntoHandler, Middleware};
use http::Method;

pub mod params;
pub mod router;

pub use params::PathParams;
pub use router::Router;

/// Single route definition.
#[derive(Clone)]
pub struct Route {
    pub method: Method,
    pub path: String,
    pub handler: HandlerFunc,
}

impl Route {
    /// Build a route and convert async function into internal HandlerFunc.
    pub fn new(method: Method, path: impl Into<String>, handler: impl IntoHandler) -> Self {
        Self {
            method,
            path: normalize_path(path.into()),
            handler: handler.into_handler(),
        }
    }

    /// Apply middlewares to this route (internal use).
    pub(crate) fn with_middlewares(self, middlewares: &[Middleware]) -> Self {
        let handler = crate::middleware::apply_middlewares(self.handler.clone(), middlewares);
        Self { handler, ..self }
    }
}

/// Join prefix with routes.
pub fn with_prefix(prefix: &str, routes: Vec<Route>) -> Vec<Route> {
    let normalized_prefix = normalize_path(prefix.to_string());
    routes
        .into_iter()
        .map(|mut route| {
            route.path = join_path(&normalized_prefix, &route.path);
            route
        })
        .collect()
}

/// Alias of `with_prefix` for root prefix.
pub fn with_root(root: &str, routes: Vec<Route>) -> Vec<Route> {
    with_prefix(root, routes)
}

/// Apply middlewares to a list of routes (keeps original order).
pub fn with_handlers(middlewares: Vec<Middleware>, routes: Vec<Route>) -> Vec<Route> {
    crate::middleware::with_middlewares(middlewares, routes)
}

fn join_path(prefix: &str, path: &str) -> String {
    let mut result = String::new();
    if !prefix.starts_with('/') {
        result.push('/');
    }
    result.push_str(prefix.trim_end_matches('/'));
    if !result.ends_with('/') {
        result.push('/');
    }
    result.push_str(path.trim_start_matches('/'));
    normalize_path(result)
}

/// Normalize path by collapsing slashes and ensuring leading slash.
fn normalize_path(path: String) -> String {
    if path.is_empty() {
        return "/".to_string();
    }
    let mut cleaned = path.replace("//", "/");
    if !cleaned.starts_with('/') {
        cleaned.insert(0, '/');
    }
    if cleaned.len() > 1 && cleaned.ends_with('/') {
        cleaned.pop();
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::Body;
    use tokio::runtime::Runtime;

    fn runtime() -> Runtime {
        Runtime::new().unwrap()
    }

    #[test]
    fn normalize_path_should_strip_extra_slash() {
        assert_eq!(normalize_path("//api//v1/".to_string()), "/api/v1");
        assert_eq!(normalize_path("api/v1".to_string()), "/api/v1");
        assert_eq!(normalize_path("/".to_string()), "/");
    }

    #[test]
    fn with_root_should_alias_prefix() {
        let routes = vec![Route::new(http::Method::GET, "/list", handler())];
        let prefixed = with_root("/api", routes);
        assert_eq!(prefixed[0].path, "/api/list");
    }

    #[test]
    fn with_prefix_should_join_correctly() {
        let routes = vec![Route::new(http::Method::GET, "/list", handler())];
        let prefixed = with_prefix("/api/v1", routes);
        assert_eq!(prefixed[0].path, "/api/v1/list");
    }

    #[test]
    fn with_middlewares_should_wrap() {
        let route = Route::new(http::Method::POST, "/test", handler());
        let mw = crate::middleware::middleware(|req, next| async move {
            let mut resp = next.call(req).await;
            resp.headers_mut().insert("X-MW", "1".parse().unwrap());
            resp
        });
        let wrapped = route.with_middlewares(&[mw]);
        let resp = runtime().block_on(
            wrapped.handler.call(
                http::Request::builder()
                    .method(http::Method::POST)
                    .uri("/test")
                    .body(Body::empty())
                    .unwrap(),
            ),
        );
        assert_eq!(resp.headers().get("X-MW").unwrap().to_str().unwrap(), "1");
    }

    fn handler() -> impl IntoHandler {
        |_: http::Request<Body>| async {
            http::Response::builder()
                .status(http::StatusCode::OK)
                .body(Body::empty())
                .unwrap()
        }
    }
}
