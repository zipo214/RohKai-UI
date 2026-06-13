//! Beginner-friendly widget builder.
//!
//! Guided inspector + canvas preview for creating new `.rkwd` descriptors
//! without exposing raw templates. Delegates to the advanced descriptor editor
//! via the "Advanced Descriptor…" button, which closes this window atomically.
//!
//! Entry: Widgets → Guided Descriptor Builder…

use crate::codegen::widget_descriptor::{
    DescriptorCodegen, DescriptorProp, DescriptorPropType, WidgetDescriptor, validate_descriptor,
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Selects how codegen templates are produced.
#[derive(Debug, Clone, PartialEq)]
pub enum BuilderWidgetType {
    /// `ui.label(...)` — no click event.
    Label,
    /// `ui.button(...)` with optional on-click stub.
    Button,
    /// Edit codegen templates directly — builder does not overwrite them.
    RawTemplate,
}

/// All state owned by the beginner widget builder window.
pub struct WidgetBuilderState {
    pub draft: WidgetDescriptor,
    pub widget_type: BuilderWidgetType,
    /// Whether to emit a handler event declaration.
    pub has_click_event: bool,
    /// The "label" property default — reflected to/from `draft.properties`.
    pub label_default: String,
    /// Handler function name used in template `{{handler}}` expansion.
    pub handler_name: String,
    pub save_msg: Option<(bool, String)>,
    /// Cached per-frame validation errors.
    pub validation_errors: Vec<String>,
    /// True while id should be auto-derived from name.
    pub id_derived: bool,
    /// One-shot signal: open descriptor editor and close builder.
    pub open_advanced_requested: bool,
}

impl WidgetBuilderState {
    pub fn new() -> Self {
        let mut draft = crate::panels::descriptor_editor::blank_descriptor();
        // Start with an empty id/name so the user fills them in.
        draft.id = String::new();
        draft.name = String::new();
        // Pre-seed a "label" property.
        draft.properties = vec![DescriptorProp {
            key: "label".to_owned(),
            ty: DescriptorPropType::String,
            default: "My Widget".to_owned(),
            display: "Label".to_owned(),
            options: vec![],
        }];
        Self {
            draft,
            widget_type: BuilderWidgetType::Label,
            has_click_event: false,
            label_default: "My Widget".to_owned(),
            handler_name: "on_click".to_owned(),
            save_msg: None,
            validation_errors: vec![],
            id_derived: true,
            open_advanced_requested: false,
        }
    }
}

impl Default for WidgetBuilderState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// sync_draft — called at frame start to keep draft consistent with UI state
// ---------------------------------------------------------------------------

fn sync_draft(state: &mut WidgetBuilderState) {
    // 1. Upsert the "label" property — find by key, never assume index 0.
    if let Some(p) = state.draft.properties.iter_mut().find(|p| p.key == "label") {
        p.default = state.label_default.clone();
    } else {
        state.draft.properties.insert(
            0,
            DescriptorProp {
                key: "label".to_owned(),
                ty: DescriptorPropType::String,
                default: state.label_default.clone(),
                display: "Label".to_owned(),
                options: vec![],
            },
        );
    }

    // 2. Add/remove handler event declaration based on toggle.
    //    Events list declares *support*, not bindings — bindings live on WidgetInstance.
    let handler = state.handler_name.trim().to_owned();
    if state.has_click_event {
        if !handler.is_empty() && !state.draft.events.contains(&handler) {
            state.draft.events.push(handler);
        }
    } else {
        // Remove any event entry that matches the current handler name.
        if !handler.is_empty() {
            state.draft.events.retain(|e| e != &handler);
        }
    }

    // 3. Overwrite codegen for managed types; leave RawTemplate alone.
    if state.widget_type != BuilderWidgetType::RawTemplate {
        state.draft.codegen = generate_templates(
            &state.widget_type,
            state.has_click_event,
            &state.handler_name,
        );
    }
}

// ---------------------------------------------------------------------------
// generate_templates
// ---------------------------------------------------------------------------

fn generate_templates(
    wt: &BuilderWidgetType,
    has_click: bool,
    _handler: &str,
) -> DescriptorCodegen {
    // Templates use {{handler}} token — expanded at preview/codegen time via apply_template.
    match wt {
        BuilderWidgetType::Label => DescriptorCodegen {
            live_preview: "        ui.label({{label}});".to_owned(),
            export: "        ui.label({{label}});".to_owned(),
            on_click_stub: String::new(),
        },
        BuilderWidgetType::Button => DescriptorCodegen {
            live_preview: "        if ui.button({{label}}).clicked() { self.{{handler}}(); }"
                .to_owned(),
            export: "        if ui.button({{label}}).clicked() { self.{{handler}}(); }".to_owned(),
            on_click_stub: if has_click {
                "    fn {{handler}}(&mut self) {}".to_owned()
            } else {
                String::new()
            },
        },
        BuilderWidgetType::RawTemplate => DescriptorCodegen {
            live_preview: String::new(),
            export: String::new(),
            on_click_stub: String::new(),
        },
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Renders the builder window.  Returns `true` while open.
pub fn show(
    ctx: &egui::Context,
    state: &mut WidgetBuilderState,
    widgets_dir: Option<std::path::PathBuf>,
) -> bool {
    // Clear one-shot signal each frame.
    state.open_advanced_requested = false;

    // Keep draft consistent with UI choices.
    sync_draft(state);

    // Refresh validation each frame (pure, no I/O).
    state.validation_errors = validate_descriptor(&state.draft);

    let mut open = true;

    let bounds = crate::panels::window_bounds::authoring_window_bounds(
        ctx.content_rect(),
        egui::vec2(780.0, 480.0),
        egui::vec2(560.0, 360.0),
    );

    egui::Window::new("Guided Widget Descriptor Builder")
        .id(egui::Id::new("widget_builder"))
        .open(&mut open)
        .default_pos(bounds.default_pos)
        .default_size(bounds.default_size)
        .min_size(bounds.min_size)
        .max_size(bounds.max_size)
        .resizable(true)
        .constrain(true)
        .show(ctx, |ui| {
            let avail = ui.available_width().min(780.0 - 16.0);
            let inspector_w = (avail * 0.40 - 4.0).max(180.0);
            let prev_w = (avail - inspector_w - 12.0).max(140.0);

            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(inspector_w);
                    show_inspector(ui, state, widgets_dir, inspector_w);
                });

                ui.separator();

                ui.vertical(|ui| {
                    ui.set_width(prev_w);
                    show_preview_pane(ui, state, prev_w);
                });
            });
        });

    open
}

