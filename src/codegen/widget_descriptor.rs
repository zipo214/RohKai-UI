//! `.rkwd` — RohKai Widget Definition
//!
//! Runtime-loadable widget descriptors that extend the palette, properties
//! panel, and code generation without recompiling RohKai.
//!
//! Files live in `<binary_dir>/widgets/*.rkwd` (JSON, schema_version 1).

use serde::{Deserialize, Serialize};
use std::path::Path;

// ---------------------------------------------------------------------------
// Descriptor types
// ---------------------------------------------------------------------------

/// Top-level descriptor — one per `.rkwd` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetDescriptor {
    pub schema_version: u32,
    /// Unique identifier, e.g. `"ply.button"`.
    pub id: String,
    /// Display name shown in the palette and canvas tag.
    pub name: String,
    /// Palette category header, e.g. `"Ply"`.
    pub category: String,
    /// Default canvas size `[width, height]` in pixels.
    pub default_size: [f32; 2],
    /// Accent colour `[R, G, B]` 0–255.
    pub accent_color: [u8; 3],
    /// Editable properties rendered in the Properties panel.
    #[serde(default)]
    pub properties: Vec<DescriptorProp>,
    /// AppState fields emitted by `state_emitter`.
    #[serde(default)]
    pub state_fields: Vec<DescriptorStateField>,
    /// Codegen templates.
    pub codegen: DescriptorCodegen,
    /// Canvas preview hint.
    pub canvas_preview: DescriptorCanvasPreview,
    /// Extra `Cargo.toml` dependencies injected into the exported project.
    #[serde(default)]
    pub cargo_deps: Vec<CargoDep>,
    /// Supported event names, e.g. `["on_click"]`.
    #[serde(default)]
    pub events: Vec<String>,
}

/// One editable property in the Properties panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescriptorProp {
    /// Machine key used in template tokens `{{prop.<key>}}`.
    pub key: String,
    /// Runtime type for the Properties panel editor widget.
    #[serde(rename = "type")]
    pub ty: DescriptorPropType,
    /// Serialised default value (always a string, parsed per type on display).
    pub default: String,
    /// Label shown beside the field in the Properties panel.
    pub display: String,
    /// Valid choices for `Enum` type.
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DescriptorPropType {
    String,
    F32,
    Bool,
    I32,
    Enum,
}

/// An AppState field generated for this descriptor widget.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescriptorStateField {
    pub key: String,
    pub rust_type: String,
    pub default_expr: String,
}

/// Codegen templates.
///
/// Token syntax: `{{label}}`, `{{binding}}`, `{{width}}`, `{{height}}`,
/// `{{name}}`, `{{handler}}`, `{{prop.<key>}}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescriptorCodegen {
    /// Emitted in the live code panel by `egui_emitter`.
    pub live_preview: String,
    /// Emitted in the exported project by `export`.
    pub export: String,
    /// Handler stub template appended when `on_click` is set (optional).
    #[serde(default)]
    pub on_click_stub: String,
}

/// How the custom widget renders on the canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescriptorCanvasPreview {
    pub mode: CanvasPreviewMode,
    /// Canvas label template.  Tokens: `{{name}}`, `{{label}}`.
    #[serde(default)]
    pub label_template: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CanvasPreviewMode {
    /// Renders the same accent-colour label box as built-in widgets.
    #[serde(rename = "label_box")]
    LabelBox,
}

/// A single `Cargo.toml` dependency injected into the exported project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoDep {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub features: Vec<String>,
}

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------

/// Load all valid `.rkwd` descriptors from `<binary_dir>/widgets/`.
///
/// Returns `(descriptors, load_errors)`.  Load errors are human-readable
/// strings; the caller may display them in a diagnostics panel.
pub fn load_from_widgets_dir() -> (Vec<WidgetDescriptor>, Vec<String>) {
    let mut descriptors = Vec::new();
    let mut errors = Vec::new();

    let dir = match std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("widgets")))
    {
        Some(d) => d,
        None => {
            errors.push("Could not determine binary path for .rkwd descriptor loader".to_owned());
            return (descriptors, errors);
        }
    };

    // A missing /widgets folder is not an error — no custom widgets yet.
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return (descriptors, errors),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rkwd") {
            continue;
        }
        match load_one(&path) {
            Ok(d) => {
                if d.schema_version == 1 {
                    descriptors.push(d);
                } else {
                    errors.push(format!(
                        "{}: unsupported schema_version {}",
                        path.display(),
                        d.schema_version
                    ));
                }
            }
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }

    // Stable alphabetical order by id
    descriptors.sort_by(|a, b| a.id.cmp(&b.id));
    (descriptors, errors)
}

