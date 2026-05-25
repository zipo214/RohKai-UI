//! In-app editor for `.rkwd` widget descriptor files.
//!
//! Split view: left = structured form, right = live canvas + code preview.
//! Opened via File → New Widget Descriptor… or the Custom widget properties button.

use crate::codegen::widget_descriptor::{
    CanvasPreviewMode, CargoDep, DescriptorCanvasPreview, DescriptorCodegen, DescriptorProp,
    DescriptorPropType, DescriptorStateField, WidgetDescriptor,
};

// ---------------------------------------------------------------------------
// Editor state (owned by RohKaiApp)
// ---------------------------------------------------------------------------

pub struct DescriptorEditorState {
    pub draft: WidgetDescriptor,
    /// Filename stem used when saving (None = new, derive from draft.id on save).
    pub original_stem: Option<String>,
    pub split_ratio: f32,
    pub save_msg: Option<(bool, String)>,
    // Scratch buffers for pending list entries
    pub new_prop_key: String,
    pub new_state_key: String,
    pub new_event: String,
    pub new_dep_name: String,
}

impl DescriptorEditorState {
    pub fn new_blank() -> Self {
        Self {
            draft: blank_descriptor(),
            original_stem: None,
            split_ratio: 0.52,
            save_msg: None,
            new_prop_key: String::new(),
            new_state_key: String::new(),
            new_event: String::new(),
            new_dep_name: String::new(),
        }
    }

    pub fn from_descriptor(d: &WidgetDescriptor) -> Self {
        let stem = sanitize_id(&d.id);
        Self {
            draft: d.clone(),
            original_stem: Some(stem),
            split_ratio: 0.52,
            save_msg: None,
            new_prop_key: String::new(),
            new_state_key: String::new(),
            new_event: String::new(),
            new_dep_name: String::new(),
        }
    }
}

fn blank_descriptor() -> WidgetDescriptor {
    WidgetDescriptor {
        schema_version: 1,
        id: "my.widget".to_owned(),
        name: "My Widget".to_owned(),
        category: "Custom".to_owned(),
        default_size: [120.0, 36.0],
        accent_color: [100, 180, 255],
        properties: vec![],
        state_fields: vec![],
        codegen: DescriptorCodegen {
            live_preview: "        // {{name}}: {{label}}".to_owned(),
            export: "        MyWidget::new({{label}}).ui(ui);".to_owned(),
            on_click_stub: String::new(),
        },
        canvas_preview: DescriptorCanvasPreview {
            mode: CanvasPreviewMode::LabelBox,
            label_template: "{{name}}: {{label}}".to_owned(),
        },
        cargo_deps: vec![],
        events: vec![],
    }
}

pub fn sanitize_id(id: &str) -> String {
    id.replace(['.', '/'], "-").replace(' ', "_")
}

// ---------------------------------------------------------------------------
// Public show function
// ---------------------------------------------------------------------------

/// Renders the descriptor editor window. Returns `true` while open.
/// Pass `descriptors` so "Save" can reload after write.
pub fn show(
    ctx: &egui::Context,
    state: &mut DescriptorEditorState,
    widgets_dir: Option<std::path::PathBuf>,
) -> bool {
    let mut open = true;

    let title = if state.original_stem.is_some() {
        format!("Edit Descriptor — {}", state.draft.id)
    } else {
        "New Widget Descriptor".to_owned()
    };

    let screen = ctx.screen_rect();
    let default_pos = egui::pos2(
        (screen.center().x - 430.0).max(screen.min.x + 20.0),
        (screen.center().y - 280.0).max(screen.min.y + 20.0),
    );

    egui::Window::new(title)
        .id(egui::Id::new("descriptor_editor"))
        .open(&mut open)
        .default_pos(default_pos)
        .default_size([860.0, 560.0])
        .min_size([600.0, 400.0])
        .max_width(screen.width() - 40.0)
        .resizable(true)
        .constrain(false)
        .show(ctx, |ui| {
            // Clamp available width so content never forces horizontal expansion.
            let avail = ui.available_width().min(860.0 - 16.0);
            state.split_ratio = state.split_ratio.clamp(0.3, 0.75);
            let form_w = (avail * state.split_ratio - 4.0).max(200.0);
            let prev_w = (avail - form_w - 12.0).max(160.0);

            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(form_w);
                    show_form(ui, state, widgets_dir, form_w);
                });

                ui.separator();

                ui.vertical(|ui| {
                    ui.set_width(prev_w);
                    show_preview(ui, &state.draft, prev_w);
                });
            });
        });

    open
}

