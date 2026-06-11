use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
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
// Built-in templates (embedded — no file I/O)
// ---------------------------------------------------------------------------

/// Returns `(name, widgets)` for every built-in preset.
/// These appear above user-saved templates in the panel, separated by a header.
pub fn builtin_templates() -> Vec<(&'static str, Vec<WidgetInstance>)> {
    vec![
        ("Form Layout", form_layout_preset()),
        ("Login Dialog", login_dialog_preset()),
    ]
}

/// A 2-column GridLayout with three label/TextInput pairs — covers the
/// Form Layout roadmap item as a GridLayout preset.
fn form_layout_preset() -> Vec<WidgetInstance> {
    use uuid::Uuid;
    // Deterministic UUIDs for the preset — chosen to avoid any real project collision.
    const GRID: u128 = 0x0000_F040_0001_0000_0000_0000_0000_0001;
    const CHILD_BASE: u128 = 0x0000_F040_0100_0000_0000_0000_0000_0000;
    let field_w = 150.0_f32;
    let field_h = 28.0_f32;
    let label_w = 80.0_f32;
    let pairs = [("Name", "name"), ("Email", "email"), ("Phone", "phone")];
    let mut widgets: Vec<WidgetInstance> = Vec::new();
    let mut child_ids: Vec<Uuid> = Vec::new();

    for (i, (lbl, binding)) in pairs.iter().enumerate() {
        let label_id = Uuid::from_u128(CHILD_BASE + i as u128 * 2);
        let field_id = Uuid::from_u128(CHILD_BASE + i as u128 * 2 + 1);
        let y = i as f32 * (field_h + 4.0);

        widgets.push(WidgetInstance {
            id: label_id,
            kind: WidgetKind::Label,
            rect: Rect {
                x: 0.0,
                y,
                w: label_w,
                h: field_h,
            },
            props: WidgetProps {
                label: format!("{lbl}:"),
                ..Default::default()
            },
            ..Default::default()
        });
        widgets.push(WidgetInstance {
            id: field_id,
            kind: WidgetKind::TextInput,
            rect: Rect {
                x: label_w + 4.0,
                y,
                w: field_w,
                h: field_h,
            },
            props: WidgetProps {
                label: lbl.to_string(),
                ..Default::default()
            },
            state_binding: Some(binding.to_string()),
            ..Default::default()
        });
        child_ids.push(label_id);
        child_ids.push(field_id);
    }

    let grid = WidgetInstance {
        id: Uuid::from_u128(GRID),
        kind: WidgetKind::GridLayout,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: label_w + field_w + 8.0,
            h: pairs.len() as f32 * (field_h + 4.0),
        },
        props: WidgetProps {
            grid_columns: 2,
            label: "Form".to_owned(),
            ..Default::default()
        },
        children: child_ids,
        ..Default::default()
    };

    let mut out = vec![grid];
    out.extend(widgets);
    out
}

/// A simple login dialog: VLayout with username/password fields + a login Button.
fn login_dialog_preset() -> Vec<WidgetInstance> {
    use uuid::Uuid;
    const LAYOUT: u128 = 0x0000_B17C_0000_0000_0000_0000_0000_0001;
    const USER: u128 = 0x0000_B17C_0000_0000_0000_0000_0000_0002;
    const PASS: u128 = 0x0000_B17C_0000_0000_0000_0000_0000_0003;
    const BTN: u128 = 0x0000_B17C_0000_0000_0000_0000_0000_0004;
    let field_h = 28.0_f32;
    let field_w = 200.0_f32;

    vec![
        WidgetInstance {
            id: Uuid::from_u128(LAYOUT),
            kind: WidgetKind::VLayout,
            rect: Rect {
                x: 20.0,
                y: 20.0,
                w: field_w,
                h: field_h * 3.0 + 16.0,
            },
            props: WidgetProps {
                label: "Login".to_owned(),
                ..Default::default()
            },
            children: vec![
                Uuid::from_u128(USER),
                Uuid::from_u128(PASS),
                Uuid::from_u128(BTN),
            ],
            ..Default::default()
        },
        WidgetInstance {
            id: Uuid::from_u128(USER),
            kind: WidgetKind::TextInput,
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: field_w,
                h: field_h,
            },
            props: WidgetProps {
                label: "Username".to_owned(),
                placeholder: "Enter username…".to_owned(),
                ..Default::default()
            },
            state_binding: Some("username".to_owned()),
            ..Default::default()
        },
        WidgetInstance {
            id: Uuid::from_u128(PASS),
            kind: WidgetKind::TextInput,
            rect: Rect {
                x: 0.0,
                y: field_h + 4.0,
                w: field_w,
                h: field_h,
            },
            props: WidgetProps {
                label: "Password".to_owned(),
                placeholder: "Enter password…".to_owned(),
                ..Default::default()
            },
            state_binding: Some("password".to_owned()),
            ..Default::default()
        },
        WidgetInstance {
            id: Uuid::from_u128(BTN),
            kind: WidgetKind::Button,
            rect: Rect {
                x: 0.0,
                y: (field_h + 4.0) * 2.0,
                w: field_w,
                h: field_h,
            },
            props: WidgetProps {
                label: "Log In".to_owned(),
                ..Default::default()
            },
            on_click: "on_login".to_owned(),
            ..Default::default()
        },
    ]
}

