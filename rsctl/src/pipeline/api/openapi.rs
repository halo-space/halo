//! OpenAPI v3 generation pipeline.
use anyhow::{Context, Result, anyhow};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Options {
    pub out_dir: PathBuf,
    pub api_file: PathBuf,
    pub filename: String,
    pub format: String,
    pub overwrite: bool,
}

pub fn run(opts: Options) -> Result<()> {
    if opts.api_file.as_os_str().is_empty() {
        return Err(anyhow!("--api is required"));
    }
    std::fs::create_dir_all(&opts.out_dir)
        .with_context(|| format!("create output dir {}", opts.out_dir.display()))?;

    let mut visited = HashSet::new();
    let ast = load_ast_recursive(&opts.api_file, true, &mut visited).context("parse api file")?;
    let mut spec = crate::semantic::api::to_spec(&ast).context("semantic to spec")?;

    let root_ast = crate::parse::api::parse_file(&opts.api_file).context("parse root api file")?;
    let root_prefix = extract_root_prefix(&root_ast)
        .or_else(|| extract_root_prefix_text(&opts.api_file).ok().flatten());
    if let Some(rp) = root_prefix {
        spec.info.root_prefix = rp;
    }

    // normalize service name
    if spec.service.name.trim().is_empty() {
        let fallback = opts
            .api_file
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "api".to_string());
        spec.service.name = fallback;
    }
    spec.validate().context("validate spec")?;

    let doc = build_openapi(&spec)?;
    let content = match opts.format.to_ascii_lowercase().as_str() {
        "yaml" => {
            let mut root = serde_yaml::Mapping::new();
            root.insert(
                serde_yaml::Value::String("openapi".to_string()),
                serde_yaml::Value::String("3.0.1".to_string()),
            );
            root.insert(
                serde_yaml::Value::String("info".to_string()),
                json_to_yaml(&doc.get("info").cloned().unwrap_or(Value::Null))?,
            );
            root.insert(
                serde_yaml::Value::String("paths".to_string()),
                json_to_yaml(&doc.get("paths").cloned().unwrap_or(Value::Null))?,
            );
            root.insert(
                serde_yaml::Value::String("components".to_string()),
                json_to_yaml(&doc.get("components").cloned().unwrap_or(Value::Null))?,
            );
            serde_yaml::to_string(&serde_yaml::Value::Mapping(root))?
        }
        _ => serde_json::to_string_pretty(&doc)?,
    };

    let filename = if opts.filename.is_empty() {
        "openapi".to_string()
    } else {
        opts.filename.clone()
    };
    let ext = if opts.format.eq_ignore_ascii_case("yaml") {
        "yaml"
    } else {
        "json"
    };
    let path = opts.out_dir.join(format!("{filename}.{ext}"));
    if path.exists() && !opts.overwrite {
        return Err(anyhow!(
            "output file exists (use --overwrite): {}",
            path.display()
        ));
    }
    std::fs::write(&path, content)
        .with_context(|| format!("write openapi to {}", path.display()))?;
    tracing::info!(path = %path.display(), "write openapi");
    Ok(())
}

