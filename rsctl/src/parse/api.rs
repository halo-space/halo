use anyhow::{Context, Result};
use pest::Parser as PestParser;
use pest_derive::Parser as PestDeriveParser;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Ast {
    /// 顶层条目列表（`service {}` 或顶层 `get/post/...` 路由）。
    pub items: Vec<Item>,
    /// info 块（目前主要关心 version）。
    pub info: Option<Info>,
    /// 导入的其他 api 文件路径。
    pub imports: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Item {
    /// `service <name> { ... }`
    Service(Service),
    /// 顶层路由（不在任何 `service {}` 块中）。
    Route(Route),
    /// `type Xxx { ... }`
    Type(TypeDef),
}

#[derive(Debug, Clone)]
pub struct Service {
    /// service 名称（来自 `service <name> {}`）。
    pub name: String,
    /// service 级别注解（紧贴在 `service` 之前的 `@server(...)` 等）。
    pub annotations: Vec<Annotation>,
    /// service 内部路由列表。
    pub routes: Vec<Route>,
}

#[derive(Debug, Clone)]
pub struct Route {
    /// 路由级别注解（紧贴在路由语句之前的 `@handler` / `@doc` 等）。
    pub annotations: Vec<Annotation>,
    /// HTTP 方法（如 `get`/`post`/`put`/`delete`/`patch`）。
    pub method: String,
    /// 路由路径（如 `/user/info/:id`）。
    pub path: String,
    /// 请求类型名（可选，对应 `post /x (Req)` 中的 `Req`）。
    pub request: Option<String>,
    /// 响应类型名（可选，对应 `returns (Resp)` 中的 `Resp`）。
    pub response: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: String,
    pub tag: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Annotation {
    /// 注解名称（`@server` / `@doc` / `@handler` 等，不含 `@`）。
    pub name: String,
    /// 注解参数（无参/单值/kv 映射）。
    pub args: AnnotationArgs,
}

#[derive(Debug, Clone, Default)]
pub struct Info {
    pub properties: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub enum AnnotationArgs {
    /// 无参数：例如 `@server` 或 `@server()`
    None,
    /// 单值参数：例如 `@doc "foo"` / `@handler login` / `@doc foo`
    Str(String),
    /// kv 参数：例如 `@server ( prefix: /v1 group: Foo )`
    Map(Vec<(String, String)>),
}

#[derive(PestDeriveParser)]
#[grammar = "parse/api.pest"]
struct Parser;

pub fn parse(input: &str) -> Result<Ast> {
    let mut pairs = Parser::parse(Rule::file, input).context("parse api dsl")?;
    let file = pairs.next().context("parse api dsl: missing file pair")?;

    let mut items: Vec<Item> = Vec::new();
    let mut pending_annotations: Vec<Annotation> = Vec::new();
    let mut info: Option<Info> = None;
    let mut imports: Vec<String> = Vec::new();

    for p in file.into_inner() {
        match p.as_rule() {
            Rule::syntax_stmt => {
                // ignore
            }
            Rule::info_block => {
                info = Some(parse_info_block(p)?);
            }
            Rule::import_stmt => {
                if let Some(path) = parse_import_stmt(p)? {
                    imports.push(path);
                }
            }
            Rule::type_group_block => {
                let tds = parse_type_group_block(p)?;
                for td in tds {
                    items.push(Item::Type(td));
                }
            }
            Rule::type_block => {
                let td = parse_type_block(p)?;
                items.push(Item::Type(td));
            }
            Rule::annotation_stmt => {
                if let Some(ann) = parse_annotation_stmt(p)? {
                    pending_annotations.push(ann);
                }
            }
            Rule::route_stmt => {
                let mut route = parse_route_stmt(p)?;
                route.annotations = std::mem::take(&mut pending_annotations);
                items.push(Item::Route(route));
            }
            Rule::service_block => {
                let mut svc = parse_service_block(p)?;
                svc.annotations = std::mem::take(&mut pending_annotations);
                items.push(Item::Service(svc));
            }
            _ => {}
        }
    }

    Ok(Ast {
        items,
        info,
        imports,
    })
}

fn parse_type_block(pair: pest::iterators::Pair<Rule>) -> Result<TypeDef> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .context("type: missing name")?
        .as_str()
        .to_string();

    let mut fields: Vec<Field> = Vec::new();
    for p in inner {
        if p.as_rule() == Rule::field_stmt {
            fields.push(parse_field_stmt(p)?);
        }
    }

    Ok(TypeDef { name, fields })
}

fn parse_field_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Field> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .context("field: missing name")?
        .as_str()
        .to_string();
    let ty = inner
        .next()
        .context("field: missing type")?
        .as_str()
        .to_string();

    let tag = inner.next().map(|p| {
        // strip surrounding backticks
        let s = p.as_str();
        s.strip_prefix('`')
            .and_then(|t| t.strip_suffix('`'))
            .unwrap_or(s)
            .to_string()
    });

    Ok(Field { name, ty, tag })
}

pub fn parse_file(path: impl AsRef<Path>) -> Result<Ast> {
    let path = path.as_ref();
    let input = std::fs::read_to_string(path)
        .with_context(|| format!("read api file: {}", path.display()))?;
    parse(&input).with_context(|| format!("parse api file: {}", path.display()))
}

fn parse_annotation_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Option<Annotation>> {
    let mut inner = pair.into_inner();
    let Some(p) = inner.next() else {
        return Ok(None);
    };
    if p.as_rule() != Rule::annotation {
        return Ok(None);
    }
    Ok(Some(parse_annotation(p)?))
}

fn parse_annotation(pair: pest::iterators::Pair<Rule>) -> Result<Annotation> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .context("annotation: missing name")?
        .as_str()
        .to_string();

    let args = match inner.next() {
        None => AnnotationArgs::None,
        Some(a) => match a.as_rule() {
            Rule::annotation_args => parse_annotation_args(a)?,
            _ => AnnotationArgs::None,
        },
    };

    Ok(Annotation { name, args })
}

