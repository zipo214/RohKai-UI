use crate::project::schema::{Rect as SchemaRect, WidgetInstance, WidgetKind};
use crate::project::ui_tree::UiTree;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// SVG texture cache for Image widgets.
//
// Key: widget UUID.  Value: (TextureHandle, scale_at_rasterization, tw, th).
// Scale = zoom × pixels_per_point.  tw/th = physical pixel dimensions used
// for the rasterization.
//
// Re-rasterize immediately when tw/th change (widget resized).
// Re-rasterize on scale drift > 20 % only when zoom is stable (no scroll
// input this frame) — during active zoom the stale texture is rendered at
// GPU scale; one clean re-rasterize fires on the first quiet frame after
// the gesture ends.
// ---------------------------------------------------------------------------

pub type SvgTextureCache = HashMap<Uuid, (egui::TextureHandle, f32, u32, u32)>;

pub fn svg_texture_cache_retain_live(cache: &mut SvgTextureCache, live_ids: &HashSet<Uuid>) {
    cache.retain(|id, _| live_ids.contains(id));
}

const HANDLE_HALF: f32 = 4.0;
const MIN_SIZE: f32 = 20.0;
const MIN_SNAP_STEP: f32 = 1.0;

// ---------------------------------------------------------------------------
// Canvas-level settings  (non-serialized rendering state)
// ---------------------------------------------------------------------------

pub struct CanvasSettings {
    pub snap_enabled: bool,
    pub snap_step: f32,
    /// Zoom scale: 0.25 = 25 %, 1.0 = 100 %, 4.0 = 400 %.
    pub zoom: f32,
    /// Extra pan offset in screen pixels from the natural centred position.
    pub pan: egui::Vec2,
    /// Whether the pixel ruler strips are visible (Ctrl+R toggle).
    pub show_rulers: bool,
    /// Set each frame by the caller when a ruler guide is being dragged;
    /// suppresses rubber-band and widget drag so they don't co-fire.
    pub guide_drag_active: bool,
    /// Set each frame only while a true modal workflow blocks the entire canvas.
    /// Floating windows are isolated through response/layer ownership instead.
    pub input_blocked: bool,
}

#[derive(Clone, Copy)]
pub struct CanvasTextSettings {
    pub label_scale: f32,
    pub tag_scale: f32,
}

impl Default for CanvasTextSettings {
    fn default() -> Self {
        Self {
            label_scale: 1.0,
            tag_scale: 1.0,
        }
    }
}

impl Default for CanvasSettings {
    fn default() -> Self {
        Self {
            snap_enabled: false,
            snap_step: 8.0,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            show_rulers: false,
            guide_drag_active: false,
            input_blocked: false,
        }
    }
}

fn snap(val: f32, step: f32) -> f32 {
    let step = step.max(MIN_SNAP_STEP);
    (val / step).round() * step
}

fn snap_rect(mut r: SchemaRect, step: f32) -> SchemaRect {
    r.x = snap(r.x, step).max(0.0);
    r.y = snap(r.y, step).max(0.0);
    r.w = snap(r.w, step).max(MIN_SIZE);
    r.h = snap(r.h, step).max(MIN_SIZE);
    r
}

// ---------------------------------------------------------------------------
// Resize handle
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResizeHandle {
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl ResizeHandle {
    const ALL: [ResizeHandle; 8] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Left,
        Self::Right,
        Self::BottomLeft,
        Self::Bottom,
        Self::BottomRight,
    ];

    fn anchor(self, rect: egui::Rect) -> egui::Pos2 {
        match self {
            Self::TopLeft => rect.left_top(),
            Self::Top => rect.center_top(),
            Self::TopRight => rect.right_top(),
            Self::Left => rect.left_center(),
            Self::Right => rect.right_center(),
            Self::BottomLeft => rect.left_bottom(),
            Self::Bottom => rect.center_bottom(),
            Self::BottomRight => rect.right_bottom(),
        }
    }

    fn hit_rect(self, rect: egui::Rect) -> egui::Rect {
        // Handles float 4 px OUTSIDE the widget rect so the widget's
        // appearance is fully visible while handles are still grabbable.
        let outer = rect.expand(4.0);
        egui::Rect::from_center_size(
            self.anchor(outer),
            egui::vec2(HANDLE_HALF * 2.0, HANDLE_HALF * 2.0),
        )
    }

    fn cursor(self) -> egui::CursorIcon {
        match self {
            Self::TopLeft | Self::BottomRight => egui::CursorIcon::ResizeNwSe,
            Self::TopRight | Self::BottomLeft => egui::CursorIcon::ResizeNeSw,
            Self::Top | Self::Bottom => egui::CursorIcon::ResizeVertical,
            Self::Left | Self::Right => egui::CursorIcon::ResizeHorizontal,
        }
    }

    fn apply_delta(self, start: &SchemaRect, delta: egui::Vec2) -> SchemaRect {
        let mut r = start.clone();
        match self {
            Self::TopLeft => {
                let dx = delta.x.min(r.w - MIN_SIZE);
                let dy = delta.y.min(r.h - MIN_SIZE);
                r.x += dx;
                r.y += dy;
                r.w -= dx;
                r.h -= dy;
            }
            Self::Top => {
                let dy = delta.y.min(r.h - MIN_SIZE);
                r.y += dy;
                r.h -= dy;
            }
            Self::TopRight => {
                let dy = delta.y.min(r.h - MIN_SIZE);
                r.y += dy;
                r.h -= dy;
                r.w = (r.w + delta.x).max(MIN_SIZE);
            }
            Self::Left => {
                let dx = delta.x.min(r.w - MIN_SIZE);
                r.x += dx;
                r.w -= dx;
            }
            Self::Right => {
                r.w = (r.w + delta.x).max(MIN_SIZE);
            }
            Self::BottomLeft => {
                let dx = delta.x.min(r.w - MIN_SIZE);
                r.x += dx;
                r.w -= dx;
                r.h = (r.h + delta.y).max(MIN_SIZE);
            }
            Self::Bottom => {
                r.h = (r.h + delta.y).max(MIN_SIZE);
            }
            Self::BottomRight => {
                r.w = (r.w + delta.x).max(MIN_SIZE);
                r.h = (r.h + delta.y).max(MIN_SIZE);
            }
        }
        r.x = r.x.max(0.0);
        r.y = r.y.max(0.0);
        r
    }
}

// ---------------------------------------------------------------------------
// Interaction state
// ---------------------------------------------------------------------------

pub struct ResizeState {
    pub id: Uuid,
    pub handle: ResizeHandle,
    pub start_rect: SchemaRect,
    pub start_pos: egui::Pos2,
}

pub struct DragState {
    pub start_pos: egui::Pos2,
    pub start_rects: Vec<(Uuid, SchemaRect)>,
}

/// Tracks an in-progress drag-reorder of a layout child.
/// Active from mouse-down on a layout-child until release.
pub struct ReorderDrag {
    /// The child being dragged.
    pub child_id: Uuid,
    /// Parent layout container.
    pub parent_id: Uuid,
    /// Current insertion index (between children) — updated every frame.
    pub insert_idx: usize,
}

#[derive(Default)]
pub struct InteractionState {
    pub drag: Option<DragState>,
    pub resize: Option<ResizeState>,
    pub rubber_band: Option<egui::Pos2>,
    pub context_menu: Option<(Uuid, egui::Pos2)>,
    /// Set each frame: Some(id) when a widget was double-clicked this frame.
    pub double_clicked_widget: Option<Uuid>,
    /// Set when a template drag is in flight (instances to place on drop).
    pub template_drag: Option<Vec<WidgetInstance>>,
    /// Inline label editing: (widget_id, current text buffer).
    /// Double-clicking a label-bearing widget on canvas starts this.
    pub inline_edit: Option<(Uuid, String)>,
    /// Keyboard shortcuts target the canvas only after the user interacts with
    /// the visible canvas surface. Clicking another panel/window clears this.
    pub canvas_focused: bool,
    /// Active drag-reorder session (VLayout / HLayout child being repositioned).
    pub reorder_drag: Option<ReorderDrag>,
}

// ---------------------------------------------------------------------------
// Widget visual helpers
// ---------------------------------------------------------------------------

pub fn kind_accent(kind: &WidgetKind) -> egui::Color32 {
    match kind {
        WidgetKind::Button => egui::Color32::from_rgb(52, 211, 153),
        WidgetKind::Label => egui::Color32::from_rgb(156, 163, 175),
        WidgetKind::TextInput => egui::Color32::from_rgb(96, 165, 250),
        WidgetKind::TextArea => egui::Color32::from_rgb(56, 145, 240),
        WidgetKind::Slider => egui::Color32::from_rgb(251, 146, 60),
        WidgetKind::SpinBox => egui::Color32::from_rgb(251, 170, 80),
        WidgetKind::Checkbox => egui::Color32::from_rgb(167, 139, 250),
        WidgetKind::Frame => egui::Color32::from_rgb(200, 200, 200),
        WidgetKind::ComboBox => egui::Color32::from_rgb(251, 191, 36),
        WidgetKind::FontComboBox => egui::Color32::from_rgb(240, 180, 40),
        WidgetKind::RadioButton => egui::Color32::from_rgb(251, 113, 133),
        WidgetKind::ProgressBar => egui::Color32::from_rgb(34, 211, 238),
        WidgetKind::Image => egui::Color32::from_rgb(248, 113, 113),
        WidgetKind::HorizontalSpacer => egui::Color32::from_rgb(100, 130, 100),
        WidgetKind::VerticalSpacer => egui::Color32::from_rgb(100, 130, 100),
        WidgetKind::GroupBox => egui::Color32::from_rgb(180, 180, 210),
        WidgetKind::VLayout => egui::Color32::from_rgb(130, 200, 160),
        WidgetKind::HLayout => egui::Color32::from_rgb(160, 200, 130),
        WidgetKind::ScrollArea => egui::Color32::from_rgb(140, 180, 200),
        WidgetKind::GridLayout => egui::Color32::from_rgb(160, 190, 150),
        WidgetKind::TabWidget => egui::Color32::from_rgb(180, 150, 200),
        WidgetKind::ToolButton => egui::Color32::from_rgb(72, 201, 173),
        WidgetKind::CommandLinkButton => egui::Color32::from_rgb(92, 211, 163),
        WidgetKind::DialogButtonBox => egui::Color32::from_rgb(52, 191, 143),
        WidgetKind::MathLabel => egui::Color32::from_rgb(120, 200, 220),
        WidgetKind::FilePicker => egui::Color32::from_rgb(116, 175, 240),
        WidgetKind::Chart => egui::Color32::from_rgb(60, 200, 200),
        WidgetKind::Table => egui::Color32::from_rgb(210, 180, 120),
        WidgetKind::ListView => egui::Color32::from_rgb(200, 190, 130),
        WidgetKind::TreeView => egui::Color32::from_rgb(190, 200, 120),
        WidgetKind::StackedWidget => egui::Color32::from_rgb(170, 160, 210),
        WidgetKind::ToolBox => egui::Color32::from_rgb(160, 170, 210),
        WidgetKind::Custom(_) => egui::Color32::from_rgb(150, 150, 220),
    }
}

pub fn kind_tag(kind: &WidgetKind) -> &'static str {
    match kind {
        WidgetKind::Button => "btn",
        WidgetKind::Label => "lbl",
        WidgetKind::TextInput => "txt",
        WidgetKind::TextArea => "area",
        WidgetKind::Slider => "sldr",
        WidgetKind::SpinBox => "spin",
        WidgetKind::Checkbox => "chk",
        WidgetKind::Frame => "frm",
        WidgetKind::ComboBox => "cmb",
        WidgetKind::FontComboBox => "font",
        WidgetKind::RadioButton => "rad",
        WidgetKind::ProgressBar => "prg",
        WidgetKind::Image => "img",
        WidgetKind::HorizontalSpacer => "h-sp",
        WidgetKind::VerticalSpacer => "v-sp",
        WidgetKind::GroupBox => "grp",
        WidgetKind::VLayout => "v-lay",
        WidgetKind::HLayout => "h-lay",
        WidgetKind::ScrollArea => "scrl",
        WidgetKind::GridLayout => "grid",
        WidgetKind::TabWidget => "tabs",
        WidgetKind::ToolButton => "tool",
        WidgetKind::CommandLinkButton => "clnk",
        WidgetKind::DialogButtonBox => "btns",
        WidgetKind::MathLabel => "math",
        WidgetKind::FilePicker => "file",
        WidgetKind::Chart => "chrt",
        WidgetKind::Table => "tbl",
        WidgetKind::ListView => "list",
        WidgetKind::TreeView => "tree",
        WidgetKind::StackedWidget => "stk",
        WidgetKind::ToolBox => "tbox",
        WidgetKind::Custom(_) => "cst",
    }
}

