use crate::http::HandlerFunc;
use crate::router::{Route, Router};
use futures::future::BoxFuture;
use http::Method;
use hyper::Body;

/// Router interface abstraction.
pub trait HttpRouter {
    fn handle(&mut self, method: Method, path: &str, handler: HandlerFunc) -> anyhow::Result<()>;
    fn set_not_found_handler(&mut self, handler: HandlerFunc);
    fn set_not_allowed_handler(&mut self, handler: HandlerFunc);
    fn dispatch(&self, req: http::Request<Body>) -> BoxFuture<'static, http::Response<Body>>;
}

impl HttpRouter for Router {
    fn handle(&mut self, method: Method, path: &str, handler: HandlerFunc) -> anyhow::Result<()> {
        self.add_route(Route::new(method, path.to_string(), handler))
    }

    fn set_not_found_handler(&mut self, handler: HandlerFunc) {
        self.set_not_found_handler(handler);
    }

    fn set_not_allowed_handler(&mut self, handler: HandlerFunc) {
        self.set_not_allowed_handler(handler);
    }

    fn dispatch(&self, req: http::Request<Body>) -> BoxFuture<'static, http::Response<Body>> {
        let inner = self.clone();
        Box::pin(async move { inner.dispatch(req).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::handler;
    use http::{Method, StatusCode};
    use hyper::Response;
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
    fn handle_and_dispatch_should_work() {
        runtime().block_on(async {
            let mut router = Router::new();
            router.handle(Method::GET, "/ping", ok_handler()).unwrap();
            let resp = router
                .dispatch(
                    http::Request::builder()
                        .method(Method::GET)
                        .uri("/ping")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await;
            assert_eq!(resp.status(), StatusCode::OK);
        });
    }

    #[test]
    fn custom_not_found_should_fire() {
        runtime().block_on(async {
            let mut router = Router::new();
            router.set_not_found_handler(handler(|_req: http::Request<Body>| async {
                Response::builder()
                    .status(StatusCode::IM_A_TEAPOT)
                    .body(Body::empty())
                    .unwrap()
            }));
            let resp = router
                .dispatch(
                    http::Request::builder()
                        .method(Method::GET)
                        .uri("/missing")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await;
            assert_eq!(resp.status(), StatusCode::IM_A_TEAPOT);
        });
    }
}
