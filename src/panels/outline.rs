//! Document outline (layers) panel.
//!
//! Shows every widget in the UiTree in draw order (index 0 = back, last = front).
//! Children of Frame widgets are indented.  Supports click-to-select,
//! Ctrl+click multi-select, double-click canvas-center, and drag-to-reorder
//! z-order.
//!
//! No retained state required — all interaction state lives in egui temporary
//! memory keyed by `"outline_drag"`.

use crate::canvas::interaction::{kind_accent, kind_tag};
use crate::project::{schema::WidgetInstance, ui_tree::UiTree};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum OutlineAction {
    Select(Uuid),
    AddToSelection(Uuid),
    /// Move widget to `to_idx` in `tree.widgets`.
    ReorderTo {
        id: Uuid,
        to_idx: usize,
    },
    /// Center canvas viewport on this widget.
    CenterOn(Uuid),
    None,
}

// ---------------------------------------------------------------------------
// Drag state — persisted across frames in egui temporary memory
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct DragMeta {
    dragged_id: Option<Uuid>,
    from_idx: usize,
}

const ROW_H: f32 = 22.0;
const DRAG_KEY: &str = "outline_drag";

// ---------------------------------------------------------------------------
// show_content
// ---------------------------------------------------------------------------

/// Renders the outline panel inside `ui`.  Returns one `OutlineAction` per
/// frame (usually `None`).
///
/// `read_only` disables drag-reorder and click-select (used in preview mode).
pub fn show_content(
    ui: &mut egui::Ui,
    tree: &UiTree,
    selected: &[Uuid],
    ctrl_held: bool,
    read_only: bool,
) -> OutlineAction {
    let drag_id = egui::Id::new(DRAG_KEY);

    // Load drag state from previous frame.
    let mut drag: DragMeta = ui.data(|d| d.get_temp(drag_id).unwrap_or_default());

    let pointer_down = ui.input(|i| i.pointer.primary_down());
    let pointer_released = ui.input(|i| i.pointer.primary_released());
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());

    // Build a set of all child IDs so we can mark them as indented.
    let child_ids: std::collections::HashSet<Uuid> = tree
        .widgets
        .iter()
        .flat_map(|w| w.children.iter().copied())
        .collect();

    let mut action = OutlineAction::None;

    // Collect per-row info for drag-target resolution, built this frame.
    let mut row_centers: Vec<(usize, Uuid, f32)> = Vec::new(); // (tree_idx, id, center_y)
    let mut drag_started: Option<(Uuid, usize)> = None;

    let total_w = ui.available_width();

    egui::ScrollArea::vertical()
        .id_salt("outline_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (tree_idx, widget) in tree.widgets.iter().enumerate() {
                let indent = if child_ids.contains(&widget.id) { 1 } else { 0 };

                let row_result = render_row(ui, total_w, widget, tree_idx, indent, selected, &drag);
                let (center_y, click, dbl_click, drag_start) = row_result;
                row_centers.push((tree_idx, widget.id, center_y));

                if !read_only {
                    if drag_start && drag.dragged_id.is_none() {
                        drag_started = Some((widget.id, tree_idx));
                    }
                    if dbl_click {
                        action = OutlineAction::CenterOn(widget.id);
                    } else if click {
                        action = if ctrl_held {
                            OutlineAction::AddToSelection(widget.id)
                        } else {
                            OutlineAction::Select(widget.id)
                        };
                    }
                }
            }

            // Draw drop-indicator line while dragging.
            if drag.dragged_id.is_some() {
                if let Some(pos) = pointer_pos {
                    if let Some(&(_, _, cy)) = row_centers
                        .iter()
                        .min_by_key(|(_, _, c)| ((pos.y - c).abs() * 1000.0) as i64)
                    {
                        let clip = ui.clip_rect();
                        ui.painter().hline(
                            clip.min.x..=clip.max.x,
                            cy,
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(96, 165, 250)),
                        );
                    }
                }
            }
        });

    // Start drag.
    if let Some((id, idx)) = drag_started {
        drag.dragged_id = Some(id);
        drag.from_idx = idx;
    }

    // Resolve drag on release.
    if pointer_released {
        if let (Some(dragged_id), Some(pos)) = (drag.dragged_id, pointer_pos) {
            let to_idx = row_centers
                .iter()
                .min_by_key(|(_, _, cy)| ((pos.y - cy).abs() * 1000.0) as i64)
                .map(|(idx, _, _)| *idx)
                .unwrap_or(drag.from_idx);
            if to_idx != drag.from_idx {
                action = OutlineAction::ReorderTo {
                    id: dragged_id,
                    to_idx,
                };
            }
        }
        drag.dragged_id = None;
        drag.from_idx = 0;
    }

    // Clear drag if pointer lifted.
    if !pointer_down {
        drag.dragged_id = None;
    }

    ui.data_mut(|d| d.insert_temp(drag_id, drag));
    action
}

// ---------------------------------------------------------------------------
// Row renderer — returns (center_y, clicked, double_clicked, drag_started)
// ---------------------------------------------------------------------------

fn render_row(
    ui: &mut egui::Ui,
    total_w: f32,
    widget: &WidgetInstance,
    tree_idx: usize,
    indent: usize,
    selected: &[Uuid],
    drag: &DragMeta,
) -> (f32, bool, bool, bool) {
    let _ = tree_idx; // used by caller for reorder indexing
    let accent = kind_accent(&widget.kind);
    let is_selected = selected.contains(&widget.id);
    let is_dragged = drag.dragged_id == Some(widget.id);

    let (row_rect, response) =
        ui.allocate_exact_size(egui::vec2(total_w, ROW_H), egui::Sense::click_and_drag());
    let center_y = row_rect.center().y;

    // Background.
    let bg = if is_selected {
        egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 45)
    } else if is_dragged {
        egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 20)
    } else if response.hovered() {
        egui::Color32::from_gray(50)
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(row_rect, 2.0, bg);

    // Accent dot.
    let indent_px = indent as f32 * 14.0;
    let dot_x = row_rect.min.x + indent_px + 10.0;
    let dot_y = row_rect.center().y;
    ui.painter()
        .circle_filled(egui::pos2(dot_x, dot_y), 4.0, accent);

    // Label text (truncated).
    let raw_label: &str = if widget.props.label.is_empty() {
        "—"
    } else {
        &widget.props.label
    };
    let truncated;
    let label: &str = if raw_label.len() > 22 {
        truncated = format!("{}…", &raw_label[..20]);
        &truncated
    } else {
        raw_label
    };

    let text_x = dot_x + 10.0;
    let text_color = if is_selected {
        accent
    } else {
        egui::Color32::from_gray(210)
    };
    ui.painter().text(
        egui::pos2(text_x, dot_y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(11.0),
        text_color,
    );

    // Kind tag — right side, muted.
    ui.painter().text(
        egui::pos2(row_rect.max.x - 4.0, dot_y),
        egui::Align2::RIGHT_CENTER,
        kind_tag(&widget.kind),
        egui::FontId::monospace(9.0),
        egui::Color32::from_gray(90),
    );

    // Drag outline for the row being dragged.
    if is_dragged {
        ui.painter()
            .rect_stroke(row_rect, 2.0, egui::Stroke::new(1.5, accent));
    }

    (
        center_y,
        response.clicked(),
        response.double_clicked(),
        response.drag_started(),
    )
}
