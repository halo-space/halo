//! Axum example with the same interface:
//! - Route: `/ai/v1/v1/api/square`
//! - Response: `{"code":200,"msg":"ok","data":{"result":"echo: <keyword>"}}`
//! Run: `cargo run -p halo-rest --example axum --release`
//! Stop: Ctrl+C

use axum::{
    Json, Router,
    body::Body,
    extract::{Query, State},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use tokio::signal;

#[derive(Clone, Default)]
struct AppContext;

#[derive(Debug, Deserialize)]
struct SquareListReq {
    #[serde(default)]
    keyword: String,
}

#[derive(Debug, Serialize)]
struct SquareListResp {
    code: i32,
    msg: String,
    data: serde_json::Value,
}

struct SquareListLogic {
    #[allow(dead_code)]
    app: AppContext,
}

impl SquareListLogic {
    fn new(app: AppContext) -> Self {
        Self { app }
    }

    async fn square_list(&self, req: &SquareListReq) -> SquareListResp {
        let echoed = format!("echo: {}", req.keyword);
        SquareListResp {
            code: 200,
            msg: "ok".to_string(),
            data: json!({ "result": echoed }),
        }
    }
}

/// Business handler: extract state + query, return JSON.
async fn square_list_handler(
    State(app): State<AppContext>,
    Query(payload): Query<SquareListReq>,
) -> impl IntoResponse {
    let logic = SquareListLogic::new(app);
    let resp = logic.square_list(&payload).await;
    (StatusCode::OK, Json(resp))
}

/// Middleware: add header X-Example: 1.
async fn add_header_middleware(req: axum::http::Request<Body>, next: Next) -> impl IntoResponse {
    let mut resp = next.run(req).await;
    resp.headers_mut()
        .insert("X-Example", HeaderValue::from_static("1"));
    resp
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = Router::new()
        .route("/ai/v1/v1/api/square", get(square_list_handler))
        .with_state(AppContext::default())
        .layer(middleware::from_fn(add_header_middleware));

    let addr: SocketAddr = "0.0.0.0:8081".parse()?;
    println!("Axum listening on http://{addr}/ai/v1/v1/api/square/");
    println!("Press Ctrl+C to stop.");

    axum::serve(
        tokio::net::TcpListener::bind(addr).await?,
        app.into_make_service(),
    )
    .with_graceful_shutdown(async {
        let _ = signal::ctrl_c().await;
    })
    .await?;

    Ok(())
}