// ---------------------------------------------------------------------------
// Left pane — inspector
// ---------------------------------------------------------------------------

fn show_inspector(
    ui: &mut egui::Ui,
    state: &mut WidgetBuilderState,
    widgets_dir: Option<std::path::PathBuf>,
    inspector_w: f32,
) {
    let field_w = (inspector_w - 80.0).max(80.0);

    egui::ScrollArea::vertical()
        .id_salt("builder_form_scroll")
        .show(ui, |ui| {
            // -- Identity --
            section(ui, "Identity");

            field_row(ui, "Name", |ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut state.draft.name)
                        .hint_text("My Widget")
                        .desired_width(field_w),
                );
                if resp.changed() && state.id_derived {
                    let derived = crate::panels::descriptor_editor::sanitize_id(&state.draft.name)
                        .to_lowercase()
                        .replace('-', "_");
                    state.draft.id = format!("custom.{derived}");
                }
            });

            field_row(ui, "ID", |ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut state.draft.id)
                        .hint_text("custom.my_widget")
                        .desired_width(field_w),
                );
                if resp.changed() {
                    state.id_derived = false;
                }
            });

            field_row(ui, "Category", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.draft.category)
                        .hint_text("Custom")
                        .desired_width(field_w),
                );
            });
            // Category preset chips
            ui.horizontal(|ui| {
                ui.add_space(72.0);
                for preset in ["Custom", "Layout", "Input", "Display"] {
                    if ui.small_button(preset).clicked() {
                        state.draft.category = preset.to_owned();
                    }
                }
            });

            // -- Geometry --
            section(ui, "Geometry");
            field_row(ui, "Size", |ui| {
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

            // -- Appearance --
            section(ui, "Appearance");
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

            // -- Widget Type --
            section(ui, "Widget Type");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.widget_type, BuilderWidgetType::Label, "Label");
                ui.selectable_value(&mut state.widget_type, BuilderWidgetType::Button, "Button");
                ui.selectable_value(
                    &mut state.widget_type,
                    BuilderWidgetType::RawTemplate,
                    "Raw Template",
                )
                .on_hover_text(
                    "Templates not auto-generated — edit them directly in the preview pane",
                );
            });

            // -- Label Default --
            section(ui, "Content");
            field_row(ui, "Label", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.label_default)
                        .hint_text("My Widget")
                        .desired_width(field_w),
                );
            });

            // -- Click Handler --
            if state.widget_type != BuilderWidgetType::Label {
                ui.add_space(4.0);
                ui.checkbox(&mut state.has_click_event, "On-click handler");
                if state.has_click_event {
                    field_row(ui, "Handler", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut state.handler_name)
                                .hint_text("on_click")
                                .desired_width(field_w),
                        );
                    });
                }
            }

            // -- Validation errors --
            if !state.validation_errors.is_empty() {
                ui.add_space(6.0);
                for err in &state.validation_errors {
                    ui.label(
                        egui::RichText::new(format!("⚠ {err}"))
                            .small()
                            .color(egui::Color32::from_rgb(248, 113, 113)),
                    );
                }
            }

            // -- Save --
            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                let can_save = !state.draft.id.trim().is_empty()
                    && widgets_dir.is_some()
                    && state.validation_errors.is_empty();
                if ui
                    .add_enabled(can_save, egui::Button::new("💾 Save to widgets/"))
                    .on_hover_text("Write .rkwd file and reload palette")
                    .clicked()
                    && let Some(ref dir) = widgets_dir
                {
                    state.save_msg =
                        crate::panels::descriptor_editor::save_descriptor(&state.draft, dir);
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

            // -- Advanced handoff --
            ui.add_space(4.0);
            if ui
                .button("Advanced Descriptor…")
                .on_hover_text(
                    "Open full descriptor editor with current draft — closes this window",
                )
                .clicked()
            {
                state.open_advanced_requested = true;
            }
        });
}

