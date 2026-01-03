fn unquote_if_needed(s: &str) -> String {
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s.trim_matches('"').to_string()
    } else {
        s.to_string()
    }
}
use anyhow::{Result, anyhow};

pub mod rs;

pub fn to_spec(ast: &crate::parse::api::Ast) -> Result<crate::spec::api::Spec> {
    let mut types = Vec::new();
    let mut groups = Vec::new();
    let mut top_routes = Vec::new();
    let mut service_name: Option<String> = None;
    let mut info_props = std::collections::BTreeMap::new();
    let mut root_prefix = String::new();
    let mut info_title = String::new();
    let mut info_desc = String::new();
    let mut info_version = String::new();
    let mut info_author = String::new();
    let mut info_email = String::new();

    if let Some(info) = &ast.info {
        for (k, v) in &info.properties {
            info_props.insert(k.clone(), v.clone());
            if k.eq_ignore_ascii_case("root_prefix") {
                let mut p = unquote_if_needed(v).trim().to_string();
                if !p.is_empty() && !p.starts_with('/') {
                    p = format!("/{p}");
                }
                if !p.is_empty() {
                    root_prefix = p;
                }
            }
            if info_title.is_empty() && k.eq_ignore_ascii_case("title") {
                info_title = unquote_if_needed(v);
            }
            if info_desc.is_empty() && k.eq_ignore_ascii_case("desc") {
                info_desc = unquote_if_needed(v);
            }
            if info_version.is_empty() && k.eq_ignore_ascii_case("version") {
                info_version = unquote_if_needed(v);
            }
            if info_author.is_empty() && k.eq_ignore_ascii_case("author") {
                info_author = unquote_if_needed(v);
            }
            if info_email.is_empty() && k.eq_ignore_ascii_case("email") {
                info_email = unquote_if_needed(v);
            }
        }
    }

    for item in &ast.items {
        match item {
            crate::parse::api::Item::Service(s) => {
                if let Some(existing) = &service_name {
                    if existing != &s.name {
                        return Err(anyhow!(
                            "service name mismatch: expected {}, found {}",
                            existing,
                            s.name
                        ));
                    }
                } else {
                    service_name = Some(s.name.clone());
                }
                groups.push(service_to_group(s)?)
            }
            crate::parse::api::Item::Route(r) => top_routes.push(route_to_spec(r)?),
            crate::parse::api::Item::Type(t) => types.push(type_to_spec(t)?),
        }
    }

    // 顶层 routes 也视作一个 group（没有 @server 注解）。
    if !top_routes.is_empty() {
        groups.insert(
            0,
            crate::spec::api::Group {
                annotations: Vec::new(),
                routes: top_routes,
            },
        );
    }

    let info = crate::spec::api::Info {
        properties: info_props,
        root_prefix,
        title: info_title,
        desc: info_desc,
        version: info_version,
        author: info_author,
        email: info_email,
    };

    Ok(crate::spec::api::Spec {
        info,
        syntax: crate::spec::api::Syntax::default(),
        imports: Vec::new(),
        service: crate::spec::api::Service {
            // 若 api 文件没有任何 service {}，这里先置空，由 pipeline 用文件名补齐。
            name: service_name.unwrap_or_default(),
            groups,
        },
        types,
    })
}

fn type_to_spec(t: &crate::parse::api::TypeDef) -> Result<crate::spec::api::TypeDef> {
    Ok(crate::spec::api::TypeDef {
        name: t.name.clone(),
        fields: t
            .fields
            .iter()
            .map(|f| crate::spec::api::Field {
                name: f.name.clone(),
                ty: parse_type_expr(&f.ty),
                tag: f.tag.clone(),
            })
            .collect(),
    })
}

fn service_to_group(svc: &crate::parse::api::Service) -> Result<crate::spec::api::Group> {
    let mut routes = Vec::new();
    for r in &svc.routes {
        routes.push(route_to_spec(r)?);
    }

    Ok(crate::spec::api::Group {
        annotations: annotations_to_spec(&svc.annotations),
        routes,
    })
}

fn route_to_spec(route: &crate::parse::api::Route) -> Result<crate::spec::api::Route> {
    let method = parse_method(&route.method)?;
    let handler = find_handler_name(&route.annotations).unwrap_or_default();
    let doc = find_doc(&route.annotations);

    Ok(crate::spec::api::Route {
        method,
        path: route.path.clone(),
        handler,
        doc,
        request: route.request.as_deref().map(parse_type_expr),
        response: route.response.as_deref().map(parse_type_expr),
    })
}

fn parse_method(s: &str) -> Result<crate::spec::api::HttpMethod> {
    match s.to_ascii_lowercase().as_str() {
        "get" => Ok(crate::spec::api::HttpMethod::Get),
        "post" => Ok(crate::spec::api::HttpMethod::Post),
        "put" => Ok(crate::spec::api::HttpMethod::Put),
        "delete" => Ok(crate::spec::api::HttpMethod::Delete),
        "patch" => Ok(crate::spec::api::HttpMethod::Patch),
        other => Err(anyhow!("unsupported http method: {other}")),
    }
}

fn annotations_to_spec(src: &[crate::parse::api::Annotation]) -> Vec<crate::spec::api::Annotation> {
    src.iter()
        .map(|a| {
            let mut properties = std::collections::BTreeMap::<String, String>::new();
            let mut text: Option<String> = None;
            match &a.args {
                crate::parse::api::AnnotationArgs::None => {}
                crate::parse::api::AnnotationArgs::Str(s) => {
                    // Keep the raw string payload; for goctl-like key-values, use `Map` form.
                    text = Some(s.clone());
                }
                crate::parse::api::AnnotationArgs::Map(kvs) => {
                    for (k, v) in kvs {
                        properties.insert(k.clone(), v.clone());
                    }
                }
            }
            crate::spec::api::Annotation {
                name: a.name.clone(),
                properties,
                text,
            }
        })
        .collect()
}

fn find_handler_name(anns: &[crate::parse::api::Annotation]) -> Option<String> {
    let a = anns.iter().find(|a| a.name == "handler")?;
    match &a.args {
        crate::parse::api::AnnotationArgs::Str(s) => Some(s.trim().to_string()),
        _ => None,
    }
}

fn find_doc(anns: &[crate::parse::api::Annotation]) -> Option<String> {
    let a = anns.iter().find(|a| a.name == "doc")?;
    match &a.args {
        crate::parse::api::AnnotationArgs::Str(s) => Some(s.trim().to_string()),
        _ => None,
    }
}

fn parse_type_expr(s: &str) -> crate::spec::api::Type {
    let s = s.trim();
    if s.is_empty() {
        return crate::spec::api::Type::Any;
    }
    if let Some(inner) = s.strip_prefix("[]") {
        return crate::spec::api::Type::Array(Box::new(parse_type_expr(inner)));
    }
    if let Some(inner) = s.strip_prefix('*') {
        return crate::spec::api::Type::Pointer(Box::new(parse_type_expr(inner)));
    }
    if s == "interface{}" || s == "any" {
        return crate::spec::api::Type::Any;
    }
    match s {
        "string" | "bool" | "int" | "int32" | "int64" | "uint" | "uint32" | "uint64"
        | "float32" | "float64" => crate::spec::api::Type::Primitive(s.to_string()),
        other => crate::spec::api::Type::Ident(other.to_string()),
    }
}
