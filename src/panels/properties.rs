use crate::project::schema::{CustomProp, CustomPropType, WidgetKind};
use crate::project::ui_tree::UiTree;
use uuid::Uuid;

/// Signals emitted by the properties panel back to the app.
pub enum PropertiesAction {
    None,
    /// Tracé — scroll the code panel to (and insert if absent) this handler.
    ScrollToHandler(String),
}

pub fn show_content(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    selected: &mut Vec<Uuid>,
    shift_held: bool,
) -> PropertiesAction {
    let max_height = ui.available_height().clamp(140.0, 360.0);
    egui::ScrollArea::vertical()
        .id_salt("properties_scroll")
        .max_height(max_height)
        .auto_shrink([false, false])
        .show(ui, |ui| show_content_inner(ui, tree, selected, shift_held))
        .inner
}

fn show_content_inner(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    selected: &mut Vec<Uuid>,
    shift_held: bool,
) -> PropertiesAction {
    if selected.is_empty() {
        ui.weak("No widget selected.");
        return PropertiesAction::None;
    }

    // Multi-select: alignment + group/ungroup tools
    if selected.len() >= 2 {
        ui.separator();
        show_alignment(ui, tree, selected, shift_held);
    }
    show_group_controls(ui, tree, selected);

    let Some(id) = selected.last().copied() else {
        return PropertiesAction::None;
    };

    let mut do_delete = false;
    let mut props_action = PropertiesAction::None;

    {
        let Some(w) = tree.get_mut(id) else {
            ui.label("Widget not found.");
            return PropertiesAction::None;
        };

        ui.separator();

        // --- Contextual fields vary per kind ---
        let kind = w.kind.clone();

        // Label field: Button, Label, Checkbox, RadioButton, Frame, ComboBox
        let show_label = matches!(
            kind,
            WidgetKind::Button
                | WidgetKind::Label
                | WidgetKind::Checkbox
                | WidgetKind::RadioButton
                | WidgetKind::Frame
                | WidgetKind::ComboBox
        );
        if show_label {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Label").small().weak());
                ui.add_space(4.0);
                ui.text_edit_singleline(&mut w.props.label);
            });
        }

        if kind == WidgetKind::ComboBox {
            ui.label(egui::RichText::new("Options").small().weak());
            let mut to_remove: Option<usize> = None;
            for (index, option) in w.props.options.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    let option_width = (ui.available_width() - 28.0).clamp(80.0, 180.0);
                    ui.add(
                        egui::TextEdit::singleline(option)
                            .hint_text(format!("Option {}", index + 1))
                            .desired_width(option_width),
                    );
                    if ui
                        .small_button("x")
                        .on_hover_text("Remove option")
                        .clicked()
                    {
                        to_remove = Some(index);
                    }
                });
            }
            if let Some(index) = to_remove {
                w.props.options.remove(index);
            }
            if ui.small_button("+ Add option").clicked() {
                w.props
                    .options
                    .push(format!("Option {}", w.props.options.len() + 1));
            }
            if w.props.options.is_empty() {
                w.props.options.push("Option A".to_owned());
            }
        }

        // State binding field: everything except Button, Frame
        let show_binding = !matches!(kind, WidgetKind::Button | WidgetKind::Frame);
        if show_binding {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Binding").small().weak());
                ui.add_space(4.0);
                let mut binding = w.state_binding.clone().unwrap_or_default();
                if ui
                    .add(egui::TextEdit::singleline(&mut binding).hint_text("field_name"))
                    .changed()
                {
                    let t = binding.trim();
                    if t.is_empty() {
                        w.state_binding = None;
                    } else if crate::codegen::rust::is_valid_identifier(t) {
                        w.state_binding = Some(t.to_owned());
                    }
                }
            });
            if let Some(b) = &w.state_binding {
                if !crate::codegen::rust::is_valid_identifier(b) {
                    ui.label(
                        egui::RichText::new("⚠ invalid identifier")
                            .small()
                            .color(egui::Color32::RED),
                    );
                }
            }
        }

        // Label binding mode (Label widget: bind label text to state field)
        if kind == WidgetKind::Label {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Text mode").small().weak());
                let is_bound = w.label_binding.is_some();
                if ui
                    .selectable_label(!is_bound, "Static")
                    .on_hover_text("Label is a string literal in codegen")
                    .clicked()
                    && is_bound
                {
                    w.label_binding = None;
                }
                if ui
                    .selectable_label(is_bound, "Bound")
                    .on_hover_text("Label reads from an AppState field at runtime")
                    .clicked()
                    && !is_bound
                {
                    let default = w
                        .state_binding
                        .as_deref()
                        .map(|b| format!("{b}_label"))
                        .unwrap_or_else(|| "label_text".to_owned());
                    w.label_binding = Some(default);
                }
            });
            if let Some(ref mut lb) = w.label_binding {
                let mut tmp = lb.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut tmp).hint_text("state_field"))
                    .changed()
                {
                    let t = tmp.trim().to_owned();
                    if t.is_empty() {
                        w.label_binding = None;
                    } else if crate::codegen::rust::is_valid_identifier(&t) {
                        *lb = t;
                    }
                }
            }
        }

        // Geometry — compact 4-column: X Y / W H
        ui.separator();
        egui::Grid::new("geom_compact")
            .num_columns(4)
            .spacing([4.0, 2.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("X").small());
                ui.add(egui::DragValue::new(&mut w.rect.x).speed(1.0));
                ui.label(egui::RichText::new("Y").small());
                ui.add(egui::DragValue::new(&mut w.rect.y).speed(1.0));
                ui.end_row();
                ui.label(egui::RichText::new("W").small());
                ui.add(egui::DragValue::new(&mut w.rect.w).speed(1.0));
                ui.label(egui::RichText::new("H").small());
                ui.add(egui::DragValue::new(&mut w.rect.h).speed(1.0));
                ui.end_row();
            });

        // Default value + Min/Max for Slider; Min/Max only for ProgressBar
        let show_minmax = matches!(kind, WidgetKind::Slider | WidgetKind::ProgressBar);
        if show_minmax {
            egui::Grid::new("range_compact")
                .num_columns(4)
                .spacing([4.0, 2.0])
                .show(ui, |ui| {
                    if kind == WidgetKind::Slider {
                        let lo = w.props.min.min(w.props.max);
                        let hi = w.props.min.max(w.props.max);
                        ui.label(egui::RichText::new("Default").small());
                        ui.add(
                            egui::DragValue::new(&mut w.props.default_value)
                                .speed(0.5)
                                .range(lo..=hi),
                        );
                        ui.end_row();
                    }
                    ui.label(egui::RichText::new("Min").small());
                    ui.add(egui::DragValue::new(&mut w.props.min).speed(0.5));
                    ui.label(egui::RichText::new("Max").small());
                    ui.add(egui::DragValue::new(&mut w.props.max).speed(0.5));
                    ui.end_row();
                });
        }

        // Tooltip — all kinds
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Tooltip").small().weak());
            if w.tooltip.is_some() {
                if ui
                    .small_button("✕")
                    .on_hover_text("Remove tooltip")
                    .clicked()
                {
                    w.tooltip = None;
                }
            } else if ui.small_button("+").clicked() {
                w.tooltip = Some(String::new());
            }
        });
        if let Some(ref mut tip) = w.tooltip {
            ui.add(
                egui::TextEdit::singleline(tip)
                    .hint_text("Hover text…")
                    .desired_width(f32::INFINITY),
            );
        }

        // Enabled toggle — not Frame, Label, ProgressBar
        let show_enabled = !matches!(
            kind,
            WidgetKind::Frame | WidgetKind::Label | WidgetKind::ProgressBar
        );
        if show_enabled {
            let currently_enabled = w.enabled.unwrap_or(true);
            let mut enabled_val = currently_enabled;
            ui.checkbox(&mut enabled_val, egui::RichText::new("Enabled").small())
                .on_hover_text("Unchecked → ui.set_enabled(false)");
            if enabled_val != currently_enabled {
                w.enabled = if enabled_val { None } else { Some(false) };
            }
        }

        // Fg color — all kinds
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Color").small().weak());
            let mut c32 = w
                .fg_color
                .map(|color| egui::Color32::from_rgb(color[0], color[1], color[2]))
                .unwrap_or(egui::Color32::WHITE);
            let before = c32;
            let response = egui::color_picker::color_edit_button_srgba(
                ui,
                &mut c32,
                egui::color_picker::Alpha::Opaque,
            );
            if response.changed() || c32 != before {
                w.fg_color = if c32 == egui::Color32::WHITE {
                    None
                } else {
                    Some([c32.r(), c32.g(), c32.b()])
                };
            }
            if w.fg_color.is_some()
                && ui
                    .small_button("x")
                    .on_hover_text("Reset to default white")
                    .clicked()
            {
                w.fg_color = None;
            }
        });

        // Corner radius — Button, Label, Frame, ComboBox, Checkbox, RadioButton
        let show_radius = matches!(
            kind,
            WidgetKind::Button
                | WidgetKind::Label
                | WidgetKind::Frame
                | WidgetKind::ComboBox
                | WidgetKind::Checkbox
                | WidgetKind::RadioButton
        );
        if show_radius {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Radius").small().weak());
                let mut r_val = w.corner_radius.unwrap_or(0.0);
                if ui
                    .add(
                        egui::DragValue::new(&mut r_val)
                            .range(0.0..=32.0_f32)
                            .speed(0.5)
                            .suffix(" px"),
                    )
                    .changed()
                {
                    w.corner_radius = if r_val <= 0.0 { None } else { Some(r_val) };
                }
                if w.corner_radius.is_some()
                    && ui
                        .small_button("✕")
                        .on_hover_text("Reset rounding")
                        .clicked()
                {
                    w.corner_radius = None;
                }
            });
        }

        // Custom props — not Slider (Slider state is simple f32 via binding)
        let show_custom = kind != WidgetKind::Slider;
        if show_custom {
            ui.separator();
            ui.label(egui::RichText::new("Custom props").small().weak());
            let mut to_remove: Option<usize> = None;
            for (i, prop) in w.custom_props.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{}: {}", prop.name, prop.ty.label()))
                            .monospace()
                            .small(),
                    );
                    if ui.small_button("✕").clicked() {
                        to_remove = Some(i);
                    }
                });
            }
            if let Some(i) = to_remove {
                w.custom_props.remove(i);
            }
            // "+ Add" form
            let add_key = egui::Id::new(("custom_prop_form", id));
            let mut form: (String, CustomPropType) = ui
                .data(|d| d.get_temp::<(String, CustomPropType)>(add_key))
                .unwrap_or_default();
            egui::CollapsingHeader::new("+ Add property")
                .id_salt(("add_prop", id))
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut form.0);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Type:");
                        egui::ComboBox::from_id_salt(("prop_type", id))
                            .selected_text(form.1.label())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut form.1, CustomPropType::String, "String");
                                ui.selectable_value(&mut form.1, CustomPropType::F32, "f32");
                                ui.selectable_value(&mut form.1, CustomPropType::Bool, "bool");
                                ui.selectable_value(&mut form.1, CustomPropType::I32, "i32");
                            });
                    });
                    let can_add = crate::codegen::rust::is_valid_identifier(form.0.trim())
                        && !w.custom_props.iter().any(|p| p.name == form.0.trim());
                    if ui.add_enabled(can_add, egui::Button::new("Add")).clicked() {
                        w.custom_props.push(CustomProp {
                            name: form.0.trim().to_owned(),
                            ty: form.1.clone(),
                        });
                        form.0.clear();
                    }
                });
            ui.data_mut(|d| d.insert_temp(add_key, form));
        }

        ui.separator();
        if ui
            .button(
                egui::RichText::new("Delete widget").color(egui::Color32::from_rgb(248, 113, 113)),
            )
            .clicked()
        {
            do_delete = true;
        }
    } // w borrow ends

    if do_delete {
        tree.remove(id);
        selected.retain(|&x| x != id);
    } else {
        tree.validate_and_repair();
    }

    // Event handler — re-borrow after validate_and_repair
    show_event_handler(ui, tree, id, &mut props_action);

    props_action
}

