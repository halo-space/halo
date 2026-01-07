use crate::http::HandlerFunc;
use crate::http::response::build_response;
use crate::middleware::IntoHandler;
use crate::router::{Route, with_prefix};
use anyhow::Context;
use http::{Method, Response, StatusCode};
use hyper::Body;
use matchit::Router as MatchRouter;
use std::collections::HashMap;

/// Router for HTTP requests, bucketing by method and matching via `matchit`.
/// HTTP router with method buckets and matchit path matching.
#[derive(Clone)]
pub struct Router {
    trees: HashMap<Method, MatchRouter<HandlerFunc>>,
    not_found: HandlerFunc,
    not_allowed: HandlerFunc,
    routes: Vec<Route>,
}

impl Router {
    /// Create an empty router with default 404/405 handlers.
    /// Create empty router with default 404/405 handlers.
    pub fn new() -> Self {
        Self {
            trees: HashMap::new(),
            not_found: default_not_found(),
            not_allowed: default_not_allowed(),
            routes: Vec::new(),
        }
    }

    /// Set custom 404 handler.
    /// Set custom 404 handler.
    #[allow(dead_code)]
    pub fn with_not_found(mut self, handler: impl IntoHandler) -> Self {
        self.not_found = handler.into_handler();
        self
    }

    /// Set custom 405 handler.
    /// Set custom 405 handler.
    #[allow(dead_code)]
    pub fn with_not_allowed(mut self, handler: impl IntoHandler) -> Self {
        self.not_allowed = handler.into_handler();
        self
    }

    /// Set custom 404 handler (mutable setter).
    /// Set custom 404 handler (mutable ref).
    pub fn set_not_found_handler(&mut self, handler: impl IntoHandler) {
        self.not_found = handler.into_handler();
    }

    /// Set custom 405 handler (mutable setter).
    /// Set custom 405 handler (mutable ref).
    pub fn set_not_allowed_handler(&mut self, handler: impl IntoHandler) {
        self.not_allowed = handler.into_handler();
    }

    /// Register a single route.
    /// Register a single route.
    #[allow(dead_code)]
    pub fn route(
        mut self,
        method: Method,
        path: impl Into<String>,
        handler: impl IntoHandler,
    ) -> anyhow::Result<Self> {
        self.add_route(Route::new(method, path, handler))?;
        Ok(self)
    }

    pub fn add_route(&mut self, route: Route) -> anyhow::Result<()> {
        validate_path(&route.path)?;
        let match_path = to_matchit_path(&route.path)?;
        self.routes.push(route.clone());
        let tree = self.trees.entry(route.method.clone()).or_default();
        tree.insert(match_path, route.handler.clone())
            .with_context(|| format!("register route {} {}", route.method, route.path))?;
        Ok(())
    }

    /// Register multiple routes.
    /// Register multiple routes.
    #[allow(dead_code)]
    pub fn add_routes<I>(&mut self, routes: I) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = Route>,
    {
        for route in routes {
            self.add_route(route)?;
        }
        Ok(())
    }

    /// Merge another router (axum-style `merge`).
    /// Merge another set of routes (similar to axum `merge`).
    #[allow(dead_code)]
    pub fn merge(mut self, other: Router) -> anyhow::Result<Self> {
        for route in other.routes {
            validate_path(&route.path)?;
            self.add_route(route)?;
        }
        Ok(self)
    }

    /// Nest a router under a path prefix (axum-style `nest`).
    /// Nest routes by prefix (similar to axum `nest`).
    #[allow(dead_code)]
    pub fn nest(mut self, prefix: &str, router: Router) -> anyhow::Result<Self> {
        validate_path(prefix)?;
        for route in router.routes {
            let routes = with_prefix(prefix, vec![route]);
            self.add_routes(routes)?;
        }
        Ok(self)
    }

    /// Dispatch an incoming request to the matched handler.
    /// Dispatch request to the matching handler.
    pub async fn dispatch(&self, mut req: http::Request<Body>) -> Response<Body> {
        let method = req.method().clone();
        let path = req.uri().path().to_string();

        if let Some(tree) = self.trees.get(&method)
            && let Ok(matched) = tree.at(&path)
        {
            if !matched.params.is_empty() {
                let mut params = HashMap::new();
                for (k, v) in matched.params.iter() {
                    params.insert(k.to_string(), v.to_string());
                }
                req.extensions_mut()
                    .insert(crate::router::params::PathParams { params });
            }
            return matched.value.clone().call(req).await;
        }

        if let Some(allows) = self.allowed_methods(&path, &method) {
            let mut resp = self.not_allowed.clone().call(req).await;
            if let Ok(value) = allows.parse() {
                resp.headers_mut().insert(http::header::ALLOW, value);
            }
            return resp;
        }

        self.not_found.clone().call(req).await
    }

