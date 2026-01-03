use anyhow::Result;

mod cli;
mod generate;
mod parse;
mod pipeline;
mod semantic;
mod spec;
mod version;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Let template resolver locate versioned templates (e.g. ~/.rsctl/<VERSION>/).
    // Rust 2024: 修改进程环境变量是 `unsafe`（与并发读写相关）。
    unsafe {
        std::env::set_var("RSCTL_VERSION", version::VERSION);
    }
    cli::run()
}