fn kind_fill(accent: egui::Color32) -> egui::Color32 {
    let [r, g, b, _] = accent.to_array();
    egui::Color32::from_rgb(
        (r as u32 * 3 / 20) as u8,
        (g as u32 * 3 / 20) as u8,
        (b as u32 * 3 / 20) as u8,
    )
}

// ---------------------------------------------------------------------------
// Dashed line / dotted rect
// ---------------------------------------------------------------------------

fn draw_dashed_line(
    painter: &egui::Painter,
    from: egui::Pos2,
    to: egui::Pos2,
    dash: f32,
    gap: f32,
    stroke: egui::Stroke,
) {
    let d = to - from;
    let len = d.length();
    if len < 0.001 {
        return;
    }
    let dir = d / len;
    let mut t = 0.0_f32;
    while t < len {
        let a = from + dir * t;
        let b = from + dir * (t + dash).min(len);
        painter.line_segment([a, b], stroke);
        t += dash + gap;
    }
}

fn draw_dotted_rect(painter: &egui::Painter, rect: egui::Rect, stroke: egui::Stroke) {
    let dash = 6.0;
    let gap = 4.0;
    draw_dashed_line(
        painter,
        rect.left_top(),
        rect.right_top(),
        dash,
        gap,
        stroke,
    );
    draw_dashed_line(
        painter,
        rect.right_top(),
        rect.right_bottom(),
        dash,
        gap,
        stroke,
    );
    draw_dashed_line(
        painter,
        rect.right_bottom(),
        rect.left_bottom(),
        dash,
        gap,
        stroke,
    );
    draw_dashed_line(
        painter,
        rect.left_bottom(),
        rect.left_top(),
        dash,
        gap,
        stroke,
    );
}

// ---------------------------------------------------------------------------
// Zoomed canvas rect helper
// ---------------------------------------------------------------------------

fn crect(w: &crate::project::schema::WidgetInstance, origin: egui::Pos2, zoom: f32) -> egui::Rect {
    crate::canvas::widget_instance::canvas_rect(w, origin, zoom)
}

/// Topmost widget id under `pos`. Children are tested before their parent Frame
/// so a child can be targeted even when the Frame also covers the cursor.
fn hit_widget_id(
    widgets: &[crate::project::schema::WidgetInstance],
    child_ids: &HashSet<Uuid>,
    pos: egui::Pos2,
    origin: egui::Pos2,
    zoom: f32,
) -> Option<Uuid> {
    widgets
        .iter()
        .rev()
        .find(|w| child_ids.contains(&w.id) && crect(w, origin, zoom).contains(pos))
        .or_else(|| {
            widgets
                .iter()
                .rev()
                .find(|w| !child_ids.contains(&w.id) && crect(w, origin, zoom).contains(pos))
        })
        .map(|w| w.id)
}

/// Returns the topmost VLayout/HLayout/GridLayout container whose canvas rect
/// contains `pos` (in screen space). Used for layout-aware rubber-band and
/// drag-reorder candidate restriction.
pub fn find_layout_container_at(
    pos: egui::Pos2,
    tree: &crate::project::ui_tree::UiTree,
    origin: egui::Pos2,
    zoom: f32,
) -> Option<Uuid> {
    tree.widgets
        .iter()
        .rev()
        .find(|w| {
            matches!(
                w.kind,
                WidgetKind::VLayout | WidgetKind::HLayout | WidgetKind::GridLayout
            ) && crect(w, origin, zoom).contains(pos)
        })
        .map(|w| w.id)
}

/// Given a dragged layout child and its current screen position, compute which
/// insertion index within the parent's `children` list best corresponds to the
/// cursor position. For VLayout, insertion is by Y; for HLayout, by X.
fn layout_insert_idx(
    parent: &crate::project::schema::WidgetInstance,
    widgets: &[crate::project::schema::WidgetInstance],
    pos_canvas: egui::Pos2,
    dragged_id: Uuid,
) -> usize {
    let mut insertion = parent.children.len(); // default: append at end
    match parent.kind {
        WidgetKind::VLayout => {
            for (i, cid) in parent.children.iter().enumerate() {
                if *cid == dragged_id {
                    continue;
                }
                if let Some(c) = widgets.iter().find(|w| w.id == *cid) {
                    let mid_y = c.rect.y + c.rect.h * 0.5;
                    if pos_canvas.y < mid_y {
                        insertion = i;
                        break;
                    }
                }
            }
        }
        WidgetKind::HLayout => {
            for (i, cid) in parent.children.iter().enumerate() {
                if *cid == dragged_id {
                    continue;
                }
                if let Some(c) = widgets.iter().find(|w| w.id == *cid) {
                    let mid_x = c.rect.x + c.rect.w * 0.5;
                    if pos_canvas.x < mid_x {
                        insertion = i;
                        break;
                    }
                }
            }
        }
        _ => {}
    }
    insertion
}

