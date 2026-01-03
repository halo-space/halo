//! Build script for `rsctl` CLI.
//!
//! We embed `../templates` into the binary via `include_dir!`.
//! Cargo does **not** automatically track changes under that directory,
//! so we explicitly register them as build inputs to ensure template edits
//! trigger a rebuild.

use std::path::{Path, PathBuf};

fn main() {
    let templates_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates");
    track_dir(&templates_root);
}

fn track_dir(dir: &Path) {
    // If templates directory doesn't exist in some contexts, just skip.
    if !dir.exists() {
        return;
    }

    println!("cargo:rerun-if-changed={}", dir.display());

    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            track_dir(&path);
        } else if path.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}
