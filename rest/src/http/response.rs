use http::{Response, StatusCode};
use hyper::Body;
use serde::Serialize;

/// 200 OK with custom body.
pub fn ok(body: impl Into<Body>) -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .body(body.into())
        .unwrap()
}

/// 200 OK with JSON body.
pub fn ok_json<T: Serialize>(val: &T) -> anyhow::Result<Response<Body>> {
    let body = serde_json::to_vec(val)?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))?)
}

/// 400 Bad Request with text body.
pub fn bad_request(msg: impl Into<Body>) -> Response<Body> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(msg.into())
        .unwrap()
}

/// Error response with custom status and message.
pub fn error(status: StatusCode, msg: impl Into<Body>) -> Response<Body> {
    Response::builder().status(status).body(msg.into()).unwrap()
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
