use crate::middleware::{Middleware, middleware};
use http::StatusCode;
use hyper::Body;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

// Reserved JWT claim keys kept for reference; currently not used.
#[allow(dead_code)]
const JWT_AUDIENCE: &str = "aud";
#[allow(dead_code)]
const JWT_EXPIRE: &str = "exp";
#[allow(dead_code)]
const JWT_ID: &str = "jti";
#[allow(dead_code)]
const JWT_ISSUED_AT: &str = "iat";
#[allow(dead_code)]
const JWT_ISSUER: &str = "iss";
#[allow(dead_code)]
const JWT_NOT_BEFORE: &str = "nbf";
#[allow(dead_code)]
const JWT_SUBJECT: &str = "sub";

/// Unauthorized callback signature.
pub type UnauthorizedCallback =
    Arc<dyn Fn(&mut http::Response<Body>, &http::Request<Body>, &anyhow::Error) + Send + Sync>;

/// Options for authorize middleware.
#[derive(Clone, Default)]
pub struct AuthorizeOptions {
    pub prev_secret: Option<String>,
    pub callback: Option<UnauthorizedCallback>,
}

/// Enable previous secret for token transition.
pub fn with_prev_secret(secret: impl Into<String>) -> impl Fn(&mut AuthorizeOptions) {
    let s = secret.into();
    move |opts: &mut AuthorizeOptions| {
        opts.prev_secret = Some(s.clone());
    }
}

/// Set unauthorized callback.
pub fn with_unauthorized_callback(
    callback: UnauthorizedCallback,
) -> impl Fn(&mut AuthorizeOptions) {
    move |opts: &mut AuthorizeOptions| {
        opts.callback = Some(callback.clone());
    }
}

/// Authorize middleware: validates JWT Bearer token with secret/prev_secret.
pub fn authorize(
    secret: impl Into<String>,
    opts: impl IntoIterator<Item = impl Fn(&mut AuthorizeOptions)>,
) -> Middleware {
    let mut options = AuthorizeOptions::default();
    for opt in opts {
        opt(&mut options);
    }
    let secret = secret.into();
    let prev_secret = options.prev_secret.clone();
    let callback = options.callback.clone();

    middleware(move |mut req: http::Request<Body>, next| {
        let secret = secret.clone();
        let prev_secret = prev_secret.clone();
        let callback = callback.clone();
        async move {
            match validate_token(&mut req, &secret, prev_secret.as_deref()) {
                Ok(Some(claims)) => {
                    req.extensions_mut().insert(claims);
                    next.call(req).await
                }
                Ok(None) => {
                    unauthorized_response(anyhow::anyhow!("missing bearer token"), callback, &req)
                }
                Err(e) => unauthorized_response(e, callback, &req),
            }
        }
    })
}

/// Extracted custom claims stored in request extensions.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthClaims {
    pub claims: HashMap<String, Value>,
}

impl AuthClaims {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.claims.get(key)
    }
}

#[derive(Debug, Deserialize)]
struct RawClaims(HashMap<String, Value>);

fn validate_token(
    req: &mut http::Request<Body>,
    secret: &str,
    prev_secret: Option<&str>,
) -> anyhow::Result<Option<AuthClaims>> {
    let token = extract_bearer(req)?;
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_aud = false;
    validation.validate_exp = true;
    let decode_with = |sec: &str| {
        decode::<RawClaims>(
            &token,
            &DecodingKey::from_secret(sec.as_bytes()),
            &validation,
        )
    };

    let decoded = decode_with(secret).or_else(|e| {
        if let Some(prev) = prev_secret {
            decode_with(prev).map_err(|_| e)
        } else {
            Err(e)
        }
    })?;

    let claims = decoded.claims.0;
    Ok(Some(AuthClaims { claims }))
}

fn extract_bearer(req: &http::Request<Body>) -> anyhow::Result<String> {
    let header = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .ok_or_else(|| anyhow::anyhow!("missing Authorization header"))?
        .to_str()
        .map_err(|_| anyhow::anyhow!("invalid Authorization header"))?;
    let parts: Vec<&str> = header.split_whitespace().collect();
    if parts.len() != 2 || parts[0] != "Bearer" {
        anyhow::bail!("invalid bearer token");
    }
    Ok(parts[1].to_string())
}

fn unauthorized_response(
    err: anyhow::Error,
    callback: Option<UnauthorizedCallback>,
    req: &http::Request<Body>,
) -> http::Response<Body> {
    let mut resp = http::Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(Body::from(err.to_string()))
        .unwrap();
    if let Some(cb) = callback {
        cb(&mut resp, req, &err);
    }
    resp
}
