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

// ---------------------------------------------------------------------------
// New enums (Part 1 of schema audit)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

// ---------------------------------------------------------------------------
// WidgetProps — per-widget content & behaviour knobs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetProps {
    // Universal
    pub label: String,

    // Numeric range (Slider / ProgressBar)
    pub min: f32,
    pub max: f32,
    #[serde(default)]
    pub default_value: f32,

    // ComboBox items
    #[serde(
        default = "default_combo_options",
        skip_serializing_if = "is_default_combo_options"
    )]
    pub options: Vec<String>,

    // Slider specific
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f32>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_value: bool,
    #[serde(default, skip_serializing_if = "is_horizontal")]
    pub orientation: Orientation,

    // TextInput specific
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub placeholder: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub password_mode: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,

    // RadioButton specific
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub radio_value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group_binding: String,

    // ProgressBar specific
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_percentage: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub animated: bool,

    // Frame specific
    #[serde(
        default = "default_inner_margin",
        skip_serializing_if = "is_default_inner_margin"
    )]
    pub inner_margin: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_color: Option<[u8; 3]>,
    #[serde(
        default = "default_stroke_width",
        skip_serializing_if = "is_default_stroke_width"
    )]
    pub stroke_width: f32,
}

// ---------------------------------------------------------------------------
// WidgetProps serde helpers
// ---------------------------------------------------------------------------

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

fn default_true() -> bool {
    true
}
fn is_true(v: &bool) -> bool {
    *v
}
fn is_false(v: &bool) -> bool {
    !v
}
fn is_horizontal(o: &Orientation) -> bool {
    *o == Orientation::Horizontal
}
fn default_inner_margin() -> f32 {
    8.0
}
fn is_default_inner_margin(v: &f32) -> bool {
    (*v - 8.0).abs() < 0.001
}
fn default_stroke_width() -> f32 {
    1.0
}
fn is_default_stroke_width(v: &f32) -> bool {
    (*v - 1.0).abs() < 0.001
}

impl Default for WidgetProps {
    fn default() -> Self {
        Self {
            label: String::from("Widget"),
            min: 0.0,
            max: 1.0,
            default_value: 0.0,
            options: default_combo_options(),
            step: None,
            show_value: true,
            orientation: Orientation::Horizontal,
            placeholder: String::new(),
            password_mode: false,
            max_length: None,
            radio_value: String::new(),
            group_binding: String::new(),
            show_percentage: false,
            animated: false,
            inner_margin: 8.0,
            stroke_color: None,
            stroke_width: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Rect
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Custom properties
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// WidgetInstance
// ---------------------------------------------------------------------------

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    /// None = always enabled; Some(false) = disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Foreground/text color [R, G, B].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fg_color: Option<[u8; 3]>,
    /// Background/fill color [R, G, B].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_color: Option<[u8; 3]>,
    /// Corner rounding radius.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f32>,
    /// Override font size (pt). None = egui default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    /// Text alignment. None = widget-kind default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_align: Option<TextAlign>,
    /// Label text sourced from AppState field (Bound mode for Label kind).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_binding: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_props: Vec<CustomProp>,

    // Stage 5.5 — Event Wiring
    /// Handler for Button.on_click.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub on_click: String,
    /// Handler for interactive widget .on_change.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub on_change: String,
    /// Legacy single handler field — kept for backward-compat with old saves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_handler: Option<String>,
}

impl Default for WidgetInstance {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            kind: WidgetKind::Button,
            rect: Rect::default(),
            props: WidgetProps::default(),
            state_binding: None,
            children: Vec::new(),
            import_metadata: None,
            tooltip: None,
            enabled: None,
            fg_color: None,
            bg_color: None,
            corner_radius: None,
            font_size: None,
            text_align: None,
            label_binding: None,
            custom_props: Vec::new(),
            on_click: String::new(),
            on_change: String::new(),
            event_handler: None,
        }
    }
}

// ---------------------------------------------------------------------------
// SVG import metadata
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvgImportMetadata {
    pub element_name: String,
    pub original_id: Option<String>,
    pub original_class: Option<String>,
    pub source_order: usize,
    pub transform_summary: String,
    pub warning_flags: Vec<String>,
}
