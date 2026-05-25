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
    let d: WidgetDescriptor = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let errs = validate_descriptor(&d);
    if errs.is_empty() {
        Ok(d)
    } else {
        Err(errs.join("; "))
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Safe Rust primitive types allowed in descriptor `state_fields`.
const SAFE_STATE_TYPES: &[&str] = &["String", "bool", "f32", "f64", "i32", "u32", "usize"];

/// Known codegen template tokens (besides `prop.<key>` variants).
const KNOWN_TOKENS: &[&str] = &["label", "binding", "width", "height", "name", "handler"];

/// Validate a parsed descriptor.  Returns a list of error strings.
/// An empty return means the descriptor is safe to load.
pub fn validate_descriptor(d: &WidgetDescriptor) -> Vec<String> {
    let mut errs = Vec::new();

    // ---- Top-level scalars ----
    if !is_descriptor_id(&d.id) {
        errs.push(format!(
            "id {:?} must be non-empty and contain only [a-zA-Z0-9._-]",
            d.id
        ));
    }
    if d.name.is_empty() || d.name.len() > 64 {
        errs.push(format!("name {:?} must be 1–64 chars", d.name));
    }
    if d.category.is_empty() || d.category.len() > 64 {
        errs.push(format!("category {:?} must be 1–64 chars", d.category));
    }
    if !d.default_size[0].is_finite()
        || !d.default_size[1].is_finite()
        || d.default_size[0] <= 0.0
        || d.default_size[1] <= 0.0
    {
        errs.push(format!(
            "default_size {:?} must be finite and positive",
            d.default_size
        ));
    }

    // ---- Properties ----
    if d.properties.len() > 64 {
        errs.push(format!(
            "too many properties ({}); max 64",
            d.properties.len()
        ));
    }
    let mut seen_prop_keys = std::collections::HashSet::new();
    for p in &d.properties {
        if !is_field_key(&p.key) {
            errs.push(format!(
                "prop key {:?} must start with a letter and contain only [a-zA-Z0-9_]",
                p.key
            ));
        } else if !seen_prop_keys.insert(p.key.clone()) {
            errs.push(format!("duplicate prop key {:?}", p.key));
        }
        if let DescriptorPropType::Enum = p.ty {
            if p.options.is_empty() {
                errs.push(format!("Enum prop {:?} has no options", p.key));
            } else if !p.options.contains(&p.default) {
                errs.push(format!(
                    "Enum prop {:?} default {:?} not in options {:?}",
                    p.key, p.default, p.options
                ));
            }
        }
    }

    // ---- State fields ----
    if d.state_fields.len() > 32 {
        errs.push(format!(
            "too many state_fields ({}); max 32",
            d.state_fields.len()
        ));
    }
    let mut seen_sf_keys = std::collections::HashSet::new();
    for sf in &d.state_fields {
        if !is_field_key(&sf.key) {
            errs.push(format!(
                "state_field key {:?} must start with a letter and contain only [a-zA-Z0-9_]",
                sf.key
            ));
        } else if !seen_sf_keys.insert(sf.key.clone()) {
            errs.push(format!("duplicate state_field key {:?}", sf.key));
        }
        if !SAFE_STATE_TYPES.contains(&sf.rust_type.as_str()) {
            errs.push(format!(
                "state_field {:?} rust_type {:?} not in allowed set {:?}",
                sf.key, sf.rust_type, SAFE_STATE_TYPES
            ));
        } else {
            // Validate default expression by type
            if let Err(e) = validate_state_default(&sf.rust_type, &sf.default_expr) {
                errs.push(format!(
                    "state_field {:?} invalid default {:?}: {e}",
                    sf.key, sf.default_expr
                ));
            }
        }
    }

    // ---- Codegen templates ----
    let prop_keys: Vec<&str> = d.properties.iter().map(|p| p.key.as_str()).collect();
    for (label, tpl) in [
        ("codegen.live_preview", d.codegen.live_preview.as_str()),
        ("codegen.export", d.codegen.export.as_str()),
    ] {
        if tpl.len() > 4096 {
            errs.push(format!("{label} template exceeds 4096 chars"));
        }
        for tok in extract_template_tokens(tpl) {
            if !KNOWN_TOKENS.contains(&tok.as_str()) {
                let prop_ref = tok.strip_prefix("prop.");
                match prop_ref {
                    Some(k) if prop_keys.contains(&k) => {}
                    Some(k) => errs.push(format!(
                        "{label}: unknown prop token {{{{prop.{k}}}}}; \
                         not declared in properties"
                    )),
                    None => errs.push(format!(
                        "{label}: unknown token {{{{{tok}}}}}; \
                         known tokens: label binding width height name handler prop.<key>"
                    )),
                }
            }
        }
    }

    // ---- Cargo deps ----
    for dep in &d.cargo_deps {
        if let Err(e) = validate_cargo_dep(dep) {
            errs.push(e);
        }
    }

    errs
}

// ---------------------------------------------------------------------------
// Cargo dep formatting
// ---------------------------------------------------------------------------

/// Validate and format one `CargoDep` as a TOML dependency line.
///
/// Returns `Err` if the dep name, version, or features contain injection-
/// hazardous characters (whitespace, quotes, backslash, braces, `=`, `<`, `>`).
///
/// ```
/// # use rohkai::codegen::widget_descriptor::{CargoDep, format_cargo_dep};
/// let dep = CargoDep { name: "ply-ui".into(), version: "0.3".into(), features: vec![] };
/// assert_eq!(format_cargo_dep(&dep).unwrap(), r#"ply-ui = "0.3""#);
/// ```
pub fn format_cargo_dep(dep: &CargoDep) -> Result<String, String> {
    validate_cargo_dep(dep)?;
    Ok(if dep.features.is_empty() {
        format!("{} = \"{}\"", dep.name, dep.version)
    } else {
        let feats = dep
            .features
            .iter()
            .map(|f| format!("\"{f}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{} = {{ version = \"{}\", features = [{feats}] }}",
            dep.name, dep.version
        )
    })
}

fn validate_cargo_dep(dep: &CargoDep) -> Result<(), String> {
    if dep.name.is_empty() || dep.name.len() > 64 {
        return Err(format!("cargo dep name {:?} must be 1–64 chars", dep.name));
    }
    if !dep
        .name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "cargo dep name {:?} must contain only [a-zA-Z0-9_-]",
            dep.name
        ));
    }
    if dep.version.is_empty() || dep.version.len() > 64 {
        return Err(format!(
            "cargo dep {:?} version {:?} must be 1–64 chars",
            dep.name, dep.version
        ));
    }
    if dep
        .version
        .chars()
        .any(|c| matches!(c, '"' | '\'' | '\\' | '{' | '}' | '<' | '>' | '\n' | '\r'))
    {
        return Err(format!(
            "cargo dep {:?} version {:?} contains disallowed characters",
            dep.name, dep.version
        ));
    }
    for feat in &dep.features {
        if feat.is_empty() || feat.len() > 64 {
            return Err(format!(
                "cargo dep {:?} feature {:?} must be 1–64 chars",
                dep.name, feat
            ));
        }
        if !feat
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '/')
        {
            return Err(format!(
                "cargo dep {:?} feature {:?} must contain only [a-zA-Z0-9_/-]",
                dep.name, feat
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Descriptor/widget ID: starts with letter, contains only `[a-zA-Z0-9._-]`.
fn is_descriptor_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .next()
            .map(|c| c.is_ascii_alphabetic())
            .unwrap_or(false)
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Prop/state-field key: starts with letter, contains only `[a-zA-Z0-9_]`.
fn is_field_key(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .next()
            .map(|c| c.is_ascii_alphabetic())
            .unwrap_or(false)
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Validate a `default_expr` string against an allowed Rust primitive type.
fn validate_state_default(rust_type: &str, default_expr: &str) -> Result<(), String> {
    match rust_type {
        "bool" => {
            if default_expr == "true" || default_expr == "false" {
                Ok(())
            } else {
                Err("bool default must be \"true\" or \"false\"".to_owned())
            }
        }
        "f32" | "f64" => default_expr.parse::<f64>().map(|_| ()).map_err(|_| {
            format!(
                "{rust_type} default {:?} is not a valid float",
                default_expr
            )
        }),
        "i32" => default_expr
            .parse::<i64>()
            .map(|_| ())
            .map_err(|_| format!("i32 default {:?} is not a valid integer", default_expr)),
        "u32" | "usize" => default_expr.parse::<u64>().map(|_| ()).map_err(|_| {
            format!(
                "{rust_type} default {:?} is not a valid non-negative integer",
                default_expr
            )
        }),
        "String" => {
            // Any string default is accepted; length limit only.
            if default_expr.len() > 256 {
                Err("String default exceeds 256 chars".to_string())
            } else {
                Ok(())
            }
        }
        _ => Err(format!("unknown type {rust_type:?}")),
    }
}

/// Extract all `{{token}}` names from a template string.
fn extract_template_tokens(template: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        rest = &rest[open + 2..];
        if let Some(close) = rest.find("}}") {
            let tok = rest[..close].trim().to_owned();
            if !tok.is_empty() {
                tokens.push(tok);
            }
            rest = &rest[close + 2..];
        } else {
            break;
        }
    }
    tokens
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

    #[test]
    fn format_cargo_dep_no_features() {
        let dep = CargoDep {
            name: "ply-ui".to_owned(),
            version: "0.3".to_owned(),
            features: vec![],
        };
        assert_eq!(format_cargo_dep(&dep).unwrap(), r#"ply-ui = "0.3""#);
    }

    #[test]
    fn format_cargo_dep_with_features() {
        let dep = CargoDep {
            name: "ply-ui".to_owned(),
            version: "0.3".to_owned(),
            features: vec!["derive".to_owned(), "async".to_owned()],
        };
        let line = format_cargo_dep(&dep).unwrap();
        assert!(line.contains("version"));
        assert!(line.contains("\"derive\""));
        assert!(line.contains("\"async\""));
    }

    #[test]
    fn format_cargo_dep_rejects_injection_in_name() {
        let dep = CargoDep {
            name: "evil\nnewline".to_owned(),
            version: "1.0".to_owned(),
            features: vec![],
        };
        assert!(format_cargo_dep(&dep).is_err());
    }

    #[test]
    fn format_cargo_dep_rejects_injection_in_version() {
        let dep = CargoDep {
            name: "ply-ui".to_owned(),
            version: r#"1.0" }\nmalicious = true\n["#.to_owned(),
            features: vec![],
        };
        assert!(format_cargo_dep(&dep).is_err());
    }

    #[test]
    fn format_cargo_dep_rejects_injection_in_features() {
        let dep = CargoDep {
            name: "ply-ui".to_owned(),
            version: "1.0".to_owned(),
            features: vec!["ok".to_owned(), "bad feature".to_owned()],
        };
        assert!(format_cargo_dep(&dep).is_err());
    }

    #[test]
    fn validate_descriptor_rejects_duplicate_prop_keys() {
        let json = r#"{
            "schema_version": 1, "id": "t.w", "name": "T", "category": "T",
            "default_size": [100.0, 40.0], "accent_color": [0,0,0],
            "properties": [
                {"key": "mode", "type": "String", "default": "a", "display": "Mode"},
                {"key": "mode", "type": "String", "default": "b", "display": "Mode2"}
            ],
            "state_fields": [],
            "codegen": {"live_preview": "// t", "export": "// t"},
            "canvas_preview": {"mode": "label_box"}
        }"#;
        let d: WidgetDescriptor = serde_json::from_str(json).unwrap();
        let errs = validate_descriptor(&d);
        assert!(
            errs.iter().any(|e| e.contains("duplicate prop key")),
            "expected duplicate error, got: {errs:?}"
        );
    }

    #[test]
    fn validate_descriptor_rejects_unsafe_state_type() {
        let json = r#"{
            "schema_version": 1, "id": "t.w", "name": "T", "category": "T",
            "default_size": [100.0, 40.0], "accent_color": [0,0,0],
            "properties": [],
            "state_fields": [{"key": "data", "rust_type": "Vec<String>", "default_expr": "vec![]"}],
            "codegen": {"live_preview": "// t", "export": "// t"},
            "canvas_preview": {"mode": "label_box"}
        }"#;
        let d: WidgetDescriptor = serde_json::from_str(json).unwrap();
        let errs = validate_descriptor(&d);
        assert!(
            errs.iter().any(|e| e.contains("not in allowed set")),
            "expected type error, got: {errs:?}"
        );
    }

    #[test]
    fn validate_descriptor_rejects_invalid_bool_default() {
        let json = r#"{
            "schema_version": 1, "id": "t.w", "name": "T", "category": "T",
            "default_size": [100.0, 40.0], "accent_color": [0,0,0],
            "properties": [],
            "state_fields": [{"key": "flag", "rust_type": "bool", "default_expr": "yes"}],
            "codegen": {"live_preview": "// t", "export": "// t"},
            "canvas_preview": {"mode": "label_box"}
        }"#;
        let d: WidgetDescriptor = serde_json::from_str(json).unwrap();
        let errs = validate_descriptor(&d);
        assert!(
            errs.iter().any(|e| e.contains("invalid default")),
            "expected default error, got: {errs:?}"
        );
    }

    #[test]
    fn validate_descriptor_rejects_unknown_template_token() {
        let json = r#"{
            "schema_version": 1, "id": "t.w", "name": "T", "category": "T",
            "default_size": [100.0, 40.0], "accent_color": [0,0,0],
            "properties": [],
            "state_fields": [],
            "codegen": {"live_preview": "{{unknown_token}}", "export": "// t"},
            "canvas_preview": {"mode": "label_box"}
        }"#;
        let d: WidgetDescriptor = serde_json::from_str(json).unwrap();
        let errs = validate_descriptor(&d);
        assert!(
            errs.iter().any(|e| e.contains("unknown token")),
            "expected unknown token error, got: {errs:?}"
        );
    }

    #[test]
    fn validate_descriptor_rejects_prop_token_not_in_properties() {
        let json = r#"{
            "schema_version": 1, "id": "t.w", "name": "T", "category": "T",
            "default_size": [100.0, 40.0], "accent_color": [0,0,0],
            "properties": [],
            "state_fields": [],
            "codegen": {"live_preview": "{{prop.missing}}", "export": "// t"},
            "canvas_preview": {"mode": "label_box"}
        }"#;
        let d: WidgetDescriptor = serde_json::from_str(json).unwrap();
        let errs = validate_descriptor(&d);
        assert!(
            errs.iter().any(|e| e.contains("unknown prop token")),
            "expected prop token error, got: {errs:?}"
        );
    }

    #[test]
    fn ply_button_rkwd_is_valid() {
        // Read the worked-example descriptor from the repo tree.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("widgets")
            .join("ply-button.rkwd");
        if !path.exists() {
            // Skip in environments where the file isn't present.
            return;
        }
        let text = std::fs::read_to_string(&path).expect("read ply-button.rkwd");
        let d: WidgetDescriptor = serde_json::from_str(&text).expect("parse ply-button.rkwd");
        let errs = validate_descriptor(&d);
        assert!(
            errs.is_empty(),
            "ply-button.rkwd failed validation: {errs:?}"
        );
    }
}
