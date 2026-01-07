use http::{Response, StatusCode};
use hyper::Body;
use serde::Serialize;

/// 200 OK with custom body.
pub fn ok(body: impl Into<Body>) -> Response<Body> {
    build_response(StatusCode::OK, body)
}

/// 200 OK with JSON body.
pub fn ok_json<T: Serialize>(val: &T) -> anyhow::Result<Response<Body>> {
    let body = serde_json::to_vec(val)?;
    let mut builder = Response::builder();
    builder = builder.status(StatusCode::OK);
    builder = builder.header(http::header::CONTENT_TYPE, "application/json");
    Ok(match builder.body(Body::from(body)) {
        Ok(resp) => resp,
        Err(err) => fallback_response(err),
    })
}

/// 400 Bad Request with text body.
pub fn bad_request(msg: impl Into<Body>) -> Response<Body> {
    build_response(StatusCode::BAD_REQUEST, msg)
}

/// Error response with custom status and message.
pub fn error(status: StatusCode, msg: impl Into<Body>) -> Response<Body> {
    build_response(status, msg)
}

pub(crate) fn build_response(status: StatusCode, body: impl Into<Body>) -> Response<Body> {
    match Response::builder().status(status).body(body.into()) {
        Ok(resp) => resp,
        Err(err) => fallback_response(err),
    }
}

fn fallback_response(err: http::Error) -> Response<Body> {
    let mut resp = Response::new(Body::from(format!("build response failed: {err}")));
    *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_should_set_status() {
        let resp = ok("hi");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn ok_json_should_serialize() {
        let resp = ok_json(&serde_json::json!({"a":1})).unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
    }

    #[test]
    fn bad_request_should_set_400() {
        let resp = bad_request("oops");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn error_should_set_status() {
        let resp = error(StatusCode::UNAUTHORIZED, "nope");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