/// Draw the insertion-placeholder line for drag-reorder feedback.
fn draw_insertion_placeholder(
    painter: &egui::Painter,
    parent: &crate::project::schema::WidgetInstance,
    widgets: &[crate::project::schema::WidgetInstance],
    insert_idx: usize,
    accent: egui::Color32,
    origin: egui::Pos2,
    zoom: f32,
) {
    let stroke = egui::Stroke::new(2.5, accent);
    let parent_rect = crect(parent, origin, zoom);

    // Determine the Y (VLayout) or X (HLayout) position of the placeholder line
    match parent.kind {
        WidgetKind::VLayout => {
            let line_y = if parent.children.is_empty() || insert_idx == 0 {
                parent_rect.min.y + 4.0
            } else {
                // Find the bottom edge of the child just before the insert index
                let ref_idx = insert_idx.saturating_sub(1).min(parent.children.len() - 1);
                if let Some(cid) = parent.children.get(ref_idx) {
                    if let Some(c) = widgets.iter().find(|w| w.id == *cid) {
                        crect(c, origin, zoom).max.y + 2.0
                    } else {
                        parent_rect.max.y - 4.0
                    }
                } else {
                    parent_rect.max.y - 4.0
                }
            };
            painter.line_segment(
                [
                    egui::pos2(parent_rect.min.x + 4.0, line_y),
                    egui::pos2(parent_rect.max.x - 4.0, line_y),
                ],
                stroke,
            );
        }
        WidgetKind::HLayout => {
            let line_x = if parent.children.is_empty() || insert_idx == 0 {
                parent_rect.min.x + 4.0
            } else {
                let ref_idx = insert_idx.saturating_sub(1).min(parent.children.len() - 1);
                if let Some(cid) = parent.children.get(ref_idx) {
                    if let Some(c) = widgets.iter().find(|w| w.id == *cid) {
                        crect(c, origin, zoom).max.x + 2.0
                    } else {
                        parent_rect.max.x - 4.0
                    }
                } else {
                    parent_rect.max.x - 4.0
                }
            };
            painter.line_segment(
                [
                    egui::pos2(line_x, parent_rect.min.y + 4.0),
                    egui::pos2(line_x, parent_rect.max.y - 4.0),
                ],
                stroke,
            );
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Per-widget canvas drawing
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct WidgetDrawFlags {
    is_selected: bool,
    is_child: bool,
    has_selected_child: bool,
}

#[allow(clippy::too_many_arguments)]
fn draw_widget(
    painter: &egui::Painter,
    widget: &crate::project::schema::WidgetInstance,
    rect: egui::Rect,
    flags: WidgetDrawFlags,
    zoom: f32,
    zoom_stable: bool,
    text_settings: CanvasTextSettings,
    svg_texture_cache: &mut SvgTextureCache,
) {
    let accent = if let WidgetKind::Custom(_) = &widget.kind {
        widget
            .descriptor_accent
            .map(|[r, g, b]| egui::Color32::from_rgb(r, g, b))
            .unwrap_or_else(|| kind_accent(&widget.kind))
    } else {
        kind_accent(&widget.kind)
    };
    let stroke_color = if flags.is_selected {
        accent
    } else {
        egui::Color32::from_gray(85)
    };
    let stroke_width = if flags.is_selected { 2.0 } else { 1.0 };
    // font_size override; falls back to zoom-scaled default.
    let label_size = widget
        .font_size
        .map(|s| (s * zoom * text_settings.label_scale).clamp(8.0, 32.0))
        .unwrap_or_else(|| (12.0 * zoom * text_settings.label_scale).clamp(8.0, 24.0));
    let tag_size = (9.0 * zoom * text_settings.tag_scale).clamp(7.0, 18.0);
    // Per-widget corner radius (corner_radius field overrides per-kind defaults).
    let rounding = widget.corner_radius.unwrap_or(3.0);
    // fg_color overrides the default text/label color for this widget.
    let fg = widget
        .fg_color
        .map(|c| egui::Color32::from_rgb(c[0], c[1], c[2]));
    // bg_color overrides the widget background fill.
    let bg = widget
        .bg_color
        .map(|c| egui::Color32::from_rgb(c[0], c[1], c[2]));

    match &widget.kind {
        // Frame: dashed outline, label at top-left, optional fill
        WidgetKind::Frame => {
            let fill_alpha = if flags.is_child { 30u8 } else { 15u8 };
            let fill = match bg {
                Some(c) => egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), fill_alpha),
                None => egui::Color32::from_rgba_unmultiplied(200, 200, 200, fill_alpha),
            };
            let (frame_sw, frame_sc) = if flags.is_selected {
                (2.0, accent)
            } else if flags.has_selected_child {
                (1.5, accent.linear_multiply(0.45))
            } else {
                let sc = fg.unwrap_or_else(|| egui::Color32::from_gray(85));
                (stroke_width, sc)
            };
            painter.rect_filled(rect, rounding, fill);
            draw_dotted_rect(painter, rect, egui::Stroke::new(frame_sw, frame_sc));
            // Suppress auto-generated "svg path/rect/circle/…" labels from SVG component import.
            let is_svg_auto =
                widget.import_metadata.is_some() && widget.props.label.starts_with("svg ");
            if !is_svg_auto {
                painter.text(
                    rect.left_top() + egui::vec2(4.0, 2.0),
                    egui::Align2::LEFT_TOP,
                    &widget.props.label,
                    egui::FontId::proportional(label_size),
                    fg.unwrap_or_else(|| accent.linear_multiply(0.9)),
                );
            }
        }

        // ProgressBar: filled left portion (60 % preview)
        WidgetKind::ProgressBar => {
            painter.rect_filled(rect, rounding, bg.unwrap_or_else(|| kind_fill(accent)));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            let fill_w = rect.width() * 0.6;
            let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
            painter.rect_filled(fill_rect, rounding, accent.linear_multiply(0.55));
            // Overlay: % or animated indicator
            let overlay = if widget.props.show_percentage {
                "60%".to_owned()
            } else if widget.props.animated {
                "~".to_owned()
            } else {
                String::new()
            };
            if !overlay.is_empty() {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &overlay,
                    egui::FontId::proportional(label_size),
                    fg.unwrap_or(egui::Color32::WHITE),
                );
            }
        }

        // RadioButton: circle indicator + label (no background)
        WidgetKind::RadioButton => {
            // Faint border to show widget area; accent border when selected
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            let r = (rect.height() * 0.28).clamp(4.0, 9.0);
            let center = egui::pos2(rect.min.x + r + 4.0, rect.center().y);
            painter.circle_filled(center, r, egui::Color32::from_gray(30));
            painter.circle_stroke(center, r, egui::Stroke::new(1.5, accent));
            painter.text(
                egui::pos2(center.x + r + 5.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                fg.unwrap_or(egui::Color32::WHITE),
            );
            // radio_value tag (bottom-right corner, teal)
            if !widget.props.radio_value.is_empty() {
                painter.text(
                    rect.right_bottom() + egui::vec2(-3.0, -2.0),
                    egui::Align2::RIGHT_BOTTOM,
                    &widget.props.radio_value,
                    egui::FontId::proportional(tag_size),
                    egui::Color32::from_rgb(52, 211, 153),
                );
            }
        }

        // ComboBox: field box + ▾ arrow
        WidgetKind::ComboBox => {
            let selected_text = widget
                .props
                .options
                .first()
                .filter(|option| !option.trim().is_empty())
                .map(String::as_str)
                .unwrap_or(&widget.props.label);
            painter.rect_filled(rect, rounding, egui::Color32::from_gray(30));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            painter.text(
                egui::pos2(rect.min.x + 6.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                selected_text,
                egui::FontId::proportional(label_size),
                fg.unwrap_or(egui::Color32::from_gray(200)),
            );
            painter.text(
                egui::pos2(rect.max.x - 6.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                "▾",
                egui::FontId::proportional(label_size),
                accent,
            );
        }

        // Slider: track line + filled portion + thumb circle
        WidgetKind::Slider => {
            painter.rect_filled(rect, rounding, kind_fill(accent));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );

            let margin_x = rect.width() * 0.1;
            let track_left = rect.min.x + margin_x;
            let track_right = rect.max.x - margin_x;
            let track_y = rect.center().y + rect.height() * 0.15;
            let track_thickness = (2.0 * zoom).clamp(1.5, 3.0);

            painter.line_segment(
                [
                    egui::pos2(track_left, track_y),
                    egui::pos2(track_right, track_y),
                ],
                egui::Stroke::new(track_thickness, egui::Color32::from_gray(70)),
            );

            let span = widget.props.max - widget.props.min;
            let t = if span.abs() <= f32::EPSILON {
                0.0
            } else {
                ((widget.props.default_value - widget.props.min) / span).clamp(0.0, 1.0)
            };
            let thumb_x = track_left + (track_right - track_left) * t;
            painter.line_segment(
                [
                    egui::pos2(track_left, track_y),
                    egui::pos2(thumb_x, track_y),
                ],
                egui::Stroke::new(track_thickness, accent),
            );

            let thumb_r = (rect.height() * 0.22).clamp(4.0, 9.0);
            painter.circle_filled(egui::pos2(thumb_x, track_y), thumb_r, accent);
            painter.circle_stroke(
                egui::pos2(thumb_x, track_y),
                thumb_r,
                egui::Stroke::new(1.0, egui::Color32::WHITE.linear_multiply(0.25)),
            );

            painter.text(
                egui::pos2(rect.center().x, rect.min.y + 3.0 + label_size * 0.5),
                egui::Align2::CENTER_CENTER,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                fg.unwrap_or(egui::Color32::WHITE),
            );
        }

        // Button: rounded surface like egui button
        WidgetKind::Button => {
            painter.rect_filled(rect, rounding, bg.unwrap_or(egui::Color32::from_gray(58)));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                fg.unwrap_or(egui::Color32::WHITE),
            );
        }

        // Label: plain text, faint border to show bounds
        WidgetKind::Label => {
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(
                    if flags.is_selected { stroke_width } else { 0.5 },
                    if flags.is_selected {
                        stroke_color
                    } else {
                        egui::Color32::from_gray(55)
                    },
                ),
            );
            painter.text(
                rect.left_center() + egui::vec2(2.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                fg.unwrap_or(egui::Color32::WHITE),
            );
        }

        // TextInput: dark field with placeholder-style text
        WidgetKind::TextInput => {
            painter.rect_filled(rect, rounding, bg.unwrap_or(egui::Color32::from_gray(30)));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            // Show placeholder if set, otherwise fall back to label
            let hint = if widget.props.placeholder.is_empty() {
                widget.props.label.as_str()
            } else {
                widget.props.placeholder.as_str()
            };
            painter.text(
                rect.left_center() + egui::vec2(6.0, 0.0),
                egui::Align2::LEFT_CENTER,
                hint,
                egui::FontId::proportional(label_size),
                fg.unwrap_or(egui::Color32::from_gray(120)),
            );
        }

        // Checkbox: square indicator + label (no background box)
        WidgetKind::Checkbox => {
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(
                    if flags.is_selected { stroke_width } else { 0.5 },
                    if flags.is_selected {
                        stroke_color
                    } else {
                        egui::Color32::from_gray(55)
                    },
                ),
            );
            let box_size = (rect.height() * 0.52).clamp(12.0, 20.0);
            let box_rect = egui::Rect::from_center_size(
                egui::pos2(rect.min.x + box_size * 0.5 + 4.0, rect.center().y),
                egui::vec2(box_size, box_size),
            );
            painter.rect_filled(box_rect, 2.0, egui::Color32::from_gray(30));
            painter.rect_stroke(box_rect, 2.0, egui::Stroke::new(1.2, accent));
            painter.text(
                egui::pos2(box_rect.max.x + 7.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                fg.unwrap_or(egui::Color32::WHITE),
            );
        }

        // TextArea: dark multi-line field
        WidgetKind::TextArea => {
            painter.rect_filled(rect, rounding, bg.unwrap_or(egui::Color32::from_gray(30)));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            let hint = if widget.props.placeholder.is_empty() {
                widget.props.label.as_str()
            } else {
                widget.props.placeholder.as_str()
            };
            painter.text(
                rect.left_top() + egui::vec2(6.0, label_size * 0.7),
                egui::Align2::LEFT_TOP,
                hint,
                egui::FontId::proportional(label_size),
                fg.unwrap_or(egui::Color32::from_gray(120)),
            );
            // Dashed lines to suggest multi-line
            for i in 1..3_i32 {
                let y = rect.min.y + i as f32 * (rect.height() / 3.5);
                if y < rect.max.y - 4.0 {
                    painter.line_segment(
                        [
                            egui::pos2(rect.min.x + 6.0, y),
                            egui::pos2(rect.max.x - 6.0, y),
                        ],
                        egui::Stroke::new(0.5, egui::Color32::from_gray(55)),
                    );
                }
            }
        }

        // SpinBox: dark field with +/- indicators
        WidgetKind::SpinBox => {
            painter.rect_filled(rect, rounding, bg.unwrap_or(egui::Color32::from_gray(30)));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            let val_str = format!("{:.1}", widget.props.default_value);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &val_str,
                egui::FontId::proportional(label_size),
                fg.unwrap_or(egui::Color32::from_gray(200)),
            );
            let arrow_x = rect.max.x - 8.0;
            painter.text(
                egui::pos2(arrow_x, rect.center().y - 4.0),
                egui::Align2::RIGHT_CENTER,
                "▲",
                egui::FontId::proportional(tag_size),
                accent,
            );
            painter.text(
                egui::pos2(arrow_x, rect.center().y + 4.0),
                egui::Align2::RIGHT_CENTER,
                "▼",
                egui::FontId::proportional(tag_size),
                accent,
            );
        }

        // FontComboBox: field + "Aa" glyph indicator
        WidgetKind::FontComboBox => {
            painter.rect_filled(rect, rounding, egui::Color32::from_gray(30));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            painter.text(
                egui::pos2(rect.min.x + 6.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                fg.unwrap_or(egui::Color32::from_gray(200)),
            );
            painter.text(
                egui::pos2(rect.max.x - 18.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                "Aa",
                egui::FontId::proportional(label_size),
                accent,
            );
            painter.text(
                egui::pos2(rect.max.x - 6.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                "▾",
                egui::FontId::proportional(label_size),
                accent,
            );
        }

        // HorizontalSpacer: thin dashed horizontal bar
        WidgetKind::HorizontalSpacer => {
            let mid_y = rect.center().y;
            let dash_len = 6.0_f32;
            let gap = 4.0_f32;
            let mut x = rect.min.x;
            while x < rect.max.x {
                let x_end = (x + dash_len).min(rect.max.x);
                painter.line_segment(
                    [egui::pos2(x, mid_y), egui::pos2(x_end, mid_y)],
                    egui::Stroke::new(1.5, accent),
                );
                x += dash_len + gap;
            }
            if flags.is_selected {
                painter.rect_stroke(rect, 1.0, egui::Stroke::new(1.0, accent));
            }
        }

        // VerticalSpacer: thin dashed vertical bar
        WidgetKind::VerticalSpacer => {
            let mid_x = rect.center().x;
            let dash_len = 6.0_f32;
            let gap = 4.0_f32;
            let mut y = rect.min.y;
            while y < rect.max.y {
                let y_end = (y + dash_len).min(rect.max.y);
                painter.line_segment(
                    [egui::pos2(mid_x, y), egui::pos2(mid_x, y_end)],
                    egui::Stroke::new(1.5, accent),
                );
                y += dash_len + gap;
            }
            if flags.is_selected {
                painter.rect_stroke(rect, 1.0, egui::Stroke::new(1.0, accent));
            }
        }

        // GroupBox: group frame with heading label top-left
        WidgetKind::GroupBox => {
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(
                    if flags.is_selected { stroke_width } else { 1.0 },
                    stroke_color,
                ),
            );
            let label_bg_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + 8.0, rect.min.y - label_size * 0.6),
                egui::vec2(
                    widget.props.label.len() as f32 * label_size * 0.55 + 8.0,
                    label_size,
                ),
            );
            painter.rect_filled(label_bg_rect, 2.0, egui::Color32::from_gray(28));
            painter.text(
                egui::pos2(rect.min.x + 12.0, rect.min.y),
                egui::Align2::LEFT_CENTER,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                fg.unwrap_or(accent),
            );
        }

        // VLayout: container with ↕ arrow indicator
        WidgetKind::VLayout => {
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(
                    if flags.is_selected { stroke_width } else { 1.0 },
                    stroke_color,
                ),
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "↕",
                egui::FontId::proportional(label_size * 1.5),
                accent.linear_multiply(0.4),
            );
            painter.text(
                rect.left_top() + egui::vec2(4.0, 2.0),
                egui::Align2::LEFT_TOP,
                &widget.props.label,
                egui::FontId::proportional(tag_size),
                accent.linear_multiply(0.7),
            );
        }

        // HLayout: container with ↔ arrow indicator
        WidgetKind::HLayout => {
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(
                    if flags.is_selected { stroke_width } else { 1.0 },
                    stroke_color,
                ),
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "↔",
                egui::FontId::proportional(label_size * 1.5),
                accent.linear_multiply(0.4),
            );
            painter.text(
                rect.left_top() + egui::vec2(4.0, 2.0),
                egui::Align2::LEFT_TOP,
                &widget.props.label,
                egui::FontId::proportional(tag_size),
                accent.linear_multiply(0.7),
            );
        }

        // ScrollArea: container with scroll-bar indicator
        WidgetKind::ScrollArea => {
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(
                    if flags.is_selected { stroke_width } else { 1.0 },
                    stroke_color,
                ),
            );
            // Simulated scrollbar on right edge
            let sb_w = 6.0_f32;
            let sb_rect = egui::Rect::from_min_max(
                egui::pos2(rect.max.x - sb_w - 2.0, rect.min.y + 4.0),
                egui::pos2(rect.max.x - 2.0, rect.max.y - 4.0),
            );
            painter.rect_filled(sb_rect, 2.0, egui::Color32::from_gray(45));
            let thumb_h = (sb_rect.height() * 0.35).max(8.0);
            let thumb_rect = egui::Rect::from_min_size(sb_rect.min, egui::vec2(sb_w, thumb_h));
            painter.rect_filled(thumb_rect, 2.0, accent.linear_multiply(0.6));
            painter.text(
                rect.left_top() + egui::vec2(4.0, 2.0),
                egui::Align2::LEFT_TOP,
                &widget.props.label,
                egui::FontId::proportional(tag_size),
                accent.linear_multiply(0.7),
            );
        }

        // GridLayout: container with grid lines
        WidgetKind::GridLayout => {
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(
                    if flags.is_selected { stroke_width } else { 1.0 },
                    stroke_color,
                ),
            );
            let cols = widget.props.grid_columns.clamp(1, 12);
            let rows = widget.children.len().div_ceil(cols).max(1);
            for c in 1..cols {
                let x = rect.min.x + (rect.width() * c as f32) / cols as f32;
                painter.line_segment(
                    [
                        egui::pos2(x, rect.min.y + 2.0),
                        egui::pos2(x, rect.max.y - 2.0),
                    ],
                    egui::Stroke::new(0.5, accent.linear_multiply(0.3)),
                );
            }
            for r in 1..rows {
                let y = rect.min.y + (rect.height() * r as f32) / rows as f32;
                painter.line_segment(
                    [
                        egui::pos2(rect.min.x + 2.0, y),
                        egui::pos2(rect.max.x - 2.0, y),
                    ],
                    egui::Stroke::new(0.5, accent.linear_multiply(0.3)),
                );
            }
            painter.text(
                rect.left_top() + egui::vec2(4.0, 2.0),
                egui::Align2::LEFT_TOP,
                &widget.props.label,
                egui::FontId::proportional(tag_size),
                accent.linear_multiply(0.7),
            );
        }

        // TabWidget: container with tab header bar
        WidgetKind::TabWidget => {
            let tab_h = (label_size + 8.0).max(20.0);
            // Tab bar background
            let tab_bar_rect =
                egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + tab_h));
            painter.rect_filled(
                tab_bar_rect,
                egui::Rounding::same(rounding),
                kind_fill(accent),
            );
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(
                    if flags.is_selected { stroke_width } else { 1.0 },
                    stroke_color,
                ),
            );
            // Draw tab labels from options
            let tabs: Vec<&str> = widget.props.options.iter().map(String::as_str).collect();
            let tab_w = if tabs.is_empty() {
                60.0
            } else {
                (rect.width() / tabs.len() as f32).min(80.0)
            };
            for (i, tab_label) in tabs.iter().take(4).enumerate() {
                let tx = rect.min.x + i as f32 * tab_w;
                let is_first = i == 0;
                let tab_rect =
                    egui::Rect::from_min_size(egui::pos2(tx, rect.min.y), egui::vec2(tab_w, tab_h));
                if is_first {
                    painter.rect_filled(
                        tab_rect,
                        egui::Rounding::same(2.0),
                        egui::Color32::from_gray(45),
                    );
                }
                painter.text(
                    tab_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    tab_label,
                    egui::FontId::proportional(tag_size),
                    if is_first {
                        accent
                    } else {
                        egui::Color32::from_gray(150)
                    },
                );
            }
        }

        // ToolButton: compact square button
        WidgetKind::ToolButton => {
            painter.rect_filled(rect, rounding, bg.unwrap_or(egui::Color32::from_gray(58)));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                fg.unwrap_or(egui::Color32::WHITE),
            );
        }

        // CommandLinkButton: title + description on a raised surface
        WidgetKind::CommandLinkButton => {
            painter.rect_filled(
                rect,
                rounding.max(4.0),
                bg.unwrap_or(egui::Color32::from_gray(52)),
            );
            painter.rect_stroke(
                rect,
                rounding.max(4.0),
                egui::Stroke::new(stroke_width, accent),
            );
            painter.text(
                rect.left_top() + egui::vec2(10.0, 8.0),
                egui::Align2::LEFT_TOP,
                &widget.props.label,
                egui::FontId::proportional(label_size + 1.0),
                fg.unwrap_or(egui::Color32::WHITE),
            );
            if !widget.props.placeholder.is_empty() {
                painter.text(
                    rect.left_top() + egui::vec2(10.0, 8.0 + label_size + 4.0),
                    egui::Align2::LEFT_TOP,
                    &widget.props.placeholder,
                    egui::FontId::proportional(label_size - 1.0),
                    egui::Color32::from_gray(160),
                );
            }
        }

        // DialogButtonBox: right-aligned row of buttons from options
        WidgetKind::DialogButtonBox => {
            let n = widget.props.options.len().max(1);
            let btn_w = 64.0_f32.min((rect.width() - 8.0) / n as f32);
            let gap = 6.0;
            let mut x = rect.max.x - 4.0;
            for opt in widget.props.options.iter().rev() {
                let b_rect = egui::Rect::from_min_max(
                    egui::pos2(x - btn_w, rect.center().y - 12.0),
                    egui::pos2(x, rect.center().y + 12.0),
                );
                painter.rect_filled(b_rect, 3.0, egui::Color32::from_gray(58));
                painter.rect_stroke(b_rect, 3.0, egui::Stroke::new(1.0, accent));
                painter.text(
                    b_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    opt,
                    egui::FontId::proportional(label_size - 1.0),
                    egui::Color32::WHITE,
                );
                x -= btn_w + gap;
            }
        }

        // MathLabel: computed-value display with fx glyph
        WidgetKind::MathLabel => {
            painter.rect_filled(rect, rounding, bg.unwrap_or(egui::Color32::from_gray(28)));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            painter.text(
                rect.left_center() + egui::vec2(6.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("ƒ {} =", widget.props.label),
                egui::FontId::proportional(label_size),
                fg.unwrap_or(egui::Color32::from_gray(200)),
            );
            painter.text(
                rect.right_center() - egui::vec2(6.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                "0.0",
                egui::FontId::monospace(label_size),
                accent,
            );
        }

        // FilePicker: browse button + path field
        WidgetKind::FilePicker => {
            let btn_w = 64.0_f32.min(rect.width() * 0.4);
            let btn_rect =
                egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + btn_w, rect.max.y));
            painter.rect_filled(btn_rect, rounding, egui::Color32::from_gray(58));
            painter.rect_stroke(btn_rect, rounding, egui::Stroke::new(1.0, accent));
            painter.text(
                btn_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Browse…",
                egui::FontId::proportional(label_size - 1.0),
                egui::Color32::WHITE,
            );
            let field_rect =
                egui::Rect::from_min_max(egui::pos2(btn_rect.max.x + 4.0, rect.min.y), rect.max);
            painter.rect_filled(field_rect, rounding, egui::Color32::from_gray(30));
            painter.text(
                field_rect.left_center() + egui::vec2(4.0, 0.0),
                egui::Align2::LEFT_CENTER,
                "(no file selected)",
                egui::FontId::proportional(label_size - 1.0),
                egui::Color32::from_gray(120),
            );
        }

        // Chart: axes + sample bars
        WidgetKind::Chart => {
            painter.rect_filled(rect, rounding, bg.unwrap_or(egui::Color32::from_gray(24)));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            // axes
            let pad = 8.0;
            let origin = egui::pos2(rect.min.x + pad, rect.max.y - pad);
            painter.line_segment(
                [origin, egui::pos2(rect.max.x - pad, rect.max.y - pad)],
                egui::Stroke::new(1.0, egui::Color32::from_gray(90)),
            );
            painter.line_segment(
                [origin, egui::pos2(rect.min.x + pad, rect.min.y + pad)],
                egui::Stroke::new(1.0, egui::Color32::from_gray(90)),
            );
            // sample bars
            let bars = [0.4_f32, 0.7, 0.5, 0.9, 0.6];
            let area_w = rect.width() - pad * 2.0;
            let area_h = rect.height() - pad * 2.0;
            let bw = area_w / (bars.len() as f32 * 1.5);
            for (i, &v) in bars.iter().enumerate() {
                let bx = rect.min.x + pad + i as f32 * bw * 1.5 + bw * 0.25;
                let bh = area_h * v;
                let b_rect = egui::Rect::from_min_max(
                    egui::pos2(bx, rect.max.y - pad - bh),
                    egui::pos2(bx + bw, rect.max.y - pad),
                );
                painter.rect_filled(b_rect, 1.0, accent);
            }
            painter.text(
                rect.left_top() + egui::vec2(4.0, 2.0),
                egui::Align2::LEFT_TOP,
                &widget.props.label,
                egui::FontId::proportional(tag_size),
                accent.linear_multiply(0.8),
            );
        }

        // Table: header row + grid lines
        WidgetKind::Table => {
            painter.rect_filled(rect, rounding, bg.unwrap_or(egui::Color32::from_gray(26)));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            let cols = widget.props.options.len().max(1);
            let col_w = rect.width() / cols as f32;
            let header_h = (label_size + 6.0).min(rect.height());
            // header background
            let header_rect =
                egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + header_h));
            painter.rect_filled(header_rect, rounding, kind_fill(accent));
            for (i, col) in widget.props.options.iter().enumerate() {
                let cx = rect.min.x + i as f32 * col_w;
                painter.text(
                    egui::pos2(cx + 4.0, rect.min.y + header_h * 0.5),
                    egui::Align2::LEFT_CENTER,
                    col,
                    egui::FontId::proportional(tag_size),
                    accent,
                );
                if i > 0 {
                    painter.line_segment(
                        [egui::pos2(cx, rect.min.y), egui::pos2(cx, rect.max.y)],
                        egui::Stroke::new(0.5, egui::Color32::from_gray(55)),
                    );
                }
            }
            // row lines
            let mut y = rect.min.y + header_h;
            while y < rect.max.y {
                painter.line_segment(
                    [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                    egui::Stroke::new(0.5, egui::Color32::from_gray(45)),
                );
                y += header_h;
            }
        }

        // ListView: vertical list of items
        WidgetKind::ListView => {
            painter.rect_filled(rect, rounding, bg.unwrap_or(egui::Color32::from_gray(28)));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            let row_h = label_size + 6.0;
            let mut y = rect.min.y + 2.0;
            for (i, item) in widget.props.options.iter().enumerate() {
                if y + row_h > rect.max.y {
                    break;
                }
                if i == 0 {
                    let sel_rect = egui::Rect::from_min_max(
                        egui::pos2(rect.min.x + 1.0, y),
                        egui::pos2(rect.max.x - 1.0, y + row_h),
                    );
                    painter.rect_filled(sel_rect, 1.0, kind_fill(accent));
                }
                painter.text(
                    egui::pos2(rect.min.x + 6.0, y + row_h * 0.5),
                    egui::Align2::LEFT_CENTER,
                    item,
                    egui::FontId::proportional(label_size - 1.0),
                    if i == 0 {
                        accent
                    } else {
                        egui::Color32::from_gray(190)
                    },
                );
                y += row_h;
            }
        }

        // TreeView: indented nodes with ▸ markers
        WidgetKind::TreeView => {
            painter.rect_filled(rect, rounding, bg.unwrap_or(egui::Color32::from_gray(28)));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            let row_h = label_size + 6.0;
            let mut y = rect.min.y + 2.0;
            for (i, node) in widget.props.options.iter().enumerate() {
                if y + row_h > rect.max.y {
                    break;
                }
                let indent = if i == 0 { 0.0 } else { 14.0 };
                let marker = if i == 0 { "▾" } else { "•" };
                painter.text(
                    egui::pos2(rect.min.x + 6.0 + indent, y + row_h * 0.5),
                    egui::Align2::LEFT_CENTER,
                    format!("{marker} {node}"),
                    egui::FontId::proportional(label_size - 1.0),
                    egui::Color32::from_gray(190),
                );
                y += row_h;
            }
        }

        // StackedWidget: page container showing active page tab
        WidgetKind::StackedWidget => {
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(
                    if flags.is_selected { stroke_width } else { 1.0 },
                    stroke_color,
                ),
            );
            let label = widget
                .props
                .options
                .first()
                .map(String::as_str)
                .unwrap_or("Page 1");
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("▣ {label}"),
                egui::FontId::proportional(label_size),
                accent.linear_multiply(0.7),
            );
            painter.text(
                rect.left_top() + egui::vec2(4.0, 2.0),
                egui::Align2::LEFT_TOP,
                &widget.props.label,
                egui::FontId::proportional(tag_size),
                accent.linear_multiply(0.7),
            );
        }

        // ToolBox: vertical collapsing section headers
        WidgetKind::ToolBox => {
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(
                    if flags.is_selected { stroke_width } else { 1.0 },
                    stroke_color,
                ),
            );
            let sec_h = label_size + 8.0;
            let mut y = rect.min.y;
            for (i, sec) in widget.props.options.iter().enumerate() {
                if y + sec_h > rect.max.y {
                    break;
                }
                let hdr_rect = egui::Rect::from_min_max(
                    egui::pos2(rect.min.x, y),
                    egui::pos2(rect.max.x, y + sec_h),
                );
                painter.rect_filled(hdr_rect, 1.0, kind_fill(accent));
                let marker = if i == 0 { "▾" } else { "▸" };
                painter.text(
                    egui::pos2(rect.min.x + 6.0, y + sec_h * 0.5),
                    egui::Align2::LEFT_CENTER,
                    format!("{marker} {sec}"),
                    egui::FontId::proportional(label_size - 1.0),
                    accent,
                );
                y += sec_h;
                // leave gap for the first (expanded) section
                if i == 0 {
                    y += sec_h;
                }
            }
        }

        // Custom: accent box with descriptor name or label.
        WidgetKind::Custom(_) => {
            let custom_accent = widget
                .descriptor_accent
                .map(|[r, g, b]| egui::Color32::from_rgb(r, g, b))
                .unwrap_or(accent);
            let fill = kind_fill(custom_accent);
            painter.rect_filled(rect, rounding, bg.unwrap_or(fill));
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(stroke_width, stroke_color),
            );
            let display = widget
                .descriptor_name
                .as_deref()
                .unwrap_or(&widget.props.label);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                display,
                egui::FontId::proportional(label_size),
                fg.unwrap_or(egui::Color32::WHITE),
            );
            if flags.is_selected {
                painter.rect_stroke(rect, rounding, egui::Stroke::new(2.0, custom_accent));
            }
        }

        // Image: rasterize SVG source to a pixel texture, cache by widget ID + scale.
        WidgetKind::Image => {
            if let Some(ref svg_text) = widget.svg_source {
                let ctx = painter.ctx();
                let ppp = ctx.pixels_per_point();
                // `rect` is already in screen-space (widget logical size × zoom).
                // Multiply by ppp only to get physical pixels — do NOT multiply
                // by zoom again; that would produce zoom² scaling and rasterize
                // needlessly large textures at high zoom levels.
                let current_scale = zoom * ppp; // kept for 20 % drift eviction
                let tw = (rect.width() * ppp).round() as u32;
                let th = (rect.height() * ppp).round() as u32;

                // Re-rasterize when:
                //   • no cache entry (first display) — always immediate
                //   • widget was resized (tw/th changed) — always immediate
                //   • scale drifted > 20 % AND zoom is stable this frame
                //     (defers rasterization during active scroll gestures)
                let needs_raster = svg_texture_cache
                    .get(&widget.id)
                    .map(|(_, cached_scale, cached_tw, cached_th)| {
                        *cached_tw != tw
                            || *cached_th != th
                            || (zoom_stable
                                && (current_scale - cached_scale).abs() / current_scale.max(0.001)
                                    > 0.20)
                    })
                    .unwrap_or(true);

                if needs_raster {
                    let image = crate::canvas::svg_rasterizer::rasterize_or_fallback(
                        svg_text,
                        tw.max(1),
                        th.max(1),
                    );
                    let tex = ctx.load_texture(
                        format!("svg_{}", widget.id),
                        image,
                        egui::TextureOptions::LINEAR,
                    );
                    svg_texture_cache.insert(widget.id, (tex, current_scale, tw, th));
                }

                if let Some((tex, _, _, _)) = svg_texture_cache.get(&widget.id) {
                    painter.image(
                        tex.id(),
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    draw_svg_placeholder(painter, rect, rounding, label_size, "SVG");
                }
            } else {
                draw_svg_placeholder(painter, rect, rounding, label_size, "SVG");
            }
            if flags.is_selected {
                painter.rect_stroke(rect, rounding, egui::Stroke::new(2.0, accent));
            }
        }
    }

    // Disabled overlay
    if widget.enabled == Some(false) {
        painter.rect_filled(
            rect,
            rounding,
            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 100),
        );
    }

    // Subtle white overlay for child widgets (shows they belong to a group)
    let is_container = matches!(
        widget.kind,
        WidgetKind::Frame
            | WidgetKind::GroupBox
            | WidgetKind::VLayout
            | WidgetKind::HLayout
            | WidgetKind::ScrollArea
            | WidgetKind::GridLayout
            | WidgetKind::TabWidget
            | WidgetKind::Image
            | WidgetKind::Table
            | WidgetKind::ListView
            | WidgetKind::TreeView
            | WidgetKind::StackedWidget
            | WidgetKind::ToolBox
            | WidgetKind::Chart
    );
    if flags.is_child && !is_container {
        painter.rect_filled(
            rect,
            rounding,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 10),
        );
    }

    // Kind tag (bottom-right) — not on Frame-like containers, Label, spacers, or Image
    let is_container_or_spacer = matches!(
        widget.kind,
        WidgetKind::Frame
            | WidgetKind::GroupBox
            | WidgetKind::VLayout
            | WidgetKind::HLayout
            | WidgetKind::ScrollArea
            | WidgetKind::GridLayout
            | WidgetKind::TabWidget
            | WidgetKind::HorizontalSpacer
            | WidgetKind::VerticalSpacer
            | WidgetKind::Label
            | WidgetKind::Image
            | WidgetKind::Table
            | WidgetKind::ListView
            | WidgetKind::TreeView
            | WidgetKind::StackedWidget
            | WidgetKind::ToolBox
            | WidgetKind::Chart
            | WidgetKind::DialogButtonBox
    );
    if !is_container_or_spacer {
        painter.text(
            rect.right_bottom() - egui::vec2(3.0, 2.0),
            egui::Align2::RIGHT_BOTTOM,
            kind_tag(&widget.kind),
            egui::FontId::proportional(tag_size),
            accent.linear_multiply(0.75),
        );
    }
}

