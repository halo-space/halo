//! `rsctl rs ...` 子命令（Rust 生成器）。

use anyhow::Result;
use clap::Subcommand;

pub mod api;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Generate model code (Rust)
    Model {
        #[command(subcommand)]
        model: crate::cli::model::Model,
    },
}

pub fn run(cmd: Command) -> Result<()> {
    match cmd {
        Command::Model { model } => crate::cli::model::run(model),
    }
}
