use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Artifact {
    /// Relative path under output root.
    pub rel_path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, Default)]
pub struct Artifacts {
    pub files: Vec<Artifact>,
}
