use crate::codegen::rust::{field_binding, string_literal};
use crate::codegen::source_map::{GeneratedCodeDocument, SourceSpan, WidgetSourceSpan};
use crate::project::schema::{Orientation, WidgetInstance, WidgetKind};
use crate::project::ui_tree::UiTree;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const MAX_GRID_COLUMNS: usize = 12;

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

        let eff = w
            .state_binding
            .as_deref()
            .map(crate::codegen::rust::effective_binding);
        let binding = field_binding(eff.as_deref());
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
        let fg_color_expr = w
            .fg_color
            .map(|c| format!("egui::Color32::from_rgb({}, {}, {})", c[0], c[1], c[2]));

        match &w.kind {
            WidgetKind::Frame => {
                // Build frame style from new fields
                let inner_m = w.props.inner_margin;
                let stroke_w = w.props.stroke_width;
                let stroke_col = w
                    .props
                    .stroke_color
                    .map(|c| format!("egui::Color32::from_rgb({}, {}, {})", c[0], c[1], c[2]))
                    .unwrap_or_else(|| "egui::Color32::from_gray(100)".to_owned());
                let mut frame_expr = format!(
                    "egui::Frame::none()\n            .inner_margin({inner_m:.1})\n            .stroke(egui::Stroke::new({stroke_w:.1}, {stroke_col}))"
                );
                if let Some(c) = w.bg_color {
                    frame_expr.push_str(&format!(
                        "\n            .fill(egui::Color32::from_rgb({}, {}, {}))",
                        c[0], c[1], c[2]
                    ));
                }
                if let Some(r) = w.corner_radius.filter(|&r| r > 0.0) {
                    frame_expr.push_str(&format!(
                        "\n            .rounding(egui::Rounding::same({r:.1}))"
                    ));
                }
                lines.push((Some(w.id), format!("        {frame_expr}.show(ui, |ui| {{")));
                lines.push((
                    Some(w.id),
                    format!(
                        "            ui.set_min_size(egui::vec2({:.1}, {:.1}));",
                        w.rect.w, w.rect.h
                    ),
                ));
                for &child_id in &w.children {
                    if let Some(child) = tree.widgets.iter().find(|cw| cw.id == child_id) {
                        emit_child_lines(child, w, &mut lines, 0);
                    }
                }
                lines.push((Some(w.id), "        });".to_owned()));
            }
            WidgetKind::Button => {
                let rounding_chain = w
                    .corner_radius
                    .filter(|&r| r > 0.0)
                    .map(|r| format!(".rounding(egui::Rounding::same({r:.1}))"))
                    .unwrap_or_default();
                let fill_chain = w
                    .bg_color
                    .map(|c| {
                        format!(
                            ".fill(egui::Color32::from_rgb({}, {}, {}))",
                            c[0], c[1], c[2]
                        )
                    })
                    .unwrap_or_default();
                let label_expr = rich_text_expr(&label_lit, w.font_size, fg_color_expr.as_deref());
                let base = format!(
                    "ui.add_sized([{:.1}, {:.1}], egui::Button::new({label_expr}){rounding_chain}{fill_chain})",
                    w.rect.w, w.rect.h
                );
                let with_tip = append_tip(base, tip.as_deref());
                let mut line = if let Some(h) = resolve_handler_click(w) {
                    format!(
                        "        let _btn_{id} = {with_tip};\n        if _btn_{id}.clicked() {{\n            self.{h}();\n        }}",
                        id = w.id.as_simple()
                    )
                } else {
                    format!("        let _btn_{id} = {with_tip};", id = w.id.as_simple())
                };
                if !w.on_double_click.is_empty() {
                    let h = &w.on_double_click;
                    let id = w.id.as_simple();
                    line.push_str(&format!(
                        "\n        if _btn_{id}.double_clicked() {{\n            self.{h}();\n        }}"
                    ));
                }
                lines.push((Some(w.id), line));
            }
            WidgetKind::Label => {
                let text_expr = if w.label_binding.is_some() {
                    label.clone()
                } else {
                    rich_text_expr(&label_lit, w.font_size, fg_color_expr.as_deref())
                };
                let mut lbl = format!("egui::Label::new({text_expr})");
                if let Some(wrap) = w.props.text_wrap {
                    if wrap {
                        lbl.push_str(".wrap()");
                    } else {
                        lbl.push_str(".extend()");
                    }
                }
                let base = format!("ui.add({lbl})");
                let line = format!("        {};", append_tip(base, tip.as_deref()));
                lines.push((Some(w.id), line));
            }
            WidgetKind::TextInput => {
                let line = match binding {
                    Some(b) => {
                        let mut te = format!("egui::TextEdit::singleline(&mut self.{b})");
                        if !w.props.placeholder.is_empty() {
                            te.push_str(&format!(
                                ".hint_text({})",
                                string_literal(&w.props.placeholder)
                            ));
                        }
                        if w.props.password_mode {
                            te.push_str(".password(true)");
                        }
                        let sized =
                            format!("ui.add_sized([{:.1}, {:.1}], {te})", w.rect.w, w.rect.h);
                        let with_tip = append_tip(sized, tip.as_deref());
                        let id = w.id.as_simple();
                        let mut parts = format!("        let _ti_{id} = {with_tip};");
                        if let Some(h) = resolve_handler_change(w) {
                            parts.push_str(&format!(
                                "\n        if _ti_{id}.changed() {{\n            self.{h}();\n        }}"
                            ));
                        }
                        if !w.on_lost_focus.is_empty() {
                            let h = &w.on_lost_focus;
                            parts.push_str(&format!(
                                "\n        if _ti_{id}.lost_focus() {{\n            self.{h}();\n        }}"
                            ));
                        }
                        parts
                    }
                    None => format!("        // TextInput {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::Slider => {
                let line = match binding {
                    Some(b) => {
                        let mut slider = format!(
                            "egui::Slider::new(&mut self.{b}, {:.1}..={:.1}).text({label_lit})",
                            w.props.min, w.props.max
                        );
                        if let Some(step) = w.props.step {
                            slider.push_str(&format!(".step_by({step} as f64)"));
                        }
                        if !w.props.show_value {
                            slider.push_str(".show_value(false)");
                        }
                        if w.props.orientation == Orientation::Vertical {
                            slider.push_str(".vertical()");
                        }
                        let sized =
                            format!("ui.add_sized([{:.1}, {:.1}], {slider})", w.rect.w, w.rect.h);
                        let with_tip = append_tip(sized, tip.as_deref());
                        let id = w.id.as_simple();
                        let mut parts = format!("        let _sl_{id} = {with_tip};");
                        if let Some(h) = resolve_handler_change(w) {
                            parts.push_str(&format!(
                                "\n        if _sl_{id}.changed() {{\n            self.{h}();\n        }}"
                            ));
                        }
                        if !w.on_drag_stopped.is_empty() {
                            let h = &w.on_drag_stopped;
                            parts.push_str(&format!(
                                "\n        if _sl_{id}.drag_stopped() {{\n            self.{h}();\n        }}"
                            ));
                        }
                        parts
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
                        let with_handler = if let Some(h) = resolve_handler_change(w) {
                            format!(
                                "if {with_tip}.changed() {{\n            self.{h}();\n        }}"
                            )
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
                        let options = combo_option_values(w);
                        let selected_expr =
                            combo_selected_text_expr(&format!("self.{b}"), &options);
                        let mut base = format!(
                            "let combo_resp = egui::ComboBox::from_label({label_lit})\n            .selected_text({selected_expr})\n            .width({:.1})\n            .show_ui(ui, |ui| {{\n",
                            w.rect.w
                        );
                        for option in options {
                            let option_lit = string_literal(&option);
                            base.push_str(&format!(
                                "                ui.selectable_value(&mut self.{b}, {option_lit}.to_owned(), {option_lit});\n"
                            ));
                        }
                        base.push_str("            });\n");
                        let handler = resolve_handler_change(w);
                        let uses_response = tip.is_some() || handler.is_some();
                        if uses_response {
                            base.push_str("        let combo_response = combo_resp.response;\n");
                        }
                        if handler.is_some() {
                            base.push_str(
                                "        let combo_changed = combo_response.changed();\n",
                            );
                        }
                        if let Some(tip) = tip.as_deref() {
                            base.push_str(&format!(
                                "        combo_response.on_hover_text({tip});\n"
                            ));
                        }
                        if let Some(h) = handler {
                            base.push_str(&format!(
                                "        if combo_changed {{\n            self.{h}();\n        }}"
                            ));
                        } else if !uses_response {
                            base.push_str("        let _ = combo_resp;");
                        }
                        format!("        {base}")
                    }
                    None => format!("        // ComboBox {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::RadioButton => {
                let line = match binding {
                    Some(b) => {
                        // radio_value is the alternative value this button represents
                        let value_lit = if w.props.radio_value.is_empty() {
                            label_lit.clone()
                        } else {
                            string_literal(&w.props.radio_value)
                        };
                        let base = format!(
                            "ui.radio_value(&mut self.{b}, {value_lit}.to_owned(), {label_lit})"
                        );
                        let with_tip = append_tip(base, tip.as_deref());
                        let line = if let Some(h) = resolve_handler_change(w) {
                            format!(
                                "if {with_tip}.clicked() {{\n            self.{h}();\n        }}"
                            )
                        } else {
                            format!("{with_tip};")
                        };
                        format!("        {line}")
                    }
                    None => format!("        // RadioButton {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::ProgressBar => {
                let line = match binding {
                    Some(b) => {
                        let mut pb = format!("egui::ProgressBar::new(self.{b})");
                        if w.props.show_percentage {
                            pb.push_str(".show_percentage()");
                        }
                        if w.props.animated {
                            pb.push_str(".animate(true)");
                        }
                        if let Some(c) = w.fg_color {
                            pb.push_str(&format!(
                                ".fill(egui::Color32::from_rgb({}, {}, {}))",
                                c[0], c[1], c[2]
                            ));
                        }
                        let sized =
                            format!("ui.add_sized([{:.1}, {:.1}], {pb})", w.rect.w, w.rect.h);
                        let with_tip = append_tip(sized, tip.as_deref());
                        format!("        {with_tip};")
                    }
                    None => format!("        // ProgressBar {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::TextArea => {
                let line = match binding {
                    Some(b) => {
                        let mut te = format!("egui::TextEdit::multiline(&mut self.{b})");
                        if !w.props.placeholder.is_empty() {
                            te.push_str(&format!(
                                ".hint_text({})",
                                string_literal(&w.props.placeholder)
                            ));
                        }
                        if w.props.text_wrap == Some(false) {
                            te.push_str(".desired_rows(1)"); // no-wrap hint
                        }
                        let sized =
                            format!("ui.add_sized([{:.1}, {:.1}], {te})", w.rect.w, w.rect.h);
                        format!("        {};", append_tip(sized, tip.as_deref()))
                    }
                    None => format!("        // TextArea {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::SpinBox => {
                let line = match binding {
                    Some(b) => {
                        let dv = format!(
                            "egui::DragValue::new(&mut self.{b}).range({:.1}..={:.1})",
                            w.props.min, w.props.max
                        );
                        let sized = format!("ui.add({dv})");
                        let with_tip = append_tip(sized, tip.as_deref());
                        let with_handler = if let Some(h) = resolve_handler_change(w) {
                            format!(
                                "if {with_tip}.changed() {{\n            self.{h}();\n        }}"
                            )
                        } else {
                            format!("{with_tip};")
                        };
                        format!("        {with_handler}")
                    }
                    None => format!("        // SpinBox {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::FontComboBox => {
                let line = match binding {
                    Some(b) => {
                        let line = format!(
                            "        egui::ComboBox::from_id_salt(\"{b}\")\n            \
                            .selected_text(&self.{b})\n            \
                            .show_ui(ui, |ui| {{\n                \
                            for font in [\"Proportional\", \"Monospace\"] {{\n                    \
                            ui.selectable_value(&mut self.{b}, font.to_owned(), font);\n                \
                            }}\n            }});"
                        );
                        line
                    }
                    None => format!("        // FontComboBox {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::HorizontalSpacer => {
                lines.push((
                    Some(w.id),
                    format!("        ui.add_space({:.1});", w.rect.w),
                ));
            }
            WidgetKind::VerticalSpacer => {
                lines.push((
                    Some(w.id),
                    format!("        ui.add_space({:.1});", w.rect.h),
                ));
            }
            WidgetKind::GroupBox => {
                let lbl = string_literal(&w.props.label);
                lines.push((
                    Some(w.id),
                    format!(
                        "        egui::Frame::group(ui.style()).show(ui, |ui| {{\n            ui.label({lbl});\n        }});"
                    ),
                ));
            }
            WidgetKind::VLayout => {
                lines.push((Some(w.id), "        ui.vertical(|ui| {".to_owned()));
                for &child_id in &w.children {
                    if let Some(child) = tree.widgets.iter().find(|cw| cw.id == child_id) {
                        emit_layout_child_lines(child, &mut lines);
                    }
                }
                lines.push((Some(w.id), "        });".to_owned()));
            }
            WidgetKind::HLayout => {
                lines.push((Some(w.id), "        ui.horizontal(|ui| {".to_owned()));
                for &child_id in &w.children {
                    if let Some(child) = tree.widgets.iter().find(|cw| cw.id == child_id) {
                        emit_layout_child_lines(child, &mut lines);
                    }
                }
                lines.push((Some(w.id), "        });".to_owned()));
            }
            WidgetKind::ScrollArea => {
                lines.push((
                    Some(w.id),
                    "        egui::ScrollArea::vertical().show(ui, |_ui| {});".to_owned(),
                ));
            }
            WidgetKind::GridLayout => {
                let columns = w.props.grid_columns.clamp(1, MAX_GRID_COLUMNS);
                lines.push((
                    Some(w.id),
                    format!(
                        "        egui::Grid::new(\"{}\").show(ui, |ui| {{",
                        w.id.as_simple()
                    ),
                ));
                for (idx, &child_id) in w.children.iter().enumerate() {
                    if let Some(child) = tree.widgets.iter().find(|cw| cw.id == child_id) {
                        emit_layout_child_lines(child, &mut lines);
                        if (idx + 1) % columns == 0 {
                            lines.push((Some(w.id), "            ui.end_row();".to_owned()));
                        }
                    }
                }
                if !w.children.is_empty() && w.children.len() % columns != 0 {
                    lines.push((Some(w.id), "            ui.end_row();".to_owned()));
                }
                lines.push((Some(w.id), "        });".to_owned()));
            }
            WidgetKind::TabWidget => {
                let tabs = w.props.options.to_vec();
                let mut tab_lines = format!(
                    "        egui::TopBottomPanel::top(\"{}_tabs\").show_inside(ui, |ui| {{\n",
                    w.id.as_simple()
                );
                for tab in &tabs {
                    tab_lines.push_str(&format!(
                        "            ui.selectable_label(false, {});\n",
                        crate::codegen::rust::string_literal(tab)
                    ));
                }
                tab_lines.push_str("        });");
                lines.push((Some(w.id), tab_lines));
            }
            WidgetKind::ToolButton => {
                let lbl = rich_text_expr(&label_lit, w.font_size, fg_color_expr.as_deref());
                let base = format!("ui.small_button({lbl})");
                let with_tip = append_tip(base, tip.as_deref());
                let line = if let Some(h) = resolve_handler_click(w) {
                    format!(
                        "        if {with_tip}.clicked() {{\n            self.{h}();\n        }}"
                    )
                } else {
                    format!("        if {with_tip}.clicked() {{}}")
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::CommandLinkButton => {
                let title = string_literal(&w.props.label);
                let desc = string_literal(&w.props.placeholder);
                let base = format!(
                    "ui.add_sized([{:.1}, {:.1}], egui::Button::new(format!(\"{{}}\\n{{}}\", {title}, {desc})))",
                    w.rect.w, w.rect.h
                );
                let with_tip = append_tip(base, tip.as_deref());
                let line = if let Some(h) = resolve_handler_click(w) {
                    format!(
                        "        if {with_tip}.clicked() {{\n            self.{h}();\n        }}"
                    )
                } else {
                    format!("        if {with_tip}.clicked() {{}}")
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::DialogButtonBox => {
                let mut s = String::from("        ui.horizontal(|ui| {\n");
                for opt in &w.props.options {
                    s.push_str(&format!(
                        "            if ui.button({}).clicked() {{}}\n",
                        string_literal(opt)
                    ));
                }
                s.push_str("        });");
                lines.push((Some(w.id), s));
            }
            WidgetKind::MathLabel => {
                let line = match binding {
                    Some(b) => {
                        let label_lit = string_literal(&w.props.label);
                        format!(
                            "        ui.label(format!(\"{{}} = {{:.2}}\", {label_lit}, self.{b}));"
                        )
                    }
                    None => format!("        // MathLabel {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::FilePicker => {
                let line = match binding {
                    Some(b) => format!(
                        "        if ui.button(\"Browse…\").clicked() {{\n            \
                        if let Some(p) = rfd::FileDialog::new().pick_file() {{\n                \
                        self.{b} = p.display().to_string();\n            }}\n        }}\n        \
                        ui.label(&self.{b});"
                    ),
                    None => format!("        // FilePicker {label_lit}: set a valid Binding"),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::Chart => {
                let line = match binding {
                    Some(b) => chart_preview_block(&format!("self.{b}"), w.rect.w, w.rect.h, 8),
                    None => format!(
                        "        // Chart {label_lit}: set a Vec<f32> Binding for painter output"
                    ),
                };
                lines.push((Some(w.id), line));
            }
            WidgetKind::Table => {
                let mut s = format!(
                    "        egui::Grid::new(\"{}\").striped(true).show(ui, |ui| {{\n",
                    w.id.as_simple()
                );
                for col in &w.props.options {
                    s.push_str(&format!("            ui.label({});\n", string_literal(col)));
                }
                s.push_str("            ui.end_row();\n        });");
                lines.push((Some(w.id), s));
            }
            WidgetKind::ListView => {
                let mut s = format!(
                    "        egui::ScrollArea::vertical().id_salt(\"{}\").show(ui, |ui| {{\n",
                    w.id.as_simple()
                );
                for item in &w.props.options {
                    s.push_str(&format!(
                        "            ui.label({});\n",
                        string_literal(item)
                    ));
                }
                s.push_str("        });");
                lines.push((Some(w.id), s));
            }
            WidgetKind::TreeView => {
                let mut s = String::new();
                let root = w
                    .props
                    .options
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Root".into());
                s.push_str(&format!(
                    "        egui::CollapsingHeader::new({}).default_open(true).show(ui, |ui| {{\n",
                    string_literal(&root)
                ));
                for child in w.props.options.iter().skip(1) {
                    s.push_str(&format!(
                        "            ui.label({});\n",
                        string_literal(child)
                    ));
                }
                s.push_str("        });");
                lines.push((Some(w.id), s));
            }
            WidgetKind::StackedWidget => {
                lines.push((
                    Some(w.id),
                    "        ui.group(|_ui| {}); // StackedWidget: show active page".to_owned(),
                ));
            }
            WidgetKind::ToolBox => {
                let mut s = String::new();
                for sec in &w.props.options {
                    s.push_str(&format!(
                        "        egui::CollapsingHeader::new({}).show(ui, |_ui| {{}});\n",
                        string_literal(sec)
                    ));
                }
                if s.ends_with('\n') {
                    s.pop();
                }
                lines.push((Some(w.id), s));
            }
            WidgetKind::Image => {
                lines.push((Some(w.id), image_preview_line(w, 8)));
            }
            WidgetKind::Custom(_) => {
                let line = if let Some(ref tpl) = w.descriptor_live_tpl {
                    crate::codegen::widget_descriptor::apply_template(
                        tpl,
                        w,
                        w.descriptor_name.as_deref().unwrap_or("Custom"),
                    )
                } else {
                    format!(
                        "        // Custom widget {:?}: descriptor not loaded",
                        w.kind
                    )
                };
                lines.push((Some(w.id), line));
            }
        }

        lines.push((Some(w.id), "    });".to_owned()));
    }

    // Design-time non-visual components (timers etc.) — emitted as update() comments.
    for line in crate::codegen::component_state::component_update_lines(&tree.app_props.components)
    {
        lines.push((None, line));
    }

    lines
}

pub fn emit_document(tree: &UiTree) -> GeneratedCodeDocument {
    document_from_indexed(emit_indexed(tree))
}

fn document_from_indexed(lines: Vec<(Option<Uuid>, String)>) -> GeneratedCodeDocument {
    let mut document = GeneratedCodeDocument::default();
    let mut widget_indices: HashMap<Uuid, usize> = HashMap::new();
    let mut byte_cursor = 0usize;
    let mut line_cursor = 1usize;

    for (index, (widget_id, fragment)) in lines.into_iter().enumerate() {
        if index > 0 {
            document.text.push('\n');
            byte_cursor += 1;
            line_cursor += 1;
            if fragment
                .trim_start()
                .starts_with("egui::Area::new(egui::Id::new(")
            {
                document.text.push('\n');
                byte_cursor += 1;
                line_cursor += 1;
            }
        }

        let byte_start = byte_cursor;
        let line_start = line_cursor;
        document.text.push_str(&fragment);
        byte_cursor += fragment.len();
        let embedded_newlines = fragment.bytes().filter(|byte| *byte == b'\n').count();
        let line_end = line_start + embedded_newlines;
        line_cursor = line_end;

        if let Some(widget_id) = widget_id {
            if let Some(existing) = widget_indices.get(&widget_id).copied() {
                document.widget_spans[existing]
                    .span
                    .extend_to(byte_cursor, line_end);
            } else {
                let entry_index = document.widget_spans.len();
                widget_indices.insert(widget_id, entry_index);
                document.widget_spans.push(WidgetSourceSpan {
                    widget_id,
                    span: SourceSpan::new(byte_start..byte_cursor, line_start..=line_end),
                });
            }
        }
    }

    document
}

// ---------------------------------------------------------------------------
// Handler resolution — new fields with legacy fallback
// ---------------------------------------------------------------------------

fn resolve_handler_click(w: &WidgetInstance) -> Option<&str> {
    if !w.on_click.is_empty() {
        return Some(w.on_click.as_str());
    }
    w.event_handler.as_deref().filter(|s| !s.is_empty())
}

fn resolve_handler_change(w: &WidgetInstance) -> Option<&str> {
    if !w.on_change.is_empty() {
        return Some(w.on_change.as_str());
    }
    w.event_handler.as_deref().filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// RichText builder — applies font_size and color if set
// ---------------------------------------------------------------------------

fn rich_text_expr(label_lit: &str, font_size: Option<f32>, fg_color: Option<&str>) -> String {
    match (font_size, fg_color) {
        (Some(size), Some(col)) => {
            format!("egui::RichText::new({label_lit}).size({size:.1}).color({col})")
        }
        (Some(size), None) => format!("egui::RichText::new({label_lit}).size({size:.1})"),
        (None, Some(col)) => format!("egui::RichText::new({label_lit}).color({col})"),
        (None, None) => label_lit.to_owned(),
    }
}

fn chart_preview_block(binding_expr: &str, width: f32, height: f32, indent: usize) -> String {
    let pad = " ".repeat(indent);
    format!(
        "{pad}let chart_size = egui::vec2({width:.1}, {height:.1});\n\
{pad}let (chart_rect, _) = ui.allocate_exact_size(chart_size, egui::Sense::hover());\n\
{pad}let chart_painter = ui.painter_at(chart_rect);\n\
{pad}chart_painter.rect_stroke(chart_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::from_gray(120)));\n\
{pad}let chart_values = &{binding_expr};\n\
{pad}if !chart_values.is_empty() {{\n\
{pad}    let chart_max = chart_values.iter().copied().fold(0.0_f32, f32::max).max(1.0);\n\
{pad}    let bar_w = chart_rect.width() / chart_values.len() as f32;\n\
{pad}    for (i, value) in chart_values.iter().enumerate() {{\n\
{pad}        let v = (*value).max(0.0) / chart_max;\n\
{pad}        let x0 = chart_rect.left() + i as f32 * bar_w + 2.0;\n\
{pad}        let x1 = (x0 + bar_w - 4.0).max(x0 + 1.0);\n\
{pad}        let y1 = chart_rect.bottom() - 2.0;\n\
{pad}        let y0 = y1 - (chart_rect.height() - 4.0) * v;\n\
{pad}        chart_painter.rect_filled(\n\
{pad}            egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1)),\n\
{pad}            1.0,\n\
{pad}            egui::Color32::from_rgb(52, 211, 153),\n\
{pad}        );\n\
{pad}    }}\n\
{pad}}}"
    )
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

/// Chain `.on_hover_text(tip)` after `expr` when tip is set.
fn append_tip(expr: String, tip: Option<&str>) -> String {
    match tip {
        Some(t) => format!("{expr}.on_hover_text({t})"),
        None => expr,
    }
}

fn combo_option_values(widget: &WidgetInstance) -> Vec<String> {
    let options: Vec<String> = widget
        .props
        .options
        .iter()
        .filter_map(|option| {
            let trimmed = option.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        })
        .collect();

    if options.is_empty() {
        vec![widget.props.label.clone()]
    } else {
        options
    }
}

fn combo_selected_text_expr(state_expr: &str, options: &[String]) -> String {
    let fallback = options.first().map(String::as_str).unwrap_or("Option A");
    let fallback_lit = string_literal(fallback);
    format!("if {state_expr}.is_empty() {{ {fallback_lit} }} else {{ {state_expr}.as_str() }}")
}

/// Emit a single child widget using ui.put() with relative position inside its parent Frame.
fn emit_child_lines(
    child: &WidgetInstance,
    parent: &WidgetInstance,
    lines: &mut Vec<(Option<Uuid>, String)>,
    depth: usize,
) {
    let indent = "    ".repeat(depth + 3);
    let rel_x = (child.rect.x - parent.rect.x).max(0.0);
    let rel_y = (child.rect.y - parent.rect.y).max(0.0);
    let rect_expr = format!(
        "egui::Rect::from_min_size(ui.min_rect().min + egui::vec2({rel_x:.1}, {rel_y:.1}), egui::vec2({:.1}, {:.1}))",
        child.rect.w, child.rect.h
    );
    let label = string_literal(&child.props.label);
    let binding = field_binding(child.state_binding.as_deref());

    lines.push((Some(child.id), format!("{indent}// widget_{}", child.id)));

    let line = match &child.kind {
        WidgetKind::Button => format!(
            "{indent}if ui.put({rect_expr}, egui::Button::new({label})).clicked() {{}}"
        ),
        WidgetKind::Label => match binding {
            Some(b) => format!("{indent}ui.put({rect_expr}, egui::Label::new(&self.{b}));"),
            None => format!("{indent}ui.put({rect_expr}, egui::Label::new({label}));"),
        },
        WidgetKind::TextInput => match binding {
            Some(b) => format!(
                "{indent}ui.put({rect_expr}, egui::TextEdit::singleline(&mut self.{b}));"
            ),
            None => format!("{indent}// TextInput {label}: set a valid Binding"),
        },
        WidgetKind::Slider => match binding {
            Some(b) => format!(
                "{indent}ui.put({rect_expr}, egui::Slider::new(&mut self.{b}, {:.1}..={:.1}).text({label}));",
                child.props.min, child.props.max
            ),
            None => format!("{indent}// Slider {label}: set a valid Binding"),
        },
        WidgetKind::Checkbox => match binding {
            Some(b) => format!(
                "{indent}ui.put({rect_expr}, egui::Checkbox::new(&mut self.{b}, {label}));"
            ),
            None => format!("{indent}// Checkbox {label}: set a valid Binding"),
        },
        WidgetKind::Frame => format!(
            "{indent}// Nested Frame {label} — grouping not recursive in codegen"
        ),
        WidgetKind::ComboBox => match binding {
            Some(b) => format!(
                "{indent}ui.put({rect_expr}, egui::Label::new(self.{b}.as_str())); // ComboBox"
            ),
            None => format!("{indent}// ComboBox {label}: set a valid Binding"),
        },
        WidgetKind::RadioButton => match binding {
            Some(b) => {
                let value_lit = if child.props.radio_value.is_empty() {
                    label.clone()
                } else {
                    string_literal(&child.props.radio_value)
                };
                format!(
                    "{indent}ui.radio_value(&mut self.{b}, {value_lit}.to_owned(), {label});"
                )
            }
            None => format!("{indent}// RadioButton {label}: set a valid Binding"),
        },
        WidgetKind::ProgressBar => match binding {
            Some(b) => {
                let mut pb = format!("egui::ProgressBar::new(self.{b})");
                if child.props.show_percentage {
                    pb.push_str(".show_percentage()");
                }
                format!("{indent}ui.put({rect_expr}, {pb});")
            }
            None => format!("{indent}// ProgressBar {label}: set a valid Binding"),
        },
        WidgetKind::TextArea => match binding {
            Some(b) => format!(
                "{indent}ui.put({rect_expr}, egui::TextEdit::multiline(&mut self.{b}));"
            ),
            None => format!("{indent}// TextArea {label}: set a valid Binding"),
        },
        WidgetKind::SpinBox => match binding {
            Some(b) => format!(
                "{indent}ui.put({rect_expr}, egui::DragValue::new(&mut self.{b}).range({:.1}..={:.1}));",
                child.props.min, child.props.max
            ),
            None => format!("{indent}// SpinBox {label}: set a valid Binding"),
        },
        WidgetKind::FontComboBox => match binding {
            Some(b) => format!(
                "{indent}ui.put({rect_expr}, egui::Label::new(self.{b}.as_str())); // FontComboBox"
            ),
            None => format!("{indent}// FontComboBox {label}: set a valid Binding"),
        },
        WidgetKind::HorizontalSpacer => {
            format!("{indent}ui.add_space({:.1}); // HorizontalSpacer", child.rect.w)
        }
        WidgetKind::VerticalSpacer => {
            format!("{indent}ui.add_space({:.1}); // VerticalSpacer", child.rect.h)
        }
        WidgetKind::GroupBox
        | WidgetKind::VLayout
        | WidgetKind::HLayout
        | WidgetKind::ScrollArea
        | WidgetKind::GridLayout
        | WidgetKind::TabWidget
        | WidgetKind::StackedWidget
        | WidgetKind::ToolBox
        | WidgetKind::Table
        | WidgetKind::ListView
        | WidgetKind::TreeView
        | WidgetKind::Chart => {
            format!("{indent}// Nested container {:?} — not expanded in child codegen", child.kind)
        }
        WidgetKind::ToolButton => {
            format!("{indent}if ui.put({rect_expr}, egui::Button::new({label}).small()).clicked() {{}}")
        }
        WidgetKind::CommandLinkButton => {
            format!("{indent}if ui.put({rect_expr}, egui::Button::new({label})).clicked() {{}}")
        }
        WidgetKind::DialogButtonBox => {
            format!("{indent}ui.put({rect_expr}, egui::Label::new({label})); // DialogButtonBox")
        }
        WidgetKind::MathLabel => match binding {
            Some(b) => format!(
                "{indent}ui.put({rect_expr}, egui::Label::new(format!(\"{{}} = {{:.2}}\", {label}, self.{b})));"
            ),
            None => format!("{indent}// MathLabel {label}: set a valid Binding"),
        },
        WidgetKind::FilePicker => match binding {
            Some(b) => format!(
                "{indent}ui.put({rect_expr}, egui::Label::new(&self.{b})); // FilePicker"
            ),
            None => format!("{indent}// FilePicker {label}: set a valid Binding"),
        },
        WidgetKind::Image => image_child_preview_line(child, &rect_expr),
        WidgetKind::Custom(_) => {
            if let Some(ref tpl) = child.descriptor_live_tpl {
                crate::codegen::widget_descriptor::apply_template(
                    tpl,
                    child,
                    child.descriptor_name.as_deref().unwrap_or("Custom"),
                )
            } else {
                format!("{indent}// Custom child {:?}: descriptor not loaded", child.kind)
            }
        }
    };
    lines.push((Some(child.id), line));
}

fn image_preview_line(widget: &WidgetInstance, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let key = string_literal(&format!("svg_{}", widget.id));
    let src_arg = svg_source_arg(widget.svg_source.as_deref(), widget.expand_svg_inline);
    format!(
        "{pad}self.show_svg_image(ui, {key}, {src_arg}, egui::vec2({:.1}, {:.1}));",
        widget.rect.w, widget.rect.h
    )
}

fn svg_source_arg(svg_source: Option<&str>, expand_inline: bool) -> String {
    match svg_source {
        Some(src) if expand_inline => {
            // Find a hash count that produces an unambiguous raw string literal.
            let hashes = (0usize..)
                .find(|&n| !src.contains(&format!("\"{})", "#".repeat(n))))
                .unwrap_or(0);
            let h = "#".repeat(hashes);
            format!("r{h}\"{src}\"{h}")
        }
        Some(src) => format!("\"[SVG: {} bytes]\"", src.len()),
        None => "\"[no SVG source]\"".to_owned(),
    }
}

fn image_child_preview_line(child: &WidgetInstance, rect_expr: &str) -> String {
    let key = string_literal(&format!("svg_{}", child.id));
    let src_arg = svg_source_arg(child.svg_source.as_deref(), child.expand_svg_inline);
    format!("            self.show_svg_image_at(ui, {rect_expr}, {key}, {src_arg});")
}

/// Emit a child owned by an egui layout container.
///
/// Unlike Frame children, layout children are not positioned with `ui.put`.
/// The layout closure owns their placement order.
fn emit_layout_child_lines(child: &WidgetInstance, lines: &mut Vec<(Option<Uuid>, String)>) {
    let label = string_literal(&child.props.label);
    let binding = field_binding(child.state_binding.as_deref());
    lines.push((
        Some(child.id),
        format!("            // widget_{}", child.id),
    ));
    let line = match &child.kind {
        WidgetKind::Button => {
            let id = child.id.as_simple();
            let mut s = format!(
                "            let _btn_{id} = ui.add_sized([{:.1}, {:.1}], egui::Button::new({label}));",
                child.rect.w, child.rect.h
            );
            if let Some(h) = resolve_handler_click(child) {
                s.push_str(&format!(
                    "\n            if _btn_{id}.clicked() {{\n                self.{h}();\n            }}"
                ));
            }
            if !child.on_double_click.is_empty() {
                let h = &child.on_double_click;
                s.push_str(&format!(
                    "\n            if _btn_{id}.double_clicked() {{\n                self.{h}();\n            }}"
                ));
            }
            s
        }
        WidgetKind::Label => match binding {
            Some(b) => format!("            ui.label(&self.{b});"),
            None => format!("            ui.label({label});"),
        },
        WidgetKind::TextInput => match binding {
            Some(b) => format!(
                "            ui.add_sized([{:.1}, {:.1}], egui::TextEdit::singleline(&mut self.{b}));",
                child.rect.w, child.rect.h
            ),
            None => format!("            // TextInput {label}: set a valid Binding"),
        },
        WidgetKind::TextArea => match binding {
            Some(b) => format!(
                "            ui.add_sized([{:.1}, {:.1}], egui::TextEdit::multiline(&mut self.{b}));",
                child.rect.w, child.rect.h
            ),
            None => format!("            // TextArea {label}: set a valid Binding"),
        },
        WidgetKind::Slider => match binding {
            Some(b) => format!(
                "            ui.add_sized([{:.1}, {:.1}], egui::Slider::new(&mut self.{b}, {:.1}..={:.1}).text({label}));",
                child.rect.w, child.rect.h, child.props.min, child.props.max
            ),
            None => format!("            // Slider {label}: set a valid Binding"),
        },
        WidgetKind::SpinBox => match binding {
            Some(b) => format!("            ui.add(egui::DragValue::new(&mut self.{b}));"),
            None => format!("            // SpinBox {label}: set a valid Binding"),
        },
        WidgetKind::Checkbox => match binding {
            Some(b) => format!("            ui.checkbox(&mut self.{b}, {label});"),
            None => format!("            // Checkbox {label}: set a valid Binding"),
        },
        WidgetKind::RadioButton => match binding {
            Some(b) => {
                let value = if child.props.radio_value.is_empty() {
                    label.clone()
                } else {
                    string_literal(&child.props.radio_value)
                };
                format!("            ui.radio_value(&mut self.{b}, {value}.to_owned(), {label});")
            }
            None => format!("            // RadioButton {label}: set a valid Binding"),
        },
        WidgetKind::ComboBox => match binding {
            Some(b) => {
                let id = child.id.as_simple();
                let mut s = format!(
                    "            egui::ComboBox::from_id_salt(\"layout_combo_{id}\")\n                .selected_text(&self.{b})\n                .show_ui(ui, |ui| {{\n"
                );
                for option in combo_option_values(child) {
                    let opt = string_literal(&option);
                    s.push_str(&format!(
                        "                    ui.selectable_value(&mut self.{b}, {opt}.to_owned(), {opt});\n"
                    ));
                }
                s.push_str("                });");
                s
            }
            None => format!("            // ComboBox {label}: set a valid Binding"),
        },
        WidgetKind::ProgressBar => match binding {
            Some(b) => format!(
                "            ui.add_sized([{:.1}, {:.1}], egui::ProgressBar::new(self.{b}));",
                child.rect.w, child.rect.h
            ),
            None => format!("            // ProgressBar {label}: set a valid Binding"),
        },
        WidgetKind::HorizontalSpacer => format!("            ui.add_space({:.1});", child.rect.w),
        WidgetKind::VerticalSpacer => format!("            ui.add_space({:.1});", child.rect.h),
        WidgetKind::Image => image_child_preview_line(child, "ui.available_rect_before_wrap()"),
        WidgetKind::Custom(_) => {
            if let Some(ref tpl) = child.descriptor_live_tpl {
                crate::codegen::widget_descriptor::apply_template(
                    tpl,
                    child,
                    child.descriptor_name.as_deref().unwrap_or("Custom"),
                )
            } else {
                format!("            // Custom child {:?}: descriptor not loaded", child.kind)
            }
        }
        _ => format!("            // Layout child {:?}: sequential export not implemented yet", child.kind),
    };
    lines.push((Some(child.id), line));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{Rect, WidgetInstance, WidgetProps};

    #[test]
    fn generated_document_maps_every_widget_and_nested_child() {
        let parent_id = Uuid::from_u128(0xA1);
        let child_id = Uuid::from_u128(0xB2);
        let parent = WidgetInstance {
            id: parent_id,
            kind: WidgetKind::VLayout,
            rect: Rect {
                x: 10.0,
                y: 20.0,
                w: 200.0,
                h: 180.0,
            },
            children: vec![child_id],
            ..Default::default()
        };
        let child = WidgetInstance {
            id: child_id,
            kind: WidgetKind::Button,
            rect: Rect {
                x: 20.0,
                y: 30.0,
                w: 100.0,
                h: 30.0,
            },
            props: WidgetProps {
                label: "Nested".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let tree = UiTree {
            widgets: vec![parent, child],
            ..Default::default()
        };

        let document = emit_document(&tree);
        assert_eq!(document.widget_spans.len(), 2);
        for id in [parent_id, child_id] {
            let entry = document
                .widget_spans
                .iter()
                .find(|entry| entry.widget_id == id)
                .expect("source span");
            let source = &document.text[entry.span.bytes.clone()];
            assert!(!source.contains("CentralPanel"));
            assert!(source.contains(&id.to_string()));
        }
    }

    #[test]
    fn image_widget_emits_svg_preview_call() {
        let tree = UiTree {
            widgets: vec![WidgetInstance {
                id: Uuid::nil(),
                kind: WidgetKind::Image,
                rect: Rect {
                    x: 10.0,
                    y: 20.0,
                    w: 120.0,
                    h: 80.0,
                },
                svg_source: Some("<svg/>".to_owned()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let generated = emit_indexed(&tree)
            .into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(generated.contains("self.show_svg_image"));
        // Live preview emits a compact size note, NOT the full raw SVG string.
        // Export (export.rs) still embeds the full source via include_str / raw literal.
        assert!(generated.contains("[SVG:"), "expected compact size note");
        assert!(
            !generated.contains("<svg/>"),
            "raw SVG must not appear in live preview"
        );
        assert!(generated.contains("120.0"));
        assert!(generated.contains("80.0"));
        assert!(!generated.contains("egui::Frame::none()"));
    }

    #[test]
    fn vlayout_emits_owned_children_sequentially() {
        let parent_id = Uuid::from_u128(1);
        let child_id = Uuid::from_u128(2);
        let tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::VLayout,
                    children: vec![child_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: child_id,
                    kind: WidgetKind::Button,
                    on_click: "handle_child".to_owned(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let generated = emit_indexed(&tree)
            .into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(generated.matches("egui::Area::new").count(), 1);
        assert!(generated.contains("ui.vertical(|ui| {"));
        assert!(generated.contains(&format!("// widget_{child_id}")));
        assert!(generated.contains("egui::Button::new"));
        assert!(generated.contains("self.handle_child();"));
    }

    #[test]
    fn hlayout_emits_owned_children_sequentially() {
        let parent_id = Uuid::from_u128(3);
        let child_id = Uuid::from_u128(4);
        let tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::HLayout,
                    children: vec![child_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: child_id,
                    kind: WidgetKind::Button,
                    on_click: "handle_horizontal".to_owned(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let generated = emit_indexed(&tree)
            .into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(generated.matches("egui::Area::new").count(), 1);
        assert!(generated.contains("ui.horizontal(|ui| {"));
        assert!(generated.contains(&format!("// widget_{child_id}")));
        assert!(generated.contains("egui::Button::new"));
        assert!(generated.contains("self.handle_horizontal();"));
    }

    #[test]
    fn gridlayout_emits_owned_children_row_major() {
        let parent_id = Uuid::from_u128(5);
        let child_ids = [Uuid::from_u128(6), Uuid::from_u128(7), Uuid::from_u128(8)];
        let mut widgets = vec![WidgetInstance {
            id: parent_id,
            kind: WidgetKind::GridLayout,
            children: child_ids.to_vec(),
            props: WidgetProps {
                grid_columns: 2,
                ..Default::default()
            },
            ..Default::default()
        }];
        widgets.extend(child_ids.iter().map(|id| WidgetInstance {
            id: *id,
            kind: WidgetKind::Button,
            on_click: format!("handle_{}", id.as_simple()),
            ..Default::default()
        }));
        let tree = UiTree {
            widgets,
            ..Default::default()
        };

        let generated = emit_indexed(&tree)
            .into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(generated.matches("egui::Area::new").count(), 1);
        assert!(generated.contains("egui::Grid::new"));
        assert_eq!(generated.matches("ui.end_row();").count(), 2);
        for child_id in child_ids {
            assert!(generated.contains(&format!("// widget_{child_id}")));
        }
    }

    #[test]
    fn hlayout_emits_reflowed_horizontal_spacer_space() {
        let parent_id = Uuid::from_u128(9);
        let left_id = Uuid::from_u128(10);
        let spacer_id = Uuid::from_u128(11);
        let right_id = Uuid::from_u128(12);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::HLayout,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 300.0,
                        h: 60.0,
                    },
                    children: vec![left_id, spacer_id, right_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: left_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 50.0,
                        h: 30.0,
                    },
                    ..Default::default()
                },
                WidgetInstance {
                    id: spacer_id,
                    kind: WidgetKind::HorizontalSpacer,
                    ..Default::default()
                },
                WidgetInstance {
                    id: right_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 70.0,
                        h: 30.0,
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        tree.reflow_layouts();

        let generated = emit_indexed(&tree)
            .into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(generated.contains("ui.horizontal(|ui| {"));
        assert!(generated.contains("ui.add_space(152.0);"));
    }

    fn emit_joined(kind: WidgetKind, setup: impl FnOnce(&mut WidgetInstance)) -> String {
        let mut w = WidgetInstance {
            id: Uuid::nil(),
            kind,
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 30.0,
            },
            ..Default::default()
        };
        setup(&mut w);
        let tree = UiTree {
            widgets: vec![w],
            ..Default::default()
        };
        emit_indexed(&tree)
            .into_iter()
            .map(|(_, l)| l)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn tool_button_emits_small_button() {
        let g = emit_joined(WidgetKind::ToolButton, |w| w.props.label = "Go".into());
        assert!(g.contains("ui.small_button"));
    }

    #[test]
    fn math_label_emits_bound_format() {
        let g = emit_joined(WidgetKind::MathLabel, |w| {
            w.props.label = "A {quoted} \"Total\"".into();
            w.state_binding = Some("total".into());
        });
        assert!(g.contains("format!(\"{} = {:.2}\""));
        assert!(g.contains("\"A {quoted} \\\"Total\\\"\""));
        assert!(g.contains("self.total"));
    }

    #[test]
    fn chart_emits_bound_painter_code() {
        let g = emit_joined(WidgetKind::Chart, |w| {
            w.rect.w = 180.0;
            w.rect.h = 90.0;
            w.state_binding = Some("values".into());
        });
        assert!(g.contains("let chart_values = &self.values"));
        assert!(g.contains("chart_painter.rect_filled"));
        assert!(g.contains("egui::vec2(180.0, 90.0)"));
        assert!(!g.contains("bind a Vec<f32> and paint"));
    }

    #[test]
    fn table_emits_grid_with_columns() {
        let g = emit_joined(WidgetKind::Table, |w| {
            w.props.options = vec!["A".into(), "B".into()];
        });
        assert!(g.contains("egui::Grid::new"));
        assert!(g.contains("\"A\""));
        assert!(g.contains("\"B\""));
    }

    #[test]
    fn list_view_emits_scroll_area_with_items() {
        let g = emit_joined(WidgetKind::ListView, |w| {
            w.props.options = vec!["One".into(), "Two".into()];
        });
        assert!(g.contains("ScrollArea"));
        assert!(g.contains("\"One\""));
    }

    #[test]
    fn dialog_button_box_emits_buttons() {
        let g = emit_joined(WidgetKind::DialogButtonBox, |w| {
            w.props.options = vec!["OK".into(), "Cancel".into()];
        });
        assert!(g.contains("\"OK\""));
        assert!(g.contains("\"Cancel\""));
        assert!(g.contains(".clicked()"));
    }

    #[test]
    fn file_picker_emits_rfd_dialog() {
        let g = emit_joined(WidgetKind::FilePicker, |w| {
            w.state_binding = Some("path".into());
        });
        assert!(g.contains("rfd::FileDialog"));
        assert!(g.contains("self.path"));
    }

    #[test]
    fn keyword_state_binding_emits_effective_field_name() {
        // A state_binding of "type" is a Rust keyword; the emitter must reference
        // self.type_value (not the bare keyword) so generated code compiles.
        let g = emit_joined(WidgetKind::Checkbox, |w| {
            w.state_binding = Some("type".into());
        });
        assert!(
            g.contains("self.type_value"),
            "keyword binding must be remapped: got\n{g}"
        );
        assert!(
            !g.contains("self.type,") && !g.contains("&mut self.type)"),
            "raw keyword must not appear as a field reference: got\n{g}"
        );
    }
}
