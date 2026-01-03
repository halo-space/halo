//! 与路径相关的公共工具函数（home 目录、`~/.rsctl` 等）。
//!
//! 该模块只负责“路径规则/路径计算”，不负责实际的文件读写与安装动作。

use anyhow::{Context, Result};
use std::path::PathBuf;

/// 获取当前用户的 home 目录。
///
/// - Unix: `HOME`
/// - Windows: `USERPROFILE`，否则 `HOMEDRIVE` + `HOMEPATH`
pub fn home_dir() -> Option<PathBuf> {
    // Unix: HOME
    if let Some(h) = std::env::var_os("HOME")
        && !h.is_empty()
    {
        return Some(PathBuf::from(h));
    }
    // Windows: USERPROFILE, or HOMEDRIVE + HOMEPATH
    if let Some(h) = std::env::var_os("USERPROFILE")
        && !h.is_empty()
    {
        return Some(PathBuf::from(h));
    }
    let drive = std::env::var_os("HOMEDRIVE");
    let path = std::env::var_os("HOMEPATH");
    match (drive, path) {
        (Some(d), Some(p)) if !d.is_empty() && !p.is_empty() => Some(PathBuf::from(d).join(p)),
        _ => None,
    }
}

/// `~/.rsctl`
pub fn rsctl_home_dir() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".rsctl"))
}

/// 返回 (~/.rsctl, ~/.rsctl/<version>)。
pub fn rsctl_version_dir(version: &str) -> Result<(PathBuf, PathBuf)> {
    let rsctl_home = rsctl_home_dir().context("cannot resolve user home dir")?;
    let version_dir = rsctl_home.join(version);
    Ok((rsctl_home, version_dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        key: &'static str,
        prev: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, val: &str) -> Self {
            let prev = std::env::var_os(key);
            // Rust 2024: 修改进程环境变量是 `unsafe`（与并发读写相关）。
            unsafe {
                std::env::set_var(key, val);
            }
            Self { key, prev }
        }

        fn remove(key: &'static str) -> Self {
            let prev = std::env::var_os(key);
            unsafe {
                std::env::remove_var(key);
            }
            Self { key, prev }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.prev.take() {
                Some(v) => unsafe {
                    std::env::set_var(self.key, v);
                },
                None => unsafe {
                    std::env::remove_var(self.key);
                },
            }
        }
    }

    #[test]
    fn test_home_dir_prefers_home_env() {
        let _g = env_lock().lock().unwrap();
        let _a = EnvGuard::set("HOME", "C:\\Users\\tester");
        let _b = EnvGuard::set("USERPROFILE", "C:\\Users\\ignored");
        let got = home_dir().unwrap();
        assert!(got.to_string_lossy().contains("tester"));
    }

    #[test]
    fn test_home_dir_falls_back_to_userprofile_on_windows() {
        let _g = env_lock().lock().unwrap();
        let _a = EnvGuard::remove("HOME");
        let _b = EnvGuard::set("USERPROFILE", "C:\\Users\\tester2");
        let got = home_dir().unwrap();
        assert!(got.to_string_lossy().contains("tester2"));
    }

    #[test]
    fn test_rsctl_version_dir_joins_version() {
        let _g = env_lock().lock().unwrap();
        let _a = EnvGuard::set("HOME", "C:\\Users\\tester3");
        let (home, vdir) = rsctl_version_dir("1.2.3").unwrap();
        assert!(
            home.to_string_lossy().ends_with("\\.rsctl")
                || home.to_string_lossy().ends_with("/.rsctl")
        );
        assert!(vdir.to_string_lossy().contains("1.2.3"));
    }
}
