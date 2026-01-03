// Code scaffolded by rsctl. Safe to edit.
// rsctl {{ version }}

{% if imports %}
{{ imports }}
{% endif %}

{% if HasDoc %}
{{ Doc }}
{% endif %}
#[rest::handler]
pub async fn {{ HandlerName }}(
    svc_ctx: Arc<ServiceContext>,
    {%- if HasRequest -%}
    mut req: http::Request<Body>,
    {%- else -%}
    _req: http::Request<Body>,
    {%- endif -%}
) -> Response<Body> {
{%- if HasRequest %}
    let parsed: {{ RequestType }} = match request::parse(&mut req).await {
        Ok(v) => v,
        Err(e) => return halo_micro::rest::http::bad_request(e.to_string()),
    };
{%- endif %}
    let l = {{ LogicType }}::new(svc_ctx);
{% if HasResp %}
    let resp = match l.{{ Call }}({% if HasRequest %}parsed{% endif %}).await {
        Ok(resp) => resp,
        Err(e) => return halo_micro::rest::http::error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    halo_micro::rest::http::ok_json(&resp).unwrap_or_else(|e| {
        halo_micro::rest::http::error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })
{% else %}
    match l.{{ Call }}({% if HasRequest %}parsed{% endif %}).await {
        Ok(()) => halo_micro::rest::http::ok(""),
        Err(e) => halo_micro::rest::http::error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
{% endif %}
}
