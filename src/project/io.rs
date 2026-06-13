use crate::project::{document::ProjectDocument, ui_tree::UiTree};
use serde::{Deserialize, Serialize};
use std::path::Path;

const PROJECT_SCHEMA_VERSION: u32 = 2;

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    schema_version: u32,
    document: ProjectDocument,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ProjectJson {
    Project(ProjectFile),
    VersionOne {
        schema_version: u32,
        tree: UiTree,
    },
    Legacy(UiTree),
}

/// Serialize `document` to pretty JSON.
/// Returns the JSON string so the caller can store it as the clean snapshot.
pub fn serialize(document: &ProjectDocument) -> Result<String, String> {
    let mut document = document.clone();
    document.validate_and_repair();
    let project = ProjectFile {
        schema_version: PROJECT_SCHEMA_VERSION,
        document,
    };
    serde_json::to_string_pretty(&project).map_err(|e| format!("Serialize error: {e}"))
}

pub fn save(path: &Path, document: &ProjectDocument) -> Result<String, String> {
    let json = serialize(document)?;
    std::fs::write(path, &json).map_err(|e| format!("Write error: {e}"))?;
    Ok(json)
}

/// Read a `.rohkai.json` file and deserialize into a project document.
pub fn load(path: &Path) -> Result<ProjectDocument, String> {
    let json = std::fs::read_to_string(path).map_err(|e| format!("Read error: {e}"))?;
    deserialize(&json)
}

/// Deserialize a project from schema v2, schema v1, or a legacy bare tree.
/// Used by both file load and the undo/redo stack.
pub fn deserialize(json: &str) -> Result<ProjectDocument, String> {
    let mut document =
        match serde_json::from_str(json).map_err(|e| format!("Parse error: {e}"))? {
        ProjectJson::Project(project) => {
            if project.schema_version > PROJECT_SCHEMA_VERSION {
                return Err(format!(
                    "Unsupported project schema version: {}",
                    project.schema_version
                ));
            }
            project.document
        }
        ProjectJson::VersionOne {
            schema_version,
            tree,
        } => {
            if schema_version > 1 {
                return Err(format!(
                    "Unsupported legacy project schema version: {schema_version}"
                ));
            }
            ProjectDocument::from_legacy_tree(tree)
        }
        ProjectJson::Legacy(tree) => ProjectDocument::from_legacy_tree(tree),
    };
    document.validate_and_repair();
    Ok(document)
}

/// Temporary compatibility serializer for the single-tree app shell.
///
/// New project features must use [`serialize`]. This exists only until
/// `RohKaiApp` switches to [`ProjectDocument`] in the next implementation slice.
pub fn serialize_tree(tree: &UiTree) -> Result<String, String> {
    serialize(&ProjectDocument::from_legacy_tree(tree.clone()))
}

/// Temporary compatibility save path for the single-tree app shell.
pub fn save_tree(path: &Path, tree: &UiTree) -> Result<String, String> {
    let json = serialize_tree(tree)?;
    std::fs::write(path, &json).map_err(|e| format!("Write error: {e}"))?;
    Ok(json)
}

/// Temporary compatibility load path returning only the migrated root tree.
pub fn load_tree(path: &Path) -> Result<UiTree, String> {
    let document = load(path)?;
    Ok(document.root_surface().tree.clone())
}

/// Temporary compatibility undo path returning only the migrated root tree.
pub fn deserialize_tree(json: &str) -> Result<UiTree, String> {
    let document = deserialize(json)?;
    Ok(document.root_surface().tree.clone())
}
