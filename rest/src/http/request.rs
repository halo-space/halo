use anyhow::Context;
use hyper::{Body, Request};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;

const MAX_FORM_PARAM_COUNT: usize = 2048;
const MAX_MEMORY: u64 = 32 << 20; // 32MB cap kept for reference
const MAX_BODY_LEN: usize = 8 << 20; // 8MB

/// Read JSON body into `T` and keep body available by re-inserting bytes.
/// Read JSON body as `T`, and put bytes back so the body stays readable.
pub async fn read_json<T: DeserializeOwned>(req: &mut Request<Body>) -> anyhow::Result<T> {
    let bytes = hyper::body::to_bytes(req.body_mut())
        .await
        .context("read request body")?;
    if bytes.len() > MAX_BODY_LEN {
        anyhow::bail!(
            "body too large: {} bytes (limit {})",
            bytes.len(),
            MAX_BODY_LEN
        );
    }
    let val: T = serde_json::from_slice(&bytes).context("deserialize json body")?;
    *req.body_mut() = Body::from(bytes);
    Ok(val)
}

/// Parse `application/json` body with size limit, return `None` if not json.
/// Parse JSON body with size limit; return None if not JSON.
pub async fn parse_json_body<T: DeserializeOwned>(
    req: &mut Request<Body>,
) -> anyhow::Result<Option<T>> {
    if !with_json_body(req) {
        return Ok(None);
    }
    let bytes = hyper::body::to_bytes(req.body_mut())
        .await
        .context("read request body")?;
    if bytes.len() > MAX_BODY_LEN {
        anyhow::bail!("request body too large");
    }
    let val: T = serde_json::from_slice(&bytes).context("deserialize json body")?;
    *req.body_mut() = Body::from(bytes);
    Ok(Some(val))
}

/// Parse URL query + x-www-form-urlencoded body into map, supporting [] / comma array syntax.
/// Parse query and form (application/x-www-form-urlencoded), supports []/comma arrays.
pub async fn get_form_values(
    req: &mut Request<Body>,
) -> anyhow::Result<HashMap<String, Vec<String>>> {
    let mut params: HashMap<String, Vec<String>> = HashMap::new();
    let mut count = 0usize;

    // query part
    if let Some(q) = req.uri().query() {
        append_pairs(q, &mut params, &mut count)?;
    }

    // body part (only for form content type)
    if let Some(ct) = req.headers().get(http::header::CONTENT_TYPE)
        && let Ok(ct) = ct.to_str()
        && ct.contains("application/x-www-form-urlencoded")
    {
        let bytes = hyper::body::to_bytes(req.body_mut())
            .await
            .context("read form body")?;
        if bytes.len() > MAX_MEMORY as usize {
            anyhow::bail!("form body too large");
        }
        append_pairs(std::str::from_utf8(&bytes)?, &mut params, &mut count)?;
        // reinsert body for downstream use
        *req.body_mut() = Body::from(bytes);
    }

    Ok(params)
}

/// Parse form (query + urlencoded body) into `T`.
/// Parse form (query + urlencoded body) into `T`.
pub async fn parse_form<T: DeserializeOwned>(req: &mut Request<Body>) -> anyhow::Result<T> {
    let params = get_form_values(req).await?;
    let json = form_map_to_json(params);
    serde_json::from_value(json).context("deserialize form to struct")
}

/// Parse path params (from extensions) into `T`.
/// Parse path params (from extensions) into `T`.
pub fn parse_path<T: DeserializeOwned>(req: &Request<Body>) -> anyhow::Result<T> {
    let params: HashMap<String, Vec<String>> = req
        .extensions()
        .get::<crate::router::params::PathParams>()
        .map(|p| {
            p.params
                .iter()
                .map(|(k, v)| (k.clone(), vec![v.clone()]))
                .collect()
        })
        .unwrap_or_default();
    let json = form_map_to_json(params);
    serde_json::from_value(json).context("deserialize path params to struct")
}

