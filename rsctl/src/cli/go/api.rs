//! `rsctl go api ...` 参数（占位）。

use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(Debug, Clone, ClapArgs)]
pub struct ApiArgs {
    /// Output directory of generated project/files
    #[arg(short = 'd', long = "dir", default_value = ".")]
    pub dir: PathBuf,

    /// Path of API description file
    #[arg(short = 'a', long = "api")]
    pub api: PathBuf,

    #[arg(short = 's', long = "style")]
    pub style: Option<String>,

    #[arg(short = 'r', long = "remote")]
    pub remote: Option<String>,

    #[arg(short = 'o', long = "overwrite", action = clap::ArgAction::SetTrue)]
    pub overwrite: bool,

    /// Target web framework name (e.g. "gin")
    #[arg(short = 'w', long = "web", default_value = "gin")]
    pub web: String,
}
