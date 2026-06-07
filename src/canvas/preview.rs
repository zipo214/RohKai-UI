//! Preview mode — renders the canvas as live interactive egui widgets
//! at 1:1 zoom, giving a faithful preview of the exported app layout.
//!
//! `PreviewState` holds mutable runtime values keyed by `state_binding`.
//! It is initialised from widget defaults when entering preview mode and
//! discarded on exit; it never touches `UiTree` or generated code.

use crate::{
    canvas::{rulers::canvas_origin, widget_instance::canvas_rect},
    project::{schema::WidgetKind, ui_tree::UiTree},
};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum PreviewValue {
    Str(String),
    Float(f32),
    Bool(bool),
}

#[derive(Clone, Default)]
pub struct PreviewState {
    pub values: HashMap<String, PreviewValue>,
}

impl PreviewState {
    /// Build initial values from widget defaults.
    pub fn init_from_tree(tree: &UiTree) -> Self {
        let mut values = HashMap::new();
        for w in &tree.widgets {
            let key = match &w.state_binding {
                Some(b) if !b.is_empty() => b.clone(),
                _ => continue,
            };
            let val = match w.kind {
                WidgetKind::Slider | WidgetKind::ProgressBar => {
                    PreviewValue::Float(w.props.default_value)
                }
                WidgetKind::Checkbox | WidgetKind::RadioButton => PreviewValue::Bool(false),
                _ => PreviewValue::Str(w.props.label.clone()),
            };
            values.insert(key, val);
        }
        Self { values }
    }
}

// ---------------------------------------------------------------------------
// render
// ---------------------------------------------------------------------------

/// Render the canvas in preview mode inside `ui` (egui `CentralPanel`).
///
/// Forces 1:1 zoom with the canvas centred in the panel.
/// Returns `true` when the user clicks "Exit Preview".
pub fn render(
    ui: &mut egui::Ui,
    tree: &UiTree,
    state: &mut PreviewState,
    panel_rect: egui::Rect,
) -> bool {
    let canvas_size = [tree.app_props.win_w, tree.app_props.win_h];
    let zoom = 1.0_f32;
    let pan = egui::Vec2::ZERO;
    let origin = canvas_origin(canvas_size, zoom, pan, panel_rect);

    // Canvas boundary.
    let boundary = egui::Rect::from_min_size(origin, egui::vec2(canvas_size[0], canvas_size[1]));
    ui.painter()
        .rect_filled(boundary, 0.0, egui::Color32::from_gray(22));
    ui.painter().rect_stroke(
        boundary,
        0.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(80)),
    );

    // Render each widget in draw order.
    for widget in &tree.widgets {
        let w_rect = canvas_rect(widget, origin, zoom);
        if !panel_rect.intersects(w_rect) {
            continue;
        }
        let binding = widget.state_binding.clone().unwrap_or_default();
        render_widget(ui, widget, w_rect, &binding, state);
    }

    // "PREVIEW" badge — top-left of panel.
    let badge_rect = egui::Rect::from_min_size(
        panel_rect.min + egui::vec2(8.0, 8.0),
        egui::vec2(70.0, 20.0),
    );
    ui.painter()
        .rect_filled(badge_rect, 4.0, egui::Color32::from_rgb(251, 191, 36));
    ui.painter().text(
        badge_rect.center(),
        egui::Align2::CENTER_CENTER,
        "PREVIEW",
        egui::FontId::proportional(10.0),
        egui::Color32::BLACK,
    );

    // "Exit Preview [F5]" button — bottom-right of panel.
    let exit_max = panel_rect.max - egui::vec2(8.0, 8.0);
    let exit_rect = egui::Rect::from_min_max(exit_max - egui::vec2(148.0, 26.0), exit_max);
    ui.put(exit_rect, egui::Button::new("Exit Preview  [F5]").small())
        .clicked()
}

// ---------------------------------------------------------------------------
// Per-widget rendering
// ---------------------------------------------------------------------------