/// Merge path + form + json into one struct.
pub async fn parse<T: DeserializeOwned>(req: &mut Request<Body>) -> anyhow::Result<T> {
    if matches!(req.method(), &http::Method::GET | &http::Method::HEAD) {
        let path_map: HashMap<String, Vec<String>> = req
            .extensions()
            .get::<crate::router::params::PathParams>()
            .map(|p| {
                p.params
                    .iter()
                    .map(|(k, v)| (k.clone(), vec![v.clone()]))
                    .collect()
            })
            .unwrap_or_default();
        let query_map = get_query_values(req)?;

        let mut merged = serde_json::Map::new();
        merge_map(&mut merged, form_map_to_json(path_map));
        merge_map(&mut merged, form_map_to_json(query_map));
        return serde_json::from_value(serde_json::Value::Object(merged))
            .context("parse merged request (GET/HEAD path+query)");
    }

    let path_map: HashMap<String, Vec<String>> = req
        .extensions()
        .get::<crate::router::params::PathParams>()
        .map(|p| {
            p.params
                .iter()
                .map(|(k, v)| (k.clone(), vec![v.clone()]))
                .collect()
        })
        .unwrap_or_default();
    let form_map = get_form_values(req).await?;
    let json_body: Option<serde_json::Value> = parse_json_body(req).await?;

    let mut merged = serde_json::Map::new();
    merge_map(&mut merged, form_map_to_json(path_map));
    merge_map(&mut merged, form_map_to_json(form_map));
    if let Some(serde_json::Value::Object(obj)) = json_body {
        for (k, v) in obj {
            merged.insert(k, v);
        }
    }

    serde_json::from_value(serde_json::Value::Object(merged)).context("parse merged request")
}

fn get_query_values(req: &Request<Body>) -> anyhow::Result<HashMap<String, Vec<String>>> {
    let mut params: HashMap<String, Vec<String>> = HashMap::new();
    let mut count = 0usize;
    if let Some(q) = req.uri().query() {
        append_pairs(q, &mut params, &mut count)?;
    }
    Ok(params)
}

/// Parse header attributes like `a=1; b=2` into map.
/// Parse header attributes like `a=1; b=2`.
pub fn parse_header(header_value: &str) -> HashMap<String, String> {
    let mut ret = HashMap::new();
    for field in header_value.split(';') {
        let field = field.trim();
        if field.is_empty() {
            continue;
        }
        if let Some((k, v)) = field.split_once('=') {
            ret.insert(k.to_string(), v.to_string());
        }
    }
    ret
}

/// Get remote addr, prefer X-Forwarded-For then extension-provided peer addr.
/// Get remote addr: prefer X-Forwarded-For, then peer addr in extensions.
pub fn get_remote_addr(req: &Request<Body>) -> String {
    if let Some(v) = req.headers().get("x-forwarded-for")
        && let Ok(s) = v.to_str()
        && !s.is_empty()
    {
        return s.to_string();
    }
    if let Some(addr) = req.extensions().get::<std::net::SocketAddr>() {
        return addr.to_string();
    }
    "unknown".to_string()
}

fn append_pairs(
    raw: &str,
    params: &mut HashMap<String, Vec<String>>,
    count: &mut usize,
) -> anyhow::Result<()> {
    let pairs: Vec<(String, String)> =
        serde_urlencoded::from_bytes(raw.as_bytes()).context("parse urlencoded form")?;
    for (mut k_owned, v) in pairs {
        if v.is_empty() {
            continue;
        }
        if *count >= MAX_FORM_PARAM_COUNT {
            anyhow::bail!("too many form values");
        }
        if k_owned.ends_with("[]") {
            k_owned.truncate(k_owned.len() - 2);
        }
        let values: Vec<String> = v
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        if values.is_empty() {
            continue;
        }
        *count += values.len();
        params.entry(k_owned).or_default().extend(values);
    }
    Ok(())
}

fn form_map_to_json(map: HashMap<String, Vec<String>>) -> Value {
    let mut obj = serde_json::Map::new();
    for (k, vals) in map {
        if vals.len() == 1 {
            obj.insert(k, str_to_json_value(&vals[0]));
        } else {
            obj.insert(
                k,
                Value::Array(vals.into_iter().map(|s| str_to_json_value(&s)).collect()),
            );
        }
    }
    Value::Object(obj)
}

fn str_to_json_value(s: &str) -> Value {
    if let Ok(n) = s.parse::<i64>() {
        return Value::Number(n.into());
    }
    Value::String(s.to_string())
}

fn merge_map(target: &mut serde_json::Map<String, Value>, src: Value) {
    if let Value::Object(obj) = src {
        for (k, v) in obj {
            target.insert(k, v);
        }
    }
}