// ---------------------------------------------------------------------------
// Panel UI
// ---------------------------------------------------------------------------

pub fn show(ui: &mut egui::Ui, template_message: &mut Option<(bool, String)>) -> TemplateAction {
    ui.label(egui::RichText::new("Templates").color(egui::Color32::from_gray(140)));

    let user_templates = list_templates();
    let builtins = builtin_templates();
    let mut action = TemplateAction::None;

    // --- Built-in presets ---
    ui.label(egui::RichText::new("Built-in").small().weak());
    for (name, widgets) in &builtins {
        let desired = egui::vec2(ui.available_width(), 20.0);
        let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
        let vis = ui.style().interact(&resp);
        ui.painter()
            .rect(rect, vis.rounding, vis.bg_fill, vis.bg_stroke);
        ui.painter().text(
            rect.left_center() + egui::vec2(4.0, 0.0),
            egui::Align2::LEFT_CENTER,
            *name,
            egui::FontId::proportional(13.0),
            vis.text_color(),
        );
        let resp = resp.on_hover_text("Click to add at canvas centre · Drag onto canvas");
        if resp.clicked() {
            action = TemplateAction::AddAtCenter(widgets.clone());
        } else if resp.drag_started() {
            action = TemplateAction::BeginDrag(widgets.clone());
        }
    }

    // --- User-saved templates ---
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Saved").small().weak());
    if user_templates.is_empty() {
        ui.label(
            egui::RichText::new("No saved templates yet.\nSelect widgets and save to create one.")
                .small()
                .weak(),
        );
    } else {
        egui::ScrollArea::vertical()
            .id_salt("tmpl_scroll")
            .max_height(180.0)
            .show(ui, |ui| {
                for (name, path) in &user_templates {
                    let desired = egui::vec2(ui.available_width(), 20.0);
                    let (rect, resp) =
                        ui.allocate_exact_size(desired, egui::Sense::click_and_drag());

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_form_layout_has_grid_parent_with_label_textinput_children() {
        let builtins = builtin_templates();
        let (name, widgets) = builtins.iter().find(|(n, _)| *n == "Form Layout").unwrap();
        assert_eq!(*name, "Form Layout");
        assert!(
            !widgets.is_empty(),
            "form layout preset must contain widgets"
        );
        let grid = widgets.iter().find(|w| w.kind == WidgetKind::GridLayout);
        assert!(grid.is_some(), "form layout must have a GridLayout parent");
        let grid = grid.unwrap();
        assert_eq!(
            grid.props.grid_columns, 2,
            "form layout grid must have 2 columns"
        );
        let labels = widgets
            .iter()
            .filter(|w| w.kind == WidgetKind::Label)
            .count();
        let inputs = widgets
            .iter()
            .filter(|w| w.kind == WidgetKind::TextInput)
            .count();
        assert!(labels >= 1, "form layout must have at least one Label");
        assert_eq!(
            labels, inputs,
            "form layout must pair each Label with a TextInput"
        );
    }

    #[test]
    fn builtin_login_dialog_has_vlayout_with_fields_and_button() {
        let builtins = builtin_templates();
        let (_, widgets) = builtins.iter().find(|(n, _)| *n == "Login Dialog").unwrap();
        let layout = widgets.iter().find(|w| w.kind == WidgetKind::VLayout);
        assert!(layout.is_some(), "login dialog must have a VLayout root");
        assert!(
            widgets.iter().any(|w| w.kind == WidgetKind::Button),
            "login dialog must have a Button"
        );
        assert!(
            widgets
                .iter()
                .filter(|w| w.kind == WidgetKind::TextInput)
                .count()
                >= 2,
            "login dialog must have at least 2 TextInput fields"
        );
    }

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
            ..Default::default()
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
