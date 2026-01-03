//! `rsctl rs api ...` 参数与入口。

use anyhow::{Context, Result, anyhow};
use clap::{Args as ClapArgs, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Clone, ClapArgs)]
pub struct ApiArgs {
    #[command(subcommand)]
    pub command: Option<ApiCommand>,

    #[command(flatten)]
    pub generate: GenerateArgs,
}

#[derive(Debug, Clone, ClapArgs)]
pub struct GenerateArgs {
    /// Output directory of generated project/files
    #[arg(short = 'd', long = "dir", default_value = ".")]
    pub dir: PathBuf,

    /// Path of API description file
    #[arg(short = 'a', long = "api")]
    pub api: Option<PathBuf>,

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

#[derive(Debug, Clone, ClapArgs)]
pub struct OpenapiArgs {
    /// Path of API description file
    #[arg(long = "api")]
    pub api: PathBuf,

    /// Output directory of generated openapi files
    #[arg(long = "dir", default_value = ".")]
    pub dir: PathBuf,

    /// Output filename without extension (default: openapi, produces openapi.json)
    #[arg(long = "filename", default_value = "openapi")]
    pub filename: String,

    /// Output format: json or yaml (default: json)
    #[arg(long = "format", default_value = "json", value_parser = ["json", "yaml"])]
    pub format: String,

    /// Overwrite existing files
    #[arg(long = "overwrite", action = clap::ArgAction::SetTrue)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ApiCommand {
    /// Generate OpenAPI v3 document
    Openapi(OpenapiArgs),
}

pub fn run(args: ApiArgs) -> Result<()> {
    match args.command {
        Some(ApiCommand::Openapi(o)) => run_openapi(o),
        None => run_generate(args.generate),
    }
}

pub(crate) fn run_generate(generate: GenerateArgs) -> Result<()> {
    let api = generate
        .api
        .clone()
        .ok_or_else(|| anyhow!("--api is required"))?;
    crate::pipeline::api::rs::run(crate::pipeline::api::rs::Options {
        out_dir: generate.dir,
        api_file: api,
        style: generate.style,
        remote: generate.remote,
        merge: generate.merge,
        fmt: generate.fmt,
        overwrite: generate.overwrite,
    })?;
    Ok(())
}

pub(crate) fn run_openapi(args: OpenapiArgs) -> Result<()> {
    crate::pipeline::api::openapi::run(crate::pipeline::api::openapi::Options {
        out_dir: args.dir,
        api_file: args.api,
        filename: args.filename,
        format: args.format,
        overwrite: args.overwrite,
    })
    .context("rsctl rs api openapi failed")
}
