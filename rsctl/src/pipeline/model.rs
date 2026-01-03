//! Model pipeline entrypoints (orchestration layer).

pub mod mysql {
    use anyhow::{Context, Result, anyhow};
    use std::fs;
    use std::path::PathBuf;

    #[derive(Debug, Clone)]
    pub struct Options {
        pub out_dir: PathBuf,
        pub cache: bool,
        pub style: Option<String>,
        /// None => local workspace templates (`templates/`)
        /// Some => either a local path, or a remote git/http URL.
        pub remote: Option<String>,
    }

    pub fn run(opts: Options) -> Result<()> {
        if opts.out_dir.as_os_str().is_empty() {
            return Err(anyhow!("--dir is required"));
        }

        fs::create_dir_all(&opts.out_dir)
            .with_context(|| format!("failed to create output dir: {}", opts.out_dir.display()))?;

        let template_root = utils::template::resolve_template_root(opts.remote.as_deref())?;

        tracing::info!(
            out_dir = %opts.out_dir.display(),
            template_root = %template_root.display(),
            cache = opts.cache,
            style = opts.style.as_deref().unwrap_or(""),
            "rsctl model mysql resolved"
        );

        Ok(())
    }
}

pub mod pg {
    use anyhow::{Context, Result, anyhow};
    use std::fs;
    use std::path::PathBuf;

    #[derive(Debug, Clone)]
    pub struct Options {
        pub out_dir: PathBuf,
        pub cache: bool,
        pub style: Option<String>,
        /// None => local workspace templates (`templates/`)
        /// Some => either a local path, or a remote git/http URL.
        pub remote: Option<String>,
    }

    pub fn run(opts: Options) -> Result<()> {
        if opts.out_dir.as_os_str().is_empty() {
            return Err(anyhow!("--dir is required"));
        }

        fs::create_dir_all(&opts.out_dir)
            .with_context(|| format!("failed to create output dir: {}", opts.out_dir.display()))?;

        let template_root = utils::template::resolve_template_root(opts.remote.as_deref())?;

        tracing::info!(
            out_dir = %opts.out_dir.display(),
            template_root = %template_root.display(),
            cache = opts.cache,
            style = opts.style.as_deref().unwrap_or(""),
            "rsctl model pg resolved"
        );

        Ok(())
    }
}
