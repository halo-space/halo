//! Go API pipeline entrypoints (placeholder).
//!
//! 目前仅用于搭建语言一级的 pipeline/cli 结构，未来实现 go 生成时在此接入 parse/semantic/codegen。

use anyhow::{Result, anyhow};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Options {
    // 这些字段是为未来 go 生成预留的；当前占位实现不会读取它们。
    #[allow(dead_code)]
    pub out_dir: PathBuf,
    #[allow(dead_code)]
    pub api_file: PathBuf,
    #[allow(dead_code)]
    pub style: Option<String>,
    #[allow(dead_code)]
    pub remote: Option<String>,
    #[allow(dead_code)]
    pub overwrite: bool,
    #[allow(dead_code)]
    pub web: String,
}

pub fn run(_opts: Options) -> Result<()> {
    Err(anyhow!(
        "go api generator is not implemented yet (planned: `rsctl go api ...` -> code::api::go::...)"
    ))
}
