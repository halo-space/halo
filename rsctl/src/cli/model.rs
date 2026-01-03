use anyhow::{Context, Result};
use clap::{Args as ClapArgs, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum Model {
    /// Generate model code for MySQL
    Mysql(MysqlArgs),
    /// Generate model code for PostgreSQL
    Pg(PgArgs),
}

#[derive(Debug, Clone, ClapArgs)]
pub struct MysqlArgs {
    /// Generate code with cache
    #[arg(short = 'c', long = "cache")]
    pub cache: bool,

    /// The target dir
    #[arg(short = 'd', long = "dir", default_value = ".")]
    pub dir: PathBuf,

    /// Template source:
    /// - git/http URL => clone to temp and use it
    /// - /xxx or relative path => local template directory
    /// - omitted => use local `templates/` under current working dir
    #[arg(short = 'r', long = "remote")]
    pub remote: Option<String>,

    /// The file naming format (reserved; affects output filenames)
    #[arg(short = 's', long = "style", default_value = "rust_zero")]
    pub style: Option<String>,
}

#[derive(Debug, Clone, ClapArgs)]
pub struct PgArgs {
    /// Generate code with cache
    #[arg(short = 'c', long = "cache")]
    pub cache: bool,

    /// The target dir
    #[arg(short = 'd', long = "dir", default_value = ".")]
    pub dir: PathBuf,

    /// Template source:
    /// - git/http URL => clone to temp and use it
    /// - /xxx or relative path => local template directory
    /// - omitted => use local `templates/` under current working dir
    #[arg(short = 'r', long = "remote")]
    pub remote: Option<String>,

    /// The file naming format (reserved; affects output filenames)
    #[arg(short = 's', long = "style", default_value = "rust_zero")]
    pub style: Option<String>,
}

pub fn run(model: Model) -> Result<()> {
    match model {
        Model::Mysql(args) => {
            crate::pipeline::model::mysql::run(crate::pipeline::model::mysql::Options {
                out_dir: args.dir,
                cache: args.cache,
                style: args.style,
                remote: args.remote,
            })
            .context("rsctl model mysql failed")?;
        }
        Model::Pg(args) => {
            crate::pipeline::model::pg::run(crate::pipeline::model::pg::Options {
                out_dir: args.dir,
                cache: args.cache,
                style: args.style,
                remote: args.remote,
            })
            .context("rsctl model pg failed")?;
        }
    }
    Ok(())
}
