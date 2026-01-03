// Code scaffolded by rsctl. Safe to edit.
// rsctl {{ version }}

{{ imports }}

{% if hasDoc %}
{{ doc }}
{% endif %}
pub struct {{ logic }} {
    svc_ctx: Arc<ServiceContext>,
}

impl {{ logic }} {
    pub fn new(svc_ctx: Arc<ServiceContext>) -> Self {
        Self { svc_ctx }
    }

{% if hasRequest %}
    pub async fn {{ function }}(&self, req: {{ requestType }}) -> {{ responseType }} {
        let _ = &self.svc_ctx;
        let _ = &req;
        // TODO: add logic
        {{ returnString }}
    }
{% else %}
    pub async fn {{ function }}(&self) -> {{ responseType }} {
        let _ = &self.svc_ctx;
        // TODO: add logic
        {{ returnString }}
    }
{% endif %}
}
