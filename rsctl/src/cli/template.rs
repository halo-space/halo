//! `rsctl template` 子命令：安装/清理/更新内置模板。

use anyhow::{Context, Result};
use clap::Subcommand;
use include_dir::{Dir, include_dir};

static EMBED_TEMPLATES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates");

#[derive(Debug, Subcommand)]
pub enum Template {
    /// 初始化：安装当前版本模板到用户目录（~/.rsctl/<version>/）
    Init,
    /// 清理：删除当前版本的模板目录（只删 ~/.rsctl/<version>/）
    #[command(alias = "celan")]
    Clean,
    /// 更新：覆盖安装当前版本的所有模板（~/.rsctl/<version>/）
    Update,
}

pub fn run(cmd: Template) -> Result<()> {
    match cmd {
        Template::Init => init(false),
        Template::Clean => clean(),
        Template::Update => init(true),
    }
}

fn init(overwrite: bool) -> Result<()> {
    let (rsctl_home, version_dir) = utils::path::rsctl_version_dir(crate::version::VERSION)?;

    std::fs::create_dir_all(&rsctl_home)
        .with_context(|| format!("create rsctl home dir failed: {}", rsctl_home.display()))?;

    if version_dir.exists() && !overwrite {
        println!("模板目录已存在：{}", version_dir.display());
        return Ok(());
    }

    if overwrite && version_dir.exists() {
        std::fs::remove_dir_all(&version_dir).with_context(|| {
            format!(
                "remove existing version template dir failed: {}",
                version_dir.display()
            )
        })?;
    }

    std::fs::create_dir_all(&version_dir).with_context(|| {
        format!(
            "create version template dir failed: {}",
            version_dir.display()
        )
    })?;

    utils::template::install::extract_dir(&EMBED_TEMPLATES, &version_dir)?;

    println!("模板已安装到：{}", version_dir.display());
    Ok(())
}

fn clean() -> Result<()> {
    let (_, version_dir) = utils::path::rsctl_version_dir(crate::version::VERSION)?;
    if !version_dir.exists() {
        println!("当前版本模板目录不存在：{}", version_dir.display());
        return Ok(());
    }

    std::fs::remove_dir_all(&version_dir)
        .with_context(|| format!("remove template dir failed: {}", version_dir.display()))?;

    println!("已删除当前版本模板目录：{}", version_dir.display());
    Ok(())
}
