use crate::project::ui_tree::UiTree;
use uuid::Uuid;

pub fn show_content(ui: &mut egui::Ui, tree: &mut UiTree, selected: &mut Vec<Uuid>) {
    ui.separator();

    if selected.is_empty() {
        ui.label("No widget selected.");
        return;
    }

    // ------------------------------------------------------------------
    // Alignment tools — only for 2+ selected widgets
    // ------------------------------------------------------------------
    if selected.len() >= 2 {
        show_alignment(ui, tree, selected);
        ui.separator();
    }

    // ------------------------------------------------------------------
    // Primary widget properties
    // ------------------------------------------------------------------
    let Some(id) = selected.last().copied() else {
        return;
    };

    let mut do_delete = false;
    {
        let Some(w) = tree.get_mut(id) else {
            ui.label("Widget not found.");
            return;
        };

        ui.label("Label:");
        ui.text_edit_singleline(&mut w.props.label);

        ui.label("Binding:");
        let mut binding = w.state_binding.clone().unwrap_or_default();
        if ui.text_edit_singleline(&mut binding).changed() {
            let trimmed = binding.trim();
            if trimmed.is_empty() {
                w.state_binding = None;
            } else if crate::codegen::rust::is_valid_identifier(trimmed) {
                w.state_binding = Some(trimmed.to_owned());
            }
        }
        if let Some(binding) = &w.state_binding {
            if !crate::codegen::rust::is_valid_identifier(binding) {
                ui.label(
                    egui::RichText::new("Binding must be a valid Rust field name.")
                        .small()
                        .color(egui::Color32::RED),
                );
            }
        }

        ui.separator();
        egui::Grid::new("rect_grid")
            .num_columns(2)
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                ui.label("X");
                ui.add(egui::DragValue::new(&mut w.rect.x).speed(1.0));
                ui.end_row();
                ui.label("Y");
                ui.add(egui::DragValue::new(&mut w.rect.y).speed(1.0));
                ui.end_row();
                ui.label("W");
                ui.add(egui::DragValue::new(&mut w.rect.w).speed(1.0));
                ui.end_row();
                ui.label("H");
                ui.add(egui::DragValue::new(&mut w.rect.h).speed(1.0));
                ui.end_row();
                ui.label("Min");
                ui.add(egui::DragValue::new(&mut w.props.min).speed(0.5));
                ui.end_row();
                ui.label("Max");
                ui.add(egui::DragValue::new(&mut w.props.max).speed(0.5));
                ui.end_row();
            });

        ui.separator();
        if ui.button("Delete widget").clicked() {
            do_delete = true;
        }
    } // w borrow ends

    if do_delete {
        tree.remove(id);
        selected.retain(|&x| x != id);
    }
    tree.validate_and_repair();
}

// ---------------------------------------------------------------------------
// Alignment tools
// ---------------------------------------------------------------------------

const GUIDE: egui::Color32 = egui::Color32::from_rgb(52, 211, 153);
const BLOCK: egui::Color32 = egui::Color32::from_gray(180);

#[derive(Clone, Copy)]
enum AlignAction {
    Left,
    CenterH,
    Right,
    Top,
    CenterV,
    Bottom,
}

fn align_button(
    ui: &mut egui::Ui,
    tooltip: &str,
    draw_fn: impl FnOnce(&egui::Painter, egui::Rect),
) -> bool {
    let size = egui::vec2(28.0, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter()
        .rect(rect, visuals.rounding, visuals.bg_fill, visuals.bg_stroke);
    let inner = rect.shrink(3.0);
    draw_fn(ui.painter(), inner);
    response.on_hover_text(tooltip).clicked()
}

fn icon_align_left(p: &egui::Painter, r: egui::Rect) {
    p.line_segment(
        [egui::pos2(r.min.x, r.min.y), egui::pos2(r.min.x, r.max.y)],
        egui::Stroke::new(1.5, GUIDE),
    );
    let x = r.min.x + 2.0;
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(x, r.min.y + 2.0), egui::vec2(15.0, 5.5)),
        0.0,
        BLOCK,
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(x, r.min.y + 13.0), egui::vec2(9.0, 5.5)),
        0.0,
        BLOCK,
    );
}

fn icon_center_h(p: &egui::Painter, r: egui::Rect) {
    let cx = r.center().x;
    p.line_segment(
        [egui::pos2(cx, r.min.y), egui::pos2(cx, r.max.y)],
        egui::Stroke::new(1.5, GUIDE),
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(cx - 7.5, r.min.y + 2.0), egui::vec2(15.0, 5.5)),
        0.0,
        BLOCK,
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(cx - 4.5, r.min.y + 13.0), egui::vec2(9.0, 5.5)),
        0.0,
        BLOCK,
    );
}

