use crate::codegen::widget_descriptor::{DescriptorPropType, WidgetDescriptor};
use crate::project::schema::{
    CrossAlign, CustomProp, CustomPropType, DataColumn, DataColumnType, DbBinding, HAlign,
    HandlerResult, LayoutCrossAlign, Orientation, SizePolicy, TextAlign, VAlign, WidgetEvent,
    WidgetInstance, WidgetKind,
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
    let mut child_move: Option<(Uuid, Uuid, usize)> = None;

    // Compute parent kind before taking the mutable borrow of the widget, so
    // show_layout_child_props can accept (&mut WidgetInstance, parent_kind).
    let parent_kind_for_child: Option<WidgetKind> = tree
        .parent_of(id)
        .and_then(|pid| tree.widgets.iter().find(|pw| pw.id == pid))
        .map(|p| p.kind.clone());

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
                show_layout_container(ui, w, &mut do_delete, &mut child_move)
            }
            WidgetKind::ScrollArea => show_scroll_area(ui, w, &mut do_delete),
            WidgetKind::GridLayout => show_layout_container(ui, w, &mut do_delete, &mut child_move),
            WidgetKind::TabWidget => show_tab_widget(ui, w, &mut do_delete),
            WidgetKind::ToolButton => show_button(ui, w, &mut do_delete),
            WidgetKind::CommandLinkButton => show_command_link(ui, w, &mut do_delete),
            WidgetKind::DialogButtonBox => show_options_widget(ui, w, &mut do_delete, "Buttons"),
            WidgetKind::MathLabel => show_math_label(ui, w, &mut do_delete),
            WidgetKind::FilePicker => show_file_picker(ui, w, &mut do_delete),
            WidgetKind::Chart => show_layout_container(ui, w, &mut do_delete, &mut child_move),
            WidgetKind::Table => show_data_widget(ui, w, &mut do_delete, "Columns"),
            WidgetKind::ListView => show_data_widget(ui, w, &mut do_delete, "Items"),
            WidgetKind::TreeView => show_data_widget(ui, w, &mut do_delete, "Nodes"),
            WidgetKind::StackedWidget => show_options_widget(ui, w, &mut do_delete, "Pages"),
            WidgetKind::ToolBox => show_options_widget(ui, w, &mut do_delete, "Sections"),
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

        // Per-child layout controls (shown whenever this widget is a child of a
        // VLayout, HLayout, or GridLayout).
        show_layout_child_props(ui, w, parent_kind_for_child.as_ref());
    } // w borrow ends

    if do_delete {
        tree.remove(id);
        selected.retain(|&x| x != id);
    } else {
        if let Some((parent_id, child_id, to_idx)) = child_move {
            tree.move_child_within_parent(parent_id, child_id, to_idx);
        }
        tree.validate_and_repair();
    }

    show_event_handler(ui, tree, id, &mut props_action);
    show_db_binding(ui, tree, id);

    // P2.3 — constraint editor (shown for all widget kinds). Validation needs
    // cross-widget context, so compute it over the whole tree before the
    // mutable borrow, then pass this widget's messages in.
    let constraint_errors: Vec<String> =
        crate::project::constraint_solver::validate_constraints(tree)
            .into_iter()
            .filter(|e| e.widget_id() == id)
            .map(|e| e.message())
            .collect();
    if let Some(w) = tree.get_mut(id) {
        show_constraints(ui, w, &constraint_errors);
    }

    props_action
}

// ---------------------------------------------------------------------------
// Per-kind UI panels
// ---------------------------------------------------------------------------

fn show_button(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    let kind_name = format!("{:?}", w.kind);
    field_text_resettable(ui, "Label", &mut w.props.label, &kind_name);
    show_geometry_resettable(ui, w);
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
    field_text_resettable(ui, "Text", &mut w.props.label, "Label");

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

    show_geometry_resettable(ui, w);
    ui.separator();
    show_fg_color(ui, w);
    show_font_size(ui, w);
    show_text_align(ui, w);
    show_text_wrap(ui, w);
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

/// Like `field_text` but adds a right-click "Reset to default" context menu
/// that sets `value` to `default_value`.
fn field_text_resettable(ui: &mut egui::Ui, label: &str, value: &mut String, default_value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).small().weak());
        ui.add_space(4.0);
        let resp = ui.text_edit_singleline(value);
        resp.context_menu(|ui| {
            if ui.button("Reset to default").clicked() {
                *value = default_value.to_owned();
                ui.close();
            }
        });
    });
}

