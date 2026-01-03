//! Top-level `rsctl api ...` commands (goctl-style: api rs / api openapi).
use anyhow::Result;
use clap::{Args as ClapArgs, Subcommand};
use std::path::PathBuf;

use crate::cli::rs::api::{GenerateArgs, OpenapiArgs};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Generate Rust API scaffold (equivalent to `rsctl rs api`)
    Rs(ApiRsArgs),
    /// Generate OpenAPI v3 document
    Openapi(OpenapiArgs),
}

#[derive(Debug, Clone, ClapArgs)]
pub struct ApiRsArgs {
    /// Output directory of generated project/files
    #[arg(short = 'd', long = "dir", default_value = ".")]
    pub dir: PathBuf,

    /// Path of API description file
    #[arg(short = 'a', long = "api")]
    pub api: PathBuf,

    /// 生成文件命名风格（影响生成的 .rs 文件名）
    ///
    /// 可选值：
    /// - rust_zero  : snake_case（默认）
    /// - rustZero   : lowerCamelCase（仅影响文件名；模块名仍为 snake_case）
    /// - RustZero   : UpperCamelCase（仅影响文件名；模块名仍为 snake_case）
    #[arg(
        short = 's',
        long = "style",
        default_value = "rust_zero",
        value_parser = ["rust_zero", "rustZero", "RustZero"]
    )]
    pub style: Option<String>,

    /// Template source:
    /// - git/http URL => clone to temp and use it
    /// - /xxx or relative path => local template directory
    /// - omitted => use local `templates/` under current working dir
    #[arg(short = 'r', long = "remote")]
    pub remote: Option<String>,

    /// Merge handlers of the same group into one file.
    #[arg(short = 'm', long = "merge", default_value_t = true, action = clap::ArgAction::Set)]
    pub merge: bool,

    /// Run `cargo fmt` after generation (default: true).
    #[arg(long = "fmt", default_value = "true")]
    pub fmt: bool,

    /// Overwrite existing files when writing to disk (default: false).
    #[arg(short = 'o', long = "overwrite", action = clap::ArgAction::SetTrue)]
    pub overwrite: bool,
}

impl From<ApiRsArgs> for GenerateArgs {
    fn from(a: ApiRsArgs) -> Self {
        GenerateArgs {
            dir: a.dir,
            api: Some(a.api),
            style: a.style,
            remote: a.remote,
            merge: a.merge,
            fmt: a.fmt,
            overwrite: a.overwrite,
        }
    }
}

pub fn run(cmd: Command) -> Result<()> {
    match cmd {
        Command::Rs(args) => crate::cli::rs::api::run_generate(args.into()),
        Command::Openapi(args) => crate::cli::rs::api::run_openapi(args),
    }
}
