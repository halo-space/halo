//! Rust API 生成用的“语义层辅助逻辑”。
//!
//! 目标：让 `code` 层只负责“把已经算好的信息渲染成文件”，
//! 解析/命名风格/类型映射/validate tag 转换等规则都放在语义层。

use anyhow::{Result, anyhow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Snake,
    LowerCamel,
    UpperCamel,
}

pub fn parse_style(style: &str) -> Result<Style> {
    match style {
        "rust_zero" => Ok(Style::Snake),
        "rustZero" => Ok(Style::LowerCamel),
        "RustZero" => Ok(Style::UpperCamel),
        other => Err(anyhow!(
            "unsupported style: {other} (expected rust_zero|rustZero|RustZero)"
        )),
    }
}

pub fn snake(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}

pub fn pascal(s: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            upper = true;
            continue;
        }
        if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn lower_camel_from_snake(s: &str) -> String {
    let p = pascal(s);
    let mut it = p.chars();
    let Some(first) = it.next() else {
        return String::new();
    };
    first.to_lowercase().collect::<String>() + it.as_str()
}

pub fn group_file_base(group_snake: &str, style: Style) -> String {
    match style {
        Style::Snake => group_snake.to_string(),
        Style::LowerCamel => lower_camel_from_snake(group_snake),
        Style::UpperCamel => pascal(group_snake),
    }
}

pub fn mod_decl_with_path(parent_dir: &str, module: &str, file_base: &str) -> String {
    // `module` stays snake_case for idiomatic rust modules.
    // Only the filename is styled; use `#[path="..."]` when they differ.
    if module == file_base {
        format!("pub mod {module};\n")
    } else {
        format!("#[path = \"{parent_dir}/{file_base}.rs\"]\npub mod {module};\n")
    }
}

// NOTE: tag parsing has moved to `crate::spec::api::{Tags, Tag}` (goctl-like).

pub fn type_to_rust(group_snake: &str, api_ty: &crate::spec::api::Type) -> String {
    match api_ty {
        crate::spec::api::Type::Any => "serde_json::Value".into(),
        crate::spec::api::Type::Primitive(p) => match p.as_str() {
            "string" => "String".into(),
            "bool" => "bool".into(),
            "int" | "int64" => "i64".into(),
            "int32" => "i32".into(),
            "uint" | "uint64" => "u64".into(),
            "uint32" => "u32".into(),
            "float32" => "f32".into(),
            "float64" => "f64".into(),
            other => other.to_string(),
        },
        crate::spec::api::Type::Ident(other) => {
            if group_snake.trim().is_empty() {
                format!("crate::types::{other}")
            } else {
                format!("crate::types::{group_snake}::types::{other}")
            }
        }
        crate::spec::api::Type::Array(inner) => {
            format!("Vec<{}>", type_to_rust(group_snake, inner))
        }
        crate::spec::api::Type::Pointer(inner) => {
            format!("Box<{}>", type_to_rust(group_snake, inner))
        }
        crate::spec::api::Type::Map { key, value } => format!(
            "std::collections::BTreeMap<{}, {}>",
            type_to_rust(group_snake, key),
            type_to_rust(group_snake, value)
        ),
    }
}

pub fn validate_tag_to_attrs(field_rust_ty: &str, validate_tag: &str) -> Vec<String> {
    // Map a small, useful subset of go-validate tags to `validator` crate:
    // - string: min/max -> length(min/max)
    // - numeric: min/max -> range(min/max)
    // - email/url/required -> corresponding validators
    let mut out = Vec::new();
    let items = validate_tag
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    let is_string = field_rust_ty == "String";
    let is_num = matches!(field_rust_ty, "i64" | "i32" | "u64" | "u32" | "f64" | "f32");

    let mut min_v: Option<i64> = None;
    let mut max_v: Option<i64> = None;
    for it in &items {
        if *it == "email" {
            out.push("#[validate(email)]".into());
            continue;
        }
        if *it == "url" {
            out.push("#[validate(url)]".into());
            continue;
        }
        if *it == "required" {
            out.push("#[validate(required)]".into());
            continue;
        }
        if let Some((k, v)) = it.split_once('=') {
            let v = v.parse::<i64>().ok();
            match (k.trim(), v) {
                ("min", Some(n)) => min_v = Some(n),
                ("max", Some(n)) => max_v = Some(n),
                _ => {}
            }
        }
    }

    if is_string {
        let mut parts = Vec::new();
        if let Some(n) = min_v
            && n > 0
        {
            parts.push(format!("min = {n}"));
        }
        if let Some(n) = max_v
            && n > 0
        {
            parts.push(format!("max = {n}"));
        }
        if !parts.is_empty() {
            out.push(format!("#[validate(length({}))]", parts.join(", ")));
        }
    } else if is_num {
        let mut parts = Vec::new();
        if let Some(n) = min_v {
            parts.push(format!("min = {n}"));
        }
        if let Some(n) = max_v {
            parts.push(format!("max = {n}"));
        }
        if !parts.is_empty() {
            out.push(format!("#[validate(range({}))]", parts.join(", ")));
        }
    }

    out
}

pub fn join_http_paths(prefix: &str, path: &str) -> String {
    let pfx = prefix.trim_end_matches('/');
    let mut p = path.to_string();
    if !p.starts_with('/') {
        p.insert(0, '/');
    }
    if pfx.is_empty() {
        return p;
    }
    if p == "/" {
        return pfx.to_string();
    }
    format!("{pfx}{p}")
}