fn with_json_body(req: &Request<Body>) -> bool {
    req.headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.contains("application/json"))
        .unwrap_or(false)
        && req
            .headers()
            .get(http::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0)
            > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::Method;
    use serde::Deserialize;
    use tokio::runtime::Runtime;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct Payload {
        name: String,
        id: u32,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct FormPayload {
        #[serde(default)]
        names: Vec<String>,
        #[serde(default)]
        ids: Vec<i32>,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        name: String,
    }

    fn runtime() -> Runtime {
        Runtime::new().unwrap()
    }

    #[test]
    fn read_json_should_parse_and_preserve_body() {
        runtime().block_on(async {
            let body = r#"{"name":"alice","id":7}"#;
            let mut req = Request::builder()
                .method(Method::POST)
                .uri("/test")
                .body(Body::from(body))
                .unwrap();

            let parsed: Payload = read_json(&mut req).await.unwrap();
            assert_eq!(
                parsed,
                Payload {
                    name: "alice".into(),
                    id: 7
                }
            );

            // body should still be readable
            let bytes = hyper::body::to_bytes(req.into_body()).await.unwrap();
            assert_eq!(bytes, body);
        });
    }

    #[test]
    fn get_form_values_should_support_array_notations() {
        runtime().block_on(async {
            let mut req = Request::builder()
                .method(Method::POST)
                .uri("/api?names=alice&names=bob&names[]=carol&names=dave,erin")
                .header(
                    http::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(Body::from("ids=1&ids=2&empty=&ids[]=3"))
                .unwrap();
            let params = get_form_values(&mut req).await.unwrap();
            assert_eq!(
                params.get("names").unwrap(),
                &vec![
                    "alice".to_string(),
                    "bob".to_string(),
                    "carol".to_string(),
                    "dave".to_string(),
                    "erin".to_string()
                ]
            );
            assert_eq!(
                params.get("ids").unwrap(),
                &vec!["1".to_string(), "2".to_string(), "3".to_string()]
            );
        });
    }

    #[test]
    fn parse_form_should_fill_struct() {
        runtime().block_on(async {
            let mut req = Request::builder()
                .method(Method::POST)
                .uri("/api?names=alice,bob&ids=1&ids=2")
                .header(
                    http::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(Body::from("tags=r&tags=go&names=carol&ids=3"))
                .unwrap();
            let fp: FormPayload = parse_form(&mut req).await.unwrap();
            assert_eq!(fp.names, vec!["alice", "bob", "carol"]);
            assert_eq!(fp.ids, vec![1, 2, 3]);
            assert_eq!(fp.tags, vec!["r", "go"]);
            assert_eq!(fp.name, "");
        });
    }

    #[test]
    fn parse_should_use_query_only_for_get() {
        runtime().block_on(async {
            #[derive(Deserialize, Debug, PartialEq)]
            struct Q {
                a: i64,
            }

            let mut req = Request::builder()
                .method(Method::GET)
                .uri("/path?a=1")
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"a":"2"}"#))
                .unwrap();

            let parsed: Q = parse(&mut req).await.unwrap();
            assert_eq!(parsed.a, 1);
            // body should remain available (unchanged)
            let bytes = hyper::body::to_bytes(req.into_body()).await.unwrap();
            assert_eq!(bytes, r#"{"a":"2"}"#);
        });
    }

    #[test]
    fn parse_should_merge_path_form_json() {
        runtime().block_on(async {
            let mut req = Request::builder()
                .method(Method::POST)
                .uri("/users/42?tags=a,b&ids=1&ids=2")
                .header(
                    http::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(Body::from("tags=c&name=bob"))
                .unwrap();
            // simulate path params injected by router
            let mut params = std::collections::HashMap::new();
            params.insert("id".to_string(), "42".to_string());
            req.extensions_mut()
                .insert(crate::router::params::PathParams { params });

            let merged: FormPayload = parse(&mut req).await.unwrap();
            assert_eq!(merged.ids, vec![1, 2]);
            assert_eq!(merged.tags, vec!["a", "b", "c"]);
            assert_eq!(merged.name, "bob");
            assert_eq!(merged.names, Vec::<String>::new());
        });
    }

    #[test]
    fn parse_header_should_split_pairs() {
        let m = parse_header("a=1; b=2; c=3");
        assert_eq!(m.get("a").unwrap(), "1");
        assert_eq!(m.get("b").unwrap(), "2");
        assert_eq!(m.get("c").unwrap(), "3");
    }

    #[test]
    fn get_remote_addr_should_prefer_header() {
        let req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header("x-forwarded-for", "1.1.1.1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(get_remote_addr(&req), "1.1.1.1");
    }
}
