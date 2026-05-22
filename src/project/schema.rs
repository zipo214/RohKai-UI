use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// App-level properties (serialized with the project)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppProps {
    pub title: String,
    pub win_w: f32,
    pub win_h: f32,
    pub icon_path: Option<String>,
}

impl Default for AppProps {
    fn default() -> Self {
        Self {
            title: String::from("My App"),
            win_w: 800.0,
            win_h: 600.0,
            icon_path: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Widget kinds
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WidgetKind {
    // Original five
    Button,
    Label,
    TextInput,
    Slider,
    Checkbox,
    // Stage 5 additions
    Frame,
    ComboBox,
    RadioButton,
    ProgressBar,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetProps {
    pub label: String,
    pub min: f32,
    pub max: f32,
    /// Preview default for Slider: thumb position + exported Default value.
    #[serde(default = "half_f32")]
    pub default_value: f32,
    /// ComboBox display options. Meaningful for ComboBox, defaulted for old projects.
    #[serde(
        default = "default_combo_options",
        skip_serializing_if = "is_default_combo_options"
    )]
    pub options: Vec<String>,
}

fn half_f32() -> f32 {
    0.5
}

pub fn default_combo_options() -> Vec<String> {
    vec![
        "Option A".to_owned(),
        "Option B".to_owned(),
        "Option C".to_owned(),
    ]
}

fn is_default_combo_options(options: &[String]) -> bool {
    options == default_combo_options().as_slice()
}

impl Default for WidgetProps {
    fn default() -> Self {
        Self {
            label: String::from("Widget"),
            min: 0.0,
            max: 100.0,
            default_value: 0.5,
            options: default_combo_options(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Default for Rect {
    fn default() -> Self {
        Self {
            x: 20.0,
            y: 20.0,
            w: 120.0,
            h: 32.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum CustomPropType {
    #[default]
    String,
    F32,
    Bool,
    I32,
}

impl CustomPropType {
    pub fn rust_type(&self) -> &'static str {
        match self {
            CustomPropType::String => "String",
            CustomPropType::F32 => "f32",
            CustomPropType::Bool => "bool",
            CustomPropType::I32 => "i32",
        }
    }
    pub fn default_expr(&self) -> &'static str {
        match self {
            CustomPropType::String => "String::new()",
            CustomPropType::F32 => "0.0",
            CustomPropType::Bool => "false",
            CustomPropType::I32 => "0",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            CustomPropType::String => "String",
            CustomPropType::F32 => "f32",
            CustomPropType::Bool => "bool",
            CustomPropType::I32 => "i32",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProp {
    pub name: String,
    #[serde(default)]
    pub ty: CustomPropType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetInstance {
    pub id: Uuid,
    pub kind: WidgetKind,
    pub rect: Rect,
    pub props: WidgetProps,
    pub state_binding: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_metadata: Option<SvgImportMetadata>,
    // Stage 5.5 — Properties Depth
    /// Hover text — generates .on_hover_text("…") in codegen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    /// None = always enabled; Some(false) = disabled (ui.set_enabled(false)).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Foreground/text color override [R, G, B].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fg_color: Option<[u8; 3]>,
    /// Corner rounding radius. 0.0 = default egui rounding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f32>,
    /// When set, label text is sourced from this AppState field (Bound mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_binding: Option<String>,
    /// Additional per-widget AppState fields added via "+ Add property".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_props: Vec<CustomProp>,
    // Stage 5.5 — Event Wiring
    /// Handler function name for on_click (Button) or on_change (others).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_handler: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvgImportMetadata {
    pub element_name: String,
    pub original_id: Option<String>,
    pub original_class: Option<String>,
    pub source_order: usize,
    pub transform_summary: String,
    pub warning_flags: Vec<String>,
}
