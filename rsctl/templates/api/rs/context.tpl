// Code scaffolded by rsctl. Safe to edit.
// rsctl {{ version }}

{{ imports }}

#[allow(dead_code)]
pub struct ServiceContext {
    pub config: {{ config }},
    {{ middleware }}
}

impl ServiceContext {
    pub fn new(config: {{ config }}) -> Self {
        Self {
            config,
            {{ middlewareAssignment }}
        }
    }
}
