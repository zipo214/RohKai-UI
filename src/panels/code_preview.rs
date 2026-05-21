use crate::codegen::{egui_emitter, state_emitter};
use crate::project::ui_tree::UiTree;
use uuid::Uuid;

const HIGHLIGHT_BG: egui::Color32 = egui::Color32::from_rgba_premultiplied(15, 60, 40, 180);
const HIGHLIGHT_FG: egui::Color32 = egui::Color32::from_rgb(52, 211, 153);

pub fn show(
    ctx: &egui::Context,
    tree: &UiTree,
    highlighted_id: Option<Uuid>,
    scroll_to: &mut bool,
) {
    egui::SidePanel::right("code_output")
        .min_width(220.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Generated Code");
            ui.separator();

            // ---- egui update() output (indexed for Lazare highlight) ----
            ui.label(egui::RichText::new("egui output").strong());
            let lines = egui_emitter::emit_indexed(tree);

            egui::ScrollArea::vertical()
                .id_salt("code_scroll")
                .max_height(ui.available_height() / 2.0)
                .show(ui, |ui| {
                    for (id, line) in &lines {
                        let is_hi = id.map(|wid| Some(wid) == highlighted_id).unwrap_or(false);
                        if is_hi {
                            if *scroll_to {
                                ui.scroll_to_cursor(Some(egui::Align::Center));
                                *scroll_to = false;
                            }
                            egui::Frame::none()
                                .fill(HIGHLIGHT_BG)
                                .inner_margin(egui::Margin::symmetric(4.0, 1.0))
                                .show(ui, |ui| {
                                    ui.monospace(egui::RichText::new(line).color(HIGHLIGHT_FG));
                                });
                        } else {
                            ui.monospace(line);
                        }
                    }
                });

            ui.separator();

            // ---- AppState output ----
            ui.label(egui::RichText::new("AppState").strong());
            let state = state_emitter::emit(tree);
            egui::ScrollArea::vertical()
                .id_salt("state_scroll")
                .show(ui, |ui| {
                    ui.monospace(&state);
                });
        });
}
