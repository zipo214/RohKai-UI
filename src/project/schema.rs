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
    /// Design-time non-visual components (timers, data sources, etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<DesignComponent>,
    /// Project assets (images, fonts, data files) referenced by path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<AssetEntry>,
    /// Stage 11 — app-wide Rust wiring (channels, iterators, trait impls).
    #[serde(default, skip_serializing_if = "rust_wiring_is_empty")]
    pub rust_wiring: RustWiring,
}

fn rust_wiring_is_empty(w: &RustWiring) -> bool {
    w.channels.is_empty() && w.iterators.is_empty() && w.trait_impls.is_empty()
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
            components: Vec::new(),
            assets: Vec::new(),
            rust_wiring: RustWiring::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// State-machine schema types (P2.5)
// ---------------------------------------------------------------------------

/// One state in a StateMachine component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateDef {
    /// Unique name for this state, e.g. `"idle"`.
    pub name: String,
    /// Optional Rust expression called on entry to this state.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub entry_action: String,
    /// Optional Rust expression called on exit from this state.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub exit_action: String,
}

impl Default for StateDef {
    fn default() -> Self {
        // name defaults to "state" (not ""), so this cannot be derived.
        Self {
            name: String::from("state"),
            entry_action: String::new(),
            exit_action: String::new(),
        }
    }
}

/// A directed transition between two states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TransitionDef {
    /// Source state name.
    pub from: String,
    /// Destination state name.
    pub to: String,
    /// Optional boolean guard expression (e.g. `"self.count > 0"`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub guard: String,
    /// Optional Rust expression to execute during the transition.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub action: String,
}

/// Full state-machine definition attached to a `ComponentKind::StateMachine`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StateMachineProps {
    /// Ordered list of states.
    #[serde(default)]
    pub states: Vec<StateDef>,
    /// Transition edges.
    #[serde(default)]
    pub transitions: Vec<TransitionDef>,
    /// Name of the initial state (must match an entry in `states`).
    #[serde(default)]
    pub initial_state: String,
}

// ---------------------------------------------------------------------------
// Design-time non-visual components
// ---------------------------------------------------------------------------

/// A non-visual design-time component (timer, data source, lifecycle hook).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignComponent {
    pub id: uuid::Uuid,
    pub kind: ComponentKind,
    /// Display name shown in the component tray.
    pub name: String,
    /// Interval in milliseconds (Timer only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_ms: Option<u32>,
    /// Handler function name emitted in codegen.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub handler: String,
    /// State-machine definition (StateMachine kind only).
    #[serde(default, skip_serializing_if = "state_machine_props_is_empty")]
    pub state_machine: StateMachineProps,
}

fn state_machine_props_is_empty(p: &StateMachineProps) -> bool {
    p.states.is_empty() && p.transitions.is_empty() && p.initial_state.is_empty()
}

/// A project asset reference (image, font, data file).
/// `name` is the in-project file name under `assets/`; `source_path` is where
/// the original lives on disk (for the manifest / future copy-on-export).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetEntry {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub source_path: String,
    #[serde(default)]
    pub kind: AssetKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum AssetKind {
    #[default]
    Other,
    Image,
    Font,
    Data,
}

// ---------------------------------------------------------------------------
// Stage 11 — Rust-centric wiring
// ---------------------------------------------------------------------------

/// Error-handling contract for a widget's event handler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum HandlerResult {
    /// `fn h(&mut self)` — no error channel.
    #[default]
    Plain,
    /// `fn h(&mut self) -> Result<(), String>` — call site logs `Err`.
    Result,
    /// `fn h(&mut self) -> Option<()>` — call site ignores `None`.
    Option,
}

impl HandlerResult {
    pub fn is_plain(&self) -> bool {
        matches!(self, HandlerResult::Plain)
    }
    /// Return-type suffix for a handler signature (empty for Plain).
    pub fn return_suffix(&self) -> &'static str {
        match self {
            HandlerResult::Plain => "",
            HandlerResult::Result => " -> Result<(), String>",
            HandlerResult::Option => " -> Option<()>",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            HandlerResult::Plain => "Plain",
            HandlerResult::Result => "Result",
            HandlerResult::Option => "Option",
        }
    }
}

/// App-wide Rust wiring: channels, iterator pipelines, trait impls.
/// Persisted on `AppProps`; emitted by `codegen::rust_wiring`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RustWiring {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<ChannelDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub iterators: Vec<IteratorPipeline>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trait_impls: Vec<TraitImpl>,
}

