use crate::codegen::widget_descriptor::{DescriptorPropType, WidgetDescriptor};
use crate::project::schema::{
    CustomProp, CustomPropType, Orientation, TextAlign, WidgetInstance, WidgetKind,
};
use crate::project::ui_tree::UiTree;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Public action type
// ---------------------------------------------------------------------------

pub enum PropertiesAction {
    None,
    /// Tracé — scroll the code panel to (and insert if absent) this handler.
    ScrollToHandler(String),
    /// Open the SVG source viewer popup for this widget.
    ShowSvgSource(uuid::Uuid),
    /// Open the descriptor editor for this descriptor id.
    EditDescriptor(String),
}

const TEAL: egui::Color32 = egui::Color32::from_rgb(52, 211, 153);
const RED_WARN: egui::Color32 = egui::Color32::from_rgb(248, 113, 113);

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn show_content(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    selected: &mut Vec<Uuid>,
    shift_held: bool,
    descriptors: &[WidgetDescriptor],
) -> PropertiesAction {
    let max_height = ui.available_height().clamp(140.0, 360.0);
    egui::ScrollArea::vertical()
        .id_salt("properties_scroll")
        .max_height(max_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            show_content_inner(ui, tree, selected, shift_held, descriptors)
        })
        .inner
}

// ---------------------------------------------------------------------------
// Inner dispatcher
// ---------------------------------------------------------------------------

fn show_content_inner(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    selected: &mut Vec<Uuid>,
    shift_held: bool,
    descriptors: &[WidgetDescriptor],
) -> PropertiesAction {
    if selected.is_empty() {
        ui.weak("No widget selected.");
        return PropertiesAction::None;
    }

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

        match w.kind.clone() {
            WidgetKind::Button => show_button(ui, w, &mut do_delete),
            WidgetKind::Label => show_label(ui, w, &mut do_delete),
            WidgetKind::TextInput => show_text_input(ui, w, &mut do_delete),
            WidgetKind::Slider => show_slider(ui, w, &mut do_delete),
            WidgetKind::Checkbox => show_checkbox(ui, w, &mut do_delete),
            WidgetKind::RadioButton => show_radio_button(ui, w, &mut do_delete),
            WidgetKind::ComboBox => show_combo_box(ui, w, &mut do_delete),
            WidgetKind::ProgressBar => show_progress_bar(ui, w, &mut do_delete),
            WidgetKind::Frame => show_frame(ui, w, &mut do_delete),
            WidgetKind::TextArea => show_text_area(ui, w, &mut do_delete),
            WidgetKind::SpinBox => show_spin_box(ui, w, &mut do_delete),
            WidgetKind::FontComboBox => show_font_combo_box(ui, w, &mut do_delete),
            WidgetKind::HorizontalSpacer | WidgetKind::VerticalSpacer => {
                show_spacer(ui, w, &mut do_delete)
            }
            WidgetKind::GroupBox => show_group_box(ui, w, &mut do_delete),
            WidgetKind::VLayout | WidgetKind::HLayout => {
                show_layout_container(ui, w, &mut do_delete)
            }
            WidgetKind::ScrollArea => show_scroll_area(ui, w, &mut do_delete),
            WidgetKind::Image => {
                if show_image(ui, w, &mut do_delete) {
                    props_action = PropertiesAction::ShowSvgSource(id);
                }
            }
            WidgetKind::Custom(ref desc_id) => {
                let desc =
                    crate::codegen::widget_descriptor::find_by_id(descriptors, desc_id).cloned();
                let desc_id_owned = desc_id.clone();
                if show_custom(ui, w, &mut do_delete, desc.as_ref()) {
                    props_action = PropertiesAction::EditDescriptor(desc_id_owned);
                }
            }
        }
    } // w borrow ends

    if do_delete {
        tree.remove(id);
        selected.retain(|&x| x != id);
    } else {
        tree.validate_and_repair();
    }

    show_event_handler(ui, tree, id, &mut props_action);

    props_action
}

// ---------------------------------------------------------------------------
// Per-kind UI panels
// ---------------------------------------------------------------------------