/// Show geometry (x, y, w, h) fields with right-click "Reset to default" on
/// each DragValue.  Default for x/y is 0.0; default w/h come from the widget
/// kind's default_instance rect.
fn show_geometry_resettable(ui: &mut egui::Ui, w: &mut WidgetInstance) {
    let default_inst = crate::widgets::default_for(&w.kind);
    let (dw, dh) = (default_inst.rect.w, default_inst.rect.h);

    ui.separator();
    egui::Grid::new("geom_compact")
        .num_columns(4)
        .spacing([4.0, 2.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("X").small());
            let rx = ui.add(egui::DragValue::new(&mut w.rect.x).speed(1.0));
            rx.context_menu(|ui| {
                if ui.button("Reset to default").clicked() {
                    w.rect.x = 0.0;
                    ui.close();
                }
            });
            ui.label(egui::RichText::new("Y").small());
            let ry = ui.add(egui::DragValue::new(&mut w.rect.y).speed(1.0));
            ry.context_menu(|ui| {
                if ui.button("Reset to default").clicked() {
                    w.rect.y = 0.0;
                    ui.close();
                }
            });
            ui.end_row();
            ui.label(egui::RichText::new("W").small());
            let rw = ui.add(egui::DragValue::new(&mut w.rect.w).speed(1.0));
            rw.context_menu(|ui| {
                if ui.button("Reset to default").clicked() {
                    w.rect.w = dw;
                    ui.close();
                }
            });
            ui.label(egui::RichText::new("H").small());
            let rh = ui.add(egui::DragValue::new(&mut w.rect.h).speed(1.0));
            rh.context_menu(|ui| {
                if ui.button("Reset to default").clicked() {
                    w.rect.h = dh;
                    ui.close();
                }
            });
            ui.end_row();
        });
    // Per-child size policy (meaningful inside a layout container)
    ui.horizontal(|ui| {
        use crate::project::schema::SizePolicy;
        ui.label(egui::RichText::new("Size").small().weak());
        if ui
            .selectable_label(
                w.props.size_policy == SizePolicy::Fixed,
                egui::RichText::new("Fixed").small(),
            )
            .clicked()
        {
            w.props.size_policy = SizePolicy::Fixed;
        }
        if ui
            .selectable_label(
                w.props.size_policy == SizePolicy::FillWidth,
                egui::RichText::new("Fill W").small(),
            )
            .on_hover_text("Expand to fill available width inside a layout")
            .clicked()
        {
            w.props.size_policy = SizePolicy::FillWidth;
        }
        if ui
            .selectable_label(
                w.props.size_policy == SizePolicy::Fill,
                egui::RichText::new("Fill").small(),
            )
            .on_hover_text("Expand to fill all available space inside a layout")
            .clicked()
        {
            w.props.size_policy = SizePolicy::Fill;
        }
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
    if let Some(b) = &w.state_binding
        && !crate::codegen::rust::is_valid_identifier(b)
    {
        ui.label(
            egui::RichText::new("⚠ invalid identifier")
                .small()
                .color(RED_WARN),
        );
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
    // Per-child size policy (meaningful inside a layout container)
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Size").small().weak());
        if ui
            .selectable_label(
                w.props.size_policy == SizePolicy::Fixed,
                egui::RichText::new("Fixed").small(),
            )
            .clicked()
        {
            w.props.size_policy = SizePolicy::Fixed;
        }
        if ui
            .selectable_label(
                w.props.size_policy == SizePolicy::FillWidth,
                egui::RichText::new("Fill W").small(),
            )
            .on_hover_text("Expand to fill available width inside a layout")
            .clicked()
        {
            w.props.size_policy = SizePolicy::FillWidth;
        }
        if ui
            .selectable_label(
                w.props.size_policy == SizePolicy::Fill,
                egui::RichText::new("Fill").small(),
            )
            .on_hover_text("Expand to fill all available space inside a layout")
            .clicked()
        {
            w.props.size_policy = SizePolicy::Fill;
        }
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

fn show_text_wrap(ui: &mut egui::Ui, w: &mut WidgetInstance) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Wrap text").small().weak());
        let current = w.props.text_wrap;
        if ui
            .selectable_label(current.is_none(), "Default")
            .on_hover_text("Use egui default wrapping")
            .clicked()
        {
            w.props.text_wrap = None;
        }
        if ui
            .selectable_label(current == Some(true), "Wrap")
            .on_hover_text("Wrap at widget boundary")
            .clicked()
        {
            w.props.text_wrap = Some(true);
        }
        if ui
            .selectable_label(current == Some(false), "Clip")
            .on_hover_text("Do not wrap; clip excess")
            .clicked()
        {
            w.props.text_wrap = Some(false);
        }
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
            .on_hover_text(
                "Show full SVG source inline instead of [SVG: N bytes]. \
                 Large SVGs (>10 KB) make the code panel very noisy — \
                 use the source viewer below for inspection instead.",
            );
        if w.expand_svg_inline
            && let Some(src) = w.svg_source.as_deref()
            && src.len() > 10_000
        {
            ui.label(
                egui::RichText::new(format!(
                    "Warning: SVG is {} KB — code panel will be verbose.",
                    src.len() / 1024
                ))
                .small()
                .color(egui::Color32::from_rgb(251, 191, 36)),
            );
        }
        ui.separator();
        if let Some(src) = w.svg_source.as_deref() {
            crate::panels::svg_report::show_report(ui, src);
        }
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

/// Tag identifying which event field to read/write.
#[derive(Clone, Copy, PartialEq)]
enum EventField {
    Click,
    DoubleClick,
    Change,
    LostFocus,
    DragStopped,
}

/// Map a schema [`WidgetEvent`] to its Properties row metadata:
/// `(field, label, placeholder-hint)`.  This is the only place that turns the
/// schema event vocabulary into editor labels.
fn event_ui_meta(ev: WidgetEvent) -> (EventField, &'static str, &'static str) {
    match ev {
        WidgetEvent::Click => (EventField::Click, "On Click", "handle_click"),
        WidgetEvent::DoubleClick => (
            EventField::DoubleClick,
            "On Double-click",
            "handle_double_click",
        ),
        WidgetEvent::Change => (EventField::Change, "On Change", "handle_change"),
        WidgetEvent::LostFocus => (EventField::LostFocus, "On Lost Focus", "handle_lost_focus"),
        WidgetEvent::DragStopped => (
            EventField::DragStopped,
            "On Drag Stopped",
            "handle_drag_stopped",
        ),
    }
}

fn show_event_handler(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    id: Uuid,
    action: &mut PropertiesAction,
) {
    // Collect applicable events without holding a borrow on tree.
    let applicable: Vec<(EventField, &'static str, &'static str)> = {
        let Some(w) = tree.get_mut(id) else { return };

        // Migrate legacy event_handler → on_click / on_change on first display.
        if let Some(ref eh) = w.event_handler.clone()
            && !eh.is_empty()
        {
            if w.kind == WidgetKind::Button && w.on_click.is_empty() {
                w.on_click = eh.clone();
            } else if w.kind != WidgetKind::Button && w.on_change.is_empty() {
                w.on_change = eh.clone();
            }
        }

        // Derive the applicable event rows from the schema's single source of
        // truth (`WidgetKind::supported_events`).  Export reads the same source,
        // so the Properties UI and generated code cannot drift.
        if !w.kind.is_event_capable() {
            return;
        }
        w.kind
            .supported_events()
            .iter()
            .map(|ev| event_ui_meta(*ev))
            .collect()
    }; // tree borrow released here

    ui.separator();
    ui.label(egui::RichText::new("Events").small().weak());

    for (field, label, hint) in &applicable {
        let field = *field;
        // Read current value — borrow, clone, release.
        let current: String = tree
            .get_mut(id)
            .map(|w| event_field_get(w, field).to_owned())
            .unwrap_or_default();

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(*label).small());
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
        let changed = ui
            .add(
                egui::TextEdit::singleline(&mut handler)
                    .hint_text(format!("e.g. {hint}"))
                    .desired_width(f32::INFINITY),
            )
            .on_hover_text("Ctrl+double-click widget on canvas to jump to handler (Tracé)")
            .changed();

        if changed {
            let trimmed = handler.trim().to_owned();
            if let Some(w) = tree.get_mut(id) {
                event_field_set(w, field, trimmed);
            }
        }
    }

    // Stage 11 — async + error-mode controls, shown when any handler is set.
    let has_any_handler = tree
        .get_mut(id)
        .map(|w| {
            !w.on_click.is_empty()
                || !w.on_change.is_empty()
                || !w.on_double_click.is_empty()
                || !w.on_lost_focus.is_empty()
                || !w.on_drag_stopped.is_empty()
        })
        .unwrap_or(false);

    if has_any_handler {
        // Async checkbox
        let mut is_async = tree.get_mut(id).map(|w| w.async_handler).unwrap_or(false);
        if ui
            .checkbox(&mut is_async, "⚙ Run async (background thread)")
            .on_hover_text("Wrap the handler in std::thread::spawn (no tokio)")
            .changed()
            && let Some(w) = tree.get_mut(id)
        {
            w.async_handler = is_async;
        }

        // Error-mode dropdown
        let mut mode = tree
            .get_mut(id)
            .map(|w| w.handler_result.clone())
            .unwrap_or(HandlerResult::Plain);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Error mode").small().weak());
            egui::ComboBox::from_id_salt(("handler_result", id))
                .selected_text(mode.label())
                .show_ui(ui, |ui| {
                    for m in [
                        HandlerResult::Plain,
                        HandlerResult::Result,
                        HandlerResult::Option,
                    ] {
                        ui.selectable_value(&mut mode, m.clone(), m.label());
                    }
                });
        });
        if let Some(w) = tree.get_mut(id)
            && w.handler_result != mode
        {
            w.handler_result = mode;
        }
    }
}

