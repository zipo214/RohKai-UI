//! Pixel rulers and guide-line overlay for the canvas panel.

use crate::project::schema::{GuideOrientation, GuideRule};

pub const RULER_SIZE: f32 = 16.0;
const GUIDE_SNAP_PX: f32 = 4.0;

const GUIDE_COLOR: egui::Color32 = egui::Color32::from_rgb(52, 211, 153);
const GUIDE_HOVER_COLOR: egui::Color32 = egui::Color32::from_rgb(120, 255, 200);
const RULER_BG: egui::Color32 = egui::Color32::from_gray(28);
const RULER_TICK: egui::Color32 = egui::Color32::from_gray(75);
const RULER_TEXT: egui::Color32 = egui::Color32::from_gray(150);

// ---------------------------------------------------------------------------

pub fn canvas_origin(
    canvas_size: [f32; 2],
    zoom: f32,
    pan: egui::Vec2,
    panel_rect: egui::Rect,
) -> egui::Pos2 {
    egui::pos2(
        panel_rect.center().x + pan.x - canvas_size[0] * zoom / 2.0,
        panel_rect.center().y + pan.y - canvas_size[1] * zoom / 2.0,
    )
}

fn major_tick_step(zoom: f32) -> f32 {
    const STEPS: &[f32] = &[1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0];
    let min_canvas_step = 50.0 / zoom;
    STEPS
        .iter()
        .copied()
        .find(|&s| s >= min_canvas_step)
        .unwrap_or(1000.0)
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Context bundle — avoids too-many-arguments lints.
// ---------------------------------------------------------------------------

pub struct RulerCtx {
    pub show_rulers: bool,
    pub zoom: f32,
    pub pan: egui::Vec2,
    pub canvas_size: [f32; 2],
    pub panel_rect: egui::Rect,
}

// Guide interaction — call BEFORE draw each frame.
// ---------------------------------------------------------------------------

pub fn handle_interaction(
    ui: &mut egui::Ui,
    guides: &mut Vec<GuideRule>,
    hovered: &mut Option<uuid::Uuid>,
    dragging: &mut Option<uuid::Uuid>,
    ctx: &RulerCtx,
    delete_pressed: bool,
) {
    let (show_rulers, zoom, pan, canvas_size, panel_rect) = (
        ctx.show_rulers,
        ctx.zoom,
        ctx.pan,
        ctx.canvas_size,
        ctx.panel_rect,
    );
    let origin = canvas_origin(canvas_size, zoom, pan, panel_rect);
    let pointer = ui.input(|i| i.pointer.interact_pos());
    let Some(pointer) = pointer else {
        *dragging = None;
        *hovered = None;
        return;
    };
    // Own the pointer only when it is over the canvas panel AND the canvas layer
    // is topmost there, so guides aren't created/dragged under floating windows
    // (mirrors the ownership check in interaction.rs).
    let pointer_owned = ui.rect_contains_pointer(panel_rect)
        && ui.ctx().layer_id_at(pointer) == Some(ui.layer_id());

    let just_pressed = ui.input(|i| i.pointer.primary_pressed());
    let is_down = ui.input(|i| i.pointer.primary_down());

    // --- Continue dragging an existing guide ---
    if let Some(drag_id) = *dragging {
        if is_down && pointer_owned {
            if let Some(g) = guides.iter_mut().find(|g| g.id == drag_id) {
                match g.orientation {
                    GuideOrientation::Horizontal => {
                        g.position = (pointer.y - origin.y) / zoom;
                    }
                    GuideOrientation::Vertical => {
                        g.position = (pointer.x - origin.x) / zoom;
                    }
                }
            }
            return;
        }
        *dragging = None;
    }

    if !pointer_owned {
        *hovered = None;
        return;
    }

    // --- Hover detection (only within canvas area, not ruler strips) ---
    let canvas_area = if show_rulers {
        egui::Rect::from_min_max(
            egui::pos2(panel_rect.min.x + RULER_SIZE, panel_rect.min.y + RULER_SIZE),
            panel_rect.max,
        )
    } else {
        panel_rect
    };

    *hovered = None;
    if canvas_area.contains(pointer) {
        for g in guides.iter() {
            let dist = match g.orientation {
                GuideOrientation::Horizontal => {
                    let sy = origin.y + g.position * zoom;
                    (pointer.y - sy).abs()
                }
                GuideOrientation::Vertical => {
                    let sx = origin.x + g.position * zoom;
                    (pointer.x - sx).abs()
                }
            };
            if dist < GUIDE_SNAP_PX {
                *hovered = Some(g.id);
                break;
            }
        }
    }

    // --- Delete hovered guide ---
    if delete_pressed && let Some(h) = *hovered {
        guides.retain(|g| g.id != h);
        *hovered = None;
        return;
    }

    // --- Start drag or create guide ---
    if just_pressed {
        if hovered.is_some() {
            *dragging = *hovered;
            return;
        }

        if !show_rulers || !panel_rect.contains(pointer) {
            return;
        }

        let in_h_ruler = pointer.y >= panel_rect.min.y
            && pointer.y < panel_rect.min.y + RULER_SIZE
            && pointer.x >= panel_rect.min.x + RULER_SIZE;
        let in_v_ruler = pointer.x >= panel_rect.min.x
            && pointer.x < panel_rect.min.x + RULER_SIZE
            && pointer.y >= panel_rect.min.y + RULER_SIZE;

        if in_h_ruler {
            let id = uuid::Uuid::new_v4();
            guides.push(GuideRule {
                id,
                orientation: GuideOrientation::Horizontal,
                position: (pointer.y - origin.y) / zoom,
            });
            *dragging = Some(id);
        } else if in_v_ruler {
            let id = uuid::Uuid::new_v4();
            guides.push(GuideRule {
                id,
                orientation: GuideOrientation::Vertical,
                position: (pointer.x - origin.x) / zoom,
            });
            *dragging = Some(id);
        }
    }
}

// ---------------------------------------------------------------------------
// Draw mock OS title-bar bezel above the canvas rect.
// ---------------------------------------------------------------------------

const BEZEL_H: f32 = 22.0;

pub fn draw_bezel(ui: &mut egui::Ui, ctx: &RulerCtx, title: &str) {
    let origin = canvas_origin(ctx.canvas_size, ctx.zoom, ctx.pan, ctx.panel_rect);
    let canvas_w = ctx.canvas_size[0] * ctx.zoom;
    let ruler_offset = if ctx.show_rulers { RULER_SIZE } else { 0.0 };

    let bezel_top = origin.y - BEZEL_H;
    let bezel_min_y = ctx.panel_rect.min.y + ruler_offset;
    if bezel_top < bezel_min_y {
        return;
    }

    let bezel_rect = egui::Rect::from_min_max(
        egui::pos2(origin.x, bezel_top),
        egui::pos2(origin.x + canvas_w, origin.y),
    );

    let painter = ui.painter_at(ctx.panel_rect);
    painter.rect_filled(
        bezel_rect,
        egui::Rounding::same(4.0),
        egui::Color32::from_gray(45),
    );

    // Traffic-light circles
    let cy = bezel_rect.center().y;
    let dot_r = 5.0;
    let dots = [
        (
            bezel_rect.min.x + 14.0,
            egui::Color32::from_rgb(255, 95, 87),
        ),
        (
            bezel_rect.min.x + 30.0,
            egui::Color32::from_rgb(255, 189, 46),
        ),
        (
            bezel_rect.min.x + 46.0,
            egui::Color32::from_rgb(40, 201, 64),
        ),
    ];
    for (cx, color) in dots {
        painter.circle_filled(egui::pos2(cx, cy), dot_r, color);
    }

    // Centered title
    painter.text(
        bezel_rect.center(),
        egui::Align2::CENTER_CENTER,
        title,
        egui::FontId::proportional(11.0),
        egui::Color32::from_gray(200),
    );
}

// ---------------------------------------------------------------------------
// Draw rulers and guide lines — call AFTER canvas content each frame.
// ---------------------------------------------------------------------------

pub fn draw(ui: &mut egui::Ui, guides: &[GuideRule], hovered: Option<uuid::Uuid>, ctx: &RulerCtx) {
    let (show_rulers, zoom, pan, canvas_size, panel_rect) = (
        ctx.show_rulers,
        ctx.zoom,
        ctx.pan,
        ctx.canvas_size,
        ctx.panel_rect,
    );
    let origin = canvas_origin(canvas_size, zoom, pan, panel_rect);
    let painter = ui.painter_at(panel_rect);

    // --- Guide lines (always drawn regardless of ruler visibility) ---
    for g in guides {
        let color = if Some(g.id) == hovered {
            GUIDE_HOVER_COLOR
        } else {
            GUIDE_COLOR
        };
        let stroke = egui::Stroke::new(1.0, color);
        match g.orientation {
            GuideOrientation::Horizontal => {
                let sy = origin.y + g.position * zoom;
                if sy >= panel_rect.min.y && sy <= panel_rect.max.y {
                    painter.line_segment(
                        [
                            egui::pos2(panel_rect.min.x, sy),
                            egui::pos2(panel_rect.max.x, sy),
                        ],
                        stroke,
                    );
                }
            }
            GuideOrientation::Vertical => {
                let sx = origin.x + g.position * zoom;
                if sx >= panel_rect.min.x && sx <= panel_rect.max.x {
                    painter.line_segment(
                        [
                            egui::pos2(sx, panel_rect.min.y),
                            egui::pos2(sx, panel_rect.max.y),
                        ],
                        stroke,
                    );
                }
            }
        }
    }

    if !show_rulers {
        return;
    }

    // --- Ruler backgrounds ---
    let h_ruler = egui::Rect::from_min_max(
        panel_rect.min,
        egui::pos2(panel_rect.max.x, panel_rect.min.y + RULER_SIZE),
    );
    let v_ruler = egui::Rect::from_min_max(
        panel_rect.min,
        egui::pos2(panel_rect.min.x + RULER_SIZE, panel_rect.max.y),
    );
    painter.rect_filled(h_ruler, 0.0, RULER_BG);
    painter.rect_filled(v_ruler, 0.0, RULER_BG);
    // Corner square over the intersection
    painter.rect_filled(
        egui::Rect::from_min_max(
            panel_rect.min,
            egui::pos2(panel_rect.min.x + RULER_SIZE, panel_rect.min.y + RULER_SIZE),
        ),
        0.0,
        egui::Color32::from_gray(40),
    );

    let major_step = major_tick_step(zoom);
    let minor_step = (major_step / 5.0).max(1.0);

    // --- Horizontal ruler ticks (X axis labels) ---
    let x_canvas_min = (panel_rect.min.x - origin.x) / zoom;
    let x_canvas_max = (panel_rect.max.x - origin.x) / zoom;
    let xi_start = (x_canvas_min / minor_step).floor() as i64;
    let xi_end = (x_canvas_max / minor_step).ceil() as i64;

    for i in xi_start..=xi_end {
        let cx = i as f32 * minor_step;
        let sx = origin.x + cx * zoom;
        if sx < panel_rect.min.x + RULER_SIZE || sx > panel_rect.max.x {
            continue;
        }
        let is_major = i % 5 == 0;
        let tick_h = if is_major {
            RULER_SIZE * 0.55
        } else {
            RULER_SIZE * 0.3
        };
        painter.line_segment(
            [
                egui::pos2(sx, panel_rect.min.y + RULER_SIZE - tick_h),
                egui::pos2(sx, panel_rect.min.y + RULER_SIZE),
            ],
            egui::Stroke::new(1.0, RULER_TICK),
        );
        if is_major {
            painter.text(
                egui::pos2(sx + 2.0, panel_rect.min.y + 1.0),
                egui::Align2::LEFT_TOP,
                format!("{}", cx as i32),
                egui::FontId::monospace(8.5),
                RULER_TEXT,
            );
        }
    }

    // --- Vertical ruler ticks (Y axis labels) ---
    let y_canvas_min = (panel_rect.min.y - origin.y) / zoom;
    let y_canvas_max = (panel_rect.max.y - origin.y) / zoom;
    let yi_start = (y_canvas_min / minor_step).floor() as i64;
    let yi_end = (y_canvas_max / minor_step).ceil() as i64;

    for i in yi_start..=yi_end {
        let cy = i as f32 * minor_step;
        let sy = origin.y + cy * zoom;
        if sy < panel_rect.min.y + RULER_SIZE || sy > panel_rect.max.y {
            continue;
        }
        let is_major = i % 5 == 0;
        let tick_w = if is_major {
            RULER_SIZE * 0.55
        } else {
            RULER_SIZE * 0.3
        };
        painter.line_segment(
            [
                egui::pos2(panel_rect.min.x + RULER_SIZE - tick_w, sy),
                egui::pos2(panel_rect.min.x + RULER_SIZE, sy),
            ],
            egui::Stroke::new(1.0, RULER_TICK),
        );
        if is_major {
            painter.text(
                egui::pos2(panel_rect.min.x + 1.0, sy + 2.0),
                egui::Align2::LEFT_TOP,
                format!("{}", cy as i32),
                egui::FontId::monospace(8.5),
                RULER_TEXT,
            );
        }
    }

    // Bottom border of H ruler and right border of V ruler
    let border_stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(55));
    painter.line_segment(
        [
            egui::pos2(panel_rect.min.x, panel_rect.min.y + RULER_SIZE),
            egui::pos2(panel_rect.max.x, panel_rect.min.y + RULER_SIZE),
        ],
        border_stroke,
    );
    painter.line_segment(
        [
            egui::pos2(panel_rect.min.x + RULER_SIZE, panel_rect.min.y),
            egui::pos2(panel_rect.min.x + RULER_SIZE, panel_rect.max.y),
        ],
        border_stroke,
    );
}