fn show_button(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Label", &mut w.props.label);
    show_geometry(ui, w);
    ui.separator();
    show_bg_color(ui, w);
    show_fg_color(ui, w);
    show_corner_radius(ui, w);
    show_font_size(ui, w);
    ui.separator();
    show_tooltip(ui, w);
    show_enabled(ui, w);
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_label(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Text", &mut w.props.label);

    // Static / Bound mode
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Mode").small().weak());
        let is_bound = w.label_binding.is_some();
        if ui
            .selectable_label(!is_bound, "Static")
            .on_hover_text("String literal in codegen")
            .clicked()
            && is_bound
        {
            w.label_binding = None;
        }
        if ui
            .selectable_label(is_bound, "Bound")
            .on_hover_text("Read from AppState field at runtime")
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

    show_geometry(ui, w);
    ui.separator();
    show_fg_color(ui, w);
    show_font_size(ui, w);
    show_text_align(ui, w);
    ui.separator();
    show_tooltip(ui, w);
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_text_input(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Placeholder", &mut w.props.placeholder);
    ui.checkbox(
        &mut w.props.password_mode,
        egui::RichText::new("Password").small(),
    )
    .on_hover_text("Mask input with •");
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Max len").small().weak());
        let mut len_str = w
            .props
            .max_length
            .map(|n| n.to_string())
            .unwrap_or_default();
        if ui
            .add(
                egui::TextEdit::singleline(&mut len_str)
                    .hint_text("none")
                    .desired_width(60.0),
            )
            .changed()
        {
            w.props.max_length = len_str.trim().parse::<usize>().ok();
        }
    });
    binding_field(ui, w);
    show_geometry(ui, w);
    ui.separator();
    show_fg_color(ui, w);
    show_font_size(ui, w);
    ui.separator();
    show_tooltip(ui, w);
    show_enabled(ui, w);
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_slider(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    binding_field(ui, w);
    egui::Grid::new("slider_range")
        .num_columns(4)
        .spacing([4.0, 2.0])
        .show(ui, |ui| {
            let lo = w.props.min.min(w.props.max);
            let hi = w.props.min.max(w.props.max);
            ui.label(egui::RichText::new("Default").small());
            ui.add(
                egui::DragValue::new(&mut w.props.default_value)
                    .speed(0.5)
                    .range(lo..=hi),
            );
            ui.end_row();
            ui.label(egui::RichText::new("Min").small());
            ui.add(egui::DragValue::new(&mut w.props.min).speed(0.5));
            ui.label(egui::RichText::new("Max").small());
            ui.add(egui::DragValue::new(&mut w.props.max).speed(0.5));
            ui.end_row();
        });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Step").small().weak());
        let mut step_str = w.props.step.map(|s| s.to_string()).unwrap_or_default();
        if ui
            .add(
                egui::TextEdit::singleline(&mut step_str)
                    .hint_text("none")
                    .desired_width(60.0),
            )
            .changed()
        {
            w.props.step = step_str.trim().parse::<f32>().ok().filter(|&s| s > 0.0);
        }
    });
    ui.checkbox(
        &mut w.props.show_value,
        egui::RichText::new("Show value").small(),
    )
    .on_hover_text("Display current value alongside slider");
    show_orientation(ui, w);
    show_geometry(ui, w);
    ui.separator();
    show_tooltip(ui, w);
    show_enabled(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_checkbox(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Label", &mut w.props.label);
    binding_field(ui, w);
    show_geometry(ui, w);
    ui.separator();
    show_fg_color(ui, w);
    show_corner_radius(ui, w);
    ui.separator();
    show_tooltip(ui, w);
    show_enabled(ui, w);
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_radio_button(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Label", &mut w.props.label);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Value").small().weak());
        ui.add(egui::TextEdit::singleline(&mut w.props.radio_value).hint_text("option_a"));
    });
    // Group binding → also synced to state_binding for codegen
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Group").small().weak());
        let mut group = w.props.group_binding.clone();
        if ui
            .add(egui::TextEdit::singleline(&mut group).hint_text("radio_group"))
            .changed()
        {
            let t = group.trim().to_owned();
            w.props.group_binding = t.clone();
            if t.is_empty() {
                w.state_binding = None;
            } else if crate::codegen::rust::is_valid_identifier(&t) {
                w.state_binding = Some(t);
            }
        }
    });
    show_geometry(ui, w);
    ui.separator();
    show_fg_color(ui, w);
    show_corner_radius(ui, w);
    ui.separator();
    show_tooltip(ui, w);
    show_enabled(ui, w);
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_combo_box(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Label", &mut w.props.label);
    ui.label(egui::RichText::new("Options").small().weak());
    let mut to_remove: Option<usize> = None;
    for (index, option) in w.props.options.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let opt_w = (ui.available_width() - 28.0).clamp(80.0, 180.0);
            ui.add(
                egui::TextEdit::singleline(option)
                    .hint_text(format!("Option {}", index + 1))
                    .desired_width(opt_w),
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
    binding_field(ui, w);
    show_geometry(ui, w);
    ui.separator();
    show_fg_color(ui, w);
    show_corner_radius(ui, w);
    ui.separator();
    show_tooltip(ui, w);
    show_enabled(ui, w);
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_progress_bar(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    binding_field(ui, w);
    egui::Grid::new("pb_range")
        .num_columns(4)
        .spacing([4.0, 2.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Min").small());
            ui.add(egui::DragValue::new(&mut w.props.min).speed(0.5));
            ui.label(egui::RichText::new("Max").small());
            ui.add(egui::DragValue::new(&mut w.props.max).speed(0.5));
            ui.end_row();
        });
    ui.checkbox(
        &mut w.props.show_percentage,
        egui::RichText::new("Show %").small(),
    )
    .on_hover_text("Overlay percentage text on bar");
    ui.checkbox(
        &mut w.props.animated,
        egui::RichText::new("Animated").small(),
    )
    .on_hover_text("Animate the progress bar fill");
    show_geometry(ui, w);
    ui.separator();
    show_fg_color(ui, w);
    show_bg_color(ui, w);
    ui.separator();
    show_tooltip(ui, w);
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_frame(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Label", &mut w.props.label);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Margin").small().weak());
        ui.add(
            egui::DragValue::new(&mut w.props.inner_margin)
                .range(0.0..=64.0_f32)
                .speed(0.5)
                .suffix(" px"),
        );
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Stroke").small().weak());
        ui.add(
            egui::DragValue::new(&mut w.props.stroke_width)
                .range(0.0..=8.0_f32)
                .speed(0.1)
                .suffix(" px"),
        );
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Stroke col").small().weak());
        let mut c32 = w
            .props
            .stroke_color
            .map(|c| egui::Color32::from_rgb(c[0], c[1], c[2]))
            .unwrap_or(egui::Color32::from_gray(100));
        let before = c32;
        let resp = egui::color_picker::color_edit_button_srgba(
            ui,
            &mut c32,
            egui::color_picker::Alpha::Opaque,
        );
        if resp.changed() || c32 != before {
            w.props.stroke_color = Some([c32.r(), c32.g(), c32.b()]);
        }
        if w.props.stroke_color.is_some()
            && ui
                .small_button("✕")
                .on_hover_text("Reset stroke color")
                .clicked()
        {
            w.props.stroke_color = None;
        }
    });
    show_geometry(ui, w);
    ui.separator();
    show_bg_color(ui, w);
    show_fg_color(ui, w);
    show_corner_radius(ui, w);
    ui.separator();
    show_tooltip(ui, w);
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