fn icon_align_right(p: &egui::Painter, r: egui::Rect) {
    p.line_segment(
        [egui::pos2(r.max.x, r.min.y), egui::pos2(r.max.x, r.max.y)],
        egui::Stroke::new(1.5, GUIDE),
    );
    let x_wide = r.max.x - 2.0 - 15.0;
    let x_narrow = r.max.x - 2.0 - 9.0;
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(x_wide, r.min.y + 2.0), egui::vec2(15.0, 5.5)),
        0.0,
        BLOCK,
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(x_narrow, r.min.y + 13.0), egui::vec2(9.0, 5.5)),
        0.0,
        BLOCK,
    );
}

fn icon_align_top(p: &egui::Painter, r: egui::Rect) {
    p.line_segment(
        [egui::pos2(r.min.x, r.min.y), egui::pos2(r.max.x, r.min.y)],
        egui::Stroke::new(1.5, GUIDE),
    );
    let y = r.min.y + 2.0;
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(r.min.x + 2.0, y), egui::vec2(7.0, 12.0)),
        0.0,
        BLOCK,
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(r.min.x + 13.0, y), egui::vec2(7.0, 7.0)),
        0.0,
        BLOCK,
    );
}

fn icon_center_v(p: &egui::Painter, r: egui::Rect) {
    let cy = r.center().y;
    p.line_segment(
        [egui::pos2(r.min.x, cy), egui::pos2(r.max.x, cy)],
        egui::Stroke::new(1.5, GUIDE),
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(r.min.x + 2.0, cy - 6.0), egui::vec2(7.0, 12.0)),
        0.0,
        BLOCK,
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(r.min.x + 13.0, cy - 3.5), egui::vec2(7.0, 7.0)),
        0.0,
        BLOCK,
    );
}

fn icon_align_bottom(p: &egui::Painter, r: egui::Rect) {
    p.line_segment(
        [egui::pos2(r.min.x, r.max.y), egui::pos2(r.max.x, r.max.y)],
        egui::Stroke::new(1.5, GUIDE),
    );
    let y_tall = r.max.y - 2.0 - 12.0;
    let y_short = r.max.y - 2.0 - 7.0;
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(r.min.x + 2.0, y_tall), egui::vec2(7.0, 12.0)),
        0.0,
        BLOCK,
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(r.min.x + 13.0, y_short), egui::vec2(7.0, 7.0)),
        0.0,
        BLOCK,
    );
}

fn show_alignment(ui: &mut egui::Ui, tree: &mut UiTree, selected: &[Uuid]) {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for &id in selected {
        if let Some(w) = tree.widgets.iter().find(|w| w.id == id) {
            min_x = min_x.min(w.rect.x);
            min_y = min_y.min(w.rect.y);
            max_x = max_x.max(w.rect.x + w.rect.w);
            max_y = max_y.max(w.rect.y + w.rect.h);
        }
    }
    if min_x > max_x {
        return;
    }

    let bb_cx = (min_x + max_x) / 2.0;
    let bb_cy = (min_y + max_y) / 2.0;
    let mut action: Option<AlignAction> = None;

    ui.label(egui::RichText::new("Align").color(egui::Color32::from_gray(140)));
    ui.horizontal(|ui| {
        if align_button(ui, "Align Left", icon_align_left) {
            action = Some(AlignAction::Left);
        }
        if align_button(ui, "Center Horizontal", icon_center_h) {
            action = Some(AlignAction::CenterH);
        }
        if align_button(ui, "Align Right", icon_align_right) {
            action = Some(AlignAction::Right);
        }
    });
    ui.horizontal(|ui| {
        if align_button(ui, "Align Top", icon_align_top) {
            action = Some(AlignAction::Top);
        }
        if align_button(ui, "Center Vertical", icon_center_v) {
            action = Some(AlignAction::CenterV);
        }
        if align_button(ui, "Align Bottom", icon_align_bottom) {
            action = Some(AlignAction::Bottom);
        }
    });

    if let Some(a) = action {
        let ids: Vec<Uuid> = selected.to_vec();
        for id in ids {
            if let Some(w) = tree.get_mut(id) {
                match a {
                    AlignAction::Left => w.rect.x = min_x,
                    AlignAction::CenterH => w.rect.x = bb_cx - w.rect.w / 2.0,
                    AlignAction::Right => w.rect.x = max_x - w.rect.w,
                    AlignAction::Top => w.rect.y = min_y,
                    AlignAction::CenterV => w.rect.y = bb_cy - w.rect.h / 2.0,
                    AlignAction::Bottom => w.rect.y = max_y - w.rect.h,
                }
            }
        }
    }
}