    fn allowed_methods(&self, path: &str, current: &Method) -> Option<String> {
        let mut allows = Vec::new();
        for (method, tree) in self.trees.iter() {
            if method == current {
                continue;
            }
            if tree.at(path).is_ok() {
                allows.push(method.to_string());
            }
        }
        if allows.is_empty() {
            None
        } else {
            Some(allows.join(", "))
        }
    }
}

fn default_not_found() -> HandlerFunc {
    IntoHandler::into_handler(|_req: http::Request<Body>| async {
        build_response(StatusCode::NOT_FOUND, Body::empty())
    })
}

fn default_not_allowed() -> HandlerFunc {
    IntoHandler::into_handler(|_req: http::Request<Body>| async {
        build_response(StatusCode::METHOD_NOT_ALLOWED, Body::empty())
    })
}

fn validate_path(path: &str) -> anyhow::Result<()> {
    if path.is_empty() || !path.starts_with('/') {
        anyhow::bail!("path must start with '/'");
    }
    Ok(())
}

/// Convert `{id}` / `{*rest}` to matchit syntax `:id` / `*rest`.
/// Convert `{id}` / `{*rest}` to matchit `:id` / `*rest`.
fn to_matchit_path(path: &str) -> anyhow::Result<String> {
    validate_path(path)?;
    let mut out = String::new();
    for (idx, seg) in path.split('/').enumerate() {
        if idx == 0 {
            // leading empty segment before first '/'
            out.push('/');
            continue;
        }
        if seg.is_empty() {
            continue;
        }
        if seg.starts_with('{') && seg.ends_with('}') {
            let inner = &seg[1..seg.len() - 1];
            if inner.starts_with('*') {
                if idx != path.split('/').count() - 1 {
                    anyhow::bail!("wildcard must be the last segment: {path}");
                }
                let name = inner.trim_start_matches('*');
                if name.is_empty() {
                    anyhow::bail!("wildcard name cannot be empty: {path}");
                }
                out.push('*');
                out.push_str(name);
            } else {
                if inner.is_empty() {
                    anyhow::bail!("param name cannot be empty: {path}");
                }
                out.push(':');
                out.push_str(inner);
            }
        } else {
            out.push_str(seg);
        }
        out.push('/');
    }
    if out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{Method, StatusCode};
    use tokio::runtime::Runtime;

    fn runtime() -> Runtime {
        Runtime::new().unwrap()
    }

    async fn dispatch(
        router: &Router,
        method: Method,
        path: &str,
    ) -> (
        StatusCode,
        http::HeaderMap,
        Option<crate::router::params::PathParams>,
    ) {
        let req = http::Request::builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .unwrap();
        let resp = router.dispatch(req).await;
        let params = resp
            .extensions()
            .get::<crate::router::params::PathParams>()
            .cloned();
        (resp.status(), resp.headers().clone(), params)
    }

    fn ok_handler() -> HandlerFunc {
        IntoHandler::into_handler(|req: http::Request<Body>| async move {
            let mut resp = Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap();
            if let Some(params) = req.extensions().get::<crate::router::params::PathParams>() {
                resp.extensions_mut().insert(params.clone());
            }
            resp
        })
    }

    #[test]
    fn should_match_and_capture_params() {
        runtime().block_on(async {
            let mut router = Router::new();
            router
                .add_route(Route::new(Method::GET, "/user/:id", ok_handler()))
                .unwrap();

            let (status, _headers, params) = dispatch(&router, Method::GET, "/user/42").await;
            assert_eq!(status, StatusCode::OK);
            let params = params.expect("params must exist");
            assert_eq!(params.params.get("id").unwrap(), "42");
        });
    }

    #[test]
    fn should_return_405_and_allow_header() {
        runtime().block_on(async {
            let mut router = Router::new();
            router
                .add_route(Route::new(Method::GET, "/ping", ok_handler()))
                .unwrap();

            let (status, headers, _params) = dispatch(&router, Method::POST, "/ping").await;
            assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
            assert_eq!(
                headers.get(http::header::ALLOW).unwrap().to_str().unwrap(),
                "GET"
            );
        });
    }

    #[test]
    fn should_return_404_when_not_found() {
        runtime().block_on(async {
            let router = Router::new();
            let (status, _, _) = dispatch(&router, Method::GET, "/missing").await;
            assert_eq!(status, StatusCode::NOT_FOUND);
        });
    }
}
