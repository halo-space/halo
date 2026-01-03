//! 配置加载工具（对齐 go-zero 的 `core/conf`）。
//!
//! 目标：
//! - 对齐 go-zero 的调用习惯：`conf::must_load(path, &mut cfg)`；
//! - 支持 YAML/JSON/TOML（按文件扩展名自动识别）；
//! - 支持环境变量展开（类似 Go 的 `os.ExpandEnv`：`${VAR}` / `$VAR`）；
//! - 同时提供非 panic 的 `load_into/load` 系列 API。

use serde::de::DeserializeOwned;
use std::path::Path;

/// 配置文件格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Yaml,
    Json,
    Toml,
}

impl Format {
    pub fn from_path(path: &Path) -> anyhow::Result<Self> {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        match ext.as_str() {
            "yaml" | "yml" => Ok(Self::Yaml),
            "json" => Ok(Self::Json),
            "toml" => Ok(Self::Toml),
            _ => Err(anyhow::anyhow!("unsupported config file type: .{ext}")),
        }
    }
}

/// 加载选项（对齐 go-zero 的 Option 思路）。
#[derive(Debug, Clone)]
pub struct Options {
    /// 是否展开环境变量（默认 true）。
    pub expand_env: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { expand_env: true }
    }
}

/// 从配置文件读取并反序列化为 `T`。
pub fn load<T: DeserializeOwned>(path: impl AsRef<Path>) -> anyhow::Result<T> {
    let mut v = None;
    load_into_with(path.as_ref(), &mut v, Options::default())?;
    Ok(v.expect("load_into_with must set value"))
}

/// 从配置文件读取并写入 `cfg`。
pub fn load_into<T: DeserializeOwned>(path: impl AsRef<Path>, cfg: &mut T) -> anyhow::Result<()> {
    load_into_with(path.as_ref(), cfg, Options::default())
}

/// 带 options 的 load_into。
pub fn load_into_with<T: DeserializeOwned>(
    path: &Path,
    cfg: &mut T,
    opts: Options,
) -> anyhow::Result<()> {
    let fmt = Format::from_path(path)?;
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("read config {}: {e}", path.display()))?;
    let raw = if opts.expand_env {
        expand_env(&raw)
    } else {
        raw
    };
    load_from_str(fmt, &raw, cfg).map_err(|e| anyhow::anyhow!("load {}: {e}", path.display()))
}

/// 从 bytes 加载（主要对齐 go-zero 的 `LoadFromBytes`）。
pub fn load_from_bytes_into<T: DeserializeOwned>(
    fmt: Format,
    bytes: &[u8],
    cfg: &mut T,
    opts: Options,
) -> anyhow::Result<()> {
    let mut s = String::from_utf8(bytes.to_vec())
        .map_err(|e| anyhow::anyhow!("config bytes not utf-8: {e}"))?;
    if opts.expand_env {
        s = expand_env(&s);
    }
    load_from_str(fmt, &s, cfg)
}

fn load_from_str<T: DeserializeOwned>(fmt: Format, s: &str, cfg: &mut T) -> anyhow::Result<()> {
    *cfg = match fmt {
        Format::Yaml => {
            serde_yaml::from_str::<T>(s).map_err(|e| anyhow::anyhow!("parse yaml: {e}"))?
        }
        Format::Json => {
            serde_json::from_str::<T>(s).map_err(|e| anyhow::anyhow!("parse json: {e}"))?
        }
        Format::Toml => toml::from_str::<T>(s).map_err(|e| anyhow::anyhow!("parse toml: {e}"))?,
    };
    Ok(())
}

/// 对齐 go-zero：加载失败直接 panic（用于快速失败的启动场景）。
pub fn must_load<T: DeserializeOwned>(path: impl AsRef<Path>, cfg: &mut T) {
    if let Err(e) = load_into(path.as_ref(), cfg) {
        panic!("conf.MustLoad failed: {e}");
    }
}

/// 环境变量展开：
/// - `${VAR}` / `$VAR` 替换为 env 值
/// - 未定义时替换为空字符串（对齐 Go 的 `os.ExpandEnv`）
fn expand_env(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'$' {
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }

        // "$$" -> "$"
        if i + 1 < bytes.len() && bytes[i + 1] == b'$' {
            out.push('$');
            i += 2;
            continue;
        }

        // ${VAR}
        if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j] != b'}' {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'}' {
                let key = &input[i + 2..j];
                out.push_str(&std::env::var(key).unwrap_or_default());
                i = j + 1;
                continue;
            }
            // unmatched "{", treat '$' as literal
            out.push('$');
            i += 1;
            continue;
        }

        // $VAR
        let mut j = i + 1;
        while j < bytes.len() {
            let c = bytes[j] as char;
            if !(c.is_ascii_alphanumeric() || c == '_') {
                break;
            }
            j += 1;
        }
        if j == i + 1 {
            out.push('$');
            i += 1;
            continue;
        }
        let key = &input[i + 1..j];
        out.push_str(&std::env::var(key).unwrap_or_default());
        i = j;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[derive(Debug, Default, serde::Deserialize, PartialEq, Eq)]
    struct TestCfg {
        name: String,
        port: u16,
    }

    fn tmp_file(name: &str, ext: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let uniq = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!("rz_core_conf_{name}_{uniq}.{ext}"));
        p
    }

    #[test]
    fn load_into_should_parse_yaml() {
        let path = tmp_file("ok", "yaml");
        std::fs::write(&path, "name: test\nport: 8080\n").unwrap();

        let mut cfg = TestCfg::default();
        load_into(&path, &mut cfg).unwrap();
        assert_eq!(
            cfg,
            TestCfg {
                name: "test".into(),
                port: 8080
            }
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_into_should_reject_non_yaml() {
        let mut cfg = TestCfg::default();
        let err = load_into("a.unknown", &mut cfg).unwrap_err();
        assert!(err.to_string().contains("unsupported config file type"));
    }

    #[test]
    fn load_into_should_parse_json() {
        let path = tmp_file("json", "json");
        std::fs::write(&path, r#"{"name":"test","port":8080}"#).unwrap();
        let mut cfg = TestCfg::default();
        load_into(&path, &mut cfg).unwrap();
        assert_eq!(
            cfg,
            TestCfg {
                name: "test".into(),
                port: 8080
            }
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_into_should_parse_toml() {
        let path = tmp_file("toml", "toml");
        std::fs::write(&path, "name = \"test\"\nport = 8080\n").unwrap();
        let mut cfg = TestCfg::default();
        load_into(&path, &mut cfg).unwrap();
        assert_eq!(
            cfg,
            TestCfg {
                name: "test".into(),
                port: 8080
            }
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn expand_env_should_work() {
        unsafe { std::env::set_var("RZ_CONF_X", "abc") };
        assert_eq!(
            super::expand_env("a=${RZ_CONF_X},b=$RZ_CONF_X"),
            "a=abc,b=abc"
        );
        unsafe { std::env::remove_var("RZ_CONF_X") };
        assert_eq!(super::expand_env("x=${RZ_CONF_X}"), "x=");
    }

    #[test]
    #[should_panic]
    fn must_load_should_panic_on_error() {
        let mut cfg = TestCfg::default();
        must_load("not-exist.yaml", &mut cfg);
    }
}
