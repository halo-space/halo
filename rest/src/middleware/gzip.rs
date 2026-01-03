use crate::middleware::{Middleware, middleware};
use hyper::Body;
use std::io::Write;

/// Combined gzip middleware: gunzip request if needed; gzip response if client accepts.
pub fn gzip() -> Middleware {
    middleware(|mut req, next| async move {
        // gunzip request
        let enc = req
            .headers()
            .get(http::header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        if enc == "gzip" {
            let bytes = match hyper::body::to_bytes(req.body_mut()).await {
                Ok(b) => b,
                Err(e) => {
                    return http::Response::builder()
                        .status(http::StatusCode::BAD_REQUEST)
                        .body(Body::from(format!("read body error: {e}")))
                        .unwrap();
                }
            };
            let mut decoder = flate2::read::GzDecoder::new(bytes.as_ref());
            let mut decoded = Vec::new();
            if let Err(e) = std::io::Read::read_to_end(&mut decoder, &mut decoded) {
                return http::Response::builder()
                    .status(http::StatusCode::BAD_REQUEST)
                    .body(Body::from(format!("gunzip error: {e}")))
                    .unwrap();
            }
            *req.body_mut() = Body::from(decoded);
            req.headers_mut().remove(http::header::CONTENT_ENCODING);
        }

        // downstream
        let resp = next.call(req).await;

        // gzip response if eligible
        if resp.headers().contains_key(http::header::CONTENT_ENCODING) {
            return resp;
        }
        let status = resp.status();
        if status.is_informational()
            || status == http::StatusCode::NO_CONTENT
            || status == http::StatusCode::NOT_MODIFIED
        {
            return resp;
        }
        let accept = resp
            .headers()
            .get(http::header::ACCEPT_ENCODING)
            .and_then(|v| v.to_str().ok());
        let accept = accept.unwrap_or("");
        if !accept.to_ascii_lowercase().contains("gzip") {
            return resp;
        }

        let (parts, body) = resp.into_parts();
        let bytes = match hyper::body::to_bytes(body).await {
            Ok(b) => b,
            Err(e) => {
                return http::Response::builder()
                    .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from(format!("compress read body error: {e}")))
                    .unwrap();
            }
        };
        if bytes.is_empty() {
            return http::Response::from_parts(parts, Body::from(bytes));
        }

        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        if let Err(e) = encoder.write_all(&bytes) {
            return http::Response::builder()
                .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("gzip error: {e}")))
                .unwrap();
        }
        let compressed = match encoder.finish() {
            Ok(v) => v,
            Err(e) => {
                return http::Response::builder()
                    .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from(format!("gzip finish error: {e}")))
                    .unwrap();
            }
        };

        let mut resp = http::Response::from_parts(parts, Body::from(compressed));
        resp.headers_mut().insert(
            http::header::CONTENT_ENCODING,
            http::HeaderValue::from_static("gzip"),
        );
        resp.headers_mut().insert(
            http::header::VARY,
            http::HeaderValue::from_static("Accept-Encoding"),
        );
        resp.headers_mut().remove(http::header::CONTENT_LENGTH);

        resp
    })
}
