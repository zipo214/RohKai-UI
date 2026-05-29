use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers for serde defaults (accent added here; default_true defined below)
// ---------------------------------------------------------------------------

fn default_accent() -> [u8; 3] {
    [52, 211, 153]
}

// ---------------------------------------------------------------------------
// Guide lines (persisted with project)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GuideOrientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuideRule {
    pub id: Uuid,
    pub orientation: GuideOrientation,
    /// Position in canvas pixels along the perpendicular axis.
    pub position: f32,
}

// ---------------------------------------------------------------------------
// Theme settings (persisted with project, also exportable)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSettings {
    #[serde(default = "default_true")]
    pub dark_mode: bool,
    #[serde(default = "default_accent")]
    pub accent_color: [u8; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_font_size: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_corner_radius: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing_scale: Option<f32>,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            dark_mode: true,
            accent_color: [52, 211, 153],
            base_font_size: None,
            global_corner_radius: None,
            spacing_scale: None,
        }
    }
}

// ---------------------------------------------------------------------------
// App-level properties (serialized with the project)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppProps {
    pub title: String,
    pub win_w: f32,
    pub win_h: f32,
    pub icon_path: Option<String>,
    /// Whether the exported app window is user-resizable.
    #[serde(default = "default_true")]
    pub resizable: bool,
    /// Minimum window size `[w, h]` for the exported app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_size: Option<[f32; 2]>,
    /// Maximum window size `[w, h]` for the exported app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_size: Option<[f32; 2]>,
    /// Designer and exported app theme.
    #[serde(default)]
    pub theme: ThemeSettings,
    /// Persistent canvas guide lines.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guides: Vec<GuideRule>,
    /// Show mock OS title-bar bezel above the canvas boundary.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub show_bezel: bool,
}

impl Default for AppProps {
    fn default() -> Self {
        Self {
            title: String::from("My App"),
            win_w: 800.0,
            win_h: 600.0,
            icon_path: None,
            resizable: true,
            min_size: None,
            max_size: None,
            theme: ThemeSettings::default(),
            guides: Vec::new(),
            show_bezel: false,
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
    // Stage 7 — SVG image import
    Image,
    // Stage 7 — runtime-loaded `.rkwd` widget descriptor
    Custom(String),
    // Stage 9 — new input widgets
    /// Multi-line text editor.
    TextArea,
    /// Numeric spinner / drag-value input.
    SpinBox,
    /// Font family picker combo-box.
    FontComboBox,
    // Stage 9 — spacers
    HorizontalSpacer,
    VerticalSpacer,
    // Stage 9 — layout containers
    /// Labeled group box (Frame with a heading).
    GroupBox,
    /// Vertical stack layout container.
    VLayout,
    /// Horizontal stack layout container.
    HLayout,
    /// Scrollable container.
    ScrollArea,
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

    // Stage 9 schema audit
    /// Wrap text at widget boundary (Label, TextArea). None = egui default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_wrap: Option<bool>,
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
            text_wrap: None,
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

    // Stage 5.5 / Stage 9 — Event Wiring
    /// Handler for Button.on_click.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub on_click: String,
    /// Handler for interactive widget .on_change.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub on_change: String,
    /// Handler for Button.double_clicked() — Stage 9.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub on_double_click: String,
    /// Handler fired when a text field loses focus — Stage 9.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub on_lost_focus: String,
    /// Handler fired when a drag interaction ends (Slider, SpinBox) — Stage 9.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub on_drag_stopped: String,
    /// Legacy single handler field — kept for backward-compat with old saves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_handler: Option<String>,
    /// Raw SVG source for Image widgets. Canvas preview is drawn by RohKai's
    /// native zero-dependency SVG placeholder painter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub svg_source: Option<String>,
    /// When true, the live code panel embeds the full SVG source inline instead
    /// of the compact `[SVG: N bytes]` placeholder.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub expand_svg_inline: bool,

    // Stage 7 — Custom widget descriptor snapshots (set at creation time)
    /// Display name from the descriptor (e.g. `"Ply Button"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor_name: Option<String>,
    /// Accent colour `[R, G, B]` from the descriptor for canvas rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor_accent: Option<[u8; 3]>,
    /// Live-preview codegen template snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor_live_tpl: Option<String>,
    /// Export codegen template snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor_export_tpl: Option<String>,
    /// Runtime values of descriptor-defined properties (key → value string).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub descriptor_props: HashMap<String, String>,
    /// Cargo dependency lines to inject into exported `Cargo.toml`
    /// (e.g. `["ply-ui = \"0.3\""]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub descriptor_cargo_deps: Vec<String>,
    /// Descriptor-defined AppState fields.  Each entry is
    /// `[key, rust_type, default_expr]` — snapshotted from the descriptor at
    /// widget-creation time so state_emitter works without re-loading descriptors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub descriptor_state_fields: Vec<[String; 3]>,
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
            on_double_click: String::new(),
            on_lost_focus: String::new(),
            on_drag_stopped: String::new(),
            event_handler: None,
            svg_source: None,
            expand_svg_inline: false,
            descriptor_name: None,
            descriptor_accent: None,
            descriptor_live_tpl: None,
            descriptor_export_tpl: None,
            descriptor_props: HashMap::new(),
            descriptor_cargo_deps: Vec::new(),
            descriptor_state_fields: Vec::new(),
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
