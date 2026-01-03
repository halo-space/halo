// Code scaffolded by rsctl. Safe to edit.
// rsctl {{ version }}

use std::sync::Arc;

use halo_micro::rest::{Server};
use crate::handler::routes::register_routes;

mod config;
mod handler;
mod logic;
mod middleware;
mod svc;
mod types;

{% if imports %}
{{ imports }}
{% endif %}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Minimal `-f <path>` parser (compatible with goctl style).
    let mut config_file = format!("etc/{}.yaml", "{{ serviceName }}");
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        if arg == "-f" {
            if let Some(v) = it.next() {
                config_file = v;
            }
        }
    }

    let mut c = config::Config::new();
    halo_micro::conf::must_load(&config_file, &mut c);

    let mut server = Server::new(c.rest.clone());
    let svc_ctx = Arc::new(svc::ServiceContext::new(c));

    register_routes(&mut server, svc_ctx.clone())?;

    println!("Starting server at {}:{}...", server.conf().host, server.conf().port);
    let handle = server.start().await.expect("start server");
    // Wait for Ctrl+C then shut down gracefully.
    if tokio::signal::ctrl_c().await.is_ok() {
        handle.stop().await.expect("stop server");
    }
    Ok(())
}