// ---------------------------------------------------------------------------
// Left pane — form
// ---------------------------------------------------------------------------

fn show_form(
    ui: &mut egui::Ui,
    state: &mut DescriptorEditorState,
    widgets_dir: Option<std::path::PathBuf>,
    form_w: f32,
) {
    let field_w = (form_w - 80.0).max(80.0);
    egui::ScrollArea::vertical()
        .id_salt("desc_form_scroll")
        .show(ui, |ui| {
            section(ui, "Identity");
            field_row(ui, "ID", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.draft.id)
                        .hint_text("e.g. my.button")
                        .desired_width(field_w),
                );
            });
            field_row(ui, "Name", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.draft.name)
                        .hint_text("Display name")
                        .desired_width(field_w),
                );
            });
            field_row(ui, "Category", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.draft.category)
                        .hint_text("Palette group")
                        .desired_width(field_w),
                );
            });
            field_row(ui, "Default size", |ui| {
                ui.add(
                    egui::DragValue::new(&mut state.draft.default_size[0])
                        .speed(1.0)
                        .range(8.0..=1200.0)
                        .prefix("W "),
                );
                ui.add(
                    egui::DragValue::new(&mut state.draft.default_size[1])
                        .speed(1.0)
                        .range(8.0..=800.0)
                        .prefix("H "),
                );
            });
            field_row(ui, "Accent RGB", |ui| {
                let [ref mut r, ref mut g, ref mut b] = state.draft.accent_color;
                ui.label("R");
                ui.add(egui::DragValue::new(r).speed(1.0).range(0..=255));
                ui.label("G");
                ui.add(egui::DragValue::new(g).speed(1.0).range(0..=255));
                ui.label("B");
                ui.add(egui::DragValue::new(b).speed(1.0).range(0..=255));
                let [rv, gv, bv] = state.draft.accent_color;
                let swatch = egui::Color32::from_rgb(rv, gv, bv);
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 3.0, swatch);
            });

            // ---- Properties ----
            section(ui, "Properties");
            show_props_list(ui, &mut state.draft.properties, &mut state.new_prop_key);

            // ---- State fields ----
            section(ui, "State Fields");
            show_state_fields(ui, &mut state.draft.state_fields, &mut state.new_state_key);

            // ---- Codegen ----
            section(ui, "Codegen Templates");
            ui.label(
                egui::RichText::new(
                    "Tokens: {{label}} {{binding}} {{width}} {{height}} {{name}} {{handler}} {{prop.<key>}}",
                )
                .small()
                .weak(),
            );
            template_field(ui, "Live preview", &mut state.draft.codegen.live_preview, form_w);
            template_field(ui, "Export", &mut state.draft.codegen.export, form_w);
            template_field(ui, "On-click stub", &mut state.draft.codegen.on_click_stub, form_w);

            // ---- Canvas preview ----
            section(ui, "Canvas Preview");
            field_row(ui, "Label template", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.draft.canvas_preview.label_template)
                        .hint_text("{{name}}: {{label}}")
                        .desired_width(field_w),
                );
            });

            // ---- Events ----
            section(ui, "Events");
            show_string_list(
                ui,
                &mut state.draft.events,
                &mut state.new_event,
                "Add event (e.g. on_click)",
            );

            // ---- Cargo deps ----
            section(ui, "Cargo Dependencies");
            show_cargo_deps(ui, &mut state.draft.cargo_deps, &mut state.new_dep_name);

            // ---- Save ----
            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                let can_save = !state.draft.id.trim().is_empty() && widgets_dir.is_some();
                if ui
                    .add_enabled(can_save, egui::Button::new("💾 Save to widgets/"))
                    .on_hover_text("Write .rkwd file and reload palette")
                    .clicked()
                {
                    if let Some(ref dir) = widgets_dir {
                        state.save_msg = save_descriptor(&state.draft, dir);
                    }
                }
                if let Some((ok, ref msg)) = state.save_msg {
                    let color = if ok {
                        egui::Color32::from_rgb(52, 211, 153)
                    } else {
                        egui::Color32::RED
                    };
                    ui.label(egui::RichText::new(msg.as_str()).small().color(color));
                }
            });
        });
}