/// A named `std::sync::mpsc` channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelDef {
    pub id: Uuid,
    pub name: String,
    /// Rust element type carried by the channel, e.g. `f32`, `String`.
    pub ty: String,
}

/// A visually-built iterator pipeline that generates a method.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IteratorPipeline {
    pub id: Uuid,
    pub name: String,
    /// Source expression iterated over, e.g. `self.state.items`.
    pub source: String,
    pub ops: Vec<IterOp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IterOp {
    Map(String),
    Filter(String),
}

/// A user-authored trait implementation for the exported app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraitImpl {
    pub id: Uuid,
    pub trait_name: String,
    /// Method signature without the trailing block, e.g. `fn run(&mut self)`.
    pub method: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ComponentKind {
    /// Periodic timer — fires `handler` every `interval_ms`.
    Timer,
    /// Data source placeholder — emits a field + fetch function stub.
    DataSource,
    /// App lifecycle hook — on_startup / on_shutdown.
    Lifecycle,
    /// State machine — generates a state enum + transition handler (Stage 10).
    StateMachine,
    /// HTTP request component — emits a fetch stub + response field (Stage 10).
    HttpRequest,
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
    /// Grid layout container (egui::Grid).
    GridLayout,
    /// Tabbed container widget.
    TabWidget,
    // Stage 10 — button family
    /// Compact tool button.
    ToolButton,
    /// Command link button — title + description.
    CommandLinkButton,
    /// Row of dialog buttons (OK / Cancel / …), from `options`.
    DialogButtonBox,
    // Stage 10 — computational / IO
    /// Displays a computed value from a bound AppState f32 field.
    MathLabel,
    /// File picker — browse button + selected-path display.
    FilePicker,
    /// 2D line / bar chart bound to a `Vec<f32>`.
    Chart,
    // Stage 10 — data views
    /// Tabular display (data table / table view), columns from `options`.
    Table,
    /// List view bound to a `Vec<String>`.
    ListView,
    /// Tree view — hierarchical data display.
    TreeView,
    // Stage 10 — additional containers
    /// Stacked container — shows one child page at a time.
    StackedWidget,
    /// Tool box — vertical collapsing sections.
    ToolBox,
}

// ---------------------------------------------------------------------------
// Widget event capability — single source of truth
//
// `WidgetKind::supported_events` is the ONE place that decides which UI events a
// widget kind exposes.  Both the Properties panel (`panels::properties`) and the
// export parity tests (`codegen::export`) derive from it, so the editor UI and
// the generated code can never silently disagree about which events exist.
// ---------------------------------------------------------------------------

/// A user-interaction event a widget can expose a handler for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetEvent {
    Click,
    DoubleClick,
    Change,
    LostFocus,
    DragStopped,
}

/// Every widget kind that exposes at least one event in the Properties panel.
/// Iterated by the export parity test so a new event-capable kind that forgets
/// its export handler wiring fails CI.  Only unit variants (no `Custom`).
///
/// Test-only: the canonical enumeration the parity test walks.  Production code
/// derives capability per-widget via [`WidgetKind::supported_events`] /
/// [`WidgetKind::primary_event`], so this list never needs to exist at runtime.
#[cfg(test)]
pub const EVENT_CAPABLE_KINDS: [WidgetKind; 9] = [
    WidgetKind::Button,
    WidgetKind::TextInput,
    WidgetKind::TextArea,
    WidgetKind::Slider,
    WidgetKind::SpinBox,
    WidgetKind::Checkbox,
    WidgetKind::ComboBox,
    WidgetKind::FontComboBox,
    WidgetKind::RadioButton,
];

