//! Rust API pipeline entrypoints.

use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Options {
    pub out_dir: PathBuf,
    pub api_file: PathBuf,
    pub style: Option<String>,
    /// None => local workspace templates (`templates/`)
    /// Some => either a local path, or a remote git/http URL.
    pub remote: Option<String>,
    /// Merge handlers of the same group into one file.
    pub merge: bool,
    /// Run `cargo fmt` after generation.
    pub fmt: bool,
    /// Whether to overwrite existing files when writing.
    pub overwrite: bool,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub out_dir: PathBuf,
    pub api_file: PathBuf,
    pub style: Option<String>,
    /// Final template root directory (expected to contain `api/rs/`).
    pub template_root: PathBuf,
    pub merge: bool,
    pub fmt: bool,
    pub overwrite: bool,
}

pub fn run(opts: Options) -> Result<Config> {
    let cfg = config(opts)?;
    pipeline(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config(opts: Options) -> Result<Config> {
    if opts.out_dir.as_os_str().is_empty() {
        return Err(anyhow!("--dir is required"));
    }
    if opts.api_file.as_os_str().is_empty() {
        return Err(anyhow!("--api is required"));
    }

    fs::create_dir_all(&opts.out_dir)
        .with_context(|| format!("failed to create output dir: {}", opts.out_dir.display()))?;

    let template_root = utils::template::resolve_template_root(opts.remote.as_deref())?;

    tracing::info!(
        out_dir = %opts.out_dir.display(),
        api = %opts.api_file.display(),
        template_root = %template_root.display(),
        style = opts.style.as_deref().unwrap_or(""),
        merge = opts.merge,
        overwrite = opts.overwrite,
        "rsctl rs api resolved"
    );

    Ok(Config {
        out_dir: opts.out_dir,
        api_file: opts.api_file,
        style: opts.style,
        template_root,
        merge: opts.merge,
        fmt: opts.fmt,
        overwrite: opts.overwrite,
    })
}

pub(crate) fn pipeline(cfg: &Config) -> Result<()> {
    // 1) parse (support import ... ; info 仅主文件)
    let mut visited = std::collections::HashSet::new();
    let ast = load_ast_recursive(&cfg.api_file, true, &mut visited).context("parse api file")?;

    // 2) semantic -> spec (stable IR)
    let mut spec = crate::semantic::api::to_spec(&ast).context("semantic to spec")?;
    // Ensure root_prefix uses only the main file's info (imports are ignored).
    let root_ast = crate::parse::api::parse_file(&cfg.api_file).context("parse root api file")?;
    let root_prefix = extract_root_prefix(&root_ast)
        .or_else(|| extract_root_prefix_text(&cfg.api_file).ok().flatten());
    if let Some(rp) = root_prefix {
        spec.info.root_prefix = rp;
    }

    // 3) normalize + validate spec invariants (like goctl: Parse -> Validate -> Generate)
    //
    // 如果用户没有写 `service {}` 块（只有顶层 routes），semantic 层会把 service.name 留空。
    // 这里用和生成入口相同的策略补齐 service 名，再做 validate。
    if spec.service.name.trim().is_empty() {
        let fallback = cfg
            .api_file
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "api".to_string());
        spec.service.name = fallback;
    }
    spec.validate().context("validate spec")?;

    // 4) gen -> artifacts
    let style = cfg.style.as_deref().unwrap_or("rust_zero");
    // 优先用 `.api` 里 `service <name>` 的名字作为工程/入口名。
    // 如果没有任何 service（只有顶层 routes），再回退到文件名。
    let service_name = spec.service.name.clone();

    let artifacts = crate::generate::api::rs::shared::generate_project(
        "rest",
        &spec,
        &service_name,
        cfg.merge,
        style,
        &cfg.out_dir,
        &cfg.template_root,
        if spec.info.root_prefix.is_empty() {
            ""
        } else {
            &spec.info.root_prefix
        },
    )?;

    // 5) write
    write_artifacts(cfg, &artifacts).context("write artifacts")?;

    // 6) fmt (optional)
    if cfg.fmt {
        run_fmt(&cfg.out_dir).context("cargo fmt")?;
    }

    Ok(())
}

fn load_ast_recursive(
    api_file: &Path,
    is_root: bool,
    visited: &mut std::collections::HashSet<std::path::PathBuf>,
) -> Result<crate::parse::api::Ast> {
    let canon = api_file
        .canonicalize()
        .with_context(|| format!("canonicalize {}", api_file.display()))?;
    if !visited.insert(canon.clone()) {
        return Err(anyhow!("cyclic import detected: {}", api_file.display()));
    }

    let mut ast = crate::parse::api::parse_file(&canon).with_context(|| {
        if is_root {
            format!("parse api file: {}", api_file.display())
        } else {
            format!("parse imported api file: {}", api_file.display())
        }
    })?;

    // Only keep root info; imported files' info are ignored.
    if !is_root {
        ast.info = None;
    }

    let base_dir = canon
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let mut merged_items = ast.items.clone();
    for import in &ast.imports {
        let child_path = base_dir.join(import);
        let child_ast = load_ast_recursive(&child_path, false, visited)?;
        merged_items.extend(child_ast.items);
    }

    ast.items = merged_items;
    ast.imports.clear();
    Ok(ast)
}

fn extract_root_prefix(ast: &crate::parse::api::Ast) -> Option<String> {
    let info = ast.info.as_ref()?;
    for (k, v) in &info.properties {
        if k.eq_ignore_ascii_case("root_prefix") {
            let mut p = v.trim_matches('"').trim().to_string();
            if !p.is_empty() && !p.starts_with('/') {
                p = format!("/{p}");
            }
            if !p.is_empty() {
                return Some(p);
            }
        }
    }
    None
}

fn extract_root_prefix_text(path: &Path) -> Result<Option<String>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("read api file for root_prefix: {}", path.display()))?;
    let lowered = content.to_ascii_lowercase();
    if let Some(idx) = lowered.find("root_prefix") {
        let rest = &content[idx + "root_prefix".len()..];
        // Prefer quoted value
        if let Some(start_q) = rest.find('"') {
            let rest_q = &rest[start_q + 1..];
            if let Some(end_q) = rest_q.find('"') {
                let mut p = rest_q[..end_q].trim().to_string();
                if !p.is_empty() && !p.starts_with('/') {
                    p = format!("/{p}");
                }
                if !p.is_empty() {
                    return Ok(Some(p));
                }
            }
        }
        // Fallback: take token after colon
        if let Some(colon_pos) = rest.find(':') {
            let token = rest[colon_pos + 1..]
                .split_whitespace()
                .next()
                .unwrap_or("");
            let token = token.trim_matches(|c| c == '"' || c == ',' || c == ')');
            let mut p = token.trim().to_string();
            if !p.is_empty() && !p.starts_with('/') {
                p = format!("/{p}");
            }
            if !p.is_empty() {
                return Ok(Some(p));
            }
        }
        // Fallback: take from first '/' onwards on the same line.
        if let Some(slash_pos) = rest.find('/') {
            let tail = &rest[slash_pos..];
            let token = tail
                .split(|c: char| c.is_whitespace() || c == '"' || c == ',' || c == ')')
                .next()
                .unwrap_or("");
            let mut p = token.trim().to_string();
            if !p.is_empty() && !p.starts_with('/') {
                p = format!("/{p}");
            }
            if !p.is_empty() {
                return Ok(Some(p));
            }
        }
    }
    Ok(None)
}

fn write_artifacts(cfg: &Config, artifacts: &crate::generate::artifact::Artifacts) -> Result<()> {
    for f in &artifacts.files {
        let path = cfg.out_dir.join(&f.rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create parent dir: {}", parent.display()))?;
        }

        if !cfg.overwrite && path.exists() {
            tracing::info!(path = %path.display(), "skip existing file (overwrite=false)");
            continue;
        }

        fs::write(&path, &f.content)
            .with_context(|| format!("failed to write {}", path.display()))?;
        tracing::info!(path = %path.display(), "write file");
    }
    Ok(())
}

fn run_fmt(out_dir: &PathBuf) -> Result<()> {
    let cargo_toml = out_dir.join("Cargo.toml");
    if !cargo_toml.is_file() {
        tracing::info!("skip cargo fmt: {} not found", cargo_toml.display());
        return Ok(());
    }
    let status = Command::new("cargo")
        .arg("fmt")
        .current_dir(out_dir)
        .status()
        .with_context(|| format!("failed to run cargo fmt in {}", out_dir.display()))?;
    if !status.success() {
        return Err(anyhow!("cargo fmt failed with status {}", status));
    }
    Ok(())
}
