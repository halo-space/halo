use std::collections::HashMap;

/// Captured path parameters stored in request extensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathParams {
    pub params: HashMap<String, String>,
}