impl WidgetKind {
    /// The authoritative list of events this kind exposes in Properties.
    ///
    /// The match is exhaustive (no wildcard) on purpose: adding a new
    /// `WidgetKind` will not compile until its event capability is declared
    /// here, which is what keeps Properties and export from drifting.
    pub fn supported_events(&self) -> &'static [WidgetEvent] {
        use WidgetEvent::*;
        match self {
            WidgetKind::Button => &[Click, DoubleClick],
            WidgetKind::TextInput | WidgetKind::TextArea => &[Change, LostFocus],
            WidgetKind::Slider | WidgetKind::SpinBox => &[Change, DragStopped],
            WidgetKind::Checkbox
            | WidgetKind::ComboBox
            | WidgetKind::FontComboBox
            | WidgetKind::RadioButton => &[Change],
            // Non-interactive / display / container / custom kinds expose no events.
            WidgetKind::Label
            | WidgetKind::Frame
            | WidgetKind::ProgressBar
            | WidgetKind::Image
            | WidgetKind::Custom(_)
            | WidgetKind::HorizontalSpacer
            | WidgetKind::VerticalSpacer
            | WidgetKind::GroupBox
            | WidgetKind::VLayout
            | WidgetKind::HLayout
            | WidgetKind::ScrollArea
            | WidgetKind::GridLayout
            | WidgetKind::TabWidget
            | WidgetKind::ToolButton
            | WidgetKind::CommandLinkButton
            | WidgetKind::DialogButtonBox
            | WidgetKind::MathLabel
            | WidgetKind::FilePicker
            | WidgetKind::Chart
            | WidgetKind::Table
            | WidgetKind::ListView
            | WidgetKind::TreeView
            | WidgetKind::StackedWidget
            | WidgetKind::ToolBox => &[],
        }
    }

    /// The first event a kind exposes (`Click` for `Button`, `Change` for
    /// value/selection widgets).  `None` when not event-capable.
    ///
    /// Test-only: export now wires **every** event from `supported_events`, not
    /// just the first, so production no longer needs a "primary" notion.  Kept
    /// for parity tests that assert per-kind classification.
    #[cfg(test)]
    pub fn primary_event(&self) -> Option<WidgetEvent> {
        self.supported_events().first().copied()
    }

    /// Whether Properties exposes any event handler for this kind.
    pub fn is_event_capable(&self) -> bool {
        !self.supported_events().is_empty()
    }
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

/// Cross-axis alignment for VLayout and HLayout containers.
/// For VLayout the cross axis is horizontal (Start = left, End = right).
/// For HLayout the cross axis is vertical (Start = top, End = bottom).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LayoutCrossAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Per-child cross-axis alignment override for a widget placed inside a
/// VLayout or HLayout. When set on the child, it overrides the parent's
/// `layout_cross_align` for that child only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CrossAlign {
    /// Align to the start of the cross axis (left for VLayout, top for HLayout).
    #[default]
    Start,
    /// Center on the cross axis.
    Center,
    /// Align to the end of the cross axis (right for VLayout, bottom for HLayout).
    End,
    /// Stretch to fill the full cross-axis extent.
    Stretch,
}

impl CrossAlign {
    pub fn label(&self) -> &'static str {
        match self {
            CrossAlign::Start => "Start",
            CrossAlign::Center => "Center",
            CrossAlign::End => "End",
            CrossAlign::Stretch => "Stretch",
        }
    }
}

/// Per-child size policy for widgets placed inside a layout container.
/// Controls how a child consumes available space along the main axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SizePolicy {
    /// Use the widget's own fixed width / height (canvas rect).
    #[default]
    Fixed,
    /// Expand to consume all available width; height stays fixed.
    FillWidth,
    /// Expand to fill all available space (width and height).
    Fill,
}

impl SizePolicy {
    pub fn is_fixed(&self) -> bool {
        matches!(self, SizePolicy::Fixed)
    }
}

// ---------------------------------------------------------------------------
// Data model types
// ---------------------------------------------------------------------------

/// Type of a data column in a Table widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DataColumnType {
    #[default]
    Text,
    Number,
    Boolean,
}

/// A named, typed column definition for Table data binding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataColumn {
    pub name: String,
    #[serde(default)]
    pub column_type: DataColumnType,
}

impl Default for DataColumn {
    fn default() -> Self {
        Self {
            name: "column".to_owned(),
            column_type: DataColumnType::Text,
        }
    }
}

impl LayoutCrossAlign {
    pub fn is_start(&self) -> bool {
        *self == LayoutCrossAlign::Start
    }
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