fn parse_info_block(pair: pest::iterators::Pair<Rule>) -> Result<Info> {
    let mut properties = Vec::new();
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::kv => push_kv(&mut properties, p)?,
            Rule::kv_list => {
                for kv in p.into_inner().filter(|x| x.as_rule() == Rule::kv) {
                    push_kv(&mut properties, kv)?;
                }
            }
            _ => {}
        }
    }
    Ok(Info { properties })
}

fn push_kv(props: &mut Vec<(String, String)>, kv: pest::iterators::Pair<Rule>) -> Result<()> {
    let mut kv_inner = kv.into_inner();
    let key = kv_inner
        .next()
        .context("info kv: missing key")?
        .as_str()
        .to_string();
    let val = kv_inner
        .next()
        .context("info kv: missing value")?
        .as_str()
        .to_string();
    props.push((key, val));
    Ok(())
}

fn parse_import_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Option<String>> {
    let mut inner = pair.into_inner();
    let Some(v) = inner.next() else {
        return Ok(None);
    };
    let s = match v.as_rule() {
        Rule::string => unquote(v.as_str()),
        _ => v.as_str().to_string(),
    };
    Ok(Some(s))
}

fn parse_annotation_args(pair: pest::iterators::Pair<Rule>) -> Result<AnnotationArgs> {
    let mut inner = pair.into_inner();
    let Some(first) = inner.next() else {
        return Ok(AnnotationArgs::None);
    };
    match first.as_rule() {
        Rule::string => Ok(AnnotationArgs::Str(unquote(first.as_str()))),
        // 单值参数：@handler login / @server() / @doc foo 等
        // 这里把 bare/path/duration 统一当做字符串存起来。
        Rule::bare | Rule::path | Rule::duration => {
            Ok(AnnotationArgs::Str(first.as_str().to_string()))
        }
        Rule::kv_list => {
            let mut kvs = Vec::new();
            for kv in first.into_inner() {
                if kv.as_rule() == Rule::kv {
                    let mut it = kv.into_inner();
                    let k = it.next().context("kv: missing key")?.as_str().to_string();
                    let v = it.next().context("kv: missing value")?.as_str().to_string();
                    kvs.push((k, unquote_if_needed(&v)));
                }
            }
            Ok(AnnotationArgs::Map(kvs))
        }
        _ => Ok(AnnotationArgs::None),
    }
}

