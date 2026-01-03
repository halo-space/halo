//! Minimal runnable web service example (chainable naming):
//! - Route: `/ai/v1/v1/api/square`
//! - Response: `{"code":200,"message":"ok","data":"hello world"}`
//!   Run: `cargo run -p halo-rest --example hello`

use http::{Method, Response, StatusCode};
use hyper::Body;
use rest::http::{ok_json, request::parse};
use rest::{HandlerFunc, RestConf, Route, Server};
use serde::{Deserialize, Serialize};
use serde_json::json;
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
        // TODO: Fill in real business logic here.
        let echoed = format!("echo: {}", req.keyword);
        SquareListResp {
            code: 200,
            msg: "ok".to_string(),
            data: json!({ "result": echoed }),
        }
    }
}

#[rest::handler]
async fn square_list_handler(app: AppContext, mut req: http::Request<Body>) -> Response<Body> {
    let parsed: SquareListReq = match parse(&mut req).await {
        Ok(v) => v,
        Err(e) => return bad_request(e.to_string()),
    };
    let logic = SquareListLogic::new(app);
    let resp = logic.square_list(&parsed).await;
    ok_json(&resp).unwrap_or_else(|e| {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(format!("encode json: {e}")))
            .unwrap()
    })
}

/// Middleware: add `X-Example: 1` header (can short-circuit or pass through)
#[rest::middleware]
async fn add_header_middleware(req: http::Request<Body>, next: HandlerFunc) -> Response<Body> {
    let mut resp = next.call(req).await;
    resp.headers_mut()
        .insert("X-Example", http::HeaderValue::from_static("1"));
    resp
}

fn main() {
    // Basic config
    let mut conf = RestConf {
        port: 8080,
        ..RestConf::default()
    };

    // Disable built-in middlewares: gzip, max_bytes, rate_limit, concurrency_limit, timeout
    conf.middlewares.gzip = false;
    conf.middlewares.max_bytes = false;
    conf.rate_limit = None;
    conf.concurrency_limit = None;
    conf.timeout = None;
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async move {
        // Set root, then prefix, then add middleware and routes (clear step-by-step)

        // 推荐写法：链式一步到位
        // // let mut server = Server::new(conf)
        //     .with_root_prefix("/ai/v1/")
        //     .with_prefix("/v1/api/square")
        //     .with_middleware(add_header_middleware())
        //     .add_routes(vec![Route::new(
        //         Method::GET,
        //         "/",
        //         square_list_handler(AppContext::default()),
        //     )])
        //     .expect("add routes");

        let mut server = Server::new(conf);
        server.with_root_prefix("/");
        // server = server
        //     .with_prefix("/v1/api/square")
        //     .with_middleware(add_header_middleware())
        //     .add_routes(vec![Route::new(
        //         Method::GET,
        //         "/",
        //         square_list_handler(AppContext::default()),
        //     )])
        //     .expect("TODO: panic message");

        server
            .with_prefix("/v1/api/square")
            .with_middlewares(vec![add_header_middleware()])
            .add_routes(vec![Route::new(
                Method::GET,
                "/",
                square_list_handler(AppContext),
            )]);

        println!("Listening on {}/ai/v1/v1/api/square", server_addr(&server));
        println!("Press Ctrl+C to stop.");
        let handle = server.start().await.expect("start server");
        // Block until Ctrl+C, then shut down gracefully
        if let Err(err) = signal::ctrl_c().await {
            eprintln!("Failed to listen for Ctrl+C: {err}");
            return;
        }
        handle.stop().await.expect("stop server");
    });
}

fn server_addr(server: &Server) -> String {
    let conf = server.conf();
    format!("http://{}:{}", conf.host, conf.port)
}

fn bad_request(msg: String) -> Response<Body> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(Body::from(msg))
        .unwrap()
}
