//! Lightweight web server built on hyper.
//!
//! Key usage:
//! - `Server::new(conf)` creates a server
//! - `server.add_routes(...)` registers routes
//! - `rest::with_middlewares([...], routes)` applies middlewares in order
//! - `ServerHandle::stop()` supports graceful shutdown for tests/integration

pub mod chain;
pub mod config;
pub mod engine;
pub mod http;
pub mod middleware;
mod router;
mod server;

pub use chain::Chain;
pub use config::RestConf;
pub use engine::Engine;
pub use http::{BoxResponseFuture, HandlerFunc};
pub use macros::{handler, middleware};
pub use middleware::auth;
pub use middleware::{
    IntoHandler, Middleware, apply_middlewares, handler, middleware, mw, with_middleware,
    with_middlewares,
};
pub use router::PathParams;
pub use router::{Route, with_handlers, with_prefix, with_root};
pub use server::{Server, ServerHandle};