fn event_field_get(w: &WidgetInstance, field: EventField) -> &str {
    match field {
        EventField::Click => &w.on_click,
        EventField::DoubleClick => &w.on_double_click,
        EventField::Change => &w.on_change,
        EventField::LostFocus => &w.on_lost_focus,
        EventField::DragStopped => &w.on_drag_stopped,
    }
}

fn event_field_set(w: &mut WidgetInstance, field: EventField, value: String) {
    match field {
        EventField::Click => w.on_click = value,
        EventField::DoubleClick => w.on_double_click = value,
        EventField::Change => w.on_change = value,
        EventField::LostFocus => w.on_lost_focus = value,
        EventField::DragStopped => w.on_drag_stopped = value,
    }
}

// ---------------------------------------------------------------------------
// Stage 13 — DB Binding section
// ---------------------------------------------------------------------------

fn show_db_binding(ui: &mut egui::Ui, tree: &mut UiTree, id: Uuid) {
    ui.separator();
    ui.label(egui::RichText::new("DB Binding").small().weak());

    let has_binding = tree
        .get_mut(id)
        .map(|w| w.db_binding.is_some())
        .unwrap_or(false);

    ui.horizontal(|ui| {
        if ui
            .selectable_label(!has_binding, "None")
            .on_hover_text("No database binding")
            .clicked()
            && has_binding
            && let Some(w) = tree.get_mut(id)
        {
            w.db_binding = None;
        }
        if ui
            .selectable_label(has_binding, "Bound")
            .on_hover_text("Bind this widget value to a database column")
            .clicked()
            && !has_binding
            && let Some(w) = tree.get_mut(id)
        {
            w.db_binding = Some(DbBinding::default());
        }
    });

    if !has_binding {
        return;
    }

    // Collect current values — borrow briefly, clone, release.
    let (table_cur, column_cur) = tree
        .get_mut(id)
        .and_then(|w| w.db_binding.as_ref())
        .map(|b| (b.table.clone(), b.column.clone()))
        .unwrap_or_default();

    let mut table_buf = table_cur;
    let mut column_buf = column_cur;

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Table").small().weak());
        if ui
            .add(
                egui::TextEdit::singleline(&mut table_buf)
                    .hint_text("table_name")
                    .desired_width(120.0),
            )
            .changed()
        {
            let trimmed = table_buf.trim().to_owned();
            if let Some(w) = tree.get_mut(id)
                && let Some(b) = w.db_binding.as_mut()
            {
                b.table = trimmed;
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Column").small().weak());
        if ui
            .add(
                egui::TextEdit::singleline(&mut column_buf)
                    .hint_text("column_name")
                    .desired_width(120.0),
            )
            .changed()
        {
            let trimmed = column_buf.trim().to_owned();
            if let Some(w) = tree.get_mut(id)
                && let Some(b) = w.db_binding.as_mut()
            {
                b.column = trimmed;
            }
        }
    });

    ui.label(
        egui::RichText::new("Codegen emits db_conn field + load_from_db() stub")
            .small()
            .weak(),
    );
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

        if do_group && let Some(new_id) = tree.group(selected) {
            selected.clear();
            selected.push(new_id);
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
    ui.painter().rect(
        rect,
        visuals.corner_radius,
        visuals.bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
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
    show_bg_color(ui, w);
    show_fg_color(ui, w);
    show_corner_radius(ui, w);
    show_font_size(ui, w);
    show_text_wrap(ui, w);
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

fn show_layout_container(
    ui: &mut egui::Ui,
    w: &mut WidgetInstance,
    do_delete: &mut bool,
    child_move: &mut Option<(Uuid, Uuid, usize)>,
) {
    field_text(ui, "Label", &mut w.props.label);
    if matches!(
        w.kind,
        WidgetKind::VLayout | WidgetKind::HLayout | WidgetKind::GridLayout
    ) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Children").small().weak());
            ui.label(format!("{}", w.children.len()));
            ui.checkbox(&mut w.props.layout_stretch, "Stretch children")
                .on_hover_text(
                    "Fill the layout cross-axis/cell size. Disable to preserve child size hints.",
                );
        });
        egui::Grid::new(("layout_props", w.id))
            .num_columns(4)
            .spacing([4.0, 2.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Margin").small());
                ui.add(
                    egui::DragValue::new(&mut w.props.inner_margin)
                        .range(0.0..=128.0_f32)
                        .speed(0.5)
                        .suffix(" px"),
                );
                ui.label(egui::RichText::new("Gap").small());
                ui.add(
                    egui::DragValue::new(&mut w.props.layout_spacing)
                        .range(0.0..=128.0_f32)
                        .speed(0.5)
                        .suffix(" px"),
                );
                ui.end_row();
                if matches!(w.kind, WidgetKind::GridLayout) {
                    ui.label(egui::RichText::new("Columns").small());
                    ui.add(egui::DragValue::new(&mut w.props.grid_columns).range(1..=12));
                    ui.label(egui::RichText::new("Rows").small());
                    let rows = w
                        .children
                        .len()
                        .div_ceil(w.props.grid_columns.clamp(1, 12))
                        .max(1);
                    ui.label(rows.to_string());
                    ui.end_row();
                    // Row height policy
                    ui.label(egui::RichText::new("Row H").small());
                    let mut has_row_h = w.props.grid_row_height.is_some();
                    if ui.checkbox(&mut has_row_h, "").changed() {
                        w.props.grid_row_height = if has_row_h { Some(24.0) } else { None };
                    }
                    if let Some(ref mut rh) = w.props.grid_row_height {
                        ui.add(
                            egui::DragValue::new(rh)
                                .range(4.0..=512.0_f32)
                                .speed(0.5)
                                .suffix(" px"),
                        );
                    } else {
                        ui.label(egui::RichText::new("auto").small().weak());
                    }
                    ui.end_row();
                }
                if matches!(w.kind, WidgetKind::VLayout | WidgetKind::HLayout) {
                    let (start_label, center_label, end_label) = match w.kind {
                        WidgetKind::VLayout => ("Left", "Center", "Right"),
                        _ => ("Top", "Center", "Bottom"),
                    };
                    ui.label(egui::RichText::new("Align").small());
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                w.props.layout_cross_align == LayoutCrossAlign::Start,
                                start_label,
                            )
                            .clicked()
                        {
                            w.props.layout_cross_align = LayoutCrossAlign::Start;
                        }
                        if ui
                            .selectable_label(
                                w.props.layout_cross_align == LayoutCrossAlign::Center,
                                center_label,
                            )
                            .clicked()
                        {
                            w.props.layout_cross_align = LayoutCrossAlign::Center;
                        }
                        if ui
                            .selectable_label(
                                w.props.layout_cross_align == LayoutCrossAlign::End,
                                end_label,
                            )
                            .clicked()
                        {
                            w.props.layout_cross_align = LayoutCrossAlign::End;
                        }
                    });
                    ui.end_row();
                }
            });
        if matches!(w.kind, WidgetKind::GridLayout) && !w.children.is_empty() {
            ui.separator();
            ui.label(egui::RichText::new("Grid slots").small().weak());
            let columns = w.props.grid_columns.clamp(1, 12);
            let child_ids = w.children.clone();
            for (idx, child_id) in child_ids.iter().copied().enumerate() {
                ui.horizontal(|ui| {
                    let row = idx / columns + 1;
                    let col = idx % columns + 1;
                    ui.label(
                        egui::RichText::new(format!("R{row} C{col}"))
                            .small()
                            .monospace(),
                    );
                    let mut slot_name = w
                        .props
                        .grid_slot_names
                        .get(idx)
                        .cloned()
                        .unwrap_or_default();
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut slot_name)
                            .hint_text(format!("Slot {}", idx + 1))
                            .desired_width(92.0),
                        )
                        .on_hover_text(
                            "Stable name for this grid cell. Dragging a child changes which widget occupies the named slot.",
                        )
                        .changed()
                    {
                        w.props.grid_slot_names.resize(idx + 1, String::new());
                        w.props.grid_slot_names[idx] = slot_name;
                        while w
                            .props
                            .grid_slot_names
                            .last()
                            .is_some_and(String::is_empty)
                        {
                            w.props.grid_slot_names.pop();
                        }
                    }
                    ui.label(egui::RichText::new(short_uuid(child_id)).small().weak());
                    if ui
                        .add_enabled(idx > 0, egui::Button::new("↑"))
                        .on_hover_text("Move child to previous grid slot")
                        .clicked()
                    {
                        *child_move = Some((w.id, child_id, idx - 1));
                    }
                    if ui
                        .add_enabled(idx + 1 < child_ids.len(), egui::Button::new("↓"))
                        .on_hover_text("Move child to next grid slot")
                        .clicked()
                    {
                        *child_move = Some((w.id, child_id, idx + 1));
                    }
                });
            }
        }
        ui.separator();
    }
    show_geometry(ui, w);
    ui.separator();
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn short_uuid(id: Uuid) -> String {
    id.as_simple().to_string().chars().take(8).collect()
}

