use anyhow::{Context as _, Result};
use minijinja::Environment;
use serde_json::{Map, Value};

/// 渲染上下文（可序列化结构），支持：
/// - 字符串/布尔/数组/对象
/// - 兼容旧逻辑：同时写入 `key` 与 `key.to_lowercase()` 两份，便于历史模板继续工作
#[derive(Debug, Clone, Default)]
pub struct Context {
    map: Map<String, Value>,
}

impl Context {
    pub fn new() -> Self {
        Self { map: Map::new() }
    }

    pub fn set_str(mut self, key: &str, val: impl Into<String>) -> Self {
        let v = Value::String(val.into());
        self.map.insert(key.to_string(), v.clone());
        self.map.insert(key.to_ascii_lowercase(), v);
        self
    }

    pub fn set_bool(mut self, key: &str, val: bool) -> Self {
        let v = Value::Bool(val);
        self.map.insert(key.to_string(), v);
        self.map.insert(key.to_ascii_lowercase(), Value::Bool(val));
        self
    }

    /// 直接写入任意 JSON 值（数组/对象都可），用于模板循环。
    pub fn set_json(mut self, key: &str, val: Value) -> Self {
        self.map.insert(key.to_string(), val.clone());
        self.map.insert(key.to_ascii_lowercase(), val);
        self
    }

    fn as_json(&self) -> Value {
        Value::Object(self.map.clone())
    }
}

/// 使用 minijinja 渲染模板：
/// - 变量：`{{ var }}`
/// - 条件：`{% if cond %} ... {% endif %}`
/// - 循环：`{% for x in xs %} ... {% endfor %}`
pub fn render(input: &str, ctx: &Context) -> Result<String> {
    let mut env = Environment::new();
    env.add_template("tpl", input)
        .with_context(|| "failed to parse template")?;
    let tpl = env
        .get_template("tpl")
        .with_context(|| "template not found")?;
    tpl.render(ctx.as_json())
        .with_context(|| "failed to render template")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn render_if_should_work() {
        let tpl = "{% if ok %}YES{% endif %}";
        let out = render(tpl, &Context::new().set_bool("ok", true)).unwrap();
        assert_eq!(out, "YES");
        let out = render(tpl, &Context::new().set_bool("ok", false)).unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn render_for_should_work() {
        let tpl = "{% for x in xs %}{{ x }}{% endfor %}";
        let out = render(tpl, &Context::new().set_json("xs", json!([1, 2, 3]))).unwrap();
        assert_eq!(out, "123");
    }
}