fn build_openapi(spec: &crate::spec::api::Spec) -> Result<Value> {
    let mut paths = Map::new();
    let root_prefix = if !spec.info.root_prefix.is_empty() {
        spec.info.root_prefix.clone()
    } else {
        "/".to_string()
    };

    // Build component schemas from type definitions.
    let mut schemas = Map::new();
    for td in &spec.types {
        let mut props = Map::new();
        for f in &td.fields {
            let pname = field_json_name(f);
            props.insert(pname, type_to_schema(&f.ty));
        }
        schemas.insert(
            td.name.clone(),
            json!({
                "type": "object",
                "properties": Value::Object(props)
            }),
        );
    }

    for grp in &spec.service.groups {
        let prefix = group_prefix(grp).unwrap_or_default();
        for r in &grp.routes {
            let full_path = normalize_path(&join_paths(&root_prefix, &prefix, &r.path));
            let method_key = method_str(r.method.clone());
            let mut op = Map::new();
            if let Some(doc) = &r.doc {
                op.insert("summary".to_string(), Value::String(doc.clone()));
            }
            op.insert("operationId".to_string(), Value::String(r.handler.clone()));
            if let Some(req_ty) = &r.request {
                op.insert(
                    "requestBody".to_string(),
                    json!({
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": type_to_schema_ref(req_ty, &schemas)
                            }
                        }
                    }),
                );
            }
            op.insert(
                "responses".to_string(),
                json!({
                    "200": {
                        "description": "OK"
                    }
                }),
            );
            let entry = paths
                .entry(full_path)
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(map) = entry {
                map.insert(method_key, Value::Object(op));
            }
        }
    }

    // Build info object with optional license.
    let mut info_obj = Map::new();
    let title = if spec.info.title.is_empty() {
        spec.service.name.clone()
    } else {
        spec.info.title.clone()
    };
    info_obj.insert("title".to_string(), Value::String(title));
    let version = if spec.info.version.is_empty() {
        "1.0.0".to_string()
    } else {
        spec.info.version.clone()
    };
    info_obj.insert("version".to_string(), Value::String(version));
    if !spec.info.desc.is_empty() {
        info_obj.insert(
            "description".to_string(),
            Value::String(spec.info.desc.clone()),
        );
    }
    let lic_name = spec
        .info
        .properties
        .get("license.name")
        .cloned()
        .filter(|s| !s.is_empty());
    let lic_url = spec
        .info
        .properties
        .get("license.url")
        .cloned()
        .filter(|s| !s.is_empty());
    if lic_name.is_some() || lic_url.is_some() {
        let mut lic = Map::new();
        if let Some(n) = lic_name {
            lic.insert("name".to_string(), Value::String(n));
        }
        if let Some(u) = lic_url {
            lic.insert("url".to_string(), Value::String(u));
        }
        info_obj.insert("license".to_string(), Value::Object(lic));
    }

    // Assemble document with stable ordering: openapi -> info -> paths -> components.
    let mut root = Map::new();
    root.insert("openapi".to_string(), Value::String("3.0.1".to_string()));
    root.insert("info".to_string(), Value::Object(info_obj));
    root.insert("paths".to_string(), Value::Object(paths));
    root.insert(
        "components".to_string(),
        Value::Object({
            let mut comp = Map::new();
            comp.insert("schemas".to_string(), Value::Object(schemas));
            comp
        }),
    );

    Ok(Value::Object(root))
}

fn json_to_yaml(v: &Value) -> Result<serde_yaml::Value> {
    use serde_yaml::Value as Y;
    Ok(match v {
        Value::Null => Y::Null,
        Value::Bool(b) => Y::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Y::Number(i.into())
            } else if let Some(u) = n.as_u64() {
                Y::Number(u.into())
            } else if let Some(f) = n.as_f64() {
                Y::Number(serde_yaml::Number::from(f))
            } else {
                Y::Null
            }
        }
        Value::String(s) => Y::String(s.clone()),
        Value::Array(arr) => Y::Sequence(arr.iter().map(|e| json_to_yaml(e).unwrap()).collect()),
        Value::Object(map) => {
            let mut m = serde_yaml::Mapping::new();
            for (k, v) in map {
                m.insert(Y::String(k.clone()), json_to_yaml(v)?);
            }
            Y::Mapping(m)
        }
    })
}

fn type_to_schema_ref(ty: &crate::spec::api::Type, schemas: &Map<String, Value>) -> Value {
    match ty {
        crate::spec::api::Type::Ident(name) if schemas.contains_key(name) => {
            json!({"$ref": format!("#/components/schemas/{name}")})
        }
        other => type_to_schema(other),
    }
}

fn type_to_schema(ty: &crate::spec::api::Type) -> Value {
    use crate::spec::api::Type::*;
    match ty {
        Any => json!({"type": "object"}),
        Primitive(p) => primitive_schema(p),
        Ident(name) => json!({"type": "object", "title": name}),
        Array(inner) => json!({"type": "array", "items": type_to_schema(inner)}),
        Pointer(inner) => type_to_schema(inner),
        Map { value, .. } => json!({
            "type": "object",
            "additionalProperties": type_to_schema(value)
        }),
    }
}

