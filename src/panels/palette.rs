use crate::project::schema::WidgetInstance;
use crate::project::ui_tree::UiTree;
use crate::widgets;

/// Inner palette content.
/// Returns `Some(instance)` if the user is dragging a widget kind from the palette
/// (caller should put it in `interaction.template_drag`).
/// Click-to-add at default position is handled internally.
pub fn show_content(ui: &mut egui::Ui, tree: &mut UiTree) -> Option<WidgetInstance> {
    ui.heading("Palette");
    ui.separator();

    let mut dragged: Option<WidgetInstance> = None;

    for kind in widgets::ALL_KINDS {
        let label = format!("{:?}", kind);
        let desired = egui::vec2(ui.available_width(), 22.0);
        let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());

        let accent = crate::canvas::interaction::kind_accent(kind);
        let vis = ui.style().interact(&resp);

        // On hover or drag: tint background + border + text with accent color
        let (bg, border, text_col) = if resp.hovered() || resp.dragged() {
            let [r, g, b, _] = accent.to_array();
            let tinted = egui::Color32::from_rgb(
                (r as u32 * 4 / 20) as u8,
                (g as u32 * 4 / 20) as u8,
                (b as u32 * 4 / 20) as u8,
            );
            (tinted, egui::Stroke::new(1.5, accent), accent)
        } else {
            (vis.bg_fill, vis.bg_stroke, vis.text_color())
        };

        ui.painter().rect(rect, vis.rounding, bg, border);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &label,
            egui::FontId::proportional(13.0),
            text_col,
        );

        let resp = resp.on_hover_text("Click to add · Drag onto canvas");

        if resp.clicked() {
            tree.add(widgets::default_for(kind));
        } else if resp.dragged() {
            dragged = Some(widgets::default_for(kind));
        }
    }

    dragged
}