// ---------------------------------------------------------------------------
// Shared field helpers
// ---------------------------------------------------------------------------

fn field_text(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).small().weak());
        ui.add_space(4.0);
        ui.text_edit_singleline(value);
    });
}

fn binding_field(ui: &mut egui::Ui, w: &mut WidgetInstance) {
    let mut binding = w.state_binding.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Binding").small().weak());
        ui.add_space(4.0);
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
                    .color(RED_WARN),
            );
        }
    }
}

fn show_geometry(ui: &mut egui::Ui, w: &mut WidgetInstance) {
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
}

fn show_tooltip(ui: &mut egui::Ui, w: &mut WidgetInstance) {
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
}

fn show_enabled(ui: &mut egui::Ui, w: &mut WidgetInstance) {
    let currently_enabled = w.enabled.unwrap_or(true);
    let mut enabled_val = currently_enabled;
    ui.checkbox(&mut enabled_val, egui::RichText::new("Enabled").small())
        .on_hover_text("Unchecked → ui.set_enabled(false)");
    if enabled_val != currently_enabled {
        w.enabled = if enabled_val { None } else { Some(false) };
    }
}

fn show_fg_color(ui: &mut egui::Ui, w: &mut WidgetInstance) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Fg color").small().weak());
        let mut c32 = w
            .fg_color
            .map(|c| egui::Color32::from_rgb(c[0], c[1], c[2]))
            .unwrap_or(egui::Color32::WHITE);
        let before = c32;
        let resp = egui::color_picker::color_edit_button_srgba(
            ui,
            &mut c32,
            egui::color_picker::Alpha::Opaque,
        );
        if resp.changed() || c32 != before {
            w.fg_color = if c32 == egui::Color32::WHITE {
                None
            } else {
                Some([c32.r(), c32.g(), c32.b()])
            };
        }
        if w.fg_color.is_some()
            && ui
                .small_button("✕")
                .on_hover_text("Reset fg color")
                .clicked()
        {
            w.fg_color = None;
        }
    });
}