fn primitive_schema(name: &str) -> Value {
    let n = name.to_ascii_lowercase();
    if n.contains("int64") {
        json!({"type": "integer", "format": "int64"})
    } else if n.contains("int32") || n == "int" {
        json!({"type": "integer", "format": "int32"})
    } else if n.contains("float") || n.contains("double") {
        json!({"type": "number"})
    } else if n.contains("bool") {
        json!({"type": "boolean"})
    } else {
        json!({"type": "string"})
    }
}

fn field_json_name(f: &crate::spec::api::Field) -> String {
    if let Ok(tags) = f.tags() {
        if let Some(t) = tags.get("json") {
            if !t.name.is_empty() && t.name != "-" {
                return t.name.clone();
            }
        }
    }
    f.name.clone()
}

fn method_str(m: crate::spec::api::HttpMethod) -> String {
    match m {
        crate::spec::api::HttpMethod::Get => "get",
        crate::spec::api::HttpMethod::Post => "post",
        crate::spec::api::HttpMethod::Put => "put",
        crate::spec::api::HttpMethod::Delete => "delete",
        crate::spec::api::HttpMethod::Patch => "patch",
    }
    .to_string()
}

fn group_prefix(group: &crate::spec::api::Group) -> Option<String> {
    group
        .annotations
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("server"))
        .and_then(|a| a.properties.get("prefix"))
        .cloned()
}

fn join_paths(root: &str, prefix: &str, path: &str) -> String {
    let mut parts = Vec::<String>::new();
    for p in [root, prefix, path] {
        if p.is_empty() {
            continue;
        }
        let trimmed = p.trim_matches('/');
        if trimmed.is_empty() {
            continue;
        }
        parts.push(trimmed.to_string());
    }
    let joined = format!("/{}", parts.join("/"));
    joined
}

fn normalize_path(p: &str) -> String {
    let mut out = p.replace("//", "/");
    if !out.starts_with('/') {
        out = format!("/{}", out);
    }
    out
}

fn load_ast_recursive(
    api_file: &PathBuf,
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

fn extract_root_prefix_text(path: &PathBuf) -> Result<Option<String>> {
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
        // Fallback: token after colon or first '/' on the same line.
        if let Some(colon_pos) = rest.find(':') {
            let token = rest[colon_pos + 1..]
                .trim_start()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}_{nanos}"))
    }

    #[test]
    fn should_generate_openapi_with_root_and_prefix() {
        let out_dir = tmp_dir("rsctl_openapi_out");
        let api_path = tmp_dir("rsctl_openapi_api").join("api.api");
        fs::create_dir_all(api_path.parent().unwrap()).unwrap();
        fs::create_dir_all(&out_dir).unwrap();

        let api_content = r#"
syntax = "v1"
info (
  root_prefix: "/api"
)
@server ( prefix: /api )
service user {
  @handler hello
  get /hello
}
"#;
        fs::write(&api_path, api_content).unwrap();

        run(Options {
            out_dir: out_dir.clone(),
            api_file: api_path.clone(),
            filename: "openapi".to_string(),
            format: "json".to_string(),
            overwrite: true,
        })
        .unwrap();

        let json_path = out_dir.join("openapi.json");
        let data = fs::read_to_string(&json_path).unwrap();
        let v: Value = serde_json::from_str(&data).unwrap();
        let paths = v.get("paths").and_then(|p| p.as_object()).unwrap();
        assert!(
            paths.contains_key("/api/api/hello"),
            "expected /api/api/hello"
        );
        let op = paths
            .get("/api/api/hello")
            .and_then(|m| m.get("get"))
            .and_then(|o| o.get("operationId"))
            .and_then(|s| s.as_str())
            .unwrap();
        assert_eq!(op, "hello");
    }
}