    // Layout containers
    #[serde(
        default = "default_layout_spacing",
        skip_serializing_if = "is_default_layout_spacing"
    )]
    pub layout_spacing: f32,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub layout_stretch: bool,
    #[serde(
        default = "default_grid_columns",
        skip_serializing_if = "is_default_grid_columns"
    )]
    pub grid_columns: usize,
    /// Optional human-readable names for GridLayout cells, indexed row-major.
    /// Names describe slots rather than children, so reordering a child moves
    /// it between stable named destinations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grid_slot_names: Vec<String>,
    #[serde(default, skip_serializing_if = "LayoutCrossAlign::is_start")]
    pub layout_cross_align: LayoutCrossAlign,

    // Formula widget
    /// Infix formula expression for MathLabel (e.g. "sqrt(a^2 + b^2)").
    /// Empty string → fall back to simple `self.{binding}` display.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub formula_expr: String,
    /// Decimal places for formula result display (default 2).
    #[serde(
        default = "default_formula_decimals",
        skip_serializing_if = "is_default_formula_decimals"
    )]
    pub formula_decimals: usize,

    // Data model binding
    /// AppState field name for model-backed Table / ListView / TreeView.
    /// When set, codegen iterates over the bound Vec instead of static options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_source_binding: Option<String>,
    /// Column definitions for model-backed Table widgets.
    /// Empty → single unnamed String column.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_columns: Vec<DataColumn>,

    // Per-child layout size policy
    /// How this widget expands inside a parent layout container.
    /// Fixed (default) uses the canvas rect dimensions.
    #[serde(default, skip_serializing_if = "SizePolicy::is_fixed")]
    pub size_policy: SizePolicy,

    // GridLayout row policy
    /// Minimum row height for GridLayout containers (in logical pixels).
    /// None = egui default (content-driven).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_row_height: Option<f32>,

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
fn default_layout_spacing() -> f32 {
    6.0
}
fn is_default_layout_spacing(v: &f32) -> bool {
    (*v - 6.0).abs() < 0.001
}
fn default_grid_columns() -> usize {
    3
}
fn is_default_grid_columns(v: &usize) -> bool {
    *v == 3
}
fn default_formula_decimals() -> usize {
    2
}
fn is_default_formula_decimals(v: &usize) -> bool {
    *v == 2
}
fn child_flex_is_zero(v: &f32) -> bool {
    *v == 0.0
}
fn default_one_u32() -> u32 {
    1
}
fn is_one_u32(v: &u32) -> bool {
    *v == 1
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
            layout_spacing: 6.0,
            layout_stretch: true,
            grid_columns: 3,
            grid_slot_names: Vec::new(),
            layout_cross_align: LayoutCrossAlign::Start,
            formula_expr: String::new(),
            formula_decimals: 2,
            data_source_binding: None,
            data_columns: Vec::new(),
            size_policy: SizePolicy::Fixed,
            grid_row_height: None,
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
// P2.3 — Constraint-Based Layout
// ---------------------------------------------------------------------------

/// Horizontal alignment anchor for constraint layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HAlign {
    /// Align to the left / leading edge.
    #[default]
    Leading,
    /// Align to the right / trailing edge.
    Trailing,
    /// Center horizontally.
    Center,
    /// Stretch to fill available width.
    Stretch,
}

/// Vertical alignment anchor for constraint layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VAlign {
    /// Align to the top edge.
    #[default]
    Top,
    /// Align to the bottom edge.
    Bottom,
    /// Center vertically.
    Center,
    /// Stretch to fill available height.
    Stretch,
}

/// Per-widget layout constraints applied by the constraint solver.
///
/// All fields are optional / defaulted so old project files deserialize cleanly.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayoutConstraints {
    /// Horizontal alignment anchor. `None` = unconstrained (position free).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_align: Option<HAlign>,
    /// Vertical alignment anchor. `None` = unconstrained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v_align: Option<VAlign>,
    /// ID of another widget whose width this widget should equal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equal_width_to: Option<String>,
    /// ID of another widget whose height this widget should equal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equal_height_to: Option<String>,
    /// Locked width-to-height aspect ratio (e.g. `Some(1.0)` for square).
    /// `None` = no lock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<f32>,
    /// Minimum width in logical pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_w: Option<f32>,
    /// Maximum width in logical pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_w: Option<f32>,
    /// Minimum height in logical pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_h: Option<f32>,
    /// Maximum height in logical pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_h: Option<f32>,
    /// Margin insets `[top, right, bottom, left]` in logical pixels.
    #[serde(default, skip_serializing_if = "margin_is_zero")]
    pub margin: [f32; 4],
}