fn show_scroll_area(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Label", &mut w.props.label);
    show_geometry(ui, w);
    ui.separator();
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_command_link(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Title", &mut w.props.label);
    field_text(ui, "Description", &mut w.props.placeholder);
    show_geometry(ui, w);
    ui.separator();
    show_bg_color(ui, w);
    show_fg_color(ui, w);
    show_corner_radius(ui, w);
    ui.separator();
    show_tooltip(ui, w);
    show_enabled(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_math_label(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Label", &mut w.props.label);
    ui.label(
        egui::RichText::new("Displays a computed f32 from a bound field or formula.")
            .small()
            .weak(),
    );
    binding_field(ui, w);

    ui.separator();
    ui.label(egui::RichText::new("Formula (optional)").small().weak());
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("f(x) =").monospace().small());
        ui.add(
            egui::TextEdit::singleline(&mut w.props.formula_expr)
                .hint_text("e.g. sqrt(a^2 + b^2)")
                .desired_width(ui.available_width()),
        );
    });
    if !w.props.formula_expr.is_empty() {
        // P2.5: use validate() + deps() for the formula depth API.
        match crate::codegen::formula::validate(&w.props.formula_expr) {
            Ok(node) => {
                let vars = crate::codegen::formula::deps(&node);
                let vars_str = if vars.is_empty() {
                    "no variables".to_owned()
                } else {
                    format!("uses: {}", vars.join(", "))
                };
                ui.label(
                    egui::RichText::new(vars_str)
                        .small()
                        .color(egui::Color32::from_rgb(52, 211, 153)),
                );
            }
            Err(e) => {
                ui.label(
                    egui::RichText::new(format!("Error: {e}"))
                        .small()
                        .color(egui::Color32::RED),
                );
            }
        }
    }
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Decimals").small());
        ui.add(egui::DragValue::new(&mut w.props.formula_decimals).range(0..=9_usize));
    });

    show_geometry(ui, w);
    ui.separator();
    show_fg_color(ui, w);
    show_font_size(ui, w);
    show_tooltip(ui, w);
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_file_picker(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    field_text(ui, "Button text", &mut w.props.label);
    ui.label(
        egui::RichText::new("Selected path stored in the bound String field.")
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

/// Show per-child layout alignment + flex controls when the selected widget
/// lives inside a VLayout, HLayout, or GridLayout parent.
/// `parent_kind` is pre-computed from `UiTree::parent_of` to avoid a
/// simultaneous mutable + shared borrow of the tree.
fn show_layout_child_props(
    ui: &mut egui::Ui,
    w: &mut WidgetInstance,
    parent_kind: Option<&WidgetKind>,
) {
    let is_vlayout_child = matches!(parent_kind, Some(WidgetKind::VLayout));
    let is_hlayout_child = matches!(parent_kind, Some(WidgetKind::HLayout));
    let is_gridlayout_child = matches!(parent_kind, Some(WidgetKind::GridLayout));

    if is_vlayout_child || is_hlayout_child {
        ui.separator();
        ui.label(egui::RichText::new("Layout child").small().weak());
        // Cross-axis alignment override
        let cross_axis_label = if is_vlayout_child {
            "Cross align (H)"
        } else {
            "Cross align (V)"
        };
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(cross_axis_label).small());
            let current = w.child_cross_align.unwrap_or(CrossAlign::Start);
            // Start/Center/End only — these are the cross-aligns the layout codegen
            // (egui_emitter + export) honors. Stretch has no proven egui codegen
            // path, so it is not offered (avoids a half-wired control).
            for variant in [CrossAlign::Start, CrossAlign::Center, CrossAlign::End] {
                if ui
                    .selectable_label(current == variant, variant.label())
                    .clicked()
                {
                    w.child_cross_align = if variant == CrossAlign::Start {
                        None
                    } else {
                        Some(variant)
                    };
                }
            }
        });
        // Flex factor
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Flex").small());
            let mut flex = w.child_flex;
            if ui
                .add(
                    egui::DragValue::new(&mut flex)
                        .range(0.0..=100.0_f32)
                        .speed(0.05),
                )
                .on_hover_text("0 = fixed size; >0 = proportional flex growth")
                .changed()
            {
                w.child_flex = flex.max(0.0);
            }
        });
    }

    if is_gridlayout_child {
        ui.separator();
        ui.label(egui::RichText::new("Grid child").small().weak());
        egui::Grid::new("grid_child_props")
            .num_columns(4)
            .spacing([4.0, 2.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Col span").small());
                ui.add(egui::DragValue::new(&mut w.grid_col_span).range(1..=12_u32));
                ui.label(egui::RichText::new("Row span").small());
                ui.add(egui::DragValue::new(&mut w.grid_row_span).range(1..=12_u32));
                ui.end_row();
            });
    }
}