fn render_widget(
    ui: &mut egui::Ui,
    widget: &crate::project::schema::WidgetInstance,
    w_rect: egui::Rect,
    binding: &str,
    state: &mut PreviewState,
) {
    let size = w_rect.size();
    match &widget.kind {
        WidgetKind::Button => {
            let label = widget.props.label.clone();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.add_sized(size, egui::Button::new(&label));
            });
        }
        WidgetKind::Label => {
            let label = widget.props.label.clone();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.add_sized(size, egui::Label::new(&label));
            });
        }
        WidgetKind::TextInput => {
            let hint = widget.props.placeholder.clone();
            let password = widget.props.password_mode;
            if let Some(PreviewValue::Str(s)) = state.values.get_mut(binding) {
                let te = egui::TextEdit::singleline(s)
                    .hint_text(&hint)
                    .password(password);
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                    ui.add_sized(size, te);
                });
            } else {
                placeholder_box(ui, w_rect, "txt");
            }
        }
        WidgetKind::Slider => {
            let min = widget.props.min;
            let max = widget.props.max;
            if let Some(PreviewValue::Float(f)) = state.values.get_mut(binding) {
                let sl = egui::Slider::new(f, min..=max);
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                    ui.add_sized(size, sl);
                });
            } else {
                placeholder_box(ui, w_rect, "sldr");
            }
        }
        WidgetKind::Checkbox => {
            let label = widget.props.label.clone();
            if let Some(PreviewValue::Bool(b)) = state.values.get_mut(binding) {
                let cb = egui::Checkbox::new(b, &label);
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                    ui.add_sized(size, cb);
                });
            } else {
                placeholder_box(ui, w_rect, "chk");
            }
        }
        WidgetKind::RadioButton => {
            let label = widget.props.label.clone();
            if let Some(PreviewValue::Bool(b)) = state.values.get_mut(binding) {
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                    // Write the selection back so the preview reacts to clicks.
                    if ui
                        .add_sized(size, egui::RadioButton::new(*b, &label))
                        .clicked()
                    {
                        *b = !*b;
                    }
                });
            } else {
                placeholder_box(ui, w_rect, "radio");
            }
        }
        WidgetKind::ProgressBar => {
            let progress = state
                .values
                .get(binding)
                .and_then(|v| {
                    if let PreviewValue::Float(f) = v {
                        Some(*f)
                    } else {
                        None
                    }
                })
                .unwrap_or(widget.props.default_value);
            let animated = widget.props.animated;
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.add_sized(size, egui::ProgressBar::new(progress).animate(animated));
            });
        }
        WidgetKind::ComboBox => {
            // Simplified: show selected text as a read-only-looking button.
            let label = state
                .values
                .get(binding)
                .and_then(|v| {
                    if let PreviewValue::Str(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| widget.props.label.clone());
            let cid = egui::Id::new(("preview_combo", widget.id));
            let mut dummy = label.clone();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                egui::ComboBox::from_id_salt(cid)
                    .selected_text(&dummy)
                    .width(size.x)
                    .show_ui(ui, |ui| {
                        for opt in &widget.props.options {
                            ui.selectable_value(&mut dummy, opt.clone(), opt);
                        }
                    });
            });
            // Write back if selection changed.
            if dummy != label {
                state
                    .values
                    .insert(binding.to_string(), PreviewValue::Str(dummy));
            }
        }
        WidgetKind::Frame => {
            let margin = widget.props.inner_margin;
            let stroke_w = widget.props.stroke_width;
            let stroke_col = widget
                .props
                .stroke_color
                .map_or(egui::Color32::from_gray(100), |[r, g, b]| {
                    egui::Color32::from_rgb(r, g, b)
                });
            let style = ui.style().clone();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                egui::Frame::group(&style)
                    .inner_margin(egui::Margin::same(margin))
                    .stroke(egui::Stroke::new(stroke_w, stroke_col))
                    .show(ui, |_ui| {});
            });
        }
        WidgetKind::TextArea => {
            let hint = widget.props.placeholder.clone();
            if let Some(PreviewValue::Str(s)) = state.values.get_mut(binding) {
                let te = egui::TextEdit::multiline(s).hint_text(&hint);
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                    ui.add_sized(size, te);
                });
            } else {
                placeholder_box(ui, w_rect, "area");
            }
        }
        WidgetKind::SpinBox => {
            let min = widget.props.min;
            let max = widget.props.max;
            if let Some(PreviewValue::Float(f)) = state.values.get_mut(binding) {
                let dv = egui::DragValue::new(f).range(min..=max);
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                    ui.add_sized(size, dv);
                });
            } else {
                placeholder_box(ui, w_rect, "spin");
            }
        }
        WidgetKind::FontComboBox => {
            const FONTS: &[&str] = &["Proportional", "Monospace"];
            let selected = state
                .values
                .get(binding)
                .and_then(|v| {
                    if let PreviewValue::Str(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "Proportional".to_owned());
            let mut sel = selected.clone();
            let cid = egui::Id::new(("preview_font_combo", widget.id));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                egui::ComboBox::from_id_salt(cid)
                    .selected_text(&sel)
                    .width(size.x)
                    .show_ui(ui, |ui| {
                        for f in FONTS {
                            ui.selectable_value(&mut sel, f.to_string(), *f);
                        }
                    });
            });
            if sel != selected {
                state
                    .values
                    .insert(binding.to_string(), PreviewValue::Str(sel));
            }
        }
        WidgetKind::HorizontalSpacer => {
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.add_space(w_rect.width());
            });
        }
        WidgetKind::VerticalSpacer => {
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.add_space(w_rect.height());
            });
        }
        WidgetKind::GroupBox => {
            let label = widget.props.label.clone();
            let style = ui.style().clone();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                egui::Frame::group(&style).show(ui, |ui| {
                    ui.label(&label);
                });
            });
        }
        WidgetKind::VLayout
        | WidgetKind::HLayout
        | WidgetKind::ScrollArea
        | WidgetKind::GridLayout
        | WidgetKind::TabWidget => {
            let tag = match &widget.kind {
                WidgetKind::VLayout => "↕",
                WidgetKind::HLayout => "↔",
                WidgetKind::GridLayout => "⊞",
                WidgetKind::TabWidget => "⊡",
                _ => "⊡",
            };
            placeholder_box(ui, w_rect, tag);
        }
        WidgetKind::ToolButton => {
            let label = widget.props.label.clone();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.add_sized(size, egui::Button::new(&label));
            });
        }
        WidgetKind::CommandLinkButton => {
            let title = widget.props.label.clone();
            let desc = widget.props.placeholder.clone();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.add_sized(size, egui::Button::new(format!("{title}\n{desc}")));
            });
        }
        WidgetKind::DialogButtonBox => {
            let opts = widget.props.options.clone();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.horizontal(|ui| {
                    for o in &opts {
                        let _ = ui.button(o);
                    }
                });
            });
        }
        WidgetKind::MathLabel => {
            let val = state.values.get(binding).and_then(|v| {
                if let PreviewValue::Float(f) = v {
                    Some(*f)
                } else {
                    None
                }
            });
            let label = widget.props.label.clone();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.label(format!("{label} = {:.2}", val.unwrap_or(0.0)));
            });
        }
        WidgetKind::FilePicker => {
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.horizontal(|ui| {
                    let _ = ui.button("Browse…");
                    let path = state.values.get(binding).and_then(|v| {
                        if let PreviewValue::Str(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    });
                    ui.label(path.unwrap_or_else(|| "(no file)".to_owned()));
                });
            });
        }
        WidgetKind::ListView => {
            let opts = widget.props.options.clone();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(("preview_list", widget.id))
                    .show(ui, |ui| {
                        for o in &opts {
                            ui.label(o);
                        }
                    });
            });
        }
        WidgetKind::Chart
        | WidgetKind::Table
        | WidgetKind::TreeView
        | WidgetKind::StackedWidget
        | WidgetKind::ToolBox => {
            let tag = match &widget.kind {
                WidgetKind::Chart => "chart",
                WidgetKind::Table => "table",
                WidgetKind::TreeView => "tree",
                WidgetKind::StackedWidget => "stack",
                _ => "toolbox",
            };
            placeholder_box(ui, w_rect, tag);
        }
        WidgetKind::Image | WidgetKind::Custom(_) => {
            let tag = match &widget.kind {
                WidgetKind::Image => "img",
                _ => "cst",
            };
            placeholder_box(ui, w_rect, tag);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn placeholder_box(ui: &mut egui::Ui, rect: egui::Rect, tag: &str) {
    ui.painter()
        .rect_filled(rect, 2.0, egui::Color32::from_gray(35));
    ui.painter().rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        tag,
        egui::FontId::monospace(9.0),
        egui::Color32::from_gray(100),
    );
}
