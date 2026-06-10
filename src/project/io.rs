use crate::project::ui_tree::UiTree;
use serde::{Deserialize, Serialize};
use std::path::Path;

const PROJECT_SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    schema_version: u32,
    tree: UiTree,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ProjectJson {
    Project(ProjectFile),
    Legacy(UiTree),
}

/// Serialize `tree` to pretty JSON, write to `path`.
/// Returns the JSON string so the caller can store it as the clean snapshot.
pub fn serialize(tree: &UiTree) -> Result<String, String> {
    let mut tree = tree.clone();
    tree.validate_and_repair();
    let project = ProjectFile {
        schema_version: PROJECT_SCHEMA_VERSION,
        tree,
    };
    serde_json::to_string_pretty(&project).map_err(|e| format!("Serialize error: {e}"))
}

pub fn save(path: &Path, tree: &UiTree) -> Result<String, String> {
    let json = serialize(tree)?;
    std::fs::write(path, &json).map_err(|e| format!("Write error: {e}"))?;
    Ok(json)
}

/// Read a `.rohkai.json` file and deserialize into a `UiTree`.
pub fn load(path: &Path) -> Result<UiTree, String> {
    let json = std::fs::read_to_string(path).map_err(|e| format!("Read error: {e}"))?;
    deserialize(&json)
}

/// Deserialize a `UiTree` from a JSON string (versioned envelope or legacy bare tree).
/// Used by both file load and the undo/redo stack.
pub fn deserialize(json: &str) -> Result<UiTree, String> {
    let mut tree = match serde_json::from_str(json).map_err(|e| format!("Parse error: {e}"))? {
        ProjectJson::Project(project) => {
            if project.schema_version > PROJECT_SCHEMA_VERSION {
                return Err(format!(
                    "Unsupported project schema version: {}",
                    project.schema_version
                ));
            }
            project.tree
        }
        ProjectJson::Legacy(tree) => tree,
    };
    tree.validate_and_repair();
    Ok(tree)
}
