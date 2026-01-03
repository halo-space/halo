use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// goctl-like API Type expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum Type {
    Any,
    Primitive(String),
    Ident(String),
    Array(Box<Type>),
    Pointer(Box<Type>),
    Map { key: Box<Type>, value: Box<Type> },
}

/// Syntax (kept for parity; currently unused).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Syntax {
    pub version: String,
    #[serde(default)]
    pub doc: Vec<String>,
    #[serde(default)]
    pub comment: Vec<String>,
}

/// Import entry (parity with go-zero; currently unused for generation).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Import {
    pub value: String,
    #[serde(default)]
    pub doc: Vec<String>,
    #[serde(default)]
    pub comment: Vec<String>,
}

/// Stable API spec IR (go-zero aligned).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub info: Info,
    pub syntax: Syntax,
    pub imports: Vec<Import>,
    pub service: Service,
    /// Type definitions (`type Xxx { ... }`) for request/response schemas.
    pub types: Vec<TypeDef>,
}

impl Spec {
    /// Validate spec invariants before code generation.
    pub fn validate(&self) -> Result<()> {
        if self.service.name.trim().is_empty() {
            return Err(anyhow!("missing service name"));
        }
        if self.service.groups.is_empty() {
            return Err(anyhow!("missing service groups"));
        }
        for g in &self.service.groups {
            for r in &g.routes {
                if r.handler.trim().is_empty() {
                    return Err(anyhow!("missing handler name for route: {:?}", r.path));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Info {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub properties: BTreeMap<String, String>,
    #[serde(default)]
    pub root_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    /// Group-level annotations (typically `@server(...)`).
    pub annotations: Vec<Annotation>,
    pub routes: Vec<Route>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub method: HttpMethod,
    pub path: String,
    /// Handler name (derived from `@handler ...`).
    pub handler: String,
    /// Route doc (derived from `@doc ...`).
    pub doc: Option<String>,
    pub request: Option<Type>,
    pub response: Option<Type>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDef {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    /// Raw api type, e.g. `string`, `int64`, `[]Foo`.
    pub ty: Type,
    /// Raw tag content inside backticks, e.g. `json:\"id\",validate:\"min=1\"`.
    pub tag: Option<String>,
}

/// goctl-like struct tag item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub key: String,
    /// Full raw value between quotes, e.g. `foo,omitempty` or `min=1,max=2`.
    pub value: String,
    /// First segment before first comma (same as goctl Tag.Name).
    pub name: String,
    /// Remaining segments after first comma (same as goctl Tag.Options).
    pub options: Vec<String>,
}

/// goctl-like parsed tags collection.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Tags {
    tags: Vec<Tag>,
}

impl Tags {
    pub fn parse(tag: &str) -> Result<Tags> {
        let mut s = tag.trim();
        s = s.strip_prefix('`').unwrap_or(s);
        s = s.strip_suffix('`').unwrap_or(s);

        let bytes = s.as_bytes();
        let mut i = 0usize;
        let mut out: Vec<Tag> = Vec::new();

        while i < bytes.len() {
            // skip spaces
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() {
                break;
            }

            // parse key
            let key_start = i;
            while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
                i += 1;
            }
            if i == key_start || i >= bytes.len() || bytes[i] != b':' {
                break;
            }
            let key = &s[key_start..i];
            i += 1; // skip ':'

            if i >= bytes.len() || bytes[i] != b'"' {
                break;
            }
            i += 1; // skip '"'
            let val_start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            if i >= bytes.len() {
                break;
            }
            let value = &s[val_start..i];
            i += 1; // skip closing '"'

            let mut parts = value.split(',').map(|p| p.trim()).collect::<Vec<_>>();
            if parts.is_empty() {
                parts.push("");
            }
            let name = parts[0].to_string();
            let options = parts.into_iter().skip(1).map(|p| p.to_string()).collect();

            out.push(Tag {
                key: key.to_string(),
                value: value.to_string(),
                name,
                options,
            });
        }

        Ok(Tags { tags: out })
    }

    pub fn get(&self, key: &str) -> Option<&Tag> {
        self.tags.iter().find(|t| t.key == key)
    }

    // keys/all methods can be added when needed by downstream generators.
}

impl Field {
    pub fn tags(&self) -> Result<Tags> {
        match &self.tag {
            None => Ok(Tags::default()),
            Some(s) if s.trim().is_empty() => Ok(Tags::default()),
            Some(s) => Tags::parse(s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub name: String,
    /// Parsed key-value properties, goctl-style.
    pub properties: BTreeMap<String, String>,
    /// Single text payload (for annotations like `@doc "..."` when not represented elsewhere).
    pub text: Option<String>,
}

// NOTE: we intentionally do NOT keep the old `AnnotationArgs` enum in spec:
// goctl stores parsed annotation key-values directly as `Properties map[string]string`.