// ---------------------------------------------------------------------------
// Right pane — live preview
// ---------------------------------------------------------------------------

fn show_preview(ui: &mut egui::Ui, draft: &WidgetDescriptor, prev_w: f32) {
    ui.heading("Live Preview");
    ui.separator();

    // Build dummy widget first so prop defaults flow into all preview surfaces.
    let dummy = make_dummy_widget(draft);

    // Canvas box preview
    let [r, g, b] = draft.accent_color;
    let accent = egui::Color32::from_rgb(r, g, b);
    let fill = egui::Color32::from_rgba_unmultiplied(r, g, b, 30);

    let w = draft.default_size[0].clamp(40.0, 300.0);
    let h = draft.default_size[1].clamp(16.0, 120.0);

    let label = expand_canvas_label(
        &draft.canvas_preview.label_template,
        &draft.name,
        &dummy.props.label,
        &dummy.descriptor_props,
    );

    ui.label(egui::RichText::new("Canvas").small().weak());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    ui.painter().rect_filled(rect, 4.0, fill);
    ui.painter()
        .rect_stroke(rect, 4.0, egui::Stroke::new(1.5, accent));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &label,
        egui::FontId::proportional(12.0),
        accent,
    );

    ui.add_space(8.0);

    // Code template expansions
    let live_expanded = crate::codegen::widget_descriptor::apply_template(
        &draft.codegen.live_preview,
        &dummy,
        &draft.name,
    );
    let export_expanded = crate::codegen::widget_descriptor::apply_template(
        &draft.codegen.export,
        &dummy,
        &draft.name,
    );

    ui.label(
        egui::RichText::new("Live preview template (expanded)")
            .small()
            .weak(),
    );
    egui::ScrollArea::vertical()
        .id_salt("live_tpl_scroll")
        .max_height(80.0)
        .show(ui, |ui| {
            let mut s = live_expanded;
            ui.add(
                egui::TextEdit::multiline(&mut s)
                    .font(egui::FontId::monospace(10.5))
                    .desired_width(prev_w)
                    .interactive(false),
            );
        });

    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Export template (expanded)")
            .small()
            .weak(),
    );
    egui::ScrollArea::vertical()
        .id_salt("export_tpl_scroll")
        .max_height(80.0)
        .show(ui, |ui| {
            let mut s = export_expanded;
            ui.add(
                egui::TextEdit::multiline(&mut s)
                    .font(egui::FontId::monospace(10.5))
                    .desired_width(prev_w)
                    .interactive(false),
            );
        });

    // Property defaults preview
    if !draft.properties.is_empty() {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Properties (with defaults)")
                .small()
                .weak(),
        );
        egui::Frame::none()
            .fill(egui::Color32::from_gray(32))
            .inner_margin(egui::Margin::same(6.0))
            .show(ui, |ui| {
                for prop in &draft.properties {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{}:", prop.display))
                                .small()
                                .strong(),
                        );
                        ui.label(egui::RichText::new(&prop.default).small().monospace());
                    });
                }
            });
    }
}

// ---------------------------------------------------------------------------
// Form sub-sections
// ---------------------------------------------------------------------------