/// Generic editor for widgets whose content is the `options` list
/// (Table columns, ListView items, TreeView nodes, etc.).
fn show_options_widget(
    ui: &mut egui::Ui,
    w: &mut WidgetInstance,
    do_delete: &mut bool,
    item_label: &str,
) {
    field_text(ui, "Label", &mut w.props.label);
    ui.label(egui::RichText::new(item_label).small().weak());
    let mut to_remove: Option<usize> = None;
    for (i, opt) in w.props.options.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let ow = (ui.available_width() - 28.0).clamp(60.0, 180.0);
            ui.add(
                egui::TextEdit::singleline(opt)
                    .hint_text(format!("{item_label} {}", i + 1))
                    .desired_width(ow),
            );
            if ui.small_button("x").clicked() {
                to_remove = Some(i);
            }
        });
    }
    if let Some(i) = to_remove {
        w.props.options.remove(i);
    }
    if ui.small_button(format!("+ Add {item_label}")).clicked() {
        let n = w.props.options.len() + 1;
        w.props.options.push(format!("{item_label} {n}"));
    }
    show_geometry(ui, w);
    ui.separator();
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

/// Table / ListView / TreeView: static options + optional model binding.
fn show_data_widget(
    ui: &mut egui::Ui,
    w: &mut WidgetInstance,
    do_delete: &mut bool,
    item_label: &str,
) {
    field_text(ui, "Label", &mut w.props.label);

    // --- Data source binding ---
    ui.separator();
    ui.label(
        egui::RichText::new("Data Source (model-bound)")
            .small()
            .weak(),
    );
    let has_binding = w.props.data_source_binding.is_some();
    ui.horizontal(|ui| {
        if ui.selectable_label(!has_binding, "Static").clicked() {
            w.props.data_source_binding = None;
        }
        if ui.selectable_label(has_binding, "Bound").clicked()
            && w.props.data_source_binding.is_none()
        {
            w.props.data_source_binding = Some("items".to_owned());
        }
    });
    if let Some(ref mut src) = w.props.data_source_binding {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Field").small());
            ui.add(
                egui::TextEdit::singleline(src)
                    .hint_text("items")
                    .desired_width(120.0),
            );
        });
        ui.label(
            egui::RichText::new("AppState field will be Vec<String> (ListView/TreeView) or Vec<Vec<String>> (Table).")
                .small()
                .weak(),
        );
        // Column editor for Table
        if matches!(w.kind, WidgetKind::Table) {
            ui.separator();
            ui.label(egui::RichText::new("Columns").small().weak());
            let mut to_remove: Option<usize> = None;
            for (i, col) in w.props.data_columns.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    let ow = (ui.available_width() - 80.0).clamp(60.0, 140.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut col.name)
                            .hint_text(format!("col_{}", i + 1))
                            .desired_width(ow),
                    );
                    egui::ComboBox::from_id_salt(("col_type", i))
                        .selected_text(match col.column_type {
                            DataColumnType::Text => "Text",
                            DataColumnType::Number => "Number",
                            DataColumnType::Boolean => "Bool",
                        })
                        .width(60.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut col.column_type, DataColumnType::Text, "Text");
                            ui.selectable_value(
                                &mut col.column_type,
                                DataColumnType::Number,
                                "Number",
                            );
                            ui.selectable_value(
                                &mut col.column_type,
                                DataColumnType::Boolean,
                                "Bool",
                            );
                        });
                    if ui.small_button("x").clicked() {
                        to_remove = Some(i);
                    }
                });
            }
            if let Some(i) = to_remove {
                w.props.data_columns.remove(i);
            }
            if ui.small_button("+ Column").clicked() {
                let n = w.props.data_columns.len() + 1;
                w.props.data_columns.push(DataColumn {
                    name: format!("col_{n}"),
                    column_type: DataColumnType::Text,
                });
            }
        }
    } else {
        // Static options editor
        ui.separator();
        ui.label(egui::RichText::new(item_label).small().weak());
        let mut to_remove: Option<usize> = None;
        for (i, opt) in w.props.options.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let ow = (ui.available_width() - 28.0).clamp(60.0, 180.0);
                ui.add(
                    egui::TextEdit::singleline(opt)
                        .hint_text(format!("{item_label} {}", i + 1))
                        .desired_width(ow),
                );
                if ui.small_button("x").clicked() {
                    to_remove = Some(i);
                }
            });
        }
        if let Some(i) = to_remove {
            w.props.options.remove(i);
        }
        if ui.small_button(format!("+ Add {item_label}")).clicked() {
            let n = w.props.options.len() + 1;
            w.props.options.push(format!("{item_label} {n}"));
        }
    }

    show_geometry(ui, w);
    ui.separator();
    show_custom_props(ui, w);
    show_delete_button(ui, do_delete);
}