fn draw_svg_placeholder(
    painter: &egui::Painter,
    rect: egui::Rect,
    rounding: f32,
    label_size: f32,
    label: &str,
) {
    painter.rect_filled(
        rect,
        rounding,
        egui::Color32::from_rgba_unmultiplied(100, 100, 100, 80),
    );
    painter.rect_stroke(
        rect,
        rounding,
        egui::Stroke::new(1.0, egui::Color32::from_gray(90)),
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(label_size),
        egui::Color32::from_gray(200),
    );
}

// ---------------------------------------------------------------------------
// Main handler
// ---------------------------------------------------------------------------

fn canvas_owns_pointer(modal_blocked: bool, contains_pointer: bool, top_layer: bool) -> bool {
    !modal_blocked && contains_pointer && top_layer
}

fn canvas_owns_keyboard(
    modal_blocked: bool,
    canvas_focused: bool,
    wants_keyboard_input: bool,
) -> bool {
    !modal_blocked && canvas_focused && !wants_keyboard_input
}

pub fn handle(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    state: &mut InteractionState,
    selected: &mut Vec<Uuid>,
    settings: &mut CanvasSettings,
    text_settings: CanvasTextSettings,
    svg_texture_cache: &mut SvgTextureCache,
) {
    // Clear per-frame signals
    state.double_clicked_widget = None;
    settings.snap_step = settings.snap_step.max(MIN_SNAP_STEP);
    let modal_blocked = settings.input_blocked;

    let (resp, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());

    // -------------------------------------------------------------------
    // Input collection
    // -------------------------------------------------------------------
    let raw_pointer = ui.input(|i| i.pointer.interact_pos());
    let pointer_owned = raw_pointer.is_some_and(|pos| {
        canvas_owns_pointer(
            modal_blocked,
            resp.contains_pointer(),
            ui.ctx().layer_id_at(pos) == Some(resp.layer_id),
        )
    });
    let any_pointer_pressed = ui.input(|i| i.pointer.any_pressed());
    if any_pointer_pressed {
        state.canvas_focused = pointer_owned;
    }
    let keyboard_owned = canvas_owns_keyboard(
        modal_blocked,
        state.canvas_focused,
        ui.ctx().wants_keyboard_input(),
    );
    let pointer = pointer_owned.then_some(raw_pointer).flatten();
    let just_pressed = pointer_owned && ui.input(|i| i.pointer.primary_pressed());
    let primary_released = pointer_owned && ui.input(|i| i.pointer.primary_released());
    let is_down = pointer_owned && ui.input(|i| i.pointer.primary_down());
    let shift_held = ui.input(|i| i.modifiers.shift);
    let ctrl_held = ui.input(|i| i.modifiers.ctrl);
    let right_clicked = pointer_owned && ui.input(|i| i.pointer.secondary_clicked());
    let middle_down =
        pointer_owned && ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle));
    let mouse_delta = if pointer_owned {
        ui.input(|i| i.pointer.delta())
    } else {
        egui::Vec2::ZERO
    };
    let scroll_y = if pointer_owned {
        ui.input(|i| i.raw_scroll_delta.y)
    } else {
        0.0
    };
    let key_g = keyboard_owned && ui.input(|i| i.key_pressed(egui::Key::G));
    let key_0 = keyboard_owned && ui.input(|i| ctrl_held && i.key_pressed(egui::Key::Num0));
    let double_clicked = pointer_owned && resp.double_clicked();

    // -------------------------------------------------------------------
    // Shortcuts
    // -------------------------------------------------------------------
    if key_g {
        settings.snap_enabled = !settings.snap_enabled;
    }
    if key_0 {
        settings.zoom = 1.0;
        settings.pan = egui::Vec2::ZERO;
    }

    // -------------------------------------------------------------------
    // Pan / zoom
    // -------------------------------------------------------------------
    if middle_down {
        settings.pan += mouse_delta;
    }

    let canvas_w = tree.app_props.win_w;
    let canvas_h = tree.app_props.win_h;
    let canvas_size_unzoomed = egui::vec2(canvas_w, canvas_h);
    let hovered = pointer.map(|p| resp.rect.contains(p)).unwrap_or(false);

    if hovered && middle_down {
        let cursor = if mouse_delta.length_sq() > 0.0 {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::Grab
        };
        ui.ctx().set_cursor_icon(cursor);
    }

    if scroll_y != 0.0 && hovered && !middle_down {
        let zoom_old = settings.zoom;
        let factor = if scroll_y > 0.0 { 1.1_f32 } else { 1.0 / 1.1 };
        let zoom_new = (zoom_old * factor).clamp(0.25, 4.0);
        if let Some(p) = pointer {
            let origin_old =
                resp.rect.center() + settings.pan - canvas_size_unzoomed * zoom_old / 2.0;
            let cursor_canvas = (p - origin_old) / zoom_old;
            let origin_new = p - cursor_canvas * zoom_new;
            settings.pan = origin_new - resp.rect.center() + canvas_size_unzoomed * zoom_new / 2.0;
        }
        settings.zoom = zoom_new;
    }

    // True when no zoom-changing input arrived this frame.  draw_widget uses
    // this to skip expensive re-rasterization while the user is scrolling.
    let zoom_stable = scroll_y == 0.0 && !key_0;

    let zoom = settings.zoom;
    let canvas_size = canvas_size_unzoomed * zoom;
    let boundary = egui::Rect::from_center_size(resp.rect.center() + settings.pan, canvas_size);
    let origin = boundary.min;

    // -------------------------------------------------------------------
    // Background + boundary + grid
    // -------------------------------------------------------------------
    painter.rect_filled(resp.rect, 0.0, egui::Color32::from_gray(35));
    painter.rect_filled(boundary, 0.0, egui::Color32::from_gray(40));
    draw_dotted_rect(
        &painter,
        boundary,
        egui::Stroke::new(1.0, egui::Color32::from_gray(110)),
    );

    if settings.snap_enabled {
        let step = settings.snap_step.max(MIN_SNAP_STEP) * zoom;
        let gc = egui::Color32::from_gray(52);
        let mut x = boundary.min.x;
        while x <= boundary.max.x {
            painter.line_segment(
                [egui::pos2(x, boundary.min.y), egui::pos2(x, boundary.max.y)],
                egui::Stroke::new(0.5, gc),
            );
            x += step;
        }
        let mut y = boundary.min.y;
        while y <= boundary.max.y {
            painter.line_segment(
                [egui::pos2(boundary.min.x, y), egui::pos2(boundary.max.x, y)],
                egui::Stroke::new(0.5, gc),
            );
            y += step;
        }
    }

    painter.text(
        boundary.right_bottom() - egui::vec2(4.0, 3.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("{}×{}", canvas_w as u32, canvas_h as u32),
        egui::FontId::proportional((9.0 * text_settings.tag_scale).clamp(7.0, 18.0)),
        egui::Color32::from_gray(90),
    );

    let primary = selected.last().copied();

    // Build set of all child IDs — used by rendering, hit testing, and context menu
    let child_ids: HashSet<Uuid> = tree
        .widgets
        .iter()
        .flat_map(|w| w.children.iter().copied())
        .collect();

    let rubber_preview: HashSet<Uuid> =
        if let (Some(band_start), Some(pos), true) = (state.rubber_band, pointer, is_down) {
            let band_rect = egui::Rect::from_two_pos(band_start, pos);
            if band_rect.width() > 4.0 || band_rect.height() > 4.0 {
                tree.widgets
                    .iter()
                    .filter(|widget| band_rect.intersects(crect(widget, origin, zoom)))
                    .map(|widget| widget.id)
                    .collect()
            } else {
                HashSet::new()
            }
        } else {
            HashSet::new()
        };

    // -------------------------------------------------------------------
    // Draw widgets (parents first, then their children as sub-pass)
    // -------------------------------------------------------------------
    for widget in &tree.widgets {
        if child_ids.contains(&widget.id) {
            continue; // drawn inside parent Frame below
        }
        let rect = crect(widget, origin, zoom);
        let is_sel = selected.contains(&widget.id) || rubber_preview.contains(&widget.id);
        let has_sel_child = widget
            .children
            .iter()
            .any(|&cid| selected.contains(&cid) || rubber_preview.contains(&cid));
        draw_widget(
            &painter,
            widget,
            rect,
            WidgetDrawFlags {
                is_selected: is_sel,
                is_child: false,
                has_selected_child: has_sel_child,
            },
            zoom,
            zoom_stable,
            text_settings,
            svg_texture_cache,
        );

        // Draw children inside this Frame
        for &child_id in &widget.children {
            if let Some(child) = tree.widgets.iter().find(|w| w.id == child_id) {
                let child_rect = crect(child, origin, zoom);
                let child_sel = selected.contains(&child.id) || rubber_preview.contains(&child.id);
                draw_widget(
                    &painter,
                    child,
                    child_rect,
                    WidgetDrawFlags {
                        is_selected: child_sel,
                        is_child: true,
                        has_selected_child: false,
                    },
                    zoom,
                    zoom_stable,
                    text_settings,
                    svg_texture_cache,
                );
            }
        }
    }

    // -------------------------------------------------------------------
    // Inline label edit overlay
    // -------------------------------------------------------------------
    if pointer_owned {
        if let Some((edit_id, ref mut edit_buf)) = state.inline_edit {
            if let Some(widget) = tree.widgets.iter().find(|w| w.id == edit_id) {
                let rect = crect(widget, origin, zoom);
                let accent = kind_accent(&widget.kind);
                // Frosted overlay behind the text edit
                painter.rect_filled(
                    rect,
                    3.0,
                    egui::Color32::from_rgba_unmultiplied(20, 20, 20, 220),
                );
                painter.rect_stroke(rect, 3.0, egui::Stroke::new(1.5, accent));

                // Place a TextEdit widget at the widget rect
                let te_resp = ui.put(
                    rect,
                    egui::TextEdit::singleline(edit_buf)
                        .font(egui::FontId::proportional(
                            (12.0 * zoom * text_settings.label_scale).clamp(8.0, 24.0),
                        ))
                        .frame(false)
                        .text_color(egui::Color32::WHITE),
                );
                te_resp.request_focus();

                // Commit on Enter or focus loss; Escape cancels
                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
                let focus_lost = !te_resp.has_focus() && !te_resp.gained_focus();

                if enter_pressed || (focus_lost && !escape_pressed) {
                    // Commit: update label in tree
                    if let Some(w) = tree.get_mut(edit_id) {
                        if !edit_buf.is_empty() {
                            w.props.label = edit_buf.clone();
                        }
                    }
                    state.inline_edit = None;
                } else if escape_pressed {
                    // Cancel: discard
                    state.inline_edit = None;
                }
            } else {
                // Widget was removed while editing
                state.inline_edit = None;
            }
        }
    }

    // -------------------------------------------------------------------
    // Resize handles — primary widget only
    // -------------------------------------------------------------------
    if let Some(prim_id) = primary {
        if let Some(widget) = tree.widgets.iter().find(|w| w.id == prim_id) {
            let rect = crect(widget, origin, zoom);
            let accent = kind_accent(&widget.kind);
            for &h in &ResizeHandle::ALL {
                let hr = h.hit_rect(rect);
                painter.rect_filled(hr, 1.0, egui::Color32::from_gray(230));
                painter.rect_stroke(hr, 1.0, egui::Stroke::new(1.0, accent));
                if let Some(pos) = pointer {
                    if hr.contains(pos) {
                        ui.ctx().set_cursor_icon(h.cursor());
                    }
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Context menu (z-order)
    // -------------------------------------------------------------------
    if right_clicked {
        if let Some(pos) = pointer {
            if let Some(id) = hit_widget_id(&tree.widgets, &child_ids, pos, origin, zoom) {
                state.context_menu = Some((id, pos));
            }
        }
    }

    let ctx_group_available = selected.len() >= 2;
    let ctx_is_ungroupable = state
        .context_menu
        .and_then(|(ctx_id, _)| tree.widgets.iter().find(|w| w.id == ctx_id))
        .map(|w| w.kind == WidgetKind::Frame)
        .unwrap_or(false);

    let mut ctx_action: Option<u8> = None;
    let mut close_ctx = false;
    let ctx_id_for_action: Option<Uuid>;
    let mut do_group = false;
    let mut do_ungroup: Option<Uuid> = None;

    if let Some((ctx_id, ctx_pos)) = state.context_menu {
        ctx_id_for_action = Some(ctx_id);
        egui::Area::new(egui::Id::new("canvas_ctx_menu"))
            .fixed_pos(ctx_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(150.0);
                    if ctx_group_available && ui.button("Group (Ctrl+G)").clicked() {
                        do_group = true;
                        close_ctx = true;
                    }
                    if ctx_is_ungroupable && ui.button("Ungroup (Ctrl+Shift+G)").clicked() {
                        do_ungroup = Some(ctx_id);
                        close_ctx = true;
                    }
                    if ctx_group_available || ctx_is_ungroupable {
                        ui.separator();
                    }
                    if ui.button("Bring to Front").clicked() {
                        ctx_action = Some(0);
                        close_ctx = true;
                    }
                    if ui.button("Bring Forward").clicked() {
                        ctx_action = Some(1);
                        close_ctx = true;
                    }
                    if ui.button("Send Back").clicked() {
                        ctx_action = Some(2);
                        close_ctx = true;
                    }
                    if ui.button("Send to Back").clicked() {
                        ctx_action = Some(3);
                        close_ctx = true;
                    }
                });
            });
        close_ctx |= primary_released;
    } else {
        ctx_id_for_action = None;
    }

    if let (Some(id), Some(action)) = (ctx_id_for_action, ctx_action) {
        match action {
            0 => tree.bring_to_front(id),
            1 => tree.bring_forward(id),
            2 => tree.send_back(id),
            _ => tree.send_to_back(id),
        }
    }
    if do_group {
        if let Some(new_id) = tree.group(selected) {
            selected.clear();
            selected.push(new_id);
        }
    }
    if let Some(frame_id) = do_ungroup {
        let children = tree.ungroup(frame_id);
        selected.clear();
        selected.extend(children);
    }
    if close_ctx {
        state.context_menu = None;
    }

    // -------------------------------------------------------------------
    // Double-click → inline label edit OR Lazare highlight
    // -------------------------------------------------------------------
    if double_clicked {
        if let Some(pos) = pointer {
            if resp.rect.contains(pos) {
                let hit = tree
                    .widgets
                    .iter()
                    .rev()
                    .find(|w| crect(w, origin, zoom).contains(pos));
                if let Some(w) = hit {
                    if ctrl_held {
                        state.double_clicked_widget = Some(w.id);
                    } else {
                        // Widgets that support meaningful inline label editing
                        let supports_inline = matches!(
                            w.kind,
                            WidgetKind::Button
                                | WidgetKind::Label
                                | WidgetKind::Checkbox
                                | WidgetKind::RadioButton
                        );
                        if supports_inline {
                            state.inline_edit = Some((w.id, w.props.label.clone()));
                        }
                    }
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Template drag drop — release in canvas
    // -------------------------------------------------------------------
    if primary_released {
        if let Some(instances) = state.template_drag.take() {
            if let Some(pos) = pointer {
                if resp.rect.contains(pos) {
                    // Convert screen pos to canvas space, place first widget there
                    let canvas_pos = (pos - origin) / zoom;
                    let offset_x = canvas_pos.x.max(0.0);
                    let offset_y = canvas_pos.y.max(0.0);
                    // Find bounding box of the template to offset correctly
                    let min_x = instances.iter().map(|w| w.rect.x).fold(f32::MAX, f32::min);
                    let min_y = instances.iter().map(|w| w.rect.y).fold(f32::MAX, f32::min);
                    for mut w in instances {
                        w.id = Uuid::new_v4();
                        w.rect.x = (w.rect.x - min_x + offset_x).max(0.0);
                        w.rect.y = (w.rect.y - min_y + offset_y).max(0.0);
                        let id = w.id;
                        let center = (w.rect.x + w.rect.w * 0.5, w.rect.y + w.rect.h * 0.5);
                        tree.add(w);
                        if !matches!(
                            tree.widgets.iter().find(|w| w.id == id).map(|w| &w.kind),
                            Some(
                                WidgetKind::VLayout | WidgetKind::HLayout | WidgetKind::GridLayout
                            )
                        ) {
                            tree.attach_to_layout_at(id, center);
                        }
                    }
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Mouse-down — resize / select / rubber-band
    // Guide drags have exclusive priority: suppress canvas interaction entirely.
    // -------------------------------------------------------------------
    if settings.guide_drag_active {
        state.rubber_band = None;
        state.drag = None;
        state.reorder_drag = None;
    }
    if just_pressed && !settings.guide_drag_active && state.context_menu.is_none() {
        state.reorder_drag = None; // clear any stale reorder from previous gesture
        if let Some(pos) = pointer {
            if resp.rect.contains(pos) {
                let mut started_resize = false;

                // 1) Resize handle on primary
                if let Some(prim_id) = primary {
                    if let Some(widget) = tree.widgets.iter().find(|w| w.id == prim_id) {
                        let rect = crect(widget, origin, zoom);
                        for &h in &ResizeHandle::ALL {
                            if h.hit_rect(rect).contains(pos) {
                                state.resize = Some(ResizeState {
                                    id: prim_id,
                                    handle: h,
                                    start_rect: widget.rect.clone(),
                                    start_pos: pos,
                                });
                                state.drag = None;
                                state.rubber_band = None;
                                started_resize = true;
                                break;
                            }
                        }
                    }
                }

                if !started_resize {
                    let hit_widget = hit_widget_id(&tree.widgets, &child_ids, pos, origin, zoom);

                    if shift_held {
                        if let Some(id) = hit_widget {
                            if selected.contains(&id) {
                                selected.retain(|&x| x != id);
                            } else {
                                selected.push(id);
                            }
                        } else {
                            state.rubber_band = Some(pos);
                            state.drag = None;
                        }
                    } else {
                        match hit_widget {
                            Some(id) if selected.contains(&id) => {
                                let mut drag_ids: Vec<Uuid> = selected.clone();
                                for &sid in selected.iter() {
                                    if let Some(w) = tree.widgets.iter().find(|w| w.id == sid) {
                                        for &cid in &w.children {
                                            if !drag_ids.contains(&cid) {
                                                drag_ids.push(cid);
                                            }
                                        }
                                    }
                                }
                                let start_rects = drag_ids
                                    .iter()
                                    .filter_map(|&did| {
                                        tree.widgets
                                            .iter()
                                            .find(|w| w.id == did)
                                            .map(|w| (did, w.rect.clone()))
                                    })
                                    .collect();
                                state.drag = Some(DragState {
                                    start_pos: pos,
                                    start_rects,
                                });
                                state.rubber_band = None;
                            }
                            Some(id) => {
                                selected.clear();
                                selected.push(id);
                                let mut drag_ids = vec![id];
                                if let Some(w) = tree.widgets.iter().find(|w| w.id == id) {
                                    for &cid in &w.children {
                                        drag_ids.push(cid);
                                    }
                                }
                                let start_rects = drag_ids
                                    .iter()
                                    .filter_map(|&did| {
                                        tree.widgets
                                            .iter()
                                            .find(|w| w.id == did)
                                            .map(|w| (did, w.rect.clone()))
                                    })
                                    .collect();
                                state.drag = Some(DragState {
                                    start_pos: pos,
                                    start_rects,
                                });
                                state.rubber_band = None;
                            }
                            None => {
                                selected.clear();
                                state.drag = None;
                                state.resize = None;
                                state.rubber_band = Some(pos);
                            }
                        }
                    }
                }

                // After drag is established, check if the dragged widget is a
                // direct child of a VLayout or HLayout — if so, start reorder.
                if state.drag.is_some() && !shift_held {
                    let drag_id = selected.last().copied();
                    if let Some(did) = drag_id {
                        let parent_id = tree.parent_of(did);
                        if let Some(pid) = parent_id {
                            if let Some(parent) = tree.widgets.iter().find(|w| w.id == pid) {
                                if matches!(parent.kind, WidgetKind::VLayout | WidgetKind::HLayout)
                                {
                                    let v = (pos - origin) / zoom;
                                    let pos_canvas = egui::pos2(v.x, v.y);
                                    let insert_idx = layout_insert_idx(
                                        parent,
                                        &tree.widgets,
                                        pos_canvas,
                                        did,
                                    );
                                    state.reorder_drag = Some(ReorderDrag {
                                        child_id: did,
                                        parent_id: pid,
                                        insert_idx,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Single strongest guide per axis: (screen_pos, span_start, span_end)
    // Written by both resize and drag sections; drawn once at the bottom.
    let mut guide_v: Option<(f32, f32, f32)> = None; // vertical line: screen_x, y0, y1
    let mut guide_h: Option<(f32, f32, f32)> = None; // horizontal line: screen_y, x0, x1

    // -------------------------------------------------------------------
    // Resize drag — with smart guide snap on moving edge(s)
    // -------------------------------------------------------------------
    if is_down {
        if let (Some(rs), Some(pos)) = (&state.resize, pointer) {
            let screen_delta = pos - rs.start_pos;
            let canvas_delta = screen_delta / zoom;
            let mut new_rect = rs.handle.apply_delta(&rs.start_rect, canvas_delta);
            if settings.snap_enabled {
                new_rect = snap_rect(new_rect, settings.snap_step);
            }

            let snap_thr = 4.0_f32 / zoom.max(0.01);
            let resize_id = rs.id;
            let handle = rs.handle;

            // Which canvas coordinate is the "moving edge" for each axis?
            let moving_x: Option<f32> = match handle {
                ResizeHandle::Left | ResizeHandle::TopLeft | ResizeHandle::BottomLeft => {
                    Some(new_rect.x)
                }
                ResizeHandle::Right | ResizeHandle::TopRight | ResizeHandle::BottomRight => {
                    Some(new_rect.x + new_rect.w)
                }
                _ => None,
            };
            let moving_y: Option<f32> = match handle {
                ResizeHandle::Top | ResizeHandle::TopLeft | ResizeHandle::TopRight => {
                    Some(new_rect.y)
                }
                ResizeHandle::Bottom | ResizeHandle::BottomLeft | ResizeHandle::BottomRight => {
                    Some(new_rect.y + new_rect.h)
                }
                _ => None,
            };

            // Find best snap adjustment per axis (scoped so tree borrow ends before get_mut)
            let (adj_x, adj_y, raw_rv, raw_rh) = {
                let mut best_x = snap_thr;
                let mut best_y = snap_thr;
                let mut ax = 0.0_f32;
                let mut ay = 0.0_f32;
                let mut rv: Option<(f32, f32, f32)> = None; // (canvas_x, y_min, y_max)
                let mut rh: Option<(f32, f32, f32)> = None; // (canvas_y, x_min, x_max)

                for sw in tree.widgets.iter().filter(|w| w.id != resize_id) {
                    let sxs = [
                        sw.rect.x,
                        sw.rect.x + sw.rect.w * 0.5,
                        sw.rect.x + sw.rect.w,
                    ];
                    let sys = [
                        sw.rect.y,
                        sw.rect.y + sw.rect.h * 0.5,
                        sw.rect.y + sw.rect.h,
                    ];
                    if let Some(mx) = moving_x {
                        for &sx in &sxs {
                            let d = (mx - sx).abs();
                            if d < best_x {
                                best_x = d;
                                ax = sx - mx;
                                rv = Some((
                                    sx,
                                    new_rect.y.min(sw.rect.y),
                                    (new_rect.y + new_rect.h).max(sw.rect.y + sw.rect.h),
                                ));
                            }
                        }
                    }
                    if let Some(my) = moving_y {
                        for &sy in &sys {
                            let d = (my - sy).abs();
                            if d < best_y {
                                best_y = d;
                                ay = sy - my;
                                rh = Some((
                                    sy,
                                    new_rect.x.min(sw.rect.x),
                                    (new_rect.x + new_rect.w).max(sw.rect.x + sw.rect.w),
                                ));
                            }
                        }
                    }
                }
                (ax, ay, rv, rh)
            };

            // Apply snap adjustment to the moving edge
            if adj_x != 0.0 {
                match handle {
                    ResizeHandle::Left | ResizeHandle::TopLeft | ResizeHandle::BottomLeft => {
                        new_rect.x = (new_rect.x + adj_x).max(0.0);
                        new_rect.w = (new_rect.w - adj_x).max(MIN_SIZE);
                    }
                    ResizeHandle::Right | ResizeHandle::TopRight | ResizeHandle::BottomRight => {
                        new_rect.w = (new_rect.w + adj_x).max(MIN_SIZE);
                    }
                    _ => {}
                }
            }
            if adj_y != 0.0 {
                match handle {
                    ResizeHandle::Top | ResizeHandle::TopLeft | ResizeHandle::TopRight => {
                        new_rect.y = (new_rect.y + adj_y).max(0.0);
                        new_rect.h = (new_rect.h - adj_y).max(MIN_SIZE);
                    }
                    ResizeHandle::Bottom | ResizeHandle::BottomLeft | ResizeHandle::BottomRight => {
                        new_rect.h = (new_rect.h + adj_y).max(MIN_SIZE);
                    }
                    _ => {}
                }
            }

            // Convert guide spans to screen space (40px overhang)
            guide_v = raw_rv.map(|(cx, y_min, y_max)| {
                (
                    origin.x + cx * zoom,
                    origin.y + y_min * zoom - 40.0,
                    origin.y + y_max * zoom + 40.0,
                )
            });
            guide_h = raw_rh.map(|(cy, x_min, x_max)| {
                (
                    origin.y + cy * zoom,
                    origin.x + x_min * zoom - 40.0,
                    origin.x + x_max * zoom + 40.0,
                )
            });

            let cursor = handle.cursor();
            if let Some(w) = tree.get_mut(resize_id) {
                w.rect = new_rect;
            }
            tree.reflow_layouts();
            ui.ctx().set_cursor_icon(cursor);
        }
    }

    // -------------------------------------------------------------------
    // Multi-drag with smart guide snap
    // -------------------------------------------------------------------

    if is_down {
        if let (Some(ds), Some(pos)) = (&state.drag, pointer) {
            let screen_delta = pos - ds.start_pos;
            let canvas_delta = screen_delta / zoom;
            let drag_id_set: HashSet<Uuid> = ds.start_rects.iter().map(|(id, _)| *id).collect();
            let snap_thr = 4.0_f32 / zoom.max(0.01);

            let drag_updates: Vec<(Uuid, f32, f32)> = {
                let static_ws: Vec<_> = tree
                    .widgets
                    .iter()
                    .filter(|w| !drag_id_set.contains(&w.id))
                    .collect();

                let mut pos_list: Vec<(Uuid, f32, f32, f32, f32)> = ds
                    .start_rects
                    .iter()
                    .filter_map(|(id, sr)| {
                        let w = tree.widgets.iter().find(|w| w.id == *id)?;
                        let (nx, ny) = if settings.snap_enabled {
                            (
                                snap((sr.x + canvas_delta.x).max(0.0), settings.snap_step),
                                snap((sr.y + canvas_delta.y).max(0.0), settings.snap_step),
                            )
                        } else {
                            (
                                (sr.x + canvas_delta.x).max(0.0),
                                (sr.y + canvas_delta.y).max(0.0),
                            )
                        };
                        Some((*id, nx, ny, w.rect.w, w.rect.h))
                    })
                    .collect();

                // Find strongest alignment per axis; record guide span alongside
                let mut best_x = snap_thr;
                let mut best_y = snap_thr;
                let mut adj_x = 0.0_f32;
                let mut adj_y = 0.0_f32;
                let mut raw_guide_v: Option<(f32, f32, f32)> = None; // (canvas_x, y_min, y_max)
                let mut raw_guide_h: Option<(f32, f32, f32)> = None; // (canvas_y, x_min, x_max)

                for (_, nx, ny, nw, nh) in &pos_list {
                    let dxs = [*nx, nx + nw * 0.5, nx + nw];
                    let dys = [*ny, ny + nh * 0.5, ny + nh];
                    for sw in &static_ws {
                        let sxs = [
                            sw.rect.x,
                            sw.rect.x + sw.rect.w * 0.5,
                            sw.rect.x + sw.rect.w,
                        ];
                        let sys = [
                            sw.rect.y,
                            sw.rect.y + sw.rect.h * 0.5,
                            sw.rect.y + sw.rect.h,
                        ];
                        for &dx in &dxs {
                            for &sx in &sxs {
                                let d = (dx - sx).abs();
                                if d < best_x {
                                    best_x = d;
                                    adj_x = sx - dx;
                                    let y_min = ny.min(sw.rect.y);
                                    let y_max = (ny + nh).max(sw.rect.y + sw.rect.h);
                                    raw_guide_v = Some((sx, y_min, y_max));
                                }
                            }
                        }
                        for &dy in &dys {
                            for &sy in &sys {
                                let d = (dy - sy).abs();
                                if d < best_y {
                                    best_y = d;
                                    adj_y = sy - dy;
                                    let x_min = nx.min(sw.rect.x);
                                    let x_max = (nx + nw).max(sw.rect.x + sw.rect.w);
                                    raw_guide_h = Some((sy, x_min, x_max));
                                }
                            }
                        }
                    }
                }

                // Guide snapping — snap widget edges/center to ruler guide lines
                for (_, nx, ny, nw, nh) in &pos_list {
                    let dxs = [*nx, nx + nw * 0.5, nx + nw];
                    let dys = [*ny, ny + nh * 0.5, ny + nh];
                    for g in &tree.app_props.guides {
                        match g.orientation {
                            crate::project::schema::GuideOrientation::Vertical => {
                                for &dx in &dxs {
                                    let d = (dx - g.position).abs();
                                    if d < best_x {
                                        best_x = d;
                                        adj_x = g.position - dx;
                                        raw_guide_v = Some((g.position, *ny, ny + nh));
                                    }
                                }
                            }
                            crate::project::schema::GuideOrientation::Horizontal => {
                                for &dy in &dys {
                                    let d = (dy - g.position).abs();
                                    if d < best_y {
                                        best_y = d;
                                        adj_y = g.position - dy;
                                        raw_guide_h = Some((g.position, *nx, nx + nw));
                                    }
                                }
                            }
                        }
                    }
                }

                for (_, nx, ny, _, _) in &mut pos_list {
                    *nx = (*nx + adj_x).max(0.0);
                    *ny = (*ny + adj_y).max(0.0);
                }

                // Convert canvas-space guide spans to screen space (with 40px overhang)
                guide_v = raw_guide_v.map(|(cx, y_min, y_max)| {
                    (
                        origin.x + cx * zoom,
                        origin.y + y_min * zoom - 40.0,
                        origin.y + y_max * zoom + 40.0,
                    )
                });
                guide_h = raw_guide_h.map(|(cy, x_min, x_max)| {
                    (
                        origin.y + cy * zoom,
                        origin.x + x_min * zoom - 40.0,
                        origin.x + x_max * zoom + 40.0,
                    )
                });

                // Equidistant spacing marks
                let mut all_pts: Vec<(f32, f32)> = pos_list
                    .iter()
                    .map(|(_, nx, ny, nw, nh)| (nx + nw * 0.5, ny + nh * 0.5))
                    .collect();
                for sw in &static_ws {
                    all_pts.push((sw.rect.x + sw.rect.w * 0.5, sw.rect.y + sw.rect.h * 0.5));
                }
                let mark_color = egui::Color32::from_rgba_unmultiplied(255, 80, 80, 180);
                let tc = 5.0_f32;

                let mut pts_x = all_pts.clone();
                pts_x.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                for i in 0..pts_x.len().saturating_sub(2) {
                    let g1 = pts_x[i + 1].0 - pts_x[i].0;
                    let g2 = pts_x[i + 2].0 - pts_x[i + 1].0;
                    if g1 > 2.0 && (g1 - g2).abs() < snap_thr {
                        let avg_sy = (pts_x[i].1 + pts_x[i + 1].1 + pts_x[i + 2].1) / 3.0;
                        let sy = origin.y + avg_sy * zoom;
                        let sx0 = origin.x + pts_x[i].0 * zoom;
                        let sx1 = origin.x + pts_x[i + 1].0 * zoom;
                        let sx2 = origin.x + pts_x[i + 2].0 * zoom;
                        for sx in [sx0, sx1, sx2] {
                            painter.line_segment(
                                [egui::pos2(sx, sy - tc), egui::pos2(sx, sy + tc)],
                                egui::Stroke::new(1.0, mark_color),
                            );
                        }
                        painter.line_segment(
                            [egui::pos2(sx0, sy), egui::pos2(sx2, sy)],
                            egui::Stroke::new(1.0, mark_color),
                        );
                    }
                }

                let mut pts_y = all_pts;
                pts_y.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                for i in 0..pts_y.len().saturating_sub(2) {
                    let g1 = pts_y[i + 1].1 - pts_y[i].1;
                    let g2 = pts_y[i + 2].1 - pts_y[i + 1].1;
                    if g1 > 2.0 && (g1 - g2).abs() < snap_thr {
                        let avg_sx = (pts_y[i].0 + pts_y[i + 1].0 + pts_y[i + 2].0) / 3.0;
                        let sx = origin.x + avg_sx * zoom;
                        let sy0 = origin.y + pts_y[i].1 * zoom;
                        let sy1 = origin.y + pts_y[i + 1].1 * zoom;
                        let sy2 = origin.y + pts_y[i + 2].1 * zoom;
                        for sy in [sy0, sy1, sy2] {
                            painter.line_segment(
                                [egui::pos2(sx - tc, sy), egui::pos2(sx + tc, sy)],
                                egui::Stroke::new(1.0, mark_color),
                            );
                        }
                        painter.line_segment(
                            [egui::pos2(sx, sy0), egui::pos2(sx, sy2)],
                            egui::Stroke::new(1.0, mark_color),
                        );
                    }
                }

                pos_list
                    .iter()
                    .map(|(id, nx, ny, _, _)| (*id, *nx, *ny))
                    .collect()
            };

            for (id, nx, ny) in drag_updates {
                if let Some(w) = tree.get_mut(id) {
                    w.rect.x = nx;
                    w.rect.y = ny;
                }
            }
        }
    }

    // Draw guide lines — dashed, rgba(255,80,80,180), only strongest per axis
    let guide_color = egui::Color32::from_rgba_unmultiplied(255, 80, 80, 180);
    let guide_stroke = egui::Stroke::new(1.0, guide_color);
    if let Some((sx, y0, y1)) = guide_v {
        draw_dashed_line(
            &painter,
            egui::pos2(sx, y0),
            egui::pos2(sx, y1),
            4.0,
            3.0,
            guide_stroke,
        );
    }
    if let Some((sy, x0, x1)) = guide_h {
        draw_dashed_line(
            &painter,
            egui::pos2(x0, sy),
            egui::pos2(x1, sy),
            4.0,
            3.0,
            guide_stroke,
        );
    }

    // -------------------------------------------------------------------
    // Reorder-drag: update insertion index + draw placeholder
    // -------------------------------------------------------------------
    if is_down {
        if let Some(ref mut rd) = state.reorder_drag {
            if let Some(pos) = pointer {
                let v = (pos - origin) / zoom;
                let pos_canvas = egui::pos2(v.x, v.y);
                if let Some(parent) =
                    tree.widgets.iter().find(|w| w.id == rd.parent_id).cloned()
                {
                    rd.insert_idx =
                        layout_insert_idx(&parent, &tree.widgets, pos_canvas, rd.child_id);
                    let accent = egui::Color32::from_rgb(52, 211, 153);
                    draw_insertion_placeholder(
                        &painter,
                        &parent,
                        &tree.widgets,
                        rd.insert_idx,
                        accent,
                        origin,
                        zoom,
                    );
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Release — rubber-band finalise
    // -------------------------------------------------------------------
    if !is_down {
        // Commit reorder-drag first (before attach_to_layout clears the parent rel)
        if let Some(rd) = state.reorder_drag.take() {
            tree.move_child_within_parent(rd.parent_id, rd.child_id, rd.insert_idx);
        }
        if let Some(ds) = &state.drag {
            let dragged_ids: Vec<Uuid> = ds.start_rects.iter().map(|(id, _)| *id).collect();
            for id in dragged_ids {
                let Some(widget) = tree.widgets.iter().find(|w| w.id == id) else {
                    continue;
                };
                if matches!(
                    widget.kind,
                    WidgetKind::VLayout | WidgetKind::HLayout | WidgetKind::GridLayout
                ) {
                    continue;
                }
                let center = (
                    widget.rect.x + widget.rect.w * 0.5,
                    widget.rect.y + widget.rect.h * 0.5,
                );
                tree.attach_to_layout_at(id, center);
            }
        }
        if let (Some(band_start), Some(pos)) = (state.rubber_band, pointer) {
            let band_rect = egui::Rect::from_two_pos(band_start, pos);
            if band_rect.width() > 4.0 || band_rect.height() > 4.0 {
                let before_len = selected.len();
                // Layout-aware rubber-band: if the band started inside a layout
                // container, restrict candidates to direct children of that container.
                let container_children: Option<HashSet<Uuid>> =
                    find_layout_container_at(band_start, tree, origin, zoom).and_then(|pid| {
                        tree.widgets
                            .iter()
                            .find(|w| w.id == pid)
                            .map(|p| p.children.iter().copied().collect())
                    });
                for widget in &tree.widgets {
                    let in_scope = container_children
                        .as_ref()
                        .map(|allowed| allowed.contains(&widget.id))
                        .unwrap_or(true);
                    if in_scope
                        && band_rect.intersects(crect(widget, origin, zoom))
                        && !selected.contains(&widget.id)
                    {
                        selected.push(widget.id);
                    }
                }
                if selected.len() != before_len {
                    ui.ctx().request_repaint();
                }
            }
        }
        state.drag = None;
        state.resize = None;
        state.rubber_band = None;
        // reorder_drag already consumed above via take(); set None in case of early exit
        state.reorder_drag = None;
    }

    // -------------------------------------------------------------------
    // Rubber-band rect
    // -------------------------------------------------------------------
    if let (Some(band_start), Some(pos), true) = (state.rubber_band, pointer, is_down) {
        let band_rect = egui::Rect::from_two_pos(band_start, pos);
        painter.rect_filled(
            band_rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(100, 180, 255, 30),
        );
        painter.rect_stroke(
            band_rect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 180, 255)),
        );
    }

    // -------------------------------------------------------------------
    // Keyboard nudge
    // -------------------------------------------------------------------
    if keyboard_owned && !selected.is_empty() {
        let nudge = if settings.snap_enabled {
            settings.snap_step.max(MIN_SNAP_STEP)
        } else {
            1.0
        };
        let left = ui.input(|i| i.key_pressed(egui::Key::ArrowLeft));
        let right = ui.input(|i| i.key_pressed(egui::Key::ArrowRight));
        let up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
        let down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
        if left || right || up || down {
            let ids: Vec<Uuid> = selected.clone();
            for id in ids {
                if let Some(w) = tree.get_mut(id) {
                    if left {
                        w.rect.x = (w.rect.x - nudge).max(0.0);
                    }
                    if right {
                        w.rect.x += nudge;
                    }
                    if up {
                        w.rect.y = (w.rect.y - nudge).max(0.0);
                    }
                    if down {
                        w.rect.y += nudge;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod resize_snap_tests {
    use super::*;

    fn schema_rect(x: f32, y: f32, w: f32, h: f32) -> SchemaRect {
        SchemaRect { x, y, w, h }
    }

    #[test]
    fn snap_value_to_grid() {
        // snap(val, step) rounds val to nearest multiple of step
        assert_eq!(snap(7.0, 10.0), 10.0);
        assert_eq!(snap(4.9, 10.0), 0.0);
        assert_eq!(snap(0.0, 10.0), 0.0);
        assert_eq!(snap(100.0, 10.0), 100.0);
        // Step below MIN_SNAP_STEP (1.0) is clamped to 1.0
        assert_eq!(snap(3.6, 0.01), 4.0);
    }

    #[test]
    fn resize_handle_hit_detection() {
        let rect = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 50.0));
        let handle = ResizeHandle::BottomRight;
        let hr = handle.hit_rect(rect);
        // The hit rect center should be at the expanded bottom-right corner
        let outer = rect.expand(4.0);
        assert!((hr.center() - outer.right_bottom()).length() < 0.5);
    }

    #[test]
    fn resize_handle_apply_delta_top_left() {
        let start = schema_rect(50.0, 50.0, 100.0, 80.0);
        let delta = egui::vec2(10.0, 5.0); // moving TL corner inward
        let result = ResizeHandle::TopLeft.apply_delta(&start, delta);
        assert_eq!(result.x, 60.0);
        assert_eq!(result.y, 55.0);
        assert_eq!(result.w, 90.0);
        assert_eq!(result.h, 75.0);
    }

    #[test]
    fn resize_handle_respects_min_size() {
        let start = schema_rect(50.0, 50.0, 22.0, 22.0);
        // Drag BottomRight far to the left / up — should clamp to MIN_SIZE
        let delta = egui::vec2(-200.0, -200.0);
        let result = ResizeHandle::BottomRight.apply_delta(&start, delta);
        assert!(result.w >= MIN_SIZE, "width must not fall below MIN_SIZE");
        assert!(result.h >= MIN_SIZE, "height must not fall below MIN_SIZE");
    }
}

#[cfg(test)]
mod input_ownership_tests {
    use super::{canvas_owns_keyboard, canvas_owns_pointer};

    #[test]
    fn floating_window_layer_blocks_canvas_pointer() {
        assert!(canvas_owns_pointer(false, true, true));
        assert!(!canvas_owns_pointer(false, true, false));
        assert!(!canvas_owns_pointer(false, false, true));
        assert!(!canvas_owns_pointer(true, true, true));
    }

    #[test]
    fn text_focus_and_modals_block_canvas_keyboard() {
        assert!(canvas_owns_keyboard(false, true, false));
        assert!(!canvas_owns_keyboard(false, true, true));
        assert!(!canvas_owns_keyboard(false, false, false));
        assert!(!canvas_owns_keyboard(true, true, false));
    }
}