fn show_props_list(ui: &mut egui::Ui, props: &mut Vec<DescriptorProp>, new_key: &mut String) {
    let mut remove_idx: Option<usize> = None;
    for (i, prop) in props.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            egui::CollapsingHeader::new(
                egui::RichText::new(format!("prop.{}", prop.key))
                    .monospace()
                    .small(),
            )
            .id_salt(format!("prop_{i}"))
            .show(ui, |ui| {
                field_row(ui, "Key", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut prop.key)
                            .desired_width(ui.available_width()),
                    );
                });
                field_row(ui, "Display", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut prop.display)
                            .desired_width(ui.available_width()),
                    );
                });
                field_row(ui, "Type", |ui| {
                    egui::ComboBox::from_id_salt(format!("prop_type_{i}"))
                        .selected_text(prop_type_label(&prop.ty))
                        .show_ui(ui, |ui| {
                            for ty in ALL_PROP_TYPES {
                                ui.selectable_value(&mut prop.ty, ty.clone(), prop_type_label(ty));
                            }
                        });
                });
                field_row(ui, "Default", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut prop.default)
                            .desired_width(ui.available_width()),
                    );
                });
                if prop.ty == DescriptorPropType::Enum {
                    field_row(ui, "Options", |ui| {
                        let mut opts = prop.options.join(", ");
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut opts)
                                    .hint_text("A, B, C")
                                    .desired_width(ui.available_width()),
                            )
                            .changed()
                        {
                            prop.options = opts
                                .split(',')
                                .map(|s| s.trim().to_owned())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                    });
                }
                if ui
                    .small_button(
                        egui::RichText::new("Remove").color(egui::Color32::from_rgb(248, 113, 113)),
                    )
                    .clicked()
                {
                    remove_idx = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove_idx {
        props.remove(i);
    }

    // Add new prop
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(new_key)
                .hint_text("new key")
                .desired_width(100.0),
        );
        if ui.small_button("+ Add property").clicked() && !new_key.trim().is_empty() {
            props.push(DescriptorProp {
                key: std::mem::take(new_key).trim().to_owned(),
                ty: DescriptorPropType::String,
                default: String::new(),
                display: String::new(),
                options: vec![],
            });
        }
    });
}

fn show_state_fields(
    ui: &mut egui::Ui,
    fields: &mut Vec<DescriptorStateField>,
    new_key: &mut String,
) {
    let mut remove_idx: Option<usize> = None;
    for (i, sf) in fields.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut sf.key)
                        .hint_text("field name")
                        .desired_width(90.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut sf.rust_type)
                        .hint_text("type (e.g. u32)")
                        .desired_width(80.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut sf.default_expr)
                        .hint_text("default (e.g. 0)")
                        .desired_width(70.0),
                );
                if ui
                    .small_button(
                        egui::RichText::new("✕").color(egui::Color32::from_rgb(248, 113, 113)),
                    )
                    .clicked()
                {
                    remove_idx = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove_idx {
        fields.remove(i);
    }
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(new_key)
                .hint_text("new field key")
                .desired_width(100.0),
        );
        if ui.small_button("+ Add state field").clicked() && !new_key.trim().is_empty() {
            fields.push(DescriptorStateField {
                key: std::mem::take(new_key).trim().to_owned(),
                rust_type: "String".to_owned(),
                default_expr: "String::new()".to_owned(),
            });
        }
    });
}

fn show_string_list(ui: &mut egui::Ui, list: &mut Vec<String>, new_val: &mut String, hint: &str) {
    let mut remove_idx: Option<usize> = None;
    for (i, item) in list.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(item).desired_width(150.0));
                if ui
                    .small_button(
                        egui::RichText::new("✕").color(egui::Color32::from_rgb(248, 113, 113)),
                    )
                    .clicked()
                {
                    remove_idx = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove_idx {
        list.remove(i);
    }
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(new_val)
                .hint_text(hint)
                .desired_width(150.0),
        );
        if ui.small_button("+ Add").clicked() && !new_val.trim().is_empty() {
            list.push(std::mem::take(new_val).trim().to_owned());
        }
    });
}