fn load_one(path: &Path) -> Result<WidgetDescriptor, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Template engine
// ---------------------------------------------------------------------------

/// Substitute tokens in a codegen template for `widget`.
///
/// | Token | Resolves to |
/// |---|---|
/// | `{{label}}` | Rust string literal of `widget.props.label` |
/// | `{{binding}}` | AppState field name or empty string |
/// | `{{width}}` / `{{height}}` | Widget pixel dimensions |
/// | `{{name}}` | `display_name` parameter |
/// | `{{handler}}` | `on_click` field or `"on_click"` placeholder |
/// | `{{prop.<key>}}` | Value from `widget.descriptor_props` |
pub fn apply_template(
    template: &str,
    widget: &crate::project::schema::WidgetInstance,
    display_name: &str,
) -> String {
    let label_lit = crate::codegen::rust::string_literal(&widget.props.label);
    let binding = widget
        .state_binding
        .as_deref()
        .and_then(|s| crate::codegen::rust::field_binding(Some(s)))
        .unwrap_or_default();
    let handler = if !widget.on_click.is_empty() {
        widget.on_click.as_str()
    } else {
        "on_click"
    };

    let mut out = template.to_owned();
    out = out.replace("{{label}}", &label_lit);
    out = out.replace("{{binding}}", binding);
    out = out.replace("{{width}}", &format!("{:.1}", widget.rect.w));
    out = out.replace("{{height}}", &format!("{:.1}", widget.rect.h));
    out = out.replace("{{name}}", display_name);
    out = out.replace("{{handler}}", handler);
    for (k, v) in &widget.descriptor_props {
        out = out.replace(&format!("{{{{prop.{k}}}}}"), v);
    }
    out
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find a descriptor by id in a loaded slice.
pub fn find_by_id<'a>(
    descriptors: &'a [WidgetDescriptor],
    id: &str,
) -> Option<&'a WidgetDescriptor> {
    descriptors.iter().find(|d| d.id == id)
}

/// Build the initial `descriptor_props` map from a descriptor's property defaults.
pub fn default_props(descriptor: &WidgetDescriptor) -> std::collections::HashMap<String, String> {
    descriptor
        .properties
        .iter()
        .map(|p| (p.key.clone(), p.default.clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};

    fn sample_descriptor() -> WidgetDescriptor {
        serde_json::from_str(
            r#"{
                "schema_version": 1,
                "id": "test.widget",
                "name": "Test Widget",
                "category": "Test",
                "default_size": [100.0, 40.0],
                "accent_color": [100, 150, 200],
                "properties": [
                    {"key": "label",  "type": "String", "default": "Hello", "display": "Label"},
                    {"key": "mode",   "type": "Enum",   "default": "A",     "display": "Mode",
                     "options": ["A", "B"]}
                ],
                "state_fields": [],
                "codegen": {
                    "live_preview": "        // {{name}}: {{label}} (mode={{prop.mode}})",
                    "export": "        custom::Widget::new({{label}}).mode({{prop.mode}}).ui(ui);"
                },
                "canvas_preview": {"mode": "label_box", "label_template": "{{name}}: {{label}}"},
                "cargo_deps": [],
                "events": []
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn descriptor_deserializes() {
        let d = sample_descriptor();
        assert_eq!(d.id, "test.widget");
        assert_eq!(d.properties.len(), 2);
        assert_eq!(d.properties[0].key, "label");
    }

    #[test]
    fn default_props_from_descriptor() {
        let d = sample_descriptor();
        let props = default_props(&d);
        assert_eq!(props.get("label").map(String::as_str), Some("Hello"));
        assert_eq!(props.get("mode").map(String::as_str), Some("A"));
    }

    #[test]
    fn apply_template_substitutes_tokens() {
        let d = sample_descriptor();
        let mut w = WidgetInstance {
            kind: WidgetKind::Custom("test.widget".to_owned()),
            props: WidgetProps {
                label: "Click me".to_owned(),
                ..Default::default()
            },
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 40.0,
            },
            ..Default::default()
        };
        w.descriptor_props = default_props(&d);

        let result = apply_template(&d.codegen.live_preview, &w, &d.name);
        assert!(result.contains("Test Widget"), "name token");
        assert!(result.contains("\"Click me\""), "label token");
        assert!(result.contains("mode=A"), "prop token");
    }

    #[test]
    fn cargo_dep_serializes() {
        let dep = CargoDep {
            name: "ply-ui".to_owned(),
            version: "0.3".to_owned(),
            features: vec!["derive".to_owned()],
        };
        let json = serde_json::to_string(&dep).unwrap();
        assert!(json.contains("ply-ui"));
        assert!(json.contains("derive"));
    }
}
