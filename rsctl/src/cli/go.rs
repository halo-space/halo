//! `rsctl go ...` 子命令（Go 生成器占位）。

use anyhow::{Context, Result};
use clap::Subcommand;

pub mod api;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Generate API scaffold (Go)
    Api(api::ApiArgs),
}

pub fn run(cmd: Command) -> Result<()> {
    match cmd {
        Command::Api(args) => {
            crate::pipeline::api::go::run(crate::pipeline::api::go::Options {
                out_dir: args.dir,
                api_file: args.api,
                style: args.style,
                remote: args.remote,
                overwrite: args.overwrite,
                web: args.web,
            })
            .context("rsctl go api failed")?;
        }
    }
    Ok(())
}