fn show_bg_color(ui: &mut egui::Ui, w: &mut WidgetInstance) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Bg color").small().weak());
        let default_bg = egui::Color32::from_gray(30);
        let mut c32 = w
            .bg_color
            .map(|c| egui::Color32::from_rgb(c[0], c[1], c[2]))
            .unwrap_or(default_bg);
        let before = c32;
        let resp = egui::color_picker::color_edit_button_srgba(
            ui,
            &mut c32,
            egui::color_picker::Alpha::Opaque,
        );
        if (resp.changed() || c32 != before) && c32 != default_bg {
            w.bg_color = Some([c32.r(), c32.g(), c32.b()]);
        }
        if w.bg_color.is_some()
            && ui
                .small_button("✕")
                .on_hover_text("Reset bg color")
                .clicked()
        {
            w.bg_color = None;
        }
    });
}

fn show_corner_radius(ui: &mut egui::Ui, w: &mut WidgetInstance) {
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

fn show_font_size(ui: &mut egui::Ui, w: &mut WidgetInstance) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Font size").small().weak());
        let mut size = w.font_size.unwrap_or(14.0);
        if ui
            .add(
                egui::DragValue::new(&mut size)
                    .range(6.0..=72.0_f32)
                    .speed(0.5)
                    .suffix(" pt"),
            )
            .changed()
        {
            w.font_size = Some(size);
        }
        if w.font_size.is_some()
            && ui
                .small_button("✕")
                .on_hover_text("Reset to default size")
                .clicked()
        {
            w.font_size = None;
        }
    });
}

fn show_text_align(ui: &mut egui::Ui, w: &mut WidgetInstance) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Align").small().weak());
        let current = w.text_align.clone().unwrap_or(TextAlign::Left);
        if ui
            .selectable_label(current == TextAlign::Left, "L")
            .on_hover_text("Left")
            .clicked()
        {
            w.text_align = Some(TextAlign::Left);
        }
        if ui
            .selectable_label(current == TextAlign::Center, "C")
            .on_hover_text("Center")
            .clicked()
        {
            w.text_align = Some(TextAlign::Center);
        }
        if ui
            .selectable_label(current == TextAlign::Right, "R")
            .on_hover_text("Right")
            .clicked()
        {
            w.text_align = Some(TextAlign::Right);
        }
        if w.text_align.is_some()
            && ui
                .small_button("✕")
                .on_hover_text("Reset alignment")
                .clicked()
        {
            w.text_align = None;
        }
    });
}

fn show_orientation(ui: &mut egui::Ui, w: &mut WidgetInstance) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Orient").small().weak());
        ui.selectable_value(&mut w.props.orientation, Orientation::Horizontal, "H")
            .on_hover_text("Horizontal");
        ui.selectable_value(&mut w.props.orientation, Orientation::Vertical, "V")
            .on_hover_text("Vertical");
    });
}