// ---------------------------------------------------------------------------
// Right pane — live preview
// ---------------------------------------------------------------------------

fn show_preview_pane(ui: &mut egui::Ui, state: &WidgetBuilderState, prev_w: f32) {
    ui.heading("Preview");
    ui.separator();

    let draft = &state.draft;
    let [r, g, b] = draft.accent_color;
    let accent = egui::Color32::from_rgb(r, g, b);
    let fill = egui::Color32::from_rgba_unmultiplied(r, g, b, 30);

    let w = draft.default_size[0].clamp(40.0, 300.0);
    let h = draft.default_size[1].clamp(16.0, 120.0);

    // Build dummy widget with handler_name so {{handler}} expands correctly.
    let mut dummy = crate::panels::descriptor_editor::make_dummy_widget(draft);
    dummy.on_click = state.handler_name.clone();

    let label = crate::panels::descriptor_editor::expand_canvas_label(
        &draft.canvas_preview.label_template,
        &draft.name,
        &dummy.props.label,
        &dummy.descriptor_props,
    );

    ui.label(egui::RichText::new("Canvas").small().weak());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    ui.painter().rect_filled(rect, 4.0, fill);
    ui.painter().rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.5, accent),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &label,
        egui::FontId::proportional(12.0),
        accent,
    );

    ui.add_space(8.0);

    // Template expansion boxes.
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

    if state.widget_type == BuilderWidgetType::RawTemplate {
        // Editable template fields for RawTemplate mode.
        ui.label(
            egui::RichText::new(
                "Tokens: {{label}} {{handler}} {{binding}} {{width}} {{height}} {{name}} {{prop.<key>}}",
            )
            .small()
            .weak(),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Live preview template")
                .small()
                .strong(),
        );
        // NOTE: we cannot mutate draft from this borrow — callers should use RawTemplate
        // fields directly in the draft; shown here as read-only with hint.
        egui::ScrollArea::vertical()
            .id_salt("builder_live_scroll")
            .max_height(70.0)
            .show(ui, |ui| {
                let mut s = live_expanded;
                ui.add(
                    egui::TextEdit::multiline(&mut s)
                        .font(egui::FontId::monospace(10.5))
                        .desired_width(prev_w)
                        .interactive(false)
                        .hint_text("Enter live template on the left — switch to Advanced Descriptor for full editing"),
                );
            });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Export template").small().strong());
        egui::ScrollArea::vertical()
            .id_salt("builder_export_scroll")
            .max_height(70.0)
            .show(ui, |ui| {
                let mut s = export_expanded;
                ui.add(
                    egui::TextEdit::multiline(&mut s)
                        .font(egui::FontId::monospace(10.5))
                        .desired_width(prev_w)
                        .interactive(false),
                );
            });
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("→ Use \"Advanced Descriptor…\" to edit templates directly")
                .small()
                .italics()
                .weak(),
        );
    } else {
        // Read-only expanded template boxes for Label/Button.
        ui.label(
            egui::RichText::new("Live preview template (expanded)")
                .small()
                .weak(),
        );
        egui::ScrollArea::vertical()
            .id_salt("builder_live_scroll")
            .max_height(70.0)
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
            .id_salt("builder_export_scroll")
            .max_height(70.0)
            .show(ui, |ui| {
                let mut s = export_expanded;
                ui.add(
                    egui::TextEdit::multiline(&mut s)
                        .font(egui::FontId::monospace(10.5))
                        .desired_width(prev_w)
                        .interactive(false),
                );
            });
    }
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::widget_descriptor::validate_descriptor;

    /// Fresh state has the label property with the correct default.
    #[test]
    fn label_prop_present_with_correct_default() {
        let mut state = WidgetBuilderState::new();
        state.label_default = "Hello".to_owned();
        sync_draft(&mut state);
        let prop = state
            .draft
            .properties
            .iter()
            .find(|p| p.key == "label")
            .expect("label property missing");
        assert_eq!(prop.default, "Hello");
    }

    /// Default builder draft passes descriptor validation.
    #[test]
    fn builder_descriptor_passes_validation() {
        let mut state = WidgetBuilderState::new();
        state.draft.id = "custom.test_widget".to_owned();
        state.draft.name = "Test Widget".to_owned();
        sync_draft(&mut state);
        let errs = validate_descriptor(&state.draft);
        assert!(errs.is_empty(), "unexpected errors: {errs:?}");
    }

    /// Enabling click event adds handler name to draft.events.
    #[test]
    fn click_event_toggle_adds_handler() {
        let mut state = WidgetBuilderState::new();
        state.draft.id = "custom.btn".to_owned();
        state.draft.name = "Btn".to_owned();
        state.widget_type = BuilderWidgetType::Button;
        state.has_click_event = true;
        state.handler_name = "on_press".to_owned();
        sync_draft(&mut state);
        assert!(state.draft.events.contains(&"on_press".to_owned()));
    }

    /// Disabling click event removes handler name from draft.events.
    #[test]
    fn click_event_toggle_removes_handler() {
        let mut state = WidgetBuilderState::new();
        state.draft.id = "custom.btn".to_owned();
        state.draft.name = "Btn".to_owned();
        state.widget_type = BuilderWidgetType::Button;
        state.has_click_event = true;
        state.handler_name = "on_press".to_owned();
        sync_draft(&mut state);
        assert!(state.draft.events.contains(&"on_press".to_owned()));

        state.has_click_event = false;
        sync_draft(&mut state);
        assert!(!state.draft.events.contains(&"on_press".to_owned()));
    }

    /// Label type generates templates containing ui.label and {{label}}; no stub.
    #[test]
    fn label_type_generates_expected_templates() {
        let tpl = generate_templates(&BuilderWidgetType::Label, false, "on_click");
        assert!(tpl.live_preview.contains("ui.label"));
        assert!(tpl.live_preview.contains("{{label}}"));
        assert!(tpl.on_click_stub.is_empty());
    }

    /// Button type with click enabled generates ui.button template and fn stub.
    #[test]
    fn button_type_generates_click_stub_when_enabled() {
        let tpl = generate_templates(&BuilderWidgetType::Button, true, "on_click");
        assert!(tpl.live_preview.contains("ui.button"));
        assert!(tpl.live_preview.contains("{{handler}}"));
        assert!(tpl.on_click_stub.contains("{{handler}}"));
    }

    /// apply_template expands {{label}} to the label default value.
    #[test]
    fn apply_template_expands_label() {
        let mut state = WidgetBuilderState::new();
        state.draft.id = "custom.exp".to_owned();
        state.draft.name = "Exp".to_owned();
        state.label_default = "World".to_owned();
        sync_draft(&mut state);
        let dummy = crate::panels::descriptor_editor::make_dummy_widget(&state.draft);
        let out = crate::codegen::widget_descriptor::apply_template(
            "ui.label({{label}});",
            &dummy,
            &state.draft.name,
        );
        assert!(out.contains("World"), "expanded: {out}");
    }

    /// Builder output can instantiate as WidgetKind::Custom with expected descriptor_props.
    #[test]
    fn builder_output_instantiates_as_custom_widget() {
        use crate::project::schema::WidgetKind;
        let mut state = WidgetBuilderState::new();
        state.draft.id = "custom.my_widget".to_owned();
        state.draft.name = "My Widget".to_owned();
        state.label_default = "Click me".to_owned();
        sync_draft(&mut state);

        let instance = crate::widgets::default_for_descriptor(&state.draft);
        assert!(
            matches!(&instance.kind, WidgetKind::Custom(id) if id == "custom.my_widget"),
            "expected Custom kind, got {:?}",
            instance.kind
        );
        assert_eq!(
            instance.descriptor_props.get("label").map(String::as_str),
            Some("Click me"),
        );
    }
}