/// Drawn after the main borrow to allow re-borrowing tree.
fn show_event_handler(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    id: Uuid,
    action: &mut PropertiesAction,
) {
    let Some(w) = tree.get_mut(id) else {
        return;
    };

    let event_label = match &w.kind {
        WidgetKind::Button => "On Click",
        WidgetKind::TextInput | WidgetKind::Slider | WidgetKind::Checkbox => "On Change",
        WidgetKind::ComboBox | WidgetKind::RadioButton => "On Change",
        _ => return, // Frame, Label, ProgressBar: no event
    };

    ui.separator();
    const TEAL: egui::Color32 = egui::Color32::from_rgb(52, 211, 153);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(event_label).small().weak());
        // When a handler is set, show a teal "→ fn name" chip that triggers Tracé on click.
        if let Some(ref name) = w.event_handler {
            if !name.is_empty() {
                let chip_text = format!("→ fn {name}");
                if ui
                    .button(egui::RichText::new(chip_text).small().color(TEAL))
                    .on_hover_text("Click to jump to handler in code panel (Tracé)")
                    .clicked()
                {
                    *action = PropertiesAction::ScrollToHandler(name.clone());
                }
            }
        }
    });

    let mut handler = w.event_handler.clone().unwrap_or_default();
    let placeholder_hint = format!(
        "e.g. handle_{}",
        event_label.to_lowercase().replace(' ', "_")
    );
    let resp = ui
        .add(
            egui::TextEdit::singleline(&mut handler)
                .hint_text(placeholder_hint)
                .desired_width(f32::INFINITY),
        )
        .on_hover_text("Ctrl+double-click widget to jump to handler");

    if resp.changed() {
        let trimmed = handler.trim().to_owned();
        w.event_handler = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
    }
}

