// Code scaffolded by rsctl. Safe to edit.
// rsctl {{ version }}

use halo_micro::rest::RestConf;
use serde::Deserialize;

{% if imports %}
{{ imports }}
{% endif %}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(flatten)]
    pub rest: RestConf,
    {{ auth }}
}

impl Config {
    pub fn new() -> Self {
        Self {
            rest: RestConf::new(),
            {{ auth }}
        }
    }
}
