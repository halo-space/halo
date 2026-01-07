use crate::http::response::build_response;
use crate::middleware::{Middleware, middleware};
use hyper::Body;
use std::time::Duration;

/// Per-request timeout; on timeout return 504.
pub fn timeout(duration: Duration) -> Middleware {
    middleware(move |req, next| async move {
        match tokio::time::timeout(duration, next.call(req)).await {
            Ok(resp) => resp,
            Err(_) => build_response(
                http::StatusCode::GATEWAY_TIMEOUT,
                Body::from("request timeout"),
            ),
        }
    })
}