// ---------------------------------------------------------------------------
// Group / Ungroup controls
// ---------------------------------------------------------------------------

fn show_group_controls(ui: &mut egui::Ui, tree: &mut UiTree, selected: &mut Vec<Uuid>) {
    let can_group = selected.len() >= 2;
    let frame_selected: Option<Uuid> = selected
        .iter()
        .find(|&&id| {
            tree.widgets
                .iter()
                .any(|w| w.id == id && w.kind == WidgetKind::Frame)
        })
        .copied();
    let can_ungroup = frame_selected.is_some();

    ui.horizontal(|ui| {
        let mut do_group = false;
        let mut do_ungroup = false;

        let gr = ui.add_enabled(
            can_group,
            egui::Button::new(egui::RichText::new("⊞ Group").small())
                .min_size(egui::vec2(60.0, 22.0)),
        );
        if gr
            .on_hover_text("Group selected widgets into a Frame (Ctrl+G)")
            .clicked()
        {
            do_group = true;
        }

        let ug = ui.add_enabled(
            can_ungroup,
            egui::Button::new(egui::RichText::new("⊟ Ungroup").small())
                .min_size(egui::vec2(60.0, 22.0)),
        );
        if ug
            .on_hover_text("Ungroup selected Frame (Ctrl+Shift+G)")
            .clicked()
        {
            do_ungroup = true;
        }

        if do_group {
            if let Some(new_id) = tree.group(selected) {
                selected.clear();
                selected.push(new_id);
            }
        }
        if do_ungroup {
            let frame_ids: Vec<Uuid> = selected
                .iter()
                .filter(|&&id| {
                    tree.widgets
                        .iter()
                        .any(|w| w.id == id && w.kind == WidgetKind::Frame)
                })
                .copied()
                .collect();
            let mut new_sel = Vec::new();
            for fid in frame_ids {
                let children = tree.ungroup(fid);
                new_sel.extend(children);
            }
            if !new_sel.is_empty() {
                *selected = new_sel;
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Alignment tools
// ---------------------------------------------------------------------------

const GUIDE: egui::Color32 = egui::Color32::from_rgb(52, 211, 153);
const BLOCK: egui::Color32 = egui::Color32::from_gray(180);

#[derive(Clone, Copy)]
enum AlignAction {
    Left,
    CenterH,
    Right,
    Top,
    CenterV,
    Bottom,
}

fn align_button(
    ui: &mut egui::Ui,
    tooltip: &str,
    draw_fn: impl FnOnce(&egui::Painter, egui::Rect),
) -> bool {
    let size = egui::vec2(28.0, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter()
        .rect(rect, visuals.rounding, visuals.bg_fill, visuals.bg_stroke);
    let inner = rect.shrink(3.0);
    draw_fn(ui.painter(), inner);
    response.on_hover_text(tooltip).clicked()
}

fn icon_align_left(p: &egui::Painter, r: egui::Rect) {
    p.line_segment(
        [egui::pos2(r.min.x, r.min.y), egui::pos2(r.min.x, r.max.y)],
        egui::Stroke::new(1.5, GUIDE),
    );
    let x = r.min.x + 2.0;
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(x, r.min.y + 2.0), egui::vec2(15.0, 5.5)),
        0.0,
        BLOCK,
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(x, r.min.y + 13.0), egui::vec2(9.0, 5.5)),
        0.0,
        BLOCK,
    );
}

fn icon_center_h(p: &egui::Painter, r: egui::Rect) {
    let cx = r.center().x;
    p.line_segment(
        [egui::pos2(cx, r.min.y), egui::pos2(cx, r.max.y)],
        egui::Stroke::new(1.5, GUIDE),
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(cx - 7.5, r.min.y + 2.0), egui::vec2(15.0, 5.5)),
        0.0,
        BLOCK,
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(cx - 4.5, r.min.y + 13.0), egui::vec2(9.0, 5.5)),
        0.0,
        BLOCK,
    );
}

fn icon_align_right(p: &egui::Painter, r: egui::Rect) {
    p.line_segment(
        [egui::pos2(r.max.x, r.min.y), egui::pos2(r.max.x, r.max.y)],
        egui::Stroke::new(1.5, GUIDE),
    );
    let x_wide = r.max.x - 2.0 - 15.0;
    let x_narrow = r.max.x - 2.0 - 9.0;
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(x_wide, r.min.y + 2.0), egui::vec2(15.0, 5.5)),
        0.0,
        BLOCK,
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(x_narrow, r.min.y + 13.0), egui::vec2(9.0, 5.5)),
        0.0,
        BLOCK,
    );
}

