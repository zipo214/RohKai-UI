use crate::project::schema::WidgetInstance;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Templates directory — next to the binary
// ---------------------------------------------------------------------------

pub fn templates_dir() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("templates"))
}

// ---------------------------------------------------------------------------
// Save / load helpers
// ---------------------------------------------------------------------------

pub fn save_template(name: &str, widgets: &[WidgetInstance]) -> Result<PathBuf, String> {
    let dir = templates_dir().ok_or_else(|| "Cannot locate binary directory".to_owned())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
    save_template_in_dir(&dir, name, widgets)
}

pub fn save_imported_svg_template(
    name: &str,
    widgets: &[WidgetInstance],
    svg_source: &str,
) -> Result<PathBuf, String> {
    let dir = templates_dir().ok_or_else(|| "Cannot locate binary directory".to_owned())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
    save_imported_svg_template_in_dir(&dir, name, widgets, svg_source)
}

fn save_imported_svg_template_in_dir(
    dir: &Path,
    name: &str,
    widgets: &[WidgetInstance],
    svg_source: &str,
) -> Result<PathBuf, String> {
    let template_path = save_template_in_dir(dir, name, widgets)?;
    let svg_path = dir.join(format!("{}.svg", safe_template_name(name)));
    std::fs::write(svg_path, svg_source).map_err(|e| format!("preserve svg: {e}"))?;
    Ok(template_path)
}

fn save_template_in_dir(
    dir: &Path,
    name: &str,
    widgets: &[WidgetInstance],
) -> Result<PathBuf, String> {
    let json = serde_json::to_string_pretty(widgets).map_err(|e| format!("serialize: {e}"))?;
    let safe = safe_template_name(name);
    let path = dir.join(format!("{safe}.rktp"));
    std::fs::write(&path, &json).map_err(|e| format!("write: {e}"))?;
    Ok(path)
}

fn safe_template_name(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.trim_matches('_').is_empty() {
        "template".to_owned()
    } else {
        safe
    }
}

pub fn list_templates() -> Vec<(String, PathBuf)> {
    let Some(dir) = templates_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "rktp").unwrap_or(false))
        .map(|e| {
            let path = e.path();
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("template")
                .to_owned();
            (name, path)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

pub fn load_template(path: &Path) -> Result<Vec<WidgetInstance>, String> {
    let json = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    serde_json::from_str(&json).map_err(|e| format!("parse: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{Rect, WidgetKind, WidgetProps};

    #[test]
    fn imported_svg_preserves_original_source_next_to_template() {
        let dir = std::env::temp_dir().join(format!("rohkai_svg_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let widgets = vec![WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Frame,
            rect: Rect {
                x: 1.0,
                y: 2.0,
                w: 30.0,
                h: 40.0,
            },
            props: WidgetProps::default(),
            state_binding: None,
            children: Vec::new(),
            import_metadata: None,
            tooltip: None,
            enabled: None,
            fg_color: None,
            corner_radius: None,
            label_binding: None,
            custom_props: Vec::new(),
            event_handler: None,
        }];
        let svg = "<svg><rect width=\"10\" height=\"10\"/></svg>";
        let template_path =
            save_imported_svg_template_in_dir(&dir, "some icon", &widgets, svg).unwrap();

        assert!(template_path.ends_with("some_icon.rktp"));
        assert_eq!(
            std::fs::read_to_string(dir.join("some_icon.svg")).unwrap(),
            svg
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}

// ---------------------------------------------------------------------------
// Action returned to the caller each frame
// ---------------------------------------------------------------------------

pub enum TemplateAction {
    None,
    /// User clicked a template — add widgets at canvas centre immediately.
    AddAtCenter(Vec<WidgetInstance>),
    /// User started dragging a template — caller should set interaction.template_drag.
    BeginDrag(Vec<WidgetInstance>),
}

// ---------------------------------------------------------------------------
// Panel UI
// ---------------------------------------------------------------------------

pub fn show(ui: &mut egui::Ui, template_message: &mut Option<(bool, String)>) -> TemplateAction {
    ui.label(egui::RichText::new("Templates").color(egui::Color32::from_gray(140)));

    let templates = list_templates();
    let mut action = TemplateAction::None;

    if templates.is_empty() {
        ui.label(
            egui::RichText::new("No templates yet.\nSave a selection to create one.")
                .small()
                .weak(),
        );
    } else {
        egui::ScrollArea::vertical()
            .id_salt("tmpl_scroll")
            .max_height(180.0)
            .show(ui, |ui| {
                for (name, path) in &templates {
                    // Use click_and_drag sense so we can detect both
                    let desired = egui::vec2(ui.available_width(), 20.0);
                    let (rect, resp) =
                        ui.allocate_exact_size(desired, egui::Sense::click_and_drag());

                    // Draw button-like appearance
                    let vis = ui.style().interact(&resp);
                    ui.painter()
                        .rect(rect, vis.rounding, vis.bg_fill, vis.bg_stroke);
                    ui.painter().text(
                        rect.left_center() + egui::vec2(4.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        name,
                        egui::FontId::proportional(13.0),
                        vis.text_color(),
                    );

                    let tooltip = "Click to add at canvas centre · Drag onto canvas";
                    let resp = resp.on_hover_text(tooltip);

                    if resp.clicked() {
                        // Immediate add — load and return AddAtCenter
                        match load_template(path) {
                            Ok(instances) => {
                                action = TemplateAction::AddAtCenter(instances);
                            }
                            Err(e) => {
                                *template_message = Some((false, format!("Load failed: {e}")));
                            }
                        }
                    } else if resp.drag_started() {
                        match load_template(path) {
                            Ok(instances) => {
                                action = TemplateAction::BeginDrag(instances);
                            }
                            Err(e) => {
                                *template_message = Some((false, format!("Load failed: {e}")));
                            }
                        }
                    }
                }
            });
    }

    let mut clear_message = false;
    if let Some((ok, msg)) = template_message {
        let color = if *ok {
            egui::Color32::from_rgb(52, 211, 153)
        } else {
            egui::Color32::RED
        };
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(msg.as_str()).small().color(color));
            clear_message = ui
                .small_button("x")
                .on_hover_text("Dismiss template message")
                .clicked();
        });
    }
    if clear_message {
        *template_message = None;
    }

    action
}
