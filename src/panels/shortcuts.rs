//! Keyboard shortcut reference and customization overlay.
//!
//! Opened via F1 or the `?` button in the menu bar.  The window has two tabs:
//!  - "Reference" — read-only list of all built-in shortcuts.
//!  - "Customize" — editable list that persists overrides in `UserSettings`.

use crate::settings::UserSettings;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Canonical built-in shortcut list (action_key → default_combo)
// ---------------------------------------------------------------------------

/// The full set of designer shortcuts with their default key combos.
/// Each entry is `(action_key, default_combo, description)`.
pub const BUILTIN_SHORTCUTS: &[(&str, &str, &str)] = &[
    ("new_project", "Ctrl+N", "New project"),
    ("open_project", "Ctrl+O", "Open project…"),
    ("save", "Ctrl+S", "Save"),
    ("save_as", "Ctrl+Shift+S", "Save As…"),
    ("toggle_rulers", "Ctrl+R", "Toggle pixel rulers"),
    ("toggle_outline", "Ctrl+L", "Toggle layers / outline panel"),
    ("toggle_preview", "F5", "Toggle preview mode"),
    ("deselect", "Escape", "Deselect all / cancel operation"),
    ("reset_zoom", "Ctrl+0", "Reset zoom and pan"),
    ("toggle_snap", "G", "Toggle grid snap on / off"),
    ("undo", "Ctrl+Z", "Undo"),
    ("redo", "Ctrl+Y", "Redo"),
    ("group", "Ctrl+G", "Group 2+ selected widgets into a Frame"),
    ("ungroup", "Ctrl+Shift+G", "Ungroup selected Frame"),
    ("shortcuts_help", "F1", "Show / hide this reference"),
    ("canvas_search", "Ctrl+F", "Open canvas widget search"),
];

// ---------------------------------------------------------------------------
// show — floating window
// ---------------------------------------------------------------------------

/// Render the shortcut reference + customization window.
///
/// * `open` — toggled by the window's X button.
/// * `settings` — `UserSettings` holding `user_shortcuts`; mutated in-place
///   when the user edits a combo in the Customize tab.
/// * `dirty` — set to `true` whenever `user_shortcuts` is modified so the
///   caller knows to persist settings.
pub fn show(ctx: &egui::Context, open: &mut bool, settings: &mut UserSettings, dirty: &mut bool) {
    if !*open {
        return;
    }

    let screen = ctx.content_rect();
    let default_pos = egui::pos2(
        (screen.center().x - 220.0).max(screen.min.x + 20.0),
        (screen.center().y - 280.0).max(screen.min.y + 20.0),
    );

    egui::Window::new("Keyboard Shortcuts")
        .id(egui::Id::new("shortcuts_window"))
        .open(open)
        .default_pos(default_pos)
        .default_size([420.0, 560.0])
        .min_size([320.0, 200.0])
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Persistent tab state via egui memory
                let tab_id = ui.id().with("shortcuts_tab");
                let mut tab: u8 = ui.data(|d| d.get_temp(tab_id).unwrap_or(0u8));

                if ui.selectable_label(tab == 0, "Reference").clicked() {
                    tab = 0;
                }
                if ui.selectable_label(tab == 1, "Customize").clicked() {
                    tab = 1;
                }
                ui.data_mut(|d| d.insert_temp(tab_id, tab));

                if tab == 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.weak("read-only");
                    });
                }
            });
            ui.separator();

            let tab_id = ui.id().with("shortcuts_tab");
            let tab: u8 = ui.data(|d| d.get_temp(tab_id).unwrap_or(0u8));

            if tab == 0 {
                show_reference(ui, &settings.user_shortcuts);
            } else {
                show_customize(ui, &mut settings.user_shortcuts, dirty);
            }
        });
}

// ---------------------------------------------------------------------------
// Reference tab
// ---------------------------------------------------------------------------