fn icon_align_top(p: &egui::Painter, r: egui::Rect) {
    p.line_segment(
        [egui::pos2(r.min.x, r.min.y), egui::pos2(r.max.x, r.min.y)],
        egui::Stroke::new(1.5, GUIDE),
    );
    let y = r.min.y + 2.0;
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(r.min.x + 2.0, y), egui::vec2(7.0, 12.0)),
        0.0,
        BLOCK,
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(r.min.x + 13.0, y), egui::vec2(7.0, 7.0)),
        0.0,
        BLOCK,
    );
}

fn icon_center_v(p: &egui::Painter, r: egui::Rect) {
    let cy = r.center().y;
    p.line_segment(
        [egui::pos2(r.min.x, cy), egui::pos2(r.max.x, cy)],
        egui::Stroke::new(1.5, GUIDE),
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(r.min.x + 2.0, cy - 6.0), egui::vec2(7.0, 12.0)),
        0.0,
        BLOCK,
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(r.min.x + 13.0, cy - 3.5), egui::vec2(7.0, 7.0)),
        0.0,
        BLOCK,
    );
}

fn icon_align_bottom(p: &egui::Painter, r: egui::Rect) {
    p.line_segment(
        [egui::pos2(r.min.x, r.max.y), egui::pos2(r.max.x, r.max.y)],
        egui::Stroke::new(1.5, GUIDE),
    );
    let y_tall = r.max.y - 2.0 - 12.0;
    let y_short = r.max.y - 2.0 - 7.0;
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(r.min.x + 2.0, y_tall), egui::vec2(7.0, 12.0)),
        0.0,
        BLOCK,
    );
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(r.min.x + 13.0, y_short), egui::vec2(7.0, 7.0)),
        0.0,
        BLOCK,
    );
}

