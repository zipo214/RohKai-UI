//! `.rkwb` — RohKai Widget Bundle
//!
//! A bundle is a JSON envelope that packages one or more `.rkwd` descriptors
//! into a single portable file.  No external zip dependency — the envelope is
//! plain JSON, same as every other RohKai file format.
//!
//! File layout:
//! ```json
//! { "format": "rkwb", "schema_version": 1, "descriptors": [ ... ] }
//! ```

use super::widget_descriptor::WidgetDescriptor;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetBundle {
    pub format: String,
    pub schema_version: u32,
    pub descriptors: Vec<WidgetDescriptor>,
}

impl WidgetBundle {
    pub fn from_descriptors(descriptors: &[WidgetDescriptor]) -> Self {
        Self {
            format: "rkwb".to_owned(),
            schema_version: 1,
            descriptors: descriptors.to_vec(),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(s: &str) -> Result<Self, BundleError> {
        let b: Self = serde_json::from_str(s).map_err(BundleError::Json)?;
        if b.format != "rkwb" {
            return Err(BundleError::WrongFormat(b.format.clone()));
        }
        if b.schema_version != 1 {
            return Err(BundleError::UnsupportedVersion(b.schema_version));
        }
        Ok(b)
    }

    /// Write each descriptor as `<id>.rkwd` inside `widgets_dir`.
    /// Returns the list of written file stems (descriptor IDs).
    pub fn extract_to(&self, widgets_dir: &Path) -> Result<Vec<String>, std::io::Error> {
        std::fs::create_dir_all(widgets_dir)?;
        let mut written = Vec::new();
        for d in &self.descriptors {
            let json = serde_json::to_string_pretty(d).map_err(std::io::Error::other)?;
            let dest = widgets_dir.join(format!("{}.rkwd", d.id));
            std::fs::write(&dest, json)?;
            written.push(d.id.clone());
        }
        Ok(written)
    }
}

// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum BundleError {
    Json(serde_json::Error),
    Io(std::io::Error),
    WrongFormat(String),
    UnsupportedVersion(u32),
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::WrongFormat(s) => write!(f, "Not a .rkwb file (format = \"{s}\")"),
            Self::UnsupportedVersion(v) => {
                write!(f, "Unsupported schema_version {v} (expected 1)")
            }
        }
    }
}

impl From<std::io::Error> for BundleError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
