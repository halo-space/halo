use anyhow::{Context, Result, anyhow};
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::path::{Path, PathBuf as StdPathBuf};
use std::process::Command;

pub mod install;

/// Resolve default template root directory from user's home.
///
/// Lookup order:
/// 1) `~/.rsctl/<current_version>/` (preferred; versioned install)
/// 2) `~/.rsctl/templates` (legacy)
/// 3) `~/.rsctl` (legacy)
/// 4) search upwards from current dir for `templates/` or `rsctl/templates/` (repo/workspace local)
/// 5) `None` (caller may fallback to `templates/` by itself)
pub fn default_template_root() -> Option<PathBuf> {
    let rsctl_home = crate::path::rsctl_home_dir()?;
    if rsctl_home.is_dir()
        && let Some(vdir) = versioned_template_dir(&rsctl_home)
        && vdir.is_dir()
    {
        return Some(vdir);
    }
    let rsctl_templates = rsctl_home.join("templates");
    if rsctl_templates.is_dir() {
        Some(rsctl_templates)
    } else if rsctl_home.is_dir() {
        Some(rsctl_home)
    } else {
        find_templates_upwards()
    }
}

/// Resolve the template root directory.
///
/// - `None` or empty => default template root (see `default_template_root`)
/// - local path => that directory
/// - git/http/ssh URL => clone and return cloned dir (preferring `<repo>/templates` if present)
pub fn resolve_template_root(remote: Option<&str>) -> Result<StdPathBuf> {
    match remote {
        None => Ok(default_template_root().unwrap_or_else(|| StdPathBuf::from("templates"))),
        Some(s) if s.trim().is_empty() => {
            Ok(default_template_root().unwrap_or_else(|| StdPathBuf::from("templates")))
        }
        Some(s) => {
            let s = s.trim();
            if looks_like_url_or_git(s) {
                let dir = clone_remote_templates(s)?;
                let repo_templates = dir.join("templates");
                if repo_templates.is_dir() {
                    Ok(repo_templates)
                } else {
                    Ok(dir)
                }
            } else {
                Ok(StdPathBuf::from(s))
            }
        }
    }
}

fn looks_like_url_or_git(s: &str) -> bool {
    s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("git@")
        || s.starts_with("ssh://")
        || s.ends_with(".git")
}

fn clone_remote_templates(remote: &str) -> Result<StdPathBuf> {
    let tmp = std::env::temp_dir();
    let repo_name = remote_repo_basename(remote).unwrap_or_else(|| "templates".to_string());
    let uniq = format!(
        "{}-{}",
        repo_name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let dest = tmp.join("rsctl").join("repos").join(uniq);
    fs::create_dir_all(&dest)
        .with_context(|| format!("failed to create temp dir: {}", dest.display()))?;

    let status = Command::new("git")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg(remote)
        .arg(&dest)
        .status()
        .with_context(|| "failed to execute `git clone` (is git installed and in PATH?)")?;

    if !status.success() {
        return Err(anyhow!("git clone failed for remote: {remote}"));
    }

    Ok(dest)
}

fn remote_repo_basename(remote: &str) -> Option<String> {
    let last = remote
        .rsplit(['/', ':'])
        .next()
        .and_then(|s| if s.is_empty() { None } else { Some(s) })?;

    Some(
        Path::new(last)
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or(last)
            .to_string(),
    )
}

fn find_templates_upwards() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        // Repo-root layout: `templates/`
        let root_templates = dir.join("templates");
        if root_templates.is_dir() {
            return Some(root_templates);
        }

        // Monorepo layout (this repo): `rsctl/templates/`
        let rsctl_templates = dir.join("rsctl").join("templates");
        if rsctl_templates.is_dir() {
            return Some(rsctl_templates);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn versioned_template_dir(rsctl_home: &Path) -> Option<PathBuf> {
    // Provided by CLI at runtime. Example: "1.9.2"
    let ver = std::env::var_os("RSCTL_VERSION")?;
    if ver.is_empty() {
        return None;
    }
    Some(rsctl_home.join(ver))
}
