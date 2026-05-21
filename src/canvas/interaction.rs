use crate::project::schema::{Rect as SchemaRect, WidgetInstance, WidgetKind};
use crate::project::ui_tree::UiTree;
use uuid::Uuid;

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
}

impl Default for CanvasSettings {
    fn default() -> Self {
        Self {
            snap_enabled: false,
            snap_step: 8.0,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
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
        egui::Rect::from_center_size(
            self.anchor(rect),
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
}

// ---------------------------------------------------------------------------
// Widget visual helpers
// ---------------------------------------------------------------------------

pub fn kind_accent(kind: &WidgetKind) -> egui::Color32 {
    match kind {
        WidgetKind::Button => egui::Color32::from_rgb(52, 211, 153),
        WidgetKind::Label => egui::Color32::from_rgb(156, 163, 175),
        WidgetKind::TextInput => egui::Color32::from_rgb(96, 165, 250),
        WidgetKind::Slider => egui::Color32::from_rgb(251, 146, 60),
        WidgetKind::Checkbox => egui::Color32::from_rgb(167, 139, 250),
        WidgetKind::Frame => egui::Color32::from_rgb(200, 200, 200),
        WidgetKind::ComboBox => egui::Color32::from_rgb(251, 191, 36),
        WidgetKind::RadioButton => egui::Color32::from_rgb(251, 113, 133),
        WidgetKind::ProgressBar => egui::Color32::from_rgb(34, 211, 238),
    }
}

fn kind_tag(kind: &WidgetKind) -> &'static str {
    match kind {
        WidgetKind::Button => "btn",
        WidgetKind::Label => "lbl",
        WidgetKind::TextInput => "txt",
        WidgetKind::Slider => "sldr",
        WidgetKind::Checkbox => "chk",
        WidgetKind::Frame => "frm",
        WidgetKind::ComboBox => "cmb",
        WidgetKind::RadioButton => "rad",
        WidgetKind::ProgressBar => "prg",
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

// ---------------------------------------------------------------------------
// Per-widget canvas drawing
// ---------------------------------------------------------------------------

fn draw_widget(
    painter: &egui::Painter,
    widget: &crate::project::schema::WidgetInstance,
    rect: egui::Rect,
    is_selected: bool,
    zoom: f32,
) {
    let accent = kind_accent(&widget.kind);
    let stroke_color = if is_selected {
        accent
    } else {
        egui::Color32::from_gray(85)
    };
    let stroke_width = if is_selected { 2.0 } else { 1.0 };
    let label_size = (12.0 * zoom).clamp(8.0, 16.0);
    let tag_size = (9.0 * zoom).clamp(7.0, 12.0);

    match &widget.kind {
        // Frame: dashed outline, label at top-left, no fill
        WidgetKind::Frame => {
            let fill = egui::Color32::from_rgba_unmultiplied(200, 200, 200, 15);
            painter.rect_filled(rect, 4.0, fill);
            draw_dotted_rect(painter, rect, egui::Stroke::new(stroke_width, stroke_color));
            painter.text(
                rect.left_top() + egui::vec2(4.0, 2.0),
                egui::Align2::LEFT_TOP,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                accent.linear_multiply(0.9),
            );
        }

        // ProgressBar: filled left portion (50 % preview)
        WidgetKind::ProgressBar => {
            painter.rect_filled(rect, 3.0, kind_fill(accent));
            painter.rect_stroke(rect, 3.0, egui::Stroke::new(stroke_width, stroke_color));
            // Preview fill at 60 %
            let fill_w = rect.width() * 0.6;
            let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
            painter.rect_filled(fill_rect, 3.0, accent.linear_multiply(0.55));
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                egui::Color32::WHITE,
            );
        }

        // RadioButton: circle indicator + label
        WidgetKind::RadioButton => {
            painter.rect_filled(rect, 3.0, kind_fill(accent));
            painter.rect_stroke(rect, 3.0, egui::Stroke::new(stroke_width, stroke_color));
            let r = (rect.height() * 0.28).clamp(4.0, 9.0);
            let center = egui::pos2(rect.min.x + r + 4.0, rect.center().y);
            painter.circle_stroke(center, r, egui::Stroke::new(1.5, accent));
            painter.text(
                egui::pos2(center.x + r + 5.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                egui::Color32::WHITE,
            );
        }

        // ComboBox: regular box + small ▾ arrow at right
        WidgetKind::ComboBox => {
            painter.rect_filled(rect, 3.0, kind_fill(accent));
            painter.rect_stroke(rect, 3.0, egui::Stroke::new(stroke_width, stroke_color));
            painter.text(
                egui::pos2(rect.min.x + 6.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                egui::Color32::WHITE,
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
            painter.rect_filled(rect, 3.0, kind_fill(accent));
            painter.rect_stroke(rect, 3.0, egui::Stroke::new(stroke_width, stroke_color));

            let margin_x = rect.width() * 0.1;
            let track_left = rect.min.x + margin_x;
            let track_right = rect.max.x - margin_x;
            let track_y = rect.center().y + rect.height() * 0.15;
            let track_thickness = (2.0 * zoom).clamp(1.5, 3.0);

            // Full track (dark)
            painter.line_segment(
                [
                    egui::pos2(track_left, track_y),
                    egui::pos2(track_right, track_y),
                ],
                egui::Stroke::new(track_thickness, egui::Color32::from_gray(70)),
            );

            // Filled portion up to thumb (42 % preview)
            let thumb_x = track_left + (track_right - track_left) * 0.42;
            painter.line_segment(
                [
                    egui::pos2(track_left, track_y),
                    egui::pos2(thumb_x, track_y),
                ],
                egui::Stroke::new(track_thickness, accent),
            );

            // Thumb circle
            let thumb_r = (rect.height() * 0.22).clamp(4.0, 9.0);
            painter.circle_filled(egui::pos2(thumb_x, track_y), thumb_r, accent);
            painter.circle_stroke(
                egui::pos2(thumb_x, track_y),
                thumb_r,
                egui::Stroke::new(1.0, egui::Color32::WHITE.linear_multiply(0.25)),
            );

            // Label above track
            painter.text(
                egui::pos2(rect.center().x, rect.min.y + 3.0 + label_size * 0.5),
                egui::Align2::CENTER_CENTER,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                egui::Color32::WHITE,
            );
        }

        // Default: filled rect + centered label + kind tag
        _ => {
            painter.rect_filled(rect, 3.0, kind_fill(accent));
            painter.rect_stroke(rect, 3.0, egui::Stroke::new(stroke_width, stroke_color));
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &widget.props.label,
                egui::FontId::proportional(label_size),
                egui::Color32::WHITE,
            );
        }
    }

    // Kind tag (bottom-right) — not on Frame (label is the identifier there)
    if widget.kind != WidgetKind::Frame {
        painter.text(
            rect.right_bottom() - egui::vec2(3.0, 2.0),
            egui::Align2::RIGHT_BOTTOM,
            kind_tag(&widget.kind),
            egui::FontId::proportional(tag_size),
            accent.linear_multiply(0.75),
        );
    }
}

// ---------------------------------------------------------------------------
// Main handler
// ---------------------------------------------------------------------------

pub fn handle(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    state: &mut InteractionState,
    selected: &mut Vec<Uuid>,
    settings: &mut CanvasSettings,
) {
    // Clear per-frame signals
    state.double_clicked_widget = None;
    settings.snap_step = settings.snap_step.max(MIN_SNAP_STEP);

    let (resp, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());

    // -------------------------------------------------------------------
    // Input collection
    // -------------------------------------------------------------------
    let pointer = ui.input(|i| i.pointer.interact_pos());
    let just_pressed = ui.input(|i| i.pointer.primary_pressed());
    let primary_released = ui.input(|i| i.pointer.primary_released());
    let is_down = ui.input(|i| i.pointer.primary_down());
    let shift_held = ui.input(|i| i.modifiers.shift);
    let ctrl_held = ui.input(|i| i.modifiers.ctrl);
    let right_clicked = ui.input(|i| i.pointer.secondary_clicked());
    let middle_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle));
    let mouse_delta = ui.input(|i| i.pointer.delta());
    let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
    let key_g = ui.input(|i| i.key_pressed(egui::Key::G));
    let key_0 = ui.input(|i| ctrl_held && i.key_pressed(egui::Key::Num0));
    let double_clicked = resp.double_clicked();

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
        egui::FontId::proportional(9.0),
        egui::Color32::from_gray(90),
    );

    let primary = selected.last().copied();

    // -------------------------------------------------------------------
    // Draw widgets
    // -------------------------------------------------------------------
    for widget in &tree.widgets {
        let rect = crect(widget, origin, zoom);
        let is_sel = selected.contains(&widget.id);
        draw_widget(&painter, widget, rect, is_sel, zoom);
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
            for widget in tree.widgets.iter().rev() {
                if crect(widget, origin, zoom).contains(pos) {
                    state.context_menu = Some((widget.id, pos));
                    break;
                }
            }
        }
    }

    let mut ctx_action: Option<u8> = None;
    let mut close_ctx = false;
    let ctx_id_for_action: Option<Uuid>;

    if let Some((ctx_id, ctx_pos)) = state.context_menu {
        ctx_id_for_action = Some(ctx_id);
        egui::Area::new(egui::Id::new("canvas_ctx_menu"))
            .fixed_pos(ctx_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(130.0);
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
    if close_ctx {
        state.context_menu = None;
    }

    // -------------------------------------------------------------------
    // Double-click → Lazare highlight
    // -------------------------------------------------------------------
    if double_clicked {
        if let Some(pos) = pointer {
            if resp.rect.contains(pos) {
                state.double_clicked_widget = tree
                    .widgets
                    .iter()
                    .rev()
                    .find(|w| crect(w, origin, zoom).contains(pos))
                    .map(|w| w.id);
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
                        tree.add(w);
                    }
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Mouse-down — resize / select / rubber-band
    // -------------------------------------------------------------------
    if just_pressed && state.context_menu.is_none() {
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
                    let hit_widget = tree
                        .widgets
                        .iter()
                        .rev()
                        .find(|w| crect(w, origin, zoom).contains(pos))
                        .map(|w| w.id);

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
                                let start_rects = selected
                                    .iter()
                                    .filter_map(|&sid| {
                                        tree.widgets
                                            .iter()
                                            .find(|w| w.id == sid)
                                            .map(|w| (sid, w.rect.clone()))
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
                                let start_rect = tree
                                    .widgets
                                    .iter()
                                    .find(|w| w.id == id)
                                    .map(|w| w.rect.clone())
                                    .unwrap();
                                state.drag = Some(DragState {
                                    start_pos: pos,
                                    start_rects: vec![(id, start_rect)],
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
            }
        }
    }

    // -------------------------------------------------------------------
    // Resize drag
    // -------------------------------------------------------------------
    if is_down {
        if let (Some(rs), Some(pos)) = (&state.resize, pointer) {
            let screen_delta = pos - rs.start_pos;
            let canvas_delta = screen_delta / zoom;
            let mut new_rect = rs.handle.apply_delta(&rs.start_rect, canvas_delta);
            if settings.snap_enabled {
                new_rect = snap_rect(new_rect, settings.snap_step);
            }
            let cursor = rs.handle.cursor();
            let id = rs.id;
            if let Some(w) = tree.get_mut(id) {
                w.rect = new_rect;
            }
            ui.ctx().set_cursor_icon(cursor);
        }
    }

    // -------------------------------------------------------------------
    // Multi-drag
    // -------------------------------------------------------------------
    if is_down {
        if let (Some(ds), Some(pos)) = (&state.drag, pointer) {
            let screen_delta = pos - ds.start_pos;
            let canvas_delta = screen_delta / zoom;
            let updates: Vec<(Uuid, f32, f32)> = ds
                .start_rects
                .iter()
                .map(|(id, sr)| {
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
                    (*id, nx, ny)
                })
                .collect();
            for (id, nx, ny) in updates {
                if let Some(w) = tree.get_mut(id) {
                    w.rect.x = nx;
                    w.rect.y = ny;
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Release — rubber-band finalise
    // -------------------------------------------------------------------
    if !is_down {
        if let (Some(band_start), Some(pos)) = (state.rubber_band, pointer) {
            let band_rect = egui::Rect::from_two_pos(band_start, pos);
            if band_rect.width() > 4.0 || band_rect.height() > 4.0 {
                for widget in &tree.widgets {
                    if band_rect.intersects(crect(widget, origin, zoom))
                        && !selected.contains(&widget.id)
                    {
                        selected.push(widget.id);
                    }
                }
            }
        }
        state.drag = None;
        state.resize = None;
        state.rubber_band = None;
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
    if !selected.is_empty() {
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