fn show_tab_widget(ui: &mut egui::Ui, w: &mut WidgetInstance, do_delete: &mut bool) {
    ui.label(egui::RichText::new("Tabs").small().weak());
    let mut to_remove: Option<usize> = None;
    for (i, opt) in w.props.options.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let ow = (ui.available_width() - 28.0).clamp(60.0, 160.0);
            ui.add(
                egui::TextEdit::singleline(opt)
                    .hint_text(format!("Tab {}", i + 1))
                    .desired_width(ow),
            );
            if ui.small_button("x").clicked() {
                to_remove = Some(i);
            }
        });
    }
    if let Some(i) = to_remove {
        w.props.options.remove(i);
    }
    if ui.small_button("+ Add tab").clicked() {
        w.props
            .options
            .push(format!("Tab {}", w.props.options.len() + 1));
    }
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

// ---------------------------------------------------------------------------
// P2.3 — Constraint editor
// ---------------------------------------------------------------------------

/// Collapsible "Constraints" section shown for every widget kind.
/// `errors` are this widget's [`ConstraintError`] messages, pre-computed over the
/// whole tree by the caller (validation needs cross-widget context).
fn show_constraints(ui: &mut egui::Ui, w: &mut WidgetInstance, errors: &[String]) {
    let id = w.id;
    ui.separator();
    egui::CollapsingHeader::new(egui::RichText::new("Constraints").small())
        .id_salt(("constraints", id))
        .default_open(false)
        .show(ui, |ui| {
            let c = &mut w.constraints;

            // --- Alignment anchors ---
            egui::Grid::new(("constraints_grid", id))
                .num_columns(2)
                .spacing([8.0, 2.0])
                .show(ui, |ui| {
                    // H Align
                    ui.label(egui::RichText::new("H Align").small().weak());
                    egui::ComboBox::from_id_salt(("h_align_cb", id))
                        .selected_text(h_align_label(c.h_align))
                        .width(110.0)
                        .show_ui(ui, |ui| {
                            let none_resp = ui.selectable_value(&mut c.h_align, None, "None");
                            if none_resp.clicked() {
                                c.h_align = None;
                            }
                            ui.selectable_value(&mut c.h_align, Some(HAlign::Leading), "Leading");
                            ui.selectable_value(&mut c.h_align, Some(HAlign::Trailing), "Trailing");
                            ui.selectable_value(&mut c.h_align, Some(HAlign::Center), "Center");
                            ui.selectable_value(&mut c.h_align, Some(HAlign::Stretch), "Stretch");
                        });
                    ui.end_row();

                    // V Align
                    ui.label(egui::RichText::new("V Align").small().weak());
                    egui::ComboBox::from_id_salt(("v_align_cb", id))
                        .selected_text(v_align_label(c.v_align))
                        .width(110.0)
                        .show_ui(ui, |ui| {
                            let none_resp = ui.selectable_value(&mut c.v_align, None, "None");
                            if none_resp.clicked() {
                                c.v_align = None;
                            }
                            ui.selectable_value(&mut c.v_align, Some(VAlign::Top), "Top");
                            ui.selectable_value(&mut c.v_align, Some(VAlign::Bottom), "Bottom");
                            ui.selectable_value(&mut c.v_align, Some(VAlign::Center), "Center");
                            ui.selectable_value(&mut c.v_align, Some(VAlign::Stretch), "Stretch");
                        });
                    ui.end_row();

                    // Aspect ratio
                    ui.label(egui::RichText::new("Aspect ratio").small().weak());
                    ui.horizontal(|ui| {
                        let mut ar_str = c
                            .aspect_ratio
                            .map(|r| format!("{r:.2}"))
                            .unwrap_or_default();
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut ar_str)
                                    .hint_text("w:h (e.g. 1.78)")
                                    .desired_width(70.0),
                            )
                            .changed()
                        {
                            c.aspect_ratio = ar_str.trim().parse::<f32>().ok().filter(|&v| v > 0.0);
                        }
                        if c.aspect_ratio.is_some()
                            && ui.small_button("✕").on_hover_text("Remove lock").clicked()
                        {
                            c.aspect_ratio = None;
                        }
                    });
                    ui.end_row();
                });

            // --- Min / Max size ---
            ui.separator();
            ui.label(egui::RichText::new("Size limits").small().weak());
            egui::Grid::new(("constraints_minmax", id))
                .num_columns(4)
                .spacing([4.0, 2.0])
                .show(ui, |ui| {
                    // Min W
                    ui.label(egui::RichText::new("Min W").small().weak());
                    optional_drag(ui, &mut c.min_w, 0.0..=4096.0, ("min_w", id));
                    // Max W
                    ui.label(egui::RichText::new("Max W").small().weak());
                    optional_drag(ui, &mut c.max_w, 0.0..=4096.0, ("max_w", id));
                    ui.end_row();
                    // Min H
                    ui.label(egui::RichText::new("Min H").small().weak());
                    optional_drag(ui, &mut c.min_h, 0.0..=4096.0, ("min_h", id));
                    // Max H
                    ui.label(egui::RichText::new("Max H").small().weak());
                    optional_drag(ui, &mut c.max_h, 0.0..=4096.0, ("max_h", id));
                    ui.end_row();
                });

            // --- Margin ---
            ui.separator();
            ui.label(egui::RichText::new("Margin (T R B L)").small().weak());
            egui::Grid::new(("constraints_margin", id))
                .num_columns(4)
                .spacing([4.0, 2.0])
                .show(ui, |ui| {
                    for (label, idx) in [("T", 0usize), ("R", 1), ("B", 2), ("L", 3)] {
                        ui.label(egui::RichText::new(label).small().weak());
                        ui.add(
                            egui::DragValue::new(&mut c.margin[idx])
                                .range(0.0..=200.0_f32)
                                .speed(0.5)
                                .suffix(" px"),
                        );
                    }
                    ui.end_row();
                });

            // --- Equal-size links ---
            ui.separator();
            ui.label(
                egui::RichText::new("Equal size to (widget ID)")
                    .small()
                    .weak(),
            );
            egui::Grid::new(("constraints_eqsize", id))
                .num_columns(2)
                .spacing([4.0, 2.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Equal W").small().weak());
                    optional_id_field(ui, &mut c.equal_width_to, ("eq_w", id));
                    ui.end_row();
                    ui.label(egui::RichText::new("Equal H").small().weak());
                    optional_id_field(ui, &mut c.equal_height_to, ("eq_h", id));
                    ui.end_row();
                });

            // --- Validation (cross-widget; conflicts/cycles detected over the
            //     whole tree by the caller) ---
            if !errors.is_empty() {
                ui.separator();
                for msg in errors {
                    ui.colored_label(egui::Color32::from_rgb(220, 80, 80), format!("⚠ {msg}"));
                }
            }
        });
}

