use crate::http::response::build_response;
use crate::middleware::{Middleware, middleware};
use hyper::Body;

/// Enforce max request body bytes; 413 if exceeded. Re-inserts body for downstream.
pub fn max_bytes(limit: u64) -> Middleware {
    middleware(move |mut req, next| {
        let limit = limit as usize;
        async move {
            if let Some(len) = req.headers().get(http::header::CONTENT_LENGTH)
                && let Some(len) = len.to_str().ok().and_then(|s| s.parse::<usize>().ok())
                && len > limit
            {
                return build_response(
                    http::StatusCode::PAYLOAD_TOO_LARGE,
                    Body::from("request body too large"),
                );
            }
            let bytes = match hyper::body::to_bytes(req.body_mut()).await {
                Ok(b) => b,
                Err(e) => {
                    return build_response(
                        http::StatusCode::BAD_REQUEST,
                        Body::from(format!("read body error: {e}")),
                    );
                }
            };
            if bytes.len() > limit {
                return build_response(
                    http::StatusCode::PAYLOAD_TOO_LARGE,
                    Body::from("request body too large"),
                );
            }
            *req.body_mut() = Body::from(bytes);
            next.call(req).await
        }
    })
}
