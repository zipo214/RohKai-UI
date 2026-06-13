//! Project tree panel — file browser + read-only viewer for the exported
//! project, plus an asset registry editor.
//!
//! Files come from `codegen::export::project_files()` (the same in-memory list
//! that disk export writes), so what you see here is exactly what export
//! produces.  Assets are declared references stored in `AppProps.assets`.

use crate::project::schema::{AssetEntry, AssetKind};
use uuid::Uuid;

#[derive(Debug)]
pub enum ProjectTreeAction {
    None,
    /// User asked to add an asset — caller opens a file picker.
    AddAsset,
    /// Remove the asset with this id.
    RemoveAsset(Uuid),
}

/// Render the project tree window.  Returns an action for asset edits.
///
/// - `files`: `(relative_path, contents)` pairs from `project_files()`.
/// - `assets`: current asset registry (read-only here; edits go via the action).
/// - `selected_file`: which file's contents are shown in the viewer pane.
pub fn show(
    ctx: &egui::Context,
    open: &mut bool,
    files: &[(String, String)],
    assets: &[AssetEntry],
    selected_file: &mut Option<String>,
) -> ProjectTreeAction {
    let mut action = ProjectTreeAction::None;
    if !*open {
        return action;
    }

    // Default selection: first file.
    if selected_file.is_none()
        && let Some((p, _)) = files.first()
    {
        *selected_file = Some(p.clone());
    }

    let screen = ctx.content_rect();
    let default_pos = egui::pos2(
        (screen.center().x - 360.0).max(screen.min.x + 20.0),
        (screen.center().y - 260.0).max(screen.min.y + 20.0),
    );

    egui::Window::new("Project Files")
        .id(egui::Id::new("project_tree_window"))
        .open(open)
        .default_pos(default_pos)
        .default_size([720.0, 500.0])
        .min_size([520.0, 320.0])
        .resizable(true)
        .show(ctx, |ui| {
            let avail = ui.available_width().min(720.0 - 16.0);
            let tree_w = (avail * 0.36 - 4.0).max(160.0);
            let view_w = (avail - tree_w - 12.0).max(200.0);

            ui.horizontal_top(|ui| {
                // Left: file tree + asset registry
                ui.vertical(|ui| {
                    ui.set_width(tree_w);
                    ui.label(egui::RichText::new("exported_app/").strong());
                    egui::ScrollArea::vertical()
                        .id_salt("project_tree_scroll")
                        .max_height(220.0)
                        .show(ui, |ui| {
                            for (path, _) in files {
                                let is_sel = selected_file.as_deref() == Some(path.as_str());
                                if ui
                                    .selectable_label(is_sel, format!("  {}", tree_label(path)))
                                    .clicked()
                                {
                                    *selected_file = Some(path.clone());
                                }
                            }
                        });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Assets").strong());
                        if ui
                            .small_button("+ Add")
                            .on_hover_text("Add an asset file")
                            .clicked()
                        {
                            action = ProjectTreeAction::AddAsset;
                        }
                    });
                    if assets.is_empty() {
                        ui.label(
                            egui::RichText::new("No assets. Click + Add to reference one.")
                                .small()
                                .weak(),
                        );
                    } else {
                        egui::ScrollArea::vertical()
                            .id_salt("asset_scroll")
                            .max_height(140.0)
                            .show(ui, |ui| {
                                for a in assets {
                                    ui.horizontal(|ui| {
                                        let (icon, color) = asset_icon(&a.kind);
                                        ui.label(egui::RichText::new(icon).color(color));
                                        ui.label(egui::RichText::new(&a.name).small())
                                            .on_hover_text(&a.source_path);
                                        if ui.small_button("x").clicked() {
                                            action = ProjectTreeAction::RemoveAsset(a.id);
                                        }
                                    });
                                }
                            });
                    }
                });

                ui.separator();

                // Right: file content viewer (read-only)
                ui.vertical(|ui| {
                    ui.set_width(view_w);
                    let content = selected_file
                        .as_deref()
                        .and_then(|sel| files.iter().find(|(p, _)| p == sel))
                        .map(|(_, c)| c.clone())
                        .unwrap_or_default();
                    let path_label = selected_file.clone().unwrap_or_default();
                    ui.label(egui::RichText::new(&path_label).monospace().small().weak());
                    ui.separator();
                    egui::ScrollArea::both()
                        .id_salt("file_view_scroll")
                        .show(ui, |ui| {
                            let mut text = content;
                            ui.add(
                                egui::TextEdit::multiline(&mut text)
                                    .font(egui::FontId::monospace(11.0))
                                    .desired_width(view_w)
                                    .interactive(false)
                                    .code_editor(),
                            );
                        });
                });
            });
        });

    action
}

fn tree_label(path: &str) -> String {
    // Indent nested paths by depth for a simple tree feel.
    let depth = path.matches('/').count();
    let name = path.rsplit('/').next().unwrap_or(path);
    format!("{}{}", "  ".repeat(depth), name)
}

fn asset_icon(kind: &AssetKind) -> (&'static str, egui::Color32) {
    match kind {
        AssetKind::Image => ("🖼", egui::Color32::from_rgb(96, 165, 250)),
        AssetKind::Font => ("🅰", egui::Color32::from_rgb(167, 139, 250)),
        AssetKind::Data => ("🗎", egui::Color32::from_rgb(52, 211, 153)),
        AssetKind::Other => ("📄", egui::Color32::from_gray(170)),
    }
}

/// Classify an asset by file extension.
pub fn classify_asset(name: &str) -> AssetKind {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "svg" | "gif" | "bmp" | "webp" => AssetKind::Image,
        "ttf" | "otf" | "woff" | "woff2" => AssetKind::Font,
        "json" | "csv" | "toml" | "ron" | "txt" | "yaml" | "yml" => AssetKind::Data,
        _ => AssetKind::Other,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_extension() {
        assert_eq!(classify_asset("logo.png"), AssetKind::Image);
        assert_eq!(classify_asset("Roboto.ttf"), AssetKind::Font);
        assert_eq!(classify_asset("data.json"), AssetKind::Data);
        assert_eq!(classify_asset("notes.md"), AssetKind::Other);
        assert_eq!(classify_asset("noext"), AssetKind::Other);
    }

    #[test]
    fn tree_label_indents_by_depth() {
        assert_eq!(tree_label("Cargo.toml"), "Cargo.toml");
        assert_eq!(tree_label("src/main.rs"), "  main.rs");
        assert_eq!(tree_label("assets/MANIFEST.txt"), "  MANIFEST.txt");
    }
}