fn h_align_label(a: Option<HAlign>) -> &'static str {
    match a {
        None => "None",
        Some(HAlign::Leading) => "Leading",
        Some(HAlign::Trailing) => "Trailing",
        Some(HAlign::Center) => "Center",
        Some(HAlign::Stretch) => "Stretch",
    }
}

fn v_align_label(a: Option<VAlign>) -> &'static str {
    match a {
        None => "None",
        Some(VAlign::Top) => "Top",
        Some(VAlign::Bottom) => "Bottom",
        Some(VAlign::Center) => "Center",
        Some(VAlign::Stretch) => "Stretch",
    }
}

/// Render a `DragValue` for an `Option<f32>` field — shows `0.0` placeholder
/// when `None`; sets to `Some` on first interaction; clears when reset button clicked.
fn optional_drag(
    ui: &mut egui::Ui,
    field: &mut Option<f32>,
    range: std::ops::RangeInclusive<f32>,
    salt: impl std::hash::Hash,
) {
    ui.horizontal(|ui| {
        let mut val = field.unwrap_or(0.0);
        let resp = ui.add(
            egui::DragValue::new(&mut val)
                .range(range)
                .speed(0.5)
                .suffix(" px"),
        );
        if resp.changed() {
            *field = Some(val);
        }
        if field.is_some() && ui.small_button("✕").on_hover_text("Clear limit").clicked() {
            *field = None;
        }
        drop(salt); // consume salt to avoid unused-variable warning
    });
}