fn show_custom_props(ui: &mut egui::Ui, w: &mut WidgetInstance) {
    ui.separator();
    ui.label(egui::RichText::new("Custom props").small().weak());
    let id = w.id;
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

/// Returns `true` if the user clicked "View SVG Source".
fn show_image(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) -> bool {
    ui.label(
        egui::RichText::new("SVG Image - source-backed canvas preview")
            .small()
            .weak(),
    );
    let mut view_clicked = false;
    if w.svg_source.is_some() {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("SVG source loaded")
                    .small()
                    .color(egui::Color32::from_rgb(52, 211, 153)),
            );
            if ui
                .small_button("View source")
                .on_hover_text("Open read-only SVG source viewer")
                .clicked()
            {
                view_clicked = true;
            }
        });
        ui.checkbox(&mut w.expand_svg_inline, "Expand SVG inline in code panel")
            .on_hover_text("Show full SVG source in the live code panel instead of [SVG: N bytes]");
    } else {
        ui.label(
            egui::RichText::new("No SVG source")
                .small()
                .color(egui::Color32::from_rgb(248, 113, 113)),
        );
    }
    show_delete_button(ui, do_delete);
    view_clicked
}

fn show_delete_button(ui: &mut egui::Ui, do_delete: &mut bool) {
    ui.separator();
    if ui
        .button(egui::RichText::new("Delete widget").color(RED_WARN))
        .clicked()
    {
        *do_delete = true;
    }
}

// ---------------------------------------------------------------------------
// Event handler (re-borrows tree after validate_and_repair)
// ---------------------------------------------------------------------------