fn margin_is_zero(m: &[f32; 4]) -> bool {
    m[0] == 0.0 && m[1] == 0.0 && m[2] == 0.0 && m[3] == 0.0
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
    /// Stage 11 — run this widget's handler on a background thread (std::thread + mpsc).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub async_handler: bool,
    /// Stage 11 — handler error-handling contract (Plain / Result / Option).
    #[serde(default, skip_serializing_if = "HandlerResult::is_plain")]
    pub handler_result: HandlerResult,
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

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_cross_align: Option<CrossAlign>,
    #[serde(default, skip_serializing_if = "child_flex_is_zero")]
    pub child_flex: f32,
    #[serde(default = "default_one_u32", skip_serializing_if = "is_one_u32")]
    pub grid_col_span: u32,
    #[serde(default = "default_one_u32", skip_serializing_if = "is_one_u32")]
    pub grid_row_span: u32,
    #[serde(default, skip_serializing_if = "constraints_are_default")]
    pub constraints: LayoutConstraints,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_binding: Option<DbBinding>,
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
            async_handler: false,
            handler_result: HandlerResult::Plain,
            svg_source: None,
            expand_svg_inline: false,
            descriptor_name: None,
            descriptor_accent: None,
            descriptor_live_tpl: None,
            descriptor_export_tpl: None,
            descriptor_props: HashMap::new(),
            descriptor_cargo_deps: Vec::new(),
            descriptor_state_fields: Vec::new(),
            child_cross_align: None,
            child_flex: 0.0,
            grid_col_span: 1,
            grid_row_span: 1,
            constraints: LayoutConstraints::default(),
            db_binding: None,
        }
    }
}

fn constraints_are_default(c: &LayoutConstraints) -> bool {
    c.h_align.is_none()
        && c.v_align.is_none()
        && c.equal_width_to.is_none()
        && c.equal_height_to.is_none()
        && c.aspect_ratio.is_none()
        && c.min_w.is_none()
        && c.max_w.is_none()
        && c.min_h.is_none()
        && c.max_h.is_none()
        && margin_is_zero(&c.margin)
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
    /// When this widget is one chunk of a multi-label SVG `<text>` import (R6),
    /// the shared group id tying the chunks of one text element together.
    #[serde(default)]
    pub text_group: Option<String>,
}

// ---------------------------------------------------------------------------
// Stage 13 — Database binding
// ---------------------------------------------------------------------------

/// A widget → database column binding.
/// When present on a `WidgetInstance`, `state_emitter` emits a
/// `rusqlite::Connection` field and a `load_from_db()` method stub.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DbBinding {
    /// SQLite table name (unquoted, validated as a plain identifier).
    pub table: String,
    /// Column name to bind the widget value to.
    pub column: String,
}

