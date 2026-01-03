use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};

pub mod api;
pub mod go;
pub mod model;
pub mod rs;
pub mod template;

#[derive(Debug, Parser)]
#[command(
    name = "rsctl",
    version,
    disable_version_flag = true,
    about = "Code generator CLI"
)]
pub struct App {
    #[arg(short = 'v', long = "version", global = true, action = clap::ArgAction::SetTrue)]
    pub version: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// API related generators (api rs / api openapi)
    Api {
        #[command(subcommand)]
        api: api::Command,
    },
    /// Rust generators (model only, api removed)
    Rs {
        #[command(subcommand)]
        command: rs::Command,
    },
    /// Go generators
    Go {
        #[command(subcommand)]
        command: go::Command,
    },
    /// Manage built-in templates (install/clean/update)
    Template {
        #[command(subcommand)]
        template: template::Template,
    },
}

pub fn run() -> Result<()> {
    // 不再兼容旧形式：`rsctl api -rs ...`
    let app = App::parse();

    if app.version {
        println!("{}", crate::version::VERSION);
        return Ok(());
    }

    match app.command {
        Some(Command::Api { api }) => api::run(api)?,
        Some(Command::Rs { command }) => rs::run(command)?,
        Some(Command::Go { command }) => go::run(command)?,
        Some(Command::Template { template }) => template::run(template)?,
        None => {
            // No subcommand, no -v: show help
            let mut cmd = App::command();
            cmd.print_help()?;
            println!();
        }
    }

    Ok(())
}
