//! Document outline (layers) panel.
//!
//! Shows every root widget in draw order (index 0 = back, last = front), with
//! owned children directly nested under their parent. Supports click-to-select,
//! Ctrl+click multi-select, double-click canvas-center, and drag-to-reorder
//! z-order.
//!
//! No retained state required — all interaction state lives in egui temporary
//! memory keyed by `"outline_drag"`.

use crate::canvas::interaction::{kind_accent, kind_tag};
use crate::project::{schema::WidgetInstance, ui_tree::UiTree};
use std::collections::HashSet;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OutlineRow {
    id: Uuid,
    tree_idx: usize,
    indent: usize,
}

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

    let outline_rows = outline_rows(tree);

    let mut action = OutlineAction::None;

    // Collect per-row info for drag-target resolution, built this frame.
    let mut row_centers: Vec<(usize, Uuid, f32)> = Vec::new(); // (tree_idx, id, center_y)
    let mut drag_started: Option<(Uuid, usize)> = None;

    let total_w = ui.available_width();

    egui::ScrollArea::vertical()
        .id_salt("outline_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for row in &outline_rows {
                let Some(widget) = tree.widgets.get(row.tree_idx) else {
                    continue;
                };
                let row_result = render_row(
                    ui,
                    total_w,
                    widget,
                    row.tree_idx,
                    row.indent,
                    selected,
                    &drag,
                );
                let (center_y, click, dbl_click, drag_start) = row_result;
                row_centers.push((row.tree_idx, row.id, center_y));

                if !read_only {
                    if drag_start && drag.dragged_id.is_none() {
                        drag_started = Some((row.id, row.tree_idx));
                    }
                    if dbl_click {
                        action = OutlineAction::CenterOn(row.id);
                    } else if click {
                        action = if ctrl_held {
                            OutlineAction::AddToSelection(row.id)
                        } else {
                            OutlineAction::Select(row.id)
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

fn outline_rows(tree: &UiTree) -> Vec<OutlineRow> {
    let child_ids: HashSet<Uuid> = tree
        .widgets
        .iter()
        .flat_map(|w| w.children.iter().copied())
        .collect();
    let mut rows = Vec::new();
    let mut visited = HashSet::new();

    for (idx, widget) in tree.widgets.iter().enumerate() {
        if child_ids.contains(&widget.id) {
            continue;
        }
        push_outline_row(tree, widget.id, idx, 0, &mut visited, &mut rows);
    }

    // Cycles or stale child metadata should not make a widget disappear.
    for (idx, widget) in tree.widgets.iter().enumerate() {
        if !visited.contains(&widget.id) {
            push_outline_row(tree, widget.id, idx, 0, &mut visited, &mut rows);
        }
    }

    rows
}

fn push_outline_row(
    tree: &UiTree,
    id: Uuid,
    tree_idx: usize,
    indent: usize,
    visited: &mut HashSet<Uuid>,
    rows: &mut Vec<OutlineRow>,
) {
    if !visited.insert(id) {
        return;
    }
    rows.push(OutlineRow {
        id,
        tree_idx,
        indent,
    });

    let Some(widget) = tree.widgets.get(tree_idx) else {
        return;
    };
    for child_id in &widget.children {
        if let Some(child_idx) = tree.widgets.iter().position(|w| w.id == *child_id) {
            push_outline_row(tree, *child_id, child_idx, indent + 1, visited, rows);
        }
    }
}

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
    let indent_px = indent as f32 * 16.0;
    let dot_x = row_rect.min.x + indent_px + 10.0;
    let dot_y = row_rect.center().y;
    if indent > 0 {
        ui.painter().text(
            egui::pos2(row_rect.min.x + indent_px - 4.0, dot_y),
            egui::Align2::CENTER_CENTER,
            "↳",
            egui::FontId::monospace(10.0),
            egui::Color32::from_gray(115),
        );
    }
    ui.painter()
        .circle_filled(egui::pos2(dot_x, dot_y), 4.0, accent);

    // Label text (truncated).
    let raw_label: &str = if widget.props.label.is_empty() {
        "—"
    } else {
        &widget.props.label
    };
    let truncated;
    let label: &str = if raw_label.chars().count() > 22 {
        // Truncate on a char boundary so multi-byte UTF-8 never panics.
        truncated = format!("{}…", raw_label.chars().take(20).collect::<String>());
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
    let tag = if widget.children.is_empty() {
        kind_tag(&widget.kind).to_owned()
    } else {
        format!("{} · {}", kind_tag(&widget.kind), widget.children.len())
    };
    ui.painter().text(
        egui::pos2(row_rect.max.x - 4.0, dot_y),
        egui::Align2::RIGHT_CENTER,
        tag,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{WidgetInstance, WidgetKind};

    #[test]
    fn outline_rows_nest_owned_children_under_parent() {
        let parent = Uuid::from_u128(1);
        let other = Uuid::from_u128(2);
        let child = Uuid::from_u128(3);
        let tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent,
                    kind: WidgetKind::GridLayout,
                    children: vec![child],
                    ..Default::default()
                },
                WidgetInstance {
                    id: other,
                    kind: WidgetKind::Button,
                    ..Default::default()
                },
                WidgetInstance {
                    id: child,
                    kind: WidgetKind::Button,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let rows = outline_rows(&tree);

        assert_eq!(
            rows.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![parent, child, other]
        );
        assert_eq!(
            rows.iter().map(|r| r.indent).collect::<Vec<_>>(),
            vec![0, 1, 0]
        );
    }
}