fn show_event_handler(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    id: Uuid,
    action: &mut PropertiesAction,
) {
    let Some(w) = tree.get_mut(id) else {
        return;
    };

    let (event_label, is_button) = match &w.kind {
        WidgetKind::Button => ("On Click", true),
        WidgetKind::TextInput
        | WidgetKind::Slider
        | WidgetKind::Checkbox
        | WidgetKind::ComboBox
        | WidgetKind::RadioButton => ("On Change", false),
        _ => return,
    };

    // Migrate legacy event_handler → on_click / on_change on first display
    if let Some(ref eh) = w.event_handler.clone() {
        if !eh.is_empty() {
            if is_button && w.on_click.is_empty() {
                w.on_click = eh.clone();
            } else if !is_button && w.on_change.is_empty() {
                w.on_change = eh.clone();
            }
        }
    }

    let current = if is_button {
        w.on_click.clone()
    } else {
        w.on_change.clone()
    };

    ui.separator();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(event_label).small().weak());
        if !current.is_empty() {
            let chip = format!("→ fn {current}");
            if ui
                .button(egui::RichText::new(chip).small().color(TEAL))
                .on_hover_text("Click to jump to handler in code panel (Tracé)")
                .clicked()
            {
                *action = PropertiesAction::ScrollToHandler(current.clone());
            }
        }
    });

    let mut handler = current;
    let hint = format!(
        "e.g. handle_{}",
        event_label.to_lowercase().replace(' ', "_")
    );
    if ui
        .add(
            egui::TextEdit::singleline(&mut handler)
                .hint_text(hint)
                .desired_width(f32::INFINITY),
        )
        .on_hover_text("Ctrl+double-click widget to jump to handler")
        .changed()
    {
        let trimmed = handler.trim().to_owned();
        if is_button {
            w.on_click = trimmed;
        } else {
            w.on_change = trimmed;
        }
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
                continue;
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

// ---------------------------------------------------------------------------
// Stage 9 widget panels
// ---------------------------------------------------------------------------

fn show_text_area(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Placeholder", &mut w.props.placeholder);
    binding_field(ui, w);
    show_geometry(ui, w);
    ui.separator();
    show_fg_color(ui, w);
    show_font_size(ui, w);
    ui.separator();
    show_tooltip(ui, w);
    show_enabled(ui, w);
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_spin_box(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Label", &mut w.props.label);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Range").small().weak());
        ui.add(
            egui::DragValue::new(&mut w.props.min)
                .speed(1.0)
                .prefix("Min "),
        );
        ui.add(
            egui::DragValue::new(&mut w.props.max)
                .speed(1.0)
                .prefix("Max "),
        );
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Default").small().weak());
        ui.add(
            egui::DragValue::new(&mut w.props.default_value)
                .speed(0.1)
                .range(w.props.min..=w.props.max),
        );
    });
    binding_field(ui, w);
    show_geometry(ui, w);
    ui.separator();
    show_fg_color(ui, w);
    show_tooltip(ui, w);
    show_enabled(ui, w);
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_font_combo_box(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    ui.label(
        egui::RichText::new("Font selector combo. Binding holds selected font name.")
            .small()
            .weak(),
    );
    binding_field(ui, w);
    show_geometry(ui, w);
    ui.separator();
    show_tooltip(ui, w);
    show_enabled(ui, w);
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_spacer(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    show_geometry(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_group_box(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Title", &mut w.props.label);
    show_geometry(ui, w);
    ui.separator();
    show_fg_color(ui, w);
    show_font_size(ui, w);
    show_tooltip(ui, w);
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_layout_container(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Label", &mut w.props.label);
    show_geometry(ui, w);
    ui.separator();
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_scroll_area(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Label", &mut w.props.label);
    show_geometry(ui, w);
    ui.separator();
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

// ---------------------------------------------------------------------------
// Custom widget (descriptor-backed)
// ---------------------------------------------------------------------------

/// Returns `true` if the user clicked "Edit Descriptor".
fn show_custom(
    ui: &mut egui::Ui,
    w: &mut WidgetInstance,
    do_delete: &mut bool,
    descriptor: Option<&WidgetDescriptor>,
) -> bool {
    // Label field — always shown
    field_text(ui, "Label", &mut w.props.label);
    show_geometry(ui, w);
    ui.separator();

    let mut edit_clicked = false;

    // Descriptor-defined properties
    if let Some(desc) = descriptor {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("Descriptor: {}", desc.id))
                    .small()
                    .weak(),
            );
            if ui
                .small_button("Edit descriptor")
                .on_hover_text("Open the .rkwd editor for this widget type")
                .clicked()
            {
                edit_clicked = true;
            }
        });
        for prop in &desc.properties {
            let current = w
                .descriptor_props
                .entry(prop.key.clone())
                .or_insert_with(|| prop.default.clone())
                .clone();

            match prop.ty {
                DescriptorPropType::String => {
                    let mut val = current;
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&prop.display).small().weak());
                        if ui.text_edit_singleline(&mut val).changed() {
                            w.descriptor_props.insert(prop.key.clone(), val);
                        }
                    });
                }
                DescriptorPropType::F32 => {
                    let mut val: f32 = current.parse().unwrap_or(0.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&prop.display).small().weak());
                        if ui.add(egui::DragValue::new(&mut val).speed(0.1)).changed() {
                            w.descriptor_props
                                .insert(prop.key.clone(), format!("{val}"));
                        }
                    });
                }
                DescriptorPropType::I32 => {
                    let mut val: i32 = current.parse().unwrap_or(0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&prop.display).small().weak());
                        if ui.add(egui::DragValue::new(&mut val)).changed() {
                            w.descriptor_props
                                .insert(prop.key.clone(), format!("{val}"));
                        }
                    });
                }
                DescriptorPropType::Bool => {
                    let mut val = current == "true";
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&prop.display).small().weak());
                        if ui.checkbox(&mut val, "").changed() {
                            w.descriptor_props.insert(
                                prop.key.clone(),
                                if val { "true" } else { "false" }.to_owned(),
                            );
                        }
                    });
                }
                DescriptorPropType::Enum => {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&prop.display).small().weak());
                        for option in &prop.options {
                            let selected = &current == option;
                            if ui.selectable_label(selected, option).clicked() && !selected {
                                w.descriptor_props.insert(prop.key.clone(), option.clone());
                            }
                        }
                    });
                }
            }
        }
        ui.separator();
    } else if !w.descriptor_props.is_empty() {
        // No descriptor found — show raw key→value table
        ui.label(
            egui::RichText::new("Properties (descriptor missing)")
                .small()
                .weak(),
        );
        let keys: Vec<String> = w.descriptor_props.keys().cloned().collect();
        for key in keys {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&key).small().weak());
                let mut val = w.descriptor_props.get(&key).cloned().unwrap_or_default();
                if ui.text_edit_singleline(&mut val).changed() {
                    w.descriptor_props.insert(key, val);
                }
            });
        }
        ui.separator();
    }

    show_tooltip(ui, w);
    show_enabled(ui, w);
    show_delete_button(ui, do_delete);
    edit_clicked
}
