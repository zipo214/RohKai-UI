//! Keyboard shortcut reference overlay.
//!
//! Opened via F1 or the `?` button in the menu bar. Floating read-only window
//! listing every keyboard shortcut active in the designer.

// ---------------------------------------------------------------------------

/// Render the shortcut reference window.  Pass `open` by reference so the
/// window X button closes it.
pub fn show(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }

    let screen = ctx.screen_rect();
    let default_pos = egui::pos2(
        (screen.center().x - 200.0).max(screen.min.x + 20.0),
        (screen.center().y - 260.0).max(screen.min.y + 20.0),
    );

    egui::Window::new("Keyboard Shortcuts")
        .id(egui::Id::new("shortcuts_window"))
        .open(open)
        .default_pos(default_pos)
        .default_size([380.0, 520.0])
        .min_size([300.0, 200.0])
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                section(ui, "File");
                row(ui, "Ctrl+N", "New project");
                row(ui, "Ctrl+O", "Open project…");
                row(ui, "Ctrl+S", "Save");
                row(ui, "Ctrl+Shift+S", "Save As…");

                section(ui, "Canvas");
                row(
                    ui,
                    "Arrow keys",
                    "Nudge selected widget(s) 1 px (or grid step)",
                );
                row(ui, "Delete", "Remove selected widget(s)");
                row(ui, "Ctrl+0", "Reset zoom and pan");
                row(ui, "Ctrl+R", "Toggle pixel rulers");
                row(ui, "Ctrl+L", "Toggle layers / outline panel");
                row(ui, "F5", "Toggle preview mode");
                row(ui, "Escape", "Deselect all / cancel operation");

                section(ui, "Selection");
                row(ui, "Click", "Select widget");
                row(ui, "Shift+Click", "Add / remove widget from selection");
                row(ui, "Drag (canvas)", "Rubber-band multi-select");
                row(ui, "Double-click", "Jump code panel to widget handler");

                section(ui, "Grid Snap");
                row(ui, "G", "Toggle grid snap on / off");

                section(ui, "Grouping");
                row(ui, "Ctrl+G", "Group 2+ selected widgets into a Frame");
                row(ui, "Ctrl+Shift+G", "Ungroup selected Frame");

                section(ui, "Help");
                row(ui, "F1  /  ?", "Show / hide this reference");

                ui.add_space(8.0);
                ui.separator();
                ui.label(
                    egui::RichText::new("Shortcuts active in Design mode only (not Preview mode)")
                        .small()
                        .weak(),
                );
            });
        });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(title).strong());
    ui.separator();
}

fn row(ui: &mut egui::Ui, key: &str, description: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [130.0, 16.0],
            egui::Label::new(
                egui::RichText::new(key)
                    .monospace()
                    .small()
                    .color(egui::Color32::from_rgb(96, 165, 250)),
            ),
        );
        ui.label(egui::RichText::new(description).small());
    });
}