fn parse_route_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Route> {
    let mut inner = pair.into_inner();
    let method = inner
        .next()
        .context("route: missing method")?
        .as_str()
        .to_string();
    let path = inner
        .next()
        .context("route: missing path")?
        .as_str()
        .to_string();

    let mut request: Option<String> = None;
    let mut response: Option<String> = None;
    for p in inner {
        match p.as_rule() {
            Rule::request_part => {
                if let Some(t) = p.into_inner().find(|x| x.as_rule() == Rule::type_ref) {
                    request = Some(t.as_str().to_string());
                }
            }
            Rule::response_part => {
                if let Some(t) = p.into_inner().find(|x| x.as_rule() == Rule::type_ref) {
                    response = Some(t.as_str().to_string());
                }
            }
            _ => {}
        }
    }

    Ok(Route {
        annotations: vec![],
        method,
        path,
        request,
        response,
    })
}

fn parse_service_block(pair: pest::iterators::Pair<Rule>) -> Result<Service> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .context("service: missing name")?
        .as_str()
        .to_string();

    let mut routes: Vec<Route> = Vec::new();
    let mut pending_annotations: Vec<Annotation> = Vec::new();

    for p in inner {
        match p.as_rule() {
            Rule::annotation_stmt => {
                if let Some(ann) = parse_annotation_stmt(p)? {
                    pending_annotations.push(ann);
                }
            }
            Rule::route_stmt => {
                let mut route = parse_route_stmt(p)?;
                route.annotations = std::mem::take(&mut pending_annotations);
                routes.push(route);
            }
            _ => {}
        }
    }

    Ok(Service {
        name,
        annotations: vec![],
        routes,
    })
}

fn unquote(s: &str) -> String {
    if let Some(stripped) = s.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        stripped.replace("\\\"", "\"")
    } else {
        s.to_string()
    }
}

fn unquote_if_needed(s: &str) -> String {
    if s.starts_with('"') && s.ends_with('"') {
        unquote(s)
    } else {
        s.to_string()
    }
}

fn parse_type_group_block(pair: pest::iterators::Pair<Rule>) -> Result<Vec<TypeDef>> {
    let mut out: Vec<TypeDef> = Vec::new();
    for p in pair.into_inner() {
        if p.as_rule() == Rule::type_group_item {
            let mut inner = p.into_inner();
            let name = inner
                .next()
                .context("type group item: missing name")?
                .as_str()
                .to_string();
            let mut fields: Vec<Field> = Vec::new();
            for fp in inner {
                if fp.as_rule() == Rule::field_stmt {
                    fields.push(parse_field_stmt(fp)?);
                }
            }
            out.push(TypeDef { name, fields });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_should_work() {
        let input = r#"
// 空内容
@server()

@doc "foo"

get /ping

@server (
  prefix: /v1
  group: Foo
)
service user {
  @doc "登录"
  @handler login
  post /user/login (LoginReq) returns (LoginResp)

  @handler getUserInfo
  get /user/info/:id (GetUserInfoReq) returns (GetUserInfoResp)
}
"#;

        let ast = parse(input).unwrap();
        assert!(!ast.items.is_empty());
    }

    #[test]
    fn parse_type_group_should_work() {
        let input = r#"
syntax = "v1"

type (
  LoginReq {
    Username string `json:"username"`
    Password string `json:"password"`
  }
  LoginResp {
    Id int64 `json:"id"`
  }
)

@server (group: login prefix: /v1)
service user {
  @handler login
  post /user/login (LoginReq) returns (LoginResp)
}
"#;

        let ast = parse(input).unwrap();
        let types = ast
            .items
            .iter()
            .filter(|it| matches!(it, Item::Type(_)))
            .count();
        assert_eq!(types, 2);
    }
}