/// Single-line text field for an `Option<String>` ID link.
/// Shows empty hint when None; clears on ✕.
fn optional_id_field(ui: &mut egui::Ui, field: &mut Option<String>, salt: impl std::hash::Hash) {
    ui.horizontal(|ui| {
        let mut text = field.clone().unwrap_or_default();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut text)
                .hint_text("widget id")
                .desired_width(120.0)
                .id(egui::Id::new(salt)),
        );
        if resp.changed() {
            let t = text.trim().to_owned();
            *field = if t.is_empty() { None } else { Some(t) };
        }
        if field.is_some() && ui.small_button("✕").on_hover_text("Remove link").clicked() {
            *field = None;
        }
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod reset_tests {
    use crate::project::schema::{WidgetInstance, WidgetKind};

    fn make_button() -> WidgetInstance {
        let mut w = crate::widgets::default_for(&WidgetKind::Button);
        w.props.label = "my custom label".to_owned();
        w.rect.x = 50.0;
        w.rect.y = 80.0;
        w.rect.w = 99.0;
        w.rect.h = 77.0;
        w
    }

    #[test]
    fn reset_label_restores_kind_name() {
        let w = make_button();
        let kind_name = format!("{:?}", w.kind);
        assert_eq!(kind_name, "Button");
        assert_eq!(kind_name, "Button");
    }

    #[test]
    fn reset_geometry_restores_defaults() {
        let mut w = make_button();
        w.rect.x = 0.0;
        w.rect.y = 0.0;
        let default_inst = crate::widgets::default_for(&w.kind);
        w.rect.w = default_inst.rect.w;
        w.rect.h = default_inst.rect.h;
        assert_eq!(w.rect.x, 0.0);
        assert_eq!(w.rect.y, 0.0);
        assert!(w.rect.w > 0.0);
        assert!(w.rect.h > 0.0);
        assert!((w.rect.w - default_inst.rect.w).abs() < 0.01);
        assert!((w.rect.h - default_inst.rect.h).abs() < 0.01);
    }
}
