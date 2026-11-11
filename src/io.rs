//! Project files: pretty-printed JSON, so they diff and merge like source.

use std::path::Path;

use crate::model::Project;

pub fn save(project: &Project, path: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(project).map_err(|e| e.to_string())?;
    // Write beside the target and rename, so a crash mid-save can't truncate the project.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn load(path: &Path) -> Result<Project, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}
