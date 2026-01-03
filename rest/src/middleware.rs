pub mod auth;
pub mod gzip;
pub mod limit;
pub mod rate;
pub mod timeout;

pub use crate::http::types::{BoxResponseFuture, HandlerFunc};
use crate::router::Route;
pub use gzip::gzip;
use http::{Request, Response};
use hyper::Body;
pub use limit::max_bytes;
pub use rate::{concurrency_limit, rate_limit};
use std::future::Future;
use std::slice;
use std::sync::Arc;
pub use timeout::timeout;

/// Middleware: `(req, next) -> Response`.
pub type Middleware = Arc<dyn Fn(Request<Body>, HandlerFunc) -> BoxResponseFuture + Send + Sync>;

/// Convert closures into `HandlerFunc` (one-time boxing).
pub trait IntoHandler {
    fn into_handler(self) -> HandlerFunc;
}

impl<F, Fut> IntoHandler for F
where
    F: Fn(Request<Body>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Response<Body>> + Send + 'static,
{
    fn into_handler(self) -> HandlerFunc {
        HandlerFunc::new(self)
    }
}

impl IntoHandler for HandlerFunc {
    fn into_handler(self) -> HandlerFunc {
        self
    }
}

/// Convert `(req) -> async Response` into `HandlerFunc` (one-time boxing).
pub fn handler<F, Fut>(f: F) -> HandlerFunc
where
    F: Fn(Request<Body>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Response<Body>> + Send + 'static,
{
    HandlerFunc::new(f)
}

/// Convert `(req, next)` into `Middleware`.
pub fn middleware<F, Fut>(f: F) -> Middleware
where
    F: Fn(Request<Body>, HandlerFunc) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Response<Body>> + Send + 'static,
{
    let f = Arc::new(f);
    Arc::new(move |req, next| {
        let f = f.clone();
        let next = next.clone();
        Box::pin(async move { f(req, next).await })
    })
}

/// Alias for shorter writing.
pub fn mw<F, Fut>(f: F) -> Middleware
where
    F: Fn(Request<Body>, HandlerFunc) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Response<Body>> + Send + 'static,
{
    middleware(f)
}

/// Apply middlewares once at registration time, keeping user-facing order.
pub fn apply_middlewares(handler: HandlerFunc, middlewares: &[Middleware]) -> HandlerFunc {
    middlewares.iter().rev().fold(handler, |next, mw| {
        let mw = mw.clone();
        HandlerFunc::new(move |req| {
            let mw = mw.clone();
            let next = next.clone();
            Box::pin(async move { mw(req, next).await })
        })
    })
}

/// Apply single middleware to routes.
pub fn with_middleware<R>(middleware: Middleware, routes: R) -> Vec<Route>
where
    R: IntoIterator<Item = Route>,
{
    routes
        .into_iter()
        .map(|route| {
            let handler = apply_middlewares(route.handler.clone(), slice::from_ref(&middleware));
            Route { handler, ..route }
        })
        .collect()
}

/// Apply multiple middlewares to routes.
pub fn with_middlewares<R, I>(middlewares: I, routes: R) -> Vec<Route>
where
    I: IntoIterator<Item = Middleware>,
    R: IntoIterator<Item = Route>,
{
    let collected: Vec<Middleware> = middlewares.into_iter().collect();
    routes
        .into_iter()
        .map(|route| {
            let handler = apply_middlewares(route.handler.clone(), &collected);
            Route { handler, ..route }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{Method, StatusCode};
    use tokio::runtime::Runtime;

    fn runtime() -> Runtime {
        Runtime::new().unwrap()
    }

    fn ok_handler() -> impl IntoHandler {
        |_req: Request<Body>| async {
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap()
        }
    }

    #[test]
    fn apply_middlewares_should_follow_order() {
        let h = IntoHandler::into_handler(ok_handler());
        let m1 = middleware(|req, next| async move {
            let mut resp = next.call(req).await;
            resp.headers_mut().append("X-Order", "m1".parse().unwrap());
            resp
        });
        let m2 = middleware(|req, next| async move {
            let mut resp = next.call(req).await;
            resp.headers_mut().append("X-Order", "m2".parse().unwrap());
            resp
        });

        let wrapped = apply_middlewares(h, &[m1, m2]);
        let resp = runtime().block_on(
            wrapped.call(
                Request::builder()
                    .method(Method::GET)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            ),
        );

        let header = resp.headers().get_all("X-Order");
        let mut vals = header.iter().map(|v| v.to_str().unwrap().to_string());
        assert_eq!(vals.next().unwrap(), "m2");
        assert_eq!(vals.next().unwrap(), "m1");
    }

    #[test]
    fn with_middleware_should_wrap_handlers() {
        let routes = vec![Route::new(Method::GET, "/", ok_handler())];
        let wrapped = with_middleware(
            middleware(|req, next| async move {
                let mut resp = next.call(req).await;
                resp.headers_mut().insert("X-Test", "1".parse().unwrap());
                resp
            }),
            routes,
        );

        assert_eq!(wrapped.len(), 1);
        let handler = wrapped[0].handler.clone();
        let resp = runtime().block_on(
            handler.call(
                Request::builder()
                    .method(Method::GET)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            ),
        );
        assert_eq!(resp.headers().get("X-Test").unwrap().to_str().unwrap(), "1");
    }

    #[test]
    fn middleware_fn_should_allow_short_circuit() {
        let routes = vec![Route::new(Method::GET, "/", ok_handler())];
        let wrapped = with_middleware(
            middleware(|_req, _next| async {
                Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(Body::empty())
                    .unwrap()
            }),
            routes,
        );

        let handler = wrapped[0].handler.clone();
        let resp = runtime().block_on(
            handler.call(
                Request::builder()
                    .method(Method::GET)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            ),
        );
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn with_middlewares_should_follow_go_zero_order() {
        let routes = vec![Route::new(Method::POST, "/test", ok_handler())];
        let wrapped = with_middlewares(
            vec![
                middleware(|req, next| async move {
                    let mut resp = next.call(req).await;
                    resp.headers_mut().append("X-Seq", "a".parse().unwrap());
                    resp
                }),
                middleware(|req, next| async move {
                    let mut resp = next.call(req).await;
                    resp.headers_mut().append("X-Seq", "b".parse().unwrap());
                    resp
                }),
            ],
            routes,
        );

        let handler = wrapped[0].handler.clone();
        let resp = runtime().block_on(
            handler.call(
                Request::builder()
                    .method(Method::POST)
                    .uri("/test")
                    .body(Body::empty())
                    .unwrap(),
            ),
        );
        let mut vals = resp
            .headers()
            .get_all("X-Seq")
            .iter()
            .map(|v| v.to_str().unwrap().to_string());
        assert_eq!(vals.next().unwrap(), "b");
        assert_eq!(vals.next().unwrap(), "a");
    }
}
