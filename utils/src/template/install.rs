//! 模板安装相关能力。
//!
//! 目前提供把编译期内嵌的 `templates/` 目录释放到指定目录的能力。

use anyhow::{Context, Result};
use include_dir::Dir;
use std::path::Path;

/// 把编译期内嵌的 templates 目录解压/写入到目标目录。
pub fn extract_dir(dir: &Dir<'_>, dest_root: &Path) -> Result<()> {
    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::Dir(d) => {
                let out_dir = dest_root.join(d.path());
                std::fs::create_dir_all(&out_dir)
                    .with_context(|| format!("create dir failed: {}", out_dir.display()))?;
                extract_dir(d, dest_root)?;
            }
            include_dir::DirEntry::File(f) => {
                let out_file = dest_root.join(f.path());
                if let Some(parent) = out_file.parent() {
                    std::fs::create_dir_all(parent).with_context(|| {
                        format!("create parent dir failed: {}", parent.display())
                    })?;
                }
                std::fs::write(&out_file, f.contents())
                    .with_context(|| format!("write file failed: {}", out_file.display()))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use include_dir::include_dir;

    static FIXTURES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/fixtures");

    #[test]
    fn test_extract_dir_writes_files() {
        let tmp = std::env::temp_dir()
            .join("rsctl")
            .join("utils")
            .join("template_install_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        extract_dir(&FIXTURES, &tmp).unwrap();

        let a = tmp.join("a.txt");
        let b = tmp.join("nested").join("b.txt");
        assert!(a.is_file());
        assert!(b.is_file());
        assert_eq!(std::fs::read_to_string(a).unwrap().trim(), "hello");
        assert_eq!(std::fs::read_to_string(b).unwrap().trim(), "world");
    }
}