impl Default for DbBinding {
    fn default() -> Self {
        Self {
            table: String::from("my_table"),
            column: String::from("my_column"),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_capable_kinds_are_all_event_capable() {
        for k in EVENT_CAPABLE_KINDS {
            assert!(
                k.is_event_capable(),
                "{k:?} is in EVENT_CAPABLE_KINDS but reports no events"
            );
            assert!(
                k.primary_event().is_some(),
                "{k:?} must have a primary event"
            );
        }
    }

    #[test]
    fn primary_event_is_click_for_button_change_for_value_widgets() {
        assert_eq!(WidgetKind::Button.primary_event(), Some(WidgetEvent::Click));
        for k in [
            WidgetKind::TextInput,
            WidgetKind::TextArea,
            WidgetKind::Slider,
            WidgetKind::SpinBox,
            WidgetKind::Checkbox,
            WidgetKind::ComboBox,
            WidgetKind::FontComboBox,
            WidgetKind::RadioButton,
        ] {
            assert_eq!(
                k.primary_event(),
                Some(WidgetEvent::Change),
                "{k:?} primary event must be Change"
            );
        }
    }

    #[test]
    fn previously_missed_kinds_are_event_capable() {
        // Regression guard for the Properties/export drift bug: these three
        // exposed On Change in Properties but were silently ignored by export.
        for k in [
            WidgetKind::TextArea,
            WidgetKind::SpinBox,
            WidgetKind::FontComboBox,
        ] {
            assert!(k.is_event_capable());
            assert!(k.supported_events().contains(&WidgetEvent::Change));
        }
    }

    #[test]
    fn db_binding_serde_default_is_none() {
        // A WidgetInstance serialised without a db_binding field must deserialise
        // with db_binding = None.  This guards the #[serde(default)] annotation.
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000000",
            "kind": "Button",
            "rect": {"x":20,"y":20,"w":120,"h":32},
            "props": {"label":"X","min":0,"max":1,"default_value":0,
                      "options":["Option A","Option B","Option C"]},
            "state_binding": null
        }"#;
        let w: WidgetInstance = serde_json::from_str(json).unwrap();
        assert!(
            w.db_binding.is_none(),
            "db_binding should default to None when absent from JSON"
        );
    }

    #[test]
    fn display_and_container_kinds_have_no_events() {
        for k in [
            WidgetKind::Label,
            WidgetKind::Frame,
            WidgetKind::ProgressBar,
            WidgetKind::Image,
            WidgetKind::GroupBox,
            WidgetKind::Table,
            WidgetKind::Chart,
            WidgetKind::Custom("anything".to_owned()),
        ] {
            assert!(!k.is_event_capable(), "{k:?} should expose no events");
            assert_eq!(k.primary_event(), None);
        }
    }

    // P2.4 tests ----------------------------------------------------------------

    #[test]
    fn child_flex_default_is_zero() {
        let w = WidgetInstance::default();
        assert_eq!(w.child_flex, 0.0, "child_flex must default to 0.0");
        assert_eq!(w.grid_col_span, 1);
        assert_eq!(w.grid_row_span, 1);
        assert!(w.child_cross_align.is_none());
    }

    #[test]
    fn layout_child_fields_serde_round_trip() {
        let w = WidgetInstance {
            child_cross_align: Some(CrossAlign::Center),
            child_flex: 1.5,
            grid_col_span: 2,
            grid_row_span: 3,
            ..Default::default()
        };

        let json = serde_json::to_string(&w).expect("serialize");
        let w2: WidgetInstance = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(w2.child_cross_align, Some(CrossAlign::Center));
        assert!((w2.child_flex - 1.5).abs() < 0.001);
        assert_eq!(w2.grid_col_span, 2);
        assert_eq!(w2.grid_row_span, 3);
    }

    #[test]
    fn layout_child_fields_skip_serialized_at_defaults() {
        let w = WidgetInstance::default();
        let json = serde_json::to_string(&w).expect("serialize");
        assert!(
            !json.contains("child_cross_align"),
            "default cross_align omitted"
        );
        assert!(!json.contains("child_flex"), "zero flex omitted");
        assert!(!json.contains("grid_col_span"), "default col_span omitted");
        assert!(!json.contains("grid_row_span"), "default row_span omitted");
    }

    // P2.5 state-machine tests -----------------------------------------------

    #[test]
    fn state_machine_default_has_empty_states() {
        let sm = StateMachineProps::default();
        assert!(
            sm.states.is_empty(),
            "default StateMachineProps must have no states"
        );
        assert!(
            sm.transitions.is_empty(),
            "default StateMachineProps must have no transitions"
        );
        assert!(
            sm.initial_state.is_empty(),
            "default initial_state must be empty"
        );
    }

    #[test]
    fn design_component_state_machine_round_trips() {
        use uuid::Uuid;
        let comp = DesignComponent {
            id: Uuid::new_v4(),
            kind: ComponentKind::StateMachine,
            name: "flow".to_owned(),
            interval_ms: None,
            handler: String::new(),
            state_machine: StateMachineProps {
                states: vec![
                    StateDef {
                        name: "idle".to_owned(),
                        entry_action: String::new(),
                        exit_action: String::new(),
                    },
                    StateDef {
                        name: "running".to_owned(),
                        entry_action: "self.start()".to_owned(),
                        exit_action: String::new(),
                    },
                ],
                transitions: vec![TransitionDef {
                    from: "idle".to_owned(),
                    to: "running".to_owned(),
                    guard: "self.ready".to_owned(),
                    action: String::new(),
                }],
                initial_state: "idle".to_owned(),
            },
        };
        let json = serde_json::to_string(&comp).expect("serialize");
        let back: DesignComponent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.state_machine.states.len(), 2);
        assert_eq!(back.state_machine.transitions.len(), 1);
        assert_eq!(back.state_machine.initial_state, "idle");
    }

    #[test]
    fn timer_default_interval_is_1000ms() {
        use uuid::Uuid;
        let comp = DesignComponent {
            id: Uuid::new_v4(),
            kind: ComponentKind::Timer,
            name: "tick".to_owned(),
            interval_ms: None,
            handler: String::new(),
            state_machine: StateMachineProps::default(),
        };
        assert_eq!(comp.interval_ms.unwrap_or(1000), 1000);
    }
}
