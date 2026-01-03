// Flattened: keep everything under `gen/src/api/rs.rs` (no `api/rs/` directory).

pub mod shared {
    use crate::generate::artifact::{Artifact, Artifacts};
    use crate::semantic::api::rs as semantic;
    use anyhow::{Context, Result, anyhow};
    use std::path::{Path, PathBuf};
    use utils::render;

    fn normalize_service_name(service_name: &str) -> String {
        semantic::snake(service_name)
    }

    fn find_kv<'a>(ann: &'a crate::spec::api::Annotation, key: &str) -> Option<&'a str> {
        ann.properties.get(key).map(|s| s.as_str())
    }

    fn find_annotation<'a>(
        anns: &'a [crate::spec::api::Annotation],
        name: &str,
    ) -> Option<&'a crate::spec::api::Annotation> {
        anns.iter().find(|a| a.name == name)
    }

    fn group_key(group: &crate::spec::api::Group) -> String {
        // - 显式 @server(group="x") => 分组 x
        // - 否则默认组 ""（不落目录）
        let Some(srv) = find_annotation(&group.annotations, "server") else {
            return String::new();
        };
        find_kv(srv, "group")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(semantic::snake)
            .unwrap_or_default()
    }

    fn group_prefix(group: &crate::spec::api::Group) -> Option<String> {
        let srv = find_annotation(&group.annotations, "server")?;
        find_kv(srv, "prefix").map(|s| s.to_string())
    }

    fn group_middleware(group: &crate::spec::api::Group) -> Option<String> {
        let srv = find_annotation(&group.annotations, "server")?;
        find_kv(srv, "middleware").map(|s| s.to_string())
    }

    fn group_auth(group: &crate::spec::api::Group) -> Option<String> {
        let srv = find_annotation(&group.annotations, "server")?;
        find_kv(srv, "auth").map(|s| s.to_string())
    }

    // tag/type/validate/path 等规则已上移到语义层：`crate::semantic::api::rs`

    fn api_template_dir(template_root: &Path) -> PathBuf {
        // API 相关模板目录：`templates/api/rs/`
        template_root.join("api").join("rs")
    }

    fn rust_template_dir(template_root: &Path) -> PathBuf {
        // Rust 通用模板目录：`templates/rs/`
        template_root.join("rs")
    }

    fn must_exist(dir: &Path, file: &str) -> Result<PathBuf> {
        let p = dir.join(file);
        if p.is_file() {
            Ok(p)
        } else {
            Err(anyhow!("missing template file: {}", p.display()))
        }
    }

    fn read_template(dir: &Path, file: &str) -> Result<String> {
        let p = must_exist(dir, file)?;
        std::fs::read_to_string(&p).with_context(|| format!("read template: {}", p.display()))
    }

    #[derive(Debug, Clone)]
    struct Templates {
        cargo_toml: String,
        context: String,
        etc: String,
        main: String,
        config: String,
        routes: String,
        handler: String,
        logic: String,
        types: String,
        middleware: String,
    }

    fn load_templates(template_root: &Path) -> Result<Templates> {
        // Fail fast if any required template is missing.
        let api_dir = api_template_dir(template_root);
        let rs_dir = rust_template_dir(template_root);

        let _ = must_exist(&rs_dir, "Cargo.api.toml.tpl")?;

        let _ = must_exist(&api_dir, "context.tpl")?;
        let _ = must_exist(&api_dir, "etc.tpl")?;
        let _ = must_exist(&api_dir, "main.tpl")?;
        let _ = must_exist(&api_dir, "config.tpl")?;
        let _ = must_exist(&api_dir, "routes.tpl")?;
        let _ = must_exist(&api_dir, "handler.tpl")?;
        let _ = must_exist(&api_dir, "logic.tpl")?;
        let _ = must_exist(&api_dir, "types.tpl")?;
        let _ = must_exist(&api_dir, "middleware.tpl")?;

        Ok(Templates {
            cargo_toml: read_template(&rs_dir, "Cargo.api.toml.tpl")?,
            context: read_template(&api_dir, "context.tpl")?,
            etc: read_template(&api_dir, "etc.tpl")?,
            main: read_template(&api_dir, "main.tpl")?,
            config: read_template(&api_dir, "config.tpl")?,
            routes: read_template(&api_dir, "routes.tpl")?,
            handler: read_template(&api_dir, "handler.tpl")?,
            logic: read_template(&api_dir, "logic.tpl")?,
            types: read_template(&api_dir, "types.tpl")?,
            middleware: read_template(&api_dir, "middleware.tpl")?,
        })
    }

    struct Generator<'a> {
        spec: &'a crate::spec::api::Spec,

        merge: bool,
        style: semantic::Style,

        service_name: String,

        rsctl_version: String,

        // group_key -> list of handler names
        // - group_key == "" 表示未显式声明 group（默认组）
        group_handlers: std::collections::BTreeMap<String, Vec<String>>,

        rest_features: String,
        validator_dep: String,
        auth_dep: String,

        templates: Templates,
        base_ctx: render::Context,
    }

    impl<'a> Generator<'a> {
        fn run(&self, files: &mut Vec<Artifact>) -> Result<()> {
            self.cargo_toml(files)?;
            self.etc(files)?;
            self.entry(files)?;
            self.config(files)?;
            self.service_context(files)?;
            self.handler_root(files);
            self.routes(files)?;
            self.middleware(files)?;
            self.logic_root(files);
            self.types_root(files);
            self.handlers(files)?;
            self.logic(files)?;
            self.types(files)?;
            Ok(())
        }

        fn cargo_toml(&self, files: &mut Vec<Artifact>) -> Result<()> {
            files.push(Artifact {
                rel_path: "Cargo.toml".into(),
                content: {
                    let tpl_ctx = self
                        .base_ctx
                        .clone()
                        .set_str("webDep", "")
                        .set_str("restFeatures", &self.rest_features)
                        .set_str("validatorDep", &self.validator_dep)
                        .set_str("authDep", &self.auth_dep);
                    render::render(&self.templates.cargo_toml, &tpl_ctx)
                        .context("render Cargo.toml.tpl")?
                },
            });
            Ok(())
        }

        fn etc(&self, files: &mut Vec<Artifact>) -> Result<()> {
            files.push(Artifact {
                rel_path: format!("etc/{}.yaml", self.service_name).into(),
                content: {
                    let c = render::render(&self.templates.etc, &self.base_ctx)
                        .context("render etc.tpl")?;
                    c
                },
            });
            Ok(())
        }

        fn entry(&self, files: &mut Vec<Artifact>) -> Result<()> {
            files.push(Artifact {
                rel_path: format!("{}.rs", self.service_name).into(),
                content: render::render(&self.templates.main, &self.base_ctx)
                    .context("render main.tpl")?,
            });
            Ok(())
        }

        fn config(&self, files: &mut Vec<Artifact>) -> Result<()> {
            files.push(Artifact {
                rel_path: "config.rs".into(),
                content: {
                    let tpl_ctx = self.base_ctx.clone().set_str("auth", "");
                    render::render(&self.templates.config, &tpl_ctx).context("render config.tpl")?
                },
            });
            Ok(())
        }

        fn service_context(&self, files: &mut Vec<Artifact>) -> Result<()> {
            use std::collections::BTreeSet;

            fn to_snake(name: &str) -> String {
                let mut out = String::new();
                let mut prev_lower = false;
                for ch in name.chars() {
                    if ch == '_' || ch == '-' {
                        out.push('_');
                        prev_lower = false;
                        continue;
                    }
                    if ch.is_ascii_uppercase() {
                        if prev_lower {
                            out.push('_');
                        }
                        out.push(ch.to_ascii_lowercase());
                        prev_lower = false;
                    } else {
                        out.push(ch);
                        prev_lower = true;
                    }
                }
                out.trim_matches('_').to_string()
            }

            let mut mws: BTreeSet<String> = BTreeSet::new();
            for g in &self.spec.service.groups {
                if let Some(m) = group_middleware(g) {
                    mws.insert(to_snake(&m));
                }
            }

            let imports = if mws.is_empty() {
                "".to_string()
            } else {
                "use halo::rest::Middleware;".to_string()
            };
            files.push(Artifact {
                rel_path: "svc.rs".into(),
                content: {
                    let tpl_ctx = self.base_ctx.clone().set_str("imports", &imports);
                    render::render(&self.templates.context, &tpl_ctx)
                        .context("render context.tpl")?
                },
            });
            Ok(())
        }

        fn handler_root(&self, files: &mut Vec<Artifact>) {
            let mut handler_root = "/// generated by rsctl\n".to_string();
            handler_root.push('\n');
            handler_root.push_str("pub mod routes;\n");
            if self.group_handlers.contains_key("") {
                handler_root.push_str("pub mod handler;\n");
            }
            for g in self.group_handlers.keys() {
                if g.is_empty() {
                    continue;
                }
                let file_base = semantic::group_file_base(g, self.style);
                handler_root.push_str(&semantic::mod_decl_with_path("handler", g, &file_base));
            }
            files.push(Artifact {
                rel_path: "handler.rs".into(),
                content: handler_root,
            });
        }
    }

    impl<'a> Generator<'a> {
        fn routes(&self, files: &mut Vec<Artifact>) -> Result<()> {
            use std::collections::BTreeMap;

            let mut routes_additions = String::new();

            // winner map to resolve conflicts with prefixes/middlewares/auth
            #[derive(Clone)]
            struct Winner {
                score: i32,
                service_idx: usize,
            }
            let mut winners: BTreeMap<String, Winner> = BTreeMap::new();

            let score_of = |prefix: &str, mw: &Option<String>, jwt: &Option<String>| -> i32 {
                let mut s = 0;
                if jwt.is_some() {
                    s += 100;
                }
                if mw.is_some() {
                    s += 10;
                }
                if !prefix.trim().is_empty() {
                    s += 1;
                }
                s
            };

            for (idx, group) in self.spec.service.groups.iter().enumerate() {
                let prefix = group_prefix(group).unwrap_or_default();
                let mw = group_middleware(group);
                let jwt = group_auth(group);
                let score = score_of(&prefix, &mw, &jwt);

                for r in &group.routes {
                    if r.handler.trim().is_empty() {
                        continue;
                    }
                    let method = match r.method {
                        crate::spec::api::HttpMethod::Get => "GET",
                        crate::spec::api::HttpMethod::Post => "POST",
                        crate::spec::api::HttpMethod::Put => "PUT",
                        crate::spec::api::HttpMethod::Delete => "DELETE",
                        crate::spec::api::HttpMethod::Patch => "PATCH",
                    };
                    let full_path = semantic::join_http_paths(&prefix, &r.path);
                    let route_key = format!("{method} {full_path}");

                    match winners.get(&route_key) {
                        None => {
                            winners.insert(
                                route_key,
                                Winner {
                                    score,
                                    service_idx: idx,
                                },
                            );
                        }
                        Some(prev) if score > prev.score => {
                            winners.insert(
                                route_key,
                                Winner {
                                    score,
                                    service_idx: idx,
                                },
                            );
                        }
                        Some(prev) if score == prev.score && prev.service_idx != idx => {
                            return Err(anyhow::anyhow!(
                                "duplicate route detected with same priority: `{}`.\nPlease remove duplicates or keep only one @server(...) variant.",
                                route_key
                            ));
                        }
                        _ => {}
                    }
                }
            }

            for (idx, grp) in self.spec.service.groups.iter().enumerate() {
                let group = group_key(grp);
                let group_mod = if group.is_empty() {
                    "handler"
                } else {
                    group.as_str()
                };
                let prefix = group_prefix(grp).unwrap_or_default();
                let _mw = group_middleware(grp);
                let _jwt = group_auth(grp);

                let mut any = false;
                let mut block = String::new();
                let mut route_entries = String::new();
                for r in &grp.routes {
                    if r.handler.trim().is_empty() {
                        continue;
                    }
                    let h = semantic::snake(&r.handler);
                    let method = match r.method {
                        crate::spec::api::HttpMethod::Get => "GET",
                        crate::spec::api::HttpMethod::Post => "POST",
                        crate::spec::api::HttpMethod::Put => "PUT",
                        crate::spec::api::HttpMethod::Delete => "DELETE",
                        crate::spec::api::HttpMethod::Patch => "PATCH",
                    };

                    let full_path = semantic::join_http_paths(&prefix, &r.path);
                    let route_key = format!("{method} {full_path}");
                    let Some(w) = winners.get(&route_key) else {
                        continue;
                    };
                    if w.service_idx != idx {
                        continue;
                    }

                    any = true;
                    route_entries.push_str(&format!(
                        "        Route::new(Method::{method}, \"{path}\", crate::handler::{group_mod}::{h}(ctx.clone())),\n",
                        path = r.path
                    ));
                }

                if any {
                    if !group.is_empty() {
                        block.push_str(&format!("    server.set_debug_group(\"{}\");\n", group));
                    }
                    block.push_str("    server\n");
                    if !prefix.trim().is_empty() {
                        block.push_str(&format!("        .with_prefix(\"{prefix}\")\n"));
                    }
                    block.push_str("        .add_routes(vec![\n");
                    block.push_str(&route_entries);
                    block.push_str("        ]);\n");
                    if !group.is_empty() {
                        block.push_str("    server.clear_debug_group();\n");
                    }
                    routes_additions.push_str("{\n");
                    routes_additions.push_str(&block);
                    routes_additions.push_str("}\n");
                }
            }

            let routes_ctx = self
                .base_ctx
                .clone()
                .set_str("imports", "")
                .set_str("routesAdditions", routes_additions);
            files.push(Artifact {
                rel_path: "handler/routes.rs".into(),
                content: render::render(&self.templates.routes, &routes_ctx)
                    .context("render routes.tpl")?,
            });
            Ok(())
        }
    }

    impl<'a> Generator<'a> {
        fn middleware(&self, files: &mut Vec<Artifact>) -> Result<()> {
            use std::collections::BTreeSet;

            fn to_snake(name: &str) -> String {
                let mut out = String::new();
                let mut prev_lower = false;
                for ch in name.chars() {
                    if ch == '_' || ch == '-' {
                        out.push('_');
                        prev_lower = false;
                        continue;
                    }
                    if ch.is_ascii_uppercase() {
                        if prev_lower {
                            out.push('_');
                        }
                        out.push(ch.to_ascii_lowercase());
                        prev_lower = false;
                    } else {
                        out.push(ch);
                        prev_lower = true;
                    }
                }
                out.trim_matches('_').to_string()
            }

            let mut mws: BTreeSet<String> = BTreeSet::new();
            for g in &self.spec.service.groups {
                if let Some(m) = group_middleware(g) {
                    mws.insert(to_snake(&m));
                }
            }

            let mut root = String::new();
            root.push_str("/// generated by rsctl\n");
            root.push_str("pub mod prelude;\n");
            for m in &mws {
                root.push_str(&format!("pub mod {m};\n"));
            }
            files.push(Artifact {
                rel_path: "middleware.rs".into(),
                content: root,
            });
            files.push(Artifact {
                rel_path: "middleware/prelude.rs".into(),
                content: "// generated by rsctl\n".to_string(),
            });

            for m in &mws {
                let name_snake = m;
                let name_pascal = semantic::pascal(name_snake);
                files.push(Artifact {
                    rel_path: format!("middleware/{name_snake}.rs").into(),
                    content: {
                        let imports = [
                            "use halo::rest::{self, HandlerFunc, Middleware};",
                            "use hyper::Body;",
                            "use http::Request;",
                        ]
                        .join("\n");
                        let ctx = self
                            .base_ctx
                            .clone()
                            .set_str("name", name_snake)
                            .set_str("Name", &name_pascal)
                            .set_str("imports", &imports);
                        render::render(&self.templates.middleware, &ctx)
                            .context("render middleware.tpl")?
                    },
                });
            }
            Ok(())
        }
    }

    impl<'a> Generator<'a> {
        fn logic_root(&self, files: &mut Vec<Artifact>) {
            let mut logic_root = "/// generated by rsctl\n".to_string();
            if self.group_handlers.contains_key("") {
                if self.merge {
                    logic_root.push_str("pub mod logic;\npub use logic::*;\n");
                } else if let Some(hs) = self.group_handlers.get("") {
                    for h in hs {
                        logic_root.push_str(&format!("pub mod {h};\n"));
                    }
                    logic_root.push('\n');
                    for h in hs {
                        let logic_name = semantic::pascal(h);
                        logic_root.push_str(&format!("pub use {h}::{logic_name};\n"));
                    }
                }
            }
            for g in self.group_handlers.keys() {
                if g.is_empty() {
                    continue;
                }
                let file_base = semantic::group_file_base(g, self.style);
                logic_root.push_str(&semantic::mod_decl_with_path("logic", g, &file_base));
            }
            files.push(Artifact {
                rel_path: "logic.rs".into(),
                content: logic_root,
            });
        }
    }

    impl<'a> Generator<'a> {
        fn types_root(&self, files: &mut Vec<Artifact>) {
            let mut types_root = "/// generated by rsctl\n".to_string();
            if self.group_handlers.contains_key("") {
                types_root.push_str("pub mod types;\npub use types::*;\n");
            }
            for g in self.group_handlers.keys() {
                if g.is_empty() {
                    continue;
                }
                let file_base = semantic::group_file_base(g, self.style);
                types_root.push_str(&semantic::mod_decl_with_path("types", g, &file_base));
            }
            files.push(Artifact {
                rel_path: "types.rs".into(),
                content: types_root,
            });
        }
    }

    impl<'a> Generator<'a> {
        fn handlers(&self, files: &mut Vec<Artifact>) -> Result<()> {
            use serde_json::Value;

            fn root_ident<'a>(t: &'a crate::spec::api::Type) -> Option<&'a str> {
                match t {
                    crate::spec::api::Type::Ident(s) => Some(s.as_str()),
                    crate::spec::api::Type::Array(inner)
                    | crate::spec::api::Type::Pointer(inner) => root_ident(inner),
                    _ => None,
                }
            }

            let type_map: std::collections::BTreeMap<String, &crate::spec::api::TypeDef> = self
                .spec
                .types
                .iter()
                .map(|t| (t.name.clone(), t))
                .collect();

            // Render a single handler (no loops in template).
            let render_one = |_imports: &str,
                              g: &str,
                              h: &str,
                              is_default_group: bool|
             -> Result<String> {
                // Find a representative route in this group with this handler.
                let route = self
                    .spec
                    .service
                    .groups
                    .iter()
                    .filter(|grp| group_key(grp) == g)
                    .flat_map(|grp| grp.routes.iter())
                    .find(|r| semantic::snake(&r.handler) == h);

                let request_spec = route.and_then(|r| r.request.as_ref());
                let response_spec = route.and_then(|r| r.response.as_ref());
                let doc = route.and_then(|r| r.doc.as_ref());

                let req_rust = request_spec
                    .map(|t| semantic::type_to_rust(g, t))
                    .unwrap_or_default();

                let mut needs_validate = false;
                if let Some(req_name) = request_spec.and_then(root_ident)
                    && let Some(td) = type_map.get(req_name)
                {
                    for f in &td.fields {
                        let tags = f.tags()?;
                        if tags.get("validate").is_some() {
                            needs_validate = true;
                        }
                    }
                }

                let has_req = request_spec.is_some();
                let has_resp = response_spec.is_some();

                // Template fields
                let doc_str = doc.map(|d| format!("/// {}\n", d)).unwrap_or_default();
                let handler_name = h.to_string();

                let svc_ctx_param = "svc_ctx: Arc<ServiceContext>".to_string();
                let req_param = "mut req: http::Request<Body>".to_string();
                let return_type = "Response<Body>".to_string();
                let logic_struct = semantic::pascal(h);
                let logic_mod = if is_default_group {
                    "crate::logic".to_string()
                } else {
                    format!("crate::logic::{g}::logic", g = g)
                };
                let logic_type = logic_struct.clone();
                let logic_ctor_args = "svc_ctx".to_string();
                let req_value_expr = if has_req {
                    "req".to_string()
                } else {
                    String::new()
                };
                let ok_resp_fmt = "halo::rest::http::ok_json(&{RESP}).unwrap_or_else(|e| halo::rest::http::error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))".to_string();
                let ok_no_resp_fmt = "halo::rest::http::ok()".to_string();
                let err_fmt =
                    "halo::rest::http::error(StatusCode::INTERNAL_SERVER_ERROR, {E}.to_string())"
                        .to_string();
                let bad_request_fmt = "halo::rest::http::bad_request({E}.to_string())".to_string();

                // imports
                let mut imports = Vec::new();
                imports.push("use std::sync::Arc;".to_string());
                imports.push("use hyper::Body;".to_string());
                imports.push("use halo::rest as rest;".to_string());
                if has_req {
                    imports.push("use halo::rest::http::request;".to_string());
                }
                imports.push("use http::{Response, StatusCode};".to_string());
                imports.push("use crate::svc::ServiceContext;".to_string());
                imports.push(format!("use {logic_mod}::{logic_type};"));

                let tpl_ctx = self
                    .base_ctx
                    .clone()
                    .set_str("imports", &imports.join("\n"))
                    // go-zero 风格（模板变量名保持稳定）
                    .set_bool("HasDoc", !doc_str.trim().is_empty())
                    .set_str("Doc", &doc_str)
                    .set_str("HandlerName", handler_name)
                    .set_bool("HasRequest", has_req)
                    .set_str("RequestType", req_rust.clone())
                    .set_bool("HasResp", has_resp)
                    .set_str("LogicName", &logic_mod)
                    .set_str("LogicType", logic_type)
                    .set_str("Call", h)
                    .set_bool("NeedsValidate", needs_validate)
                    // 框架差异变量（只做“替换”，不做“拼接整段 handler 逻辑”）
                    .set_str("SvcCtxParam", svc_ctx_param)
                    .set_str("ReqParam", req_param)
                    .set_str("ReturnType", return_type)
                    .set_str("LogicCtorArgs", logic_ctor_args)
                    .set_str("ReqValueExpr", req_value_expr)
                    .set_str("OkRespFmt", ok_resp_fmt)
                    .set_str("OkNoRespFmt", ok_no_resp_fmt)
                    .set_str("ErrFmt", err_fmt)
                    .set_str("BadRequestFmt", bad_request_fmt)
                    .set_str("BuildParts", "")
                    .set_json("unused", Value::Null); // keep API for future structured fields

                render::render(&self.templates.handler, &tpl_ctx).context("render handler.tpl")
            };

            for (g, hs) in &self.group_handlers {
                let is_default_group = g.is_empty();

                // For non-default group, create a wrapper module file so `crate::handler::<group>::<handler_fn>`
                // works (routes expect `crate::handler::{group}::{handler}`).
                if !is_default_group {
                    let g_file = semantic::group_file_base(g, self.style);
                    files.push(Artifact {
                    rel_path: format!("handler/{g_file}.rs").into(),
                    content: format!(
                        "/// generated by rsctl\n#[path = \"{g}/handler.rs\"]\npub mod handler;\npub use handler::*;\n"
                    ),
                });
                }

                if self.merge {
                    // Merge all handlers of the same group into one file (dedup imports).
                    let mut first_content = String::new();
                    let mut bodies = Vec::<String>::new();
                    let mut extra_logic_imports = Vec::<String>::new();
                    let mut first = true;
                    for h in hs {
                        let mut one = render_one("", g, h, is_default_group)?;
                        let logic_import = if is_default_group {
                            format!(
                                "use crate::logic::{logic_type};",
                                logic_type = semantic::pascal(h)
                            )
                        } else {
                            format!(
                                "use crate::logic::{g}::logic::{logic_type};",
                                logic_type = semantic::pascal(h),
                                g = g
                            )
                        };
                        if !first {
                            if let Some(idx) = one.find("\n#[rest::handler]") {
                                one = one.split_off(idx + 1);
                            }
                            extra_logic_imports.push(logic_import);
                            bodies.push(one);
                        } else {
                            first_content = one;
                            first = false;
                        }
                    }

                    if !extra_logic_imports.is_empty() {
                        if let Some(idx) = first_content.find("\n#[rest::handler]") {
                            let insert_pos = idx;
                            let mut prefix = first_content[..insert_pos].to_string();
                            prefix.push_str(&extra_logic_imports.join("\n"));
                            prefix.push('\n');
                            let suffix = first_content[insert_pos..].to_string();
                            first_content = format!("{prefix}{suffix}");
                        }
                    }

                    let mut merged = first_content;
                    if !bodies.is_empty() {
                        merged.push_str("\n\n");
                        merged.push_str(&bodies.join("\n\n"));
                    }
                    files.push(Artifact {
                        rel_path: if is_default_group {
                            "handler/handler.rs".into()
                        } else {
                            format!("handler/{g}/handler.rs").into()
                        },
                        content: merged,
                    });
                } else {
                    // Flat layout under group: each handler => one file, plus a hub `handler.rs` for exports.
                    let mut hub = String::new();
                    hub.push_str("/// generated by rsctl\n");
                    for h in hs {
                        hub.push_str(&format!("pub mod {h};\n"));
                    }
                    hub.push('\n');
                    for h in hs {
                        hub.push_str(&format!("pub use {h}::{h};\n"));
                    }
                    files.push(Artifact {
                        rel_path: if is_default_group {
                            "handler/handler.rs".into()
                        } else {
                            format!("handler/{g}/handler.rs").into()
                        },
                        content: hub,
                    });

                    for h in hs {
                        files.push(Artifact {
                            rel_path: if is_default_group {
                                format!("handler/{h}.rs").into()
                            } else {
                                format!("handler/{g}/{h}.rs").into()
                            },
                            content: render_one("", g, h, is_default_group)?,
                        });
                    }
                }
            }

            Ok(())
        }
    }

    impl<'a> Generator<'a> {
        fn logic(&self, files: &mut Vec<Artifact>) -> Result<()> {
            for (g, hs) in &self.group_handlers {
                let is_default_group = g.is_empty();

                if !is_default_group {
                    let g_file = semantic::group_file_base(g, self.style);
                    files.push(Artifact {
                        rel_path: format!("logic/{g_file}.rs").into(),
                        content: format!(
                            "/// generated by rsctl\n#[path = \"{g}/logic.rs\"]\npub mod logic;\n"
                        ),
                    });
                }

                let logic_header = format!(
                    "// Code scaffolded by rsctl. Safe to edit.\n// rsctl {version}\n\n",
                    version = self.rsctl_version
                );

                if self.merge {
                    files.push(Artifact {
                    rel_path: if is_default_group {
                        "logic/logic.rs".into()
                    } else {
                        format!("logic/{g}/logic.rs").into()
                    },
                    content: {
                        let mut rendered = Vec::<String>::new();
                        let mut bodies = Vec::<String>::new();
                        let mut first = true;
                        for h in hs {
                            let route = self.spec
                                .service
                                .groups
                                .iter()
                                .filter(|grp| group_key(grp) == *g)
                                .flat_map(|grp| grp.routes.iter())
                            .find(|r| semantic::snake(&r.handler) == *h);

                            let request_spec = route.and_then(|r| r.request.as_ref());
                            let response_spec = route.and_then(|r| r.response.as_ref());

                            let logic_name = semantic::pascal(h);
                            let has_req = request_spec.is_some();
                            let req_type = request_spec
                                .map(|t| semantic::type_to_rust(g, t))
                                .unwrap_or_default();

                            let resp_rust = response_spec
                                .map(|t| semantic::type_to_rust(g, t))
                                .unwrap_or_else(|| "()".to_string());

                            let response_type = format!("anyhow::Result<{resp_rust}>");
                            let return_string = if response_spec.is_some() {
                                if !has_req && h == "healthz" && resp_rust.ends_with("::HealthzResp") {
                                    format!(
                                        "Ok({resp_rust} {{ code: 0, message: \"ok\".to_string(), data: Default::default() }})"
                                    )
                                } else if matches!(response_spec, Some(crate::spec::api::Type::Array(_))) {
                                    "Ok(Vec::new())".to_string()
                                } else {
                                    format!("Ok({resp_rust}::default())")
                                }
                            } else {
                                "Ok(())".to_string()
                            };

                            let imports = [
                                "use std::sync::Arc;",
                                "use crate::svc::ServiceContext;",
                            ]
                            .join("\n");

                            let tpl_ctx = self
                                .base_ctx
                                .clone()
                                .set_str("logic", &logic_name)
                                .set_str("function", h)
                                .set_bool("hasRequest", has_req)
                                .set_str("requestType", &req_type)
                                .set_str("responseType", &response_type)
                                .set_str("returnString", &return_string)
                                .set_bool("hasDoc", false)
                                .set_str("doc", "")
                                .set_str("imports", &imports);
                            let mut one = render::render(&self.templates.logic, &tpl_ctx)
                                .context("render logic.tpl")?;
                            if !first {
                                if let Some(idx) = one.find("\npub struct") {
                                    one = one.split_off(idx + 1);
                                }
                                bodies.push(one);
                            } else {
                                rendered.push(one);
                                first = false;
                            }
                        }

                        let mut merged = rendered.join("\n\n");
                        if !bodies.is_empty() {
                            if !merged.ends_with('\n') {
                                merged.push('\n');
                            }
                            merged.push('\n');
                            merged.push_str(&bodies.join("\n\n"));
                        }

                        format!("{logic_header}{merged}")
                    },
                });
                } else if is_default_group {
                    for h in hs {
                        let route = self
                            .spec
                            .service
                            .groups
                            .iter()
                            .filter(|grp| group_key(grp) == *g)
                            .flat_map(|grp| grp.routes.iter())
                            .find(|r| semantic::snake(&r.handler) == *h);

                        let request_spec = route.and_then(|r| r.request.as_ref());
                        let response_spec = route.and_then(|r| r.response.as_ref());

                        let logic_name = semantic::pascal(h);
                        let has_req = request_spec.is_some();
                        let req_type = request_spec
                            .map(|t| semantic::type_to_rust(g, t))
                            .unwrap_or_default();

                        let resp_rust = response_spec
                            .map(|t| semantic::type_to_rust(g, t))
                            .unwrap_or_else(|| "()".to_string());

                        let response_type = format!("anyhow::Result<{resp_rust}>");
                        let return_string = if response_spec.is_some() {
                            if !has_req && h == "healthz" && resp_rust.ends_with("::HealthzResp") {
                                format!(
                                    "Ok({resp_rust} {{ code: 0, message: \"ok\".to_string(), data: Default::default() }})"
                                )
                            } else if matches!(
                                response_spec,
                                Some(crate::spec::api::Type::Array(_))
                            ) {
                                "Ok(Vec::new())".to_string()
                            } else {
                                format!("Ok({resp_rust}::default())")
                            }
                        } else {
                            "Ok(())".to_string()
                        };

                        let imports =
                            ["use std::sync::Arc;", "use crate::svc::ServiceContext;"].join("\n");

                        let tpl_ctx = self
                            .base_ctx
                            .clone()
                            .set_str("logic", &logic_name)
                            .set_str("function", h)
                            .set_bool("hasRequest", has_req)
                            .set_str("requestType", &req_type)
                            .set_str("responseType", &response_type)
                            .set_str("returnString", &return_string)
                            .set_bool("hasDoc", false)
                            .set_str("doc", "")
                            .set_str("imports", &imports);
                        let body = render::render(&self.templates.logic, &tpl_ctx)
                            .context("render logic.tpl")?;

                        files.push(Artifact {
                            rel_path: format!("logic/{h}.rs").into(),
                            content: format!("{logic_header}{body}"),
                        });
                    }
                } else {
                    let mut hub = String::new();
                    hub.push_str("/// generated by rsctl\n");
                    for h in hs {
                        hub.push_str(&format!("pub mod {h};\n"));
                    }
                    hub.push('\n');
                    for h in hs {
                        let logic_name = semantic::pascal(h);
                        hub.push_str(&format!("pub use {h}::{logic_name};\n"));
                    }
                    files.push(Artifact {
                        rel_path: format!("logic/{g}/logic.rs").into(),
                        content: hub,
                    });

                    for h in hs {
                        let route = self
                            .spec
                            .service
                            .groups
                            .iter()
                            .filter(|grp| group_key(grp) == *g)
                            .flat_map(|grp| grp.routes.iter())
                            .find(|r| semantic::snake(&r.handler) == *h);

                        let request_spec = route.and_then(|r| r.request.as_ref());
                        let response_spec = route.and_then(|r| r.response.as_ref());

                        let logic_name = semantic::pascal(h);
                        let has_req = request_spec.is_some();
                        let req_type = request_spec
                            .map(|t| semantic::type_to_rust(g, t))
                            .unwrap_or_default();

                        let resp_rust = response_spec
                            .map(|t| semantic::type_to_rust(g, t))
                            .unwrap_or_else(|| "()".to_string());

                        let response_type = format!("anyhow::Result<{resp_rust}>");
                        let return_string = if response_spec.is_some() {
                            if !has_req && h == "healthz" && resp_rust.ends_with("::HealthzResp") {
                                format!(
                                    "Ok({resp_rust} {{ code: 0, message: \"ok\".to_string(), data: Default::default() }})"
                                )
                            } else if matches!(
                                response_spec,
                                Some(crate::spec::api::Type::Array(_))
                            ) {
                                "Ok(Vec::new())".to_string()
                            } else {
                                format!("Ok({resp_rust}::default())")
                            }
                        } else {
                            "Ok(())".to_string()
                        };

                        let imports =
                            ["use std::sync::Arc;", "use crate::svc::ServiceContext;"].join("\n");

                        let tpl_ctx = self
                            .base_ctx
                            .clone()
                            .set_str("logic", &logic_name)
                            .set_str("function", h)
                            .set_bool("hasRequest", has_req)
                            .set_str("requestType", &req_type)
                            .set_str("responseType", &response_type)
                            .set_str("returnString", &return_string)
                            .set_bool("hasDoc", false)
                            .set_str("doc", "")
                            .set_str("imports", &imports);
                        let body = render::render(&self.templates.logic, &tpl_ctx)
                            .context("render logic.tpl")?;

                        files.push(Artifact {
                            rel_path: format!("logic/{g}/{h}.rs").into(),
                            content: format!("{logic_header}{body}"),
                        });
                    }
                }
            }
            Ok(())
        }
    }

    impl<'a> Generator<'a> {
        fn types(&self, files: &mut Vec<Artifact>) -> Result<()> {
            for (g, _hs) in &self.group_handlers {
                let is_default_group = g.is_empty();
                if !is_default_group {
                    let g_file = semantic::group_file_base(g, self.style);
                    files.push(Artifact {
                        rel_path: format!("types/{g_file}.rs").into(),
                        content: format!(
                            "/// generated by rsctl\n#[path = \"{g}/types.rs\"]\npub mod types;\n"
                        ),
                    });
                }
                files.push(Artifact {
                rel_path: if is_default_group {
                    "types/types.rs".into()
                } else {
                    format!("types/{g}/types.rs").into()
                },
                content: {
                    let mut decls: Vec<String> = Vec::new();
                    let mut required = std::collections::BTreeSet::<String>::new();
                    let mut queue = std::collections::VecDeque::<String>::new();
                    let type_map: std::collections::BTreeMap<String, &crate::spec::api::TypeDef> =
                        self.spec.types.iter().map(|t| (t.name.clone(), t)).collect();

                    fn enqueue(queue: &mut std::collections::VecDeque<String>, t: &crate::spec::api::Type) {
                        match t {
                            crate::spec::api::Type::Ident(s) => queue.push_back(s.clone()),
                            crate::spec::api::Type::Array(inner)
                            | crate::spec::api::Type::Pointer(inner) => enqueue(queue, inner),
                            crate::spec::api::Type::Map { key, value } => {
                                enqueue(queue, key);
                                enqueue(queue, value);
                            }
                            _ => {}
                        }
                    }

                    for grp in &self.spec.service.groups {
                        let gg = group_key(grp);
                        if gg != *g {
                            continue;
                        }
                        for r in &grp.routes {
                            if let Some(t) = &r.request {
                                enqueue(&mut queue, t);
                            }
                            if let Some(t) = &r.response {
                                enqueue(&mut queue, t);
                            }
                        }
                    }

                    while let Some(t) = queue.pop_front() {
                        if !required.insert(t.clone()) {
                            continue;
                        }

                        if let Some(td) = type_map.get(&t) {
                            for f in &td.fields {
                                enqueue(&mut queue, &f.ty);
                            }
                        }
                    }

                    for t in required {
                        if let Some(td) = type_map.get(&t) {
                            let mut s = String::new();
                            let mut has_validate = false;
                            for f in &td.fields {
                                let tags = f.tags()?;
                                if tags.get("validate").is_some() {
                                    has_validate = true;
                                }
                            }
                            if has_validate {
                                s.push_str("#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate, Default)]\n");
                            } else {
                                s.push_str("#[derive(Debug, Clone, Serialize, Deserialize, Default)]\n");
                            }
                            s.push_str(&format!("pub struct {} {{\n", td.name));

                            for f in &td.fields {
                                let rust_ty = semantic::type_to_rust(g, &f.ty);
                                if f.tag.is_some() {
                                    let tags = f.tags()?;
                                    if let Some(t) = tags
                                        .get("json")
                                        .or_else(|| tags.get("form"))
                                        .or_else(|| tags.get("path"))
                                    {
                                        let rename = if t.name == "-" {
                                            semantic::snake(&f.name)
                                        } else {
                                            t.name.clone()
                                        };
                                        s.push_str(&format!("    #[serde(rename = \"{}\")]\n", rename));
                                    }
                                    if let Some(v) = tags.get("validate") {
                                        for a in semantic::validate_tag_to_attrs(rust_ty.as_str(), &v.value) {
                                            s.push_str(&format!("    {}\n", a));
                                        }
                                    }
                                }
                                s.push_str(&format!(
                                    "    pub {}: {},\n",
                                    semantic::snake(&f.name),
                                    rust_ty
                                ));
                            }
                            s.push_str("}\n");
                            decls.push(s);
                        } else {
                            decls.push(format!(
                                "#[derive(Debug, Clone, Serialize, Deserialize, Default)]\npub struct {} {{}}\n",
                                t
                            ));
                        }
                    }

                    let types_ctx = self
                        .base_ctx
                        .clone()
                        .set_str("imports", "use serde::{Deserialize, Serialize};")
                        .set_str("types", decls.join("\n"));
                    render::render(&self.templates.types, &types_ctx).context("render types.tpl")?
                },
            });
            }
            Ok(())
        }
    }

    // 命名风格相关规则已上移到语义层：`crate::semantic::api::rs`

    pub fn generate_project(
        _web: &str,
        spec: &crate::spec::api::Spec,
        service_name: &str,
        merge: bool,
        style: &str,
        _out_dir: &Path,
        template_root: &Path,
        root_prefix_override: &str,
    ) -> Result<Artifacts> {
        // NOTE:
        // - api 模板在 `templates/api/rs/`
        // - rust 通用模板在 `templates/rs/`
        let templates = load_templates(template_root)?;
        let service_name = normalize_service_name(service_name);
        let style = semantic::parse_style(style)?;

        use std::collections::BTreeMap;

        // goctl 风格：工程名/入口名/默认配置路径都由 service_name 推导。

        let mut group_handlers: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for grp in &spec.service.groups {
            let gk = group_key(grp);
            let mut hs: Vec<String> = grp
                .routes
                .iter()
                .map(|r| semantic::snake(&r.handler))
                .collect();
            hs.sort();
            hs.dedup();
            group_handlers.entry(gk).or_default().extend(hs);
        }
        for v in group_handlers.values_mut() {
            v.sort();
            v.dedup();
        }

        // rsctl 自身版本号（编译期注入），用于在生成代码注释里标记生成器版本，便于追溯。
        let rsctl_version = option_env!("CARGO_PKG_VERSION")
            .unwrap_or("unknown")
            .to_string();
        let mut rest_features: Vec<&str> = Vec::new();
        let root_prefix = if !root_prefix_override.is_empty() {
            root_prefix_override.to_string()
        } else if !spec.info.root_prefix.is_empty() {
            spec.info.root_prefix.clone()
        } else {
            "/".to_string()
        };

        fn to_snake(name: &str) -> String {
            let mut out = String::new();
            let mut prev_lower = false;
            for ch in name.chars() {
                if ch == '_' || ch == '-' {
                    out.push('_');
                    prev_lower = false;
                    continue;
                }
                if ch.is_ascii_uppercase() {
                    if prev_lower {
                        out.push('_');
                    }
                    out.push(ch.to_ascii_lowercase());
                    prev_lower = false;
                } else {
                    out.push(ch);
                    prev_lower = true;
                }
            }
            out.trim_matches('_').to_string()
        }

        let mut use_validator = false;
        for td in &spec.types {
            for f in &td.fields {
                let tags = f.tags()?;
                if tags.get("validate").is_some() {
                    use_validator = true;
                    break;
                }
            }
            if use_validator {
                break;
            }
        }
        let validator_dep = if use_validator {
            rest_features.push("validator");
            "validator = { version = \"0.19\", features = [\"derive\"] }"
        } else {
            ""
        };
        let rest_features = format!(
            "[{}]",
            rest_features
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let auth_dep = String::new();

        // collect middlewares (snake & pascal)
        let mut mw_defs: Vec<(String, String)> = Vec::new();
        if let Some(svc) = spec.service.groups.get(0).map(|_| &spec.service) {
            for g in &svc.groups {
                if let Some(m) = group_middleware(g) {
                    let snake = to_snake(&m);
                    let pascal = semantic::pascal(&snake);
                    mw_defs.push((snake, pascal));
                }
            }
        }

        let middleware_fields = if mw_defs.is_empty() {
            "".to_string()
        } else {
            mw_defs
                .iter()
                .map(|(s, _)| format!("    pub {s}: Middleware,", s = s))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let middleware_assignment = if mw_defs.is_empty() {
            "".to_string()
        } else {
            mw_defs
                .iter()
                .map(|(s, p)| {
                    format!(
                        "            {s}: crate::middleware::{s}::{p}::new().handle(),",
                        s = s,
                        p = p
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let main_attr = "#[tokio::main]";
        let base_ctx = render::Context::new()
            .set_str("version", &rsctl_version)
            .set_str("serviceName", &service_name)
            .set_str("host", "0.0.0.0")
            .set_str("port", "8888")
            .set_str("imports", "")
            .set_str("config", "crate::config::Config")
            .set_str("middleware", &middleware_fields)
            .set_str("middlewareAssignment", &middleware_assignment)
            .set_str("mainAttr", main_attr)
            .set_str("rootPrefix", &root_prefix)
            .set_str("serverStart", "");

        let generator = Generator {
            spec,
            merge,
            style,
            service_name: service_name.clone(),
            rsctl_version,
            group_handlers,
            rest_features: rest_features.to_string(),
            validator_dep: validator_dep.to_string(),
            auth_dep,
            templates,
            base_ctx,
        };

        let mut files: Vec<Artifact> = Vec::new();
        generator.run(&mut files)?;

        // NOTE: /healthz 等默认路由应由 api.api 显式声明，避免生成器暗含行为。

        Ok(Artifacts { files })
    }
}
