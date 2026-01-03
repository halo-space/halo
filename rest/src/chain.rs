use crate::http::HandlerFunc;
use crate::middleware::{Middleware, apply_middlewares, handler};

/// Chain builder for middlewares.
#[derive(Clone, Default)]
pub struct Chain {
    mws: Vec<Middleware>,
}

impl Chain {
    /// Create empty chain.
    /// Create an empty chain.
    pub fn new() -> Self {
        Self { mws: Vec::new() }
    }

    /// Append middleware to chain.
    /// Append a middleware.
    pub fn append(mut self, mw: Middleware) -> Self {
        self.mws.push(mw);
        self
    }

    /// Append middleware mutably.
    /// Append by mutable reference.
    pub fn append_mut(&mut self, mw: Middleware) {
        self.mws.push(mw);
    }

    /// Wrap handler with chain middlewares.
    pub fn then(self, handler: HandlerFunc) -> HandlerFunc {
        apply_middlewares(handler, &self.mws)
    }

    /// Wrap async fn -> HandlerFunc and return HandlerFunc.
    pub fn then_func<F, Fut>(self, f: F) -> HandlerFunc
    where
        F: Fn(http::Request<hyper::Body>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = http::Response<hyper::Body>> + Send + 'static,
    {
        self.then(handler(f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{Method, StatusCode};
    use hyper::{Body, Response};
    use tokio::runtime::Runtime;

    fn runtime() -> Runtime {
        Runtime::new().unwrap()
    }

    fn ok_handler() -> HandlerFunc {
        handler(|_req: http::Request<Body>| async {
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap()
        })
    }

    #[test]
    fn chain_should_apply_middlewares_in_order() {
        runtime().block_on(async {
            let chain =
                Chain::new().append(crate::middleware::middleware(|req, next| async move {
                    let mut resp = next.call(req).await;
                    resp.headers_mut().append("X-C", "a".parse().unwrap());
                    resp
                }));
            let h = chain.then(ok_handler());
            let resp = h
                .call(
                    http::Request::builder()
                        .method(Method::GET)
                        .uri("/")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await;
            assert_eq!(resp.headers().get("X-C").unwrap().to_str().unwrap(), "a");
        });
    }
}