fn show_alignment(ui: &mut egui::Ui, tree: &mut UiTree, selected: &[Uuid], shift_held: bool) {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for &id in selected {
        if let Some(w) = tree.widgets.iter().find(|w| w.id == id) {
            min_x = min_x.min(w.rect.x);
            min_y = min_y.min(w.rect.y);
            max_x = max_x.max(w.rect.x + w.rect.w);
            max_y = max_y.max(w.rect.y + w.rect.h);
        }
    }
    if min_x > max_x {
        return;
    }

    // Key-object alignment: when Shift is held, align relative to last selected widget
    let (ref_min_x, ref_min_y, ref_max_x, ref_max_y) = if shift_held {
        selected
            .last()
            .and_then(|&key_id| tree.widgets.iter().find(|w| w.id == key_id))
            .map(|kw| {
                (
                    kw.rect.x,
                    kw.rect.y,
                    kw.rect.x + kw.rect.w,
                    kw.rect.y + kw.rect.h,
                )
            })
            .unwrap_or((min_x, min_y, max_x, max_y))
    } else {
        (min_x, min_y, max_x, max_y)
    };

    let bb_cx = (ref_min_x + ref_max_x) / 2.0;
    let bb_cy = (ref_min_y + ref_max_y) / 2.0;
    let mut action: Option<AlignAction> = None;

    let label_text = if shift_held {
        "Align (key obj)"
    } else {
        "Align"
    };
    ui.label(egui::RichText::new(label_text).color(egui::Color32::from_gray(140)));
    ui.horizontal(|ui| {
        if align_button(ui, "Align Left", icon_align_left) {
            action = Some(AlignAction::Left);
        }
        if align_button(ui, "Center Horizontal", icon_center_h) {
            action = Some(AlignAction::CenterH);
        }
        if align_button(ui, "Align Right", icon_align_right) {
            action = Some(AlignAction::Right);
        }
    });
    ui.horizontal(|ui| {
        if align_button(ui, "Align Top", icon_align_top) {
            action = Some(AlignAction::Top);
        }
        if align_button(ui, "Center Vertical", icon_center_v) {
            action = Some(AlignAction::CenterV);
        }
        if align_button(ui, "Align Bottom", icon_align_bottom) {
            action = Some(AlignAction::Bottom);
        }
    });

    if let Some(a) = action {
        let key_id = if shift_held {
            selected.last().copied()
        } else {
            None
        };
        let ids: Vec<Uuid> = selected.to_vec();
        for id in ids {
            if Some(id) == key_id {
                continue; // key object is the reference — it never moves
            }
            if let Some(w) = tree.get_mut(id) {
                match a {
                    AlignAction::Left => w.rect.x = ref_min_x,
                    AlignAction::CenterH => w.rect.x = bb_cx - w.rect.w / 2.0,
                    AlignAction::Right => w.rect.x = ref_max_x - w.rect.w,
                    AlignAction::Top => w.rect.y = ref_min_y,
                    AlignAction::CenterV => w.rect.y = bb_cy - w.rect.h / 2.0,
                    AlignAction::Bottom => w.rect.y = ref_max_y - w.rect.h,
                }
            }
        }
    }
}