fn show_cargo_deps(ui: &mut egui::Ui, deps: &mut Vec<CargoDep>, new_name: &mut String) {
    let mut remove_idx: Option<usize> = None;
    for (i, dep) in deps.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut dep.name)
                        .hint_text("crate name")
                        .desired_width(100.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut dep.version)
                        .hint_text("0.1")
                        .desired_width(50.0),
                );
                let mut feat_str = dep.features.join(", ");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut feat_str)
                            .hint_text("features")
                            .desired_width(100.0),
                    )
                    .changed()
                {
                    dep.features = feat_str
                        .split(',')
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                if ui
                    .small_button(
                        egui::RichText::new("✕").color(egui::Color32::from_rgb(248, 113, 113)),
                    )
                    .clicked()
                {
                    remove_idx = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove_idx {
        deps.remove(i);
    }
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(new_name)
                .hint_text("crate-name")
                .desired_width(100.0),
        );
        if ui.small_button("+ Add dep").clicked() && !new_name.trim().is_empty() {
            deps.push(CargoDep {
                name: std::mem::take(new_name).trim().to_owned(),
                version: "*".to_owned(),
                features: vec![],
            });
        }
    });
}

fn template_field(ui: &mut egui::Ui, label: &str, value: &mut String, max_w: f32) {
    ui.label(egui::RichText::new(label).small().strong());
    ui.add(
        egui::TextEdit::multiline(value)
            .font(egui::FontId::monospace(11.0))
            .desired_width(max_w)
            .desired_rows(3),
    );
}

// ---------------------------------------------------------------------------
// Layout helpers
// ---------------------------------------------------------------------------

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(title).strong());
    ui.separator();
}

fn field_row(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [70.0, 18.0],
            egui::Label::new(egui::RichText::new(label).small().weak()),
        );
        content(ui);
    });
}

// ---------------------------------------------------------------------------
// Preview helpers
// ---------------------------------------------------------------------------

fn expand_canvas_label(
    template: &str,
    name: &str,
    label: &str,
    props: &std::collections::HashMap<String, String>,
) -> String {
    let base = if template.is_empty() {
        name.to_owned()
    } else {
        template
            .replace("{{name}}", name)
            .replace("{{label}}", label)
    };
    // Substitute {{prop.KEY}} tokens with property defaults, matching apply_template.
    props
        .iter()
        .fold(base, |s, (k, v)| s.replace(&format!("{{{{prop.{k}}}}}"), v))
}

fn make_dummy_widget(draft: &WidgetDescriptor) -> crate::project::schema::WidgetInstance {
    use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
    let label = draft
        .properties
        .iter()
        .find(|p| p.ty == DescriptorPropType::String)
        .map(|p| p.default.clone())
        .unwrap_or_else(|| "Example".to_owned());
    let props = crate::codegen::widget_descriptor::default_props(draft);
    WidgetInstance {
        kind: WidgetKind::Custom(draft.id.clone()),
        rect: Rect {
            x: 0.0,
            y: 0.0,
            w: draft.default_size[0],
            h: draft.default_size[1],
        },
        props: WidgetProps {
            label,
            ..Default::default()
        },
        descriptor_props: props,
        descriptor_name: Some(draft.name.clone()),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Property type helpers
// ---------------------------------------------------------------------------

const ALL_PROP_TYPES: &[DescriptorPropType] = &[
    DescriptorPropType::String,
    DescriptorPropType::F32,
    DescriptorPropType::I32,
    DescriptorPropType::Bool,
    DescriptorPropType::Enum,
];

fn prop_type_label(ty: &DescriptorPropType) -> &'static str {
    match ty {
        DescriptorPropType::String => "String",
        DescriptorPropType::F32 => "F32",
        DescriptorPropType::I32 => "I32",
        DescriptorPropType::Bool => "Bool",
        DescriptorPropType::Enum => "Enum",
    }
}

// ---------------------------------------------------------------------------
// Save
// ---------------------------------------------------------------------------

fn save_descriptor(draft: &WidgetDescriptor, dir: &std::path::Path) -> Option<(bool, String)> {
    let json = match serde_json::to_string_pretty(draft) {
        Ok(j) => j,
        Err(e) => return Some((false, format!("Serialise error: {e}"))),
    };
    if let Err(e) = std::fs::create_dir_all(dir) {
        return Some((false, format!("Could not create widgets/ dir: {e}")));
    }
    let filename = format!("{}.rkwd", sanitize_id(&draft.id));
    let path = dir.join(filename);
    match std::fs::write(&path, json.as_bytes()) {
        Ok(()) => Some((true, format!("Saved → {}", path.display()))),
        Err(e) => Some((false, format!("Write error: {e}"))),
    }
}
