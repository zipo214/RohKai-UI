use crate::codegen::rust::{field_binding, string_literal};
use crate::project::schema::{WidgetInstance, WidgetKind};
use crate::project::ui_tree::UiTree;
use std::collections::HashSet;
use uuid::Uuid;

/// Returns (widget_id_or_none, code_line) for every line in the generated body.
/// Preamble/closing lines have `None` as the id.
pub fn emit_indexed(tree: &UiTree) -> Vec<(Option<Uuid>, String)> {
    let mut lines: Vec<(Option<Uuid>, String)> = Vec::new();

    lines.push((
        None,
        "egui::CentralPanel::default().show(ctx, |_ui| {});".to_owned(),
    ));

    // Children are emitted inside their parent Frame — skip them in the top-level loop
    let child_ids: HashSet<Uuid> = tree
        .widgets
        .iter()
        .flat_map(|w| w.children.iter().copied())
        .collect();

    for w in &tree.widgets {
        if child_ids.contains(&w.id) {
            continue;
        }

        let area_id = string_literal(&format!("widget_{}", w.id));
        lines.push((
            Some(w.id),
            format!("egui::Area::new(egui::Id::new({area_id}))"),
        ));
        lines.push((
            Some(w.id),
            format!(
                "    .fixed_pos(egui::pos2({:.1}, {:.1}))",
                w.rect.x, w.rect.y
            ),
        ));
        lines.push((Some(w.id), "    .show(ctx, |ui| {".to_owned()));
        lines.push((
            Some(w.id),
            format!(
                "        ui.set_min_size(egui::vec2({:.1}, {:.1}));",
                w.rect.w, w.rect.h
            ),
        ));

        // enabled = false → ui.set_enabled(false)
        if w.enabled == Some(false) {
            lines.push((Some(w.id), "        ui.set_enabled(false);".to_owned()));
        }

        let binding = field_binding(w.state_binding.as_deref());
        // Bound label mode: label_binding overrides the static label literal
        let label = if let Some(ref lb) = w.label_binding {
            if let Some(b) = field_binding(Some(lb.as_str())) {
                format!("&self.{b}")
            } else {
                string_literal(&w.props.label)
            }
        } else {
            string_literal(&w.props.label)
        };
        let label_lit = string_literal(&w.props.label); // always the static version

        let tip = w.tooltip.as_deref().map(string_literal);
        // fg_color → emitted as RichText/Label color override where the API allows it
        let fg_color_expr = w
            .fg_color
            .map(|c| format!("egui::Color32::from_rgb({}, {}, {})", c[0], c[1], c[2]));

        match &w.kind {
            WidgetKind::Frame => {
                lines.push((
                    Some(w.id),
                    "        egui::Frame::group(ui.style()).show(ui, |ui| {".to_owned(),
                ));
                lines.push((
                    Some(w.id),
                    format!(
                        "            ui.set_min_size(egui::vec2({:.1}, {:.1}));",
                        w.rect.w, w.rect.h
                    ),
                ));
                for &child_id in &w.children {
                    if let Some(child) = tree.widgets.iter().find(|cw| cw.id == child_id) {
                        emit_child_lines(child, w, &mut lines);
                    }
                }
                lines.push((Some(w.id), "        });".to_owned()));
            }
            WidgetKind::Button => {
                let rounding_chain = w
                    .corner_radius
                    .filter(|&r| r > 0.0)
                    .map(|r| format!(".rounding(egui::Rounding::same({r:.1}))",))
                    .unwrap_or_default();
                let base = format!(
                    "ui.add_sized([{:.1}, {:.1}], egui::Button::new({label_lit}){rounding_chain})",
                    w.rect.w, w.rect.h
                );
                let with_tip = append_tip(base, tip.as_deref());
                let line = if let Some(ref h) = w.event_handler {
                    format!(
                        "        if {with_tip}.clicked() {{\n            Self::{h}(&mut self.state);\n        }}"
                    )
                } else {
                    format!("        if {with_tip}.clicked() {{}}")
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::Label => {
                let text_expr = if w.label_binding.is_some() {
                    label.clone() // &self.binding
                } else if let Some(ref col) = fg_color_expr {
                    // Static label with color override via RichText
                    format!("egui::RichText::new({label_lit}).color({col})")
                } else {
                    label_lit.clone()
                };
                let base = format!("ui.label({text_expr})");
                let line = format!("        {};", append_tip(base, tip.as_deref()));
                lines.push((Some(w.id), line));
            }
            WidgetKind::TextInput => {
                let line = match binding {
                    Some(b) => {
                        let base = format!(
                            "ui.add_sized([{:.1}, {:.1}], egui::TextEdit::singleline(&mut self.{b}))",
                            w.rect.w, w.rect.h
                        );
                        let with_tip = append_tip(base, tip.as_deref());
                        let with_handler = if let Some(ref h) = w.event_handler {
                            format!("if {with_tip}.changed() {{\n            Self::{h}(&mut self.state);\n        }}")
                        } else {
                            format!("{with_tip};")
                        };
                        format!("        {with_handler}")
                    }
                    None => format!("        // TextInput {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::Slider => {
                let line = match binding {
                    Some(b) => {
                        let base = format!(
                            "ui.add_sized([{:.1}, {:.1}], egui::Slider::new(&mut self.{b}, {:.1}..={:.1}).text({label_lit}))",
                            w.rect.w, w.rect.h, w.props.min, w.props.max
                        );
                        let with_tip = append_tip(base, tip.as_deref());
                        let with_handler = if let Some(ref h) = w.event_handler {
                            format!("if {with_tip}.changed() {{\n            Self::{h}(&mut self.state);\n        }}")
                        } else {
                            format!("{with_tip};")
                        };
                        format!("        {with_handler}")
                    }
                    None => format!("        // Slider {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::Checkbox => {
                let line = match binding {
                    Some(b) => {
                        let base = format!(
                            "ui.add_sized([{:.1}, {:.1}], egui::Checkbox::new(&mut self.{b}, {label_lit}))",
                            w.rect.w, w.rect.h
                        );
                        let with_tip = append_tip(base, tip.as_deref());
                        let with_handler = if let Some(ref h) = w.event_handler {
                            format!("if {with_tip}.changed() {{\n            Self::{h}(&mut self.state);\n        }}")
                        } else {
                            format!("{with_tip};")
                        };
                        format!("        {with_handler}")
                    }
                    None => format!("        // Checkbox {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::ComboBox => {
                let line = match binding {
                    Some(b) => {
                        let base = format!(
                            "egui::ComboBox::from_label({label_lit}).selected_text(self.{b}.as_str()).width({:.1}).show_ui(ui, |_ui| {{}})",
                            w.rect.w
                        );
                        let with_tip = append_tip(base, tip.as_deref());
                        let with_handler = if let Some(ref h) = w.event_handler {
                            format!("if {with_tip}.response.changed() {{\n            Self::{h}(&mut self.state);\n        }}")
                        } else {
                            format!("{with_tip}.response;")
                        };
                        format!("        {with_handler}")
                    }
                    None => format!("        // ComboBox {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::RadioButton => {
                let line = match binding {
                    Some(b) => {
                        let base = format!(
                            "ui.radio_value(&mut self.{b}, {label_lit}.to_owned(), {label_lit})"
                        );
                        let with_tip = append_tip(base, tip.as_deref());
                        format!("        {with_tip};")
                    }
                    None => format!("        // RadioButton {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::ProgressBar => {
                let line = match binding {
                    Some(b) => {
                        let base = format!(
                            "ui.add_sized([{:.1}, {:.1}], egui::ProgressBar::new(self.{b}).text({label_lit}))",
                            w.rect.w, w.rect.h
                        );
                        let with_tip = append_tip(base, tip.as_deref());
                        format!("        {with_tip};")
                    }
                    None => format!("        // ProgressBar {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
        }

        lines.push((Some(w.id), "    });".to_owned()));
    }
    lines
}

/// Chain `.on_hover_text(tip)` after `expr` when tip is set.
fn append_tip(expr: String, tip: Option<&str>) -> String {
    match tip {
        Some(t) => format!("{expr}.on_hover_text({t})"),
        None => expr,
    }
}

/// Emit a single child widget using ui.put() with relative position inside its parent Frame.
fn emit_child_lines(
    child: &WidgetInstance,
    parent: &WidgetInstance,
    lines: &mut Vec<(Option<Uuid>, String)>,
) {
    let rel_x = (child.rect.x - parent.rect.x).max(0.0);
    let rel_y = (child.rect.y - parent.rect.y).max(0.0);
    let rect_expr = format!(
        "egui::Rect::from_min_size(ui.min_rect().min + egui::vec2({rel_x:.1}, {rel_y:.1}), egui::vec2({:.1}, {:.1}))",
        child.rect.w, child.rect.h
    );
    let label = string_literal(&child.props.label);
    let binding = field_binding(child.state_binding.as_deref());

    lines.push((
        Some(child.id),
        format!("            // widget_{}", child.id),
    ));

    let line = match &child.kind {
        WidgetKind::Button => format!(
            "            if ui.put({rect_expr}, egui::Button::new({label})).clicked() {{}}"
        ),
        WidgetKind::Label => match binding {
            Some(b) => format!("            ui.put({rect_expr}, egui::Label::new(&self.{b}));"),
            None => format!("            ui.put({rect_expr}, egui::Label::new({label}));"),
        },
        WidgetKind::TextInput => match binding {
            Some(b) => format!(
                "            ui.put({rect_expr}, egui::TextEdit::singleline(&mut self.{b}));"
            ),
            None => format!("            // TextInput {label}: set a valid Binding"),
        },
        WidgetKind::Slider => match binding {
            Some(b) => format!(
                "            ui.put({rect_expr}, egui::Slider::new(&mut self.{b}, {:.1}..={:.1}).text({label}));",
                child.props.min, child.props.max
            ),
            None => format!("            // Slider {label}: set a valid Binding"),
        },
        WidgetKind::Checkbox => match binding {
            Some(b) => format!(
                "            ui.put({rect_expr}, egui::Checkbox::new(&mut self.{b}, {label}));"
            ),
            None => format!("            // Checkbox {label}: set a valid Binding"),
        },
        WidgetKind::Frame => format!("            // Nested Frame {label} — grouping not recursive in codegen"),
        WidgetKind::ComboBox => match binding {
            Some(b) => format!(
                "            ui.put({rect_expr}, egui::Label::new(self.{b}.as_str())); // ComboBox"
            ),
            None => format!("            // ComboBox {label}: set a valid Binding"),
        },
        WidgetKind::RadioButton => match binding {
            Some(b) => format!(
                "            ui.radio_value(&mut self.{b}, {label}.to_owned(), {label});"
            ),
            None => format!("            // RadioButton {label}: set a valid Binding"),
        },
        WidgetKind::ProgressBar => match binding {
            Some(b) => format!(
                "            ui.put({rect_expr}, egui::ProgressBar::new(self.{b}).text({label}));"
            ),
            None => format!("            // ProgressBar {label}: set a valid Binding"),
        },
    };
    lines.push((Some(child.id), line));
}