fn show_reference(ui: &mut egui::Ui, user_shortcuts: &HashMap<String, String>) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        section(ui, "File");
        ref_row(ui, "new_project", user_shortcuts);
        ref_row(ui, "open_project", user_shortcuts);
        ref_row(ui, "save", user_shortcuts);
        ref_row(ui, "save_as", user_shortcuts);

        section(ui, "Canvas");
        row(
            ui,
            "Arrow keys",
            "Nudge selected widget(s) 1 px (or grid step)",
        );
        row(ui, "Delete", "Remove selected widget(s)");
        ref_row(ui, "reset_zoom", user_shortcuts);
        ref_row(ui, "toggle_rulers", user_shortcuts);
        ref_row(ui, "toggle_outline", user_shortcuts);
        ref_row(ui, "toggle_preview", user_shortcuts);
        ref_row(ui, "deselect", user_shortcuts);
        ref_row(ui, "canvas_search", user_shortcuts);

        section(ui, "Selection");
        row(ui, "Click", "Select widget");
        row(ui, "Shift+Click", "Add / remove widget from selection");
        row(ui, "Drag (canvas)", "Rubber-band multi-select");
        row(ui, "Double-click", "Jump code panel to widget handler");

        section(ui, "Grid Snap");
        ref_row(ui, "toggle_snap", user_shortcuts);

        section(ui, "Edit");
        ref_row(ui, "undo", user_shortcuts);
        ref_row(ui, "redo", user_shortcuts);

        section(ui, "Grouping");
        ref_row(ui, "group", user_shortcuts);
        ref_row(ui, "ungroup", user_shortcuts);

        section(ui, "Help");
        ref_row(ui, "shortcuts_help", user_shortcuts);

        ui.add_space(8.0);
        ui.separator();
        ui.label(
            egui::RichText::new("Shortcuts active in Design mode only (not Preview mode)")
                .small()
                .weak(),
        );
    });
}

/// Render one reference row, showing the user override if present.
fn ref_row(ui: &mut egui::Ui, action: &str, user_shortcuts: &HashMap<String, String>) {
    let (_, default_combo, description) = BUILTIN_SHORTCUTS
        .iter()
        .find(|(k, _, _)| *k == action)
        .copied()
        .unwrap_or((action, action, action));

    let combo = user_shortcuts
        .get(action)
        .map(|s| s.as_str())
        .unwrap_or(default_combo);

    let has_override = user_shortcuts.contains_key(action);
    ui.horizontal(|ui| {
        let color = if has_override {
            egui::Color32::from_rgb(251, 191, 36) // amber = overridden
        } else {
            egui::Color32::from_rgb(96, 165, 250)
        };
        ui.add_sized(
            [130.0, 16.0],
            egui::Label::new(egui::RichText::new(combo).monospace().small().color(color)),
        );
        ui.label(egui::RichText::new(description).small());
        if has_override {
            ui.label(egui::RichText::new("(custom)").small().weak());
        }
    });
}

// ---------------------------------------------------------------------------
// Customize tab
// ---------------------------------------------------------------------------

fn show_customize(
    ui: &mut egui::Ui,
    user_shortcuts: &mut HashMap<String, String>,
    dirty: &mut bool,
) {
    ui.label(
        egui::RichText::new(
            "Edit the key combo for any action below. Leave blank to restore the default.",
        )
        .small()
        .weak(),
    );
    ui.add_space(4.0);

    let mut reset_action: Option<String> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("shortcuts_customize_grid")
            .num_columns(4)
            .spacing([6.0, 3.0])
            .striped(true)
            .show(ui, |ui| {
                // Header
                ui.label(egui::RichText::new("Action").small().strong());
                ui.label(egui::RichText::new("Default").small().strong());
                ui.label(egui::RichText::new("Override").small().strong());
                ui.label(""); // reset column
                ui.end_row();

                for &(action_key, default_combo, description) in BUILTIN_SHORTCUTS {
                    ui.label(egui::RichText::new(description).small());
                    ui.label(
                        egui::RichText::new(default_combo)
                            .monospace()
                            .small()
                            .color(egui::Color32::from_gray(160)),
                    );

                    // Editable override field
                    let current = user_shortcuts.get(action_key).cloned().unwrap_or_default();
                    let mut buf = current.clone();
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut buf)
                            .hint_text(default_combo)
                            .desired_width(110.0),
                    );
                    if resp.changed() {
                        *dirty = true;
                        // Treat whitespace-only input as blank so the field
                        // reliably restores the default (matches the UI hint).
                        let normalized = buf.trim();
                        if normalized.is_empty() {
                            user_shortcuts.remove(action_key);
                        } else {
                            user_shortcuts.insert(action_key.to_owned(), normalized.to_owned());
                        }
                    }

                    // Reset button (only visible when there is an override)
                    if user_shortcuts.contains_key(action_key) {
                        if ui
                            .small_button("↺")
                            .on_hover_text("Restore default")
                            .clicked()
                        {
                            reset_action = Some(action_key.to_owned());
                        }
                    } else {
                        ui.label(""); // spacer
                    }

                    ui.end_row();
                }
            });

        ui.add_space(6.0);
        if ui.small_button("Reset all to defaults").clicked() {
            user_shortcuts.clear();
            *dirty = true;
        }
    });

    if let Some(key) = reset_action {
        user_shortcuts.remove(&key);
        *dirty = true;
    }
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
