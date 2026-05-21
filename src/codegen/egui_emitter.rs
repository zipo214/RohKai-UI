use crate::codegen::rust::{field_binding, string_literal};
use crate::project::{schema::WidgetKind, ui_tree::UiTree};
use uuid::Uuid;

/// Returns (widget_id_or_none, code_line) for every line in the generated body.
/// Preamble/closing lines have `None` as the id.
pub fn emit_indexed(tree: &UiTree) -> Vec<(Option<Uuid>, String)> {
    let mut lines: Vec<(Option<Uuid>, String)> = Vec::new();

    lines.push((
        None,
        "egui::CentralPanel::default().show(ctx, |_ui| {});".to_owned(),
    ));

    for w in &tree.widgets {
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

        let binding = field_binding(w.state_binding.as_deref());
        let label = string_literal(&w.props.label);
        let line = match &w.kind {
            WidgetKind::Button => {
                format!(
                    "        if ui.add_sized([{:.1}, {:.1}], egui::Button::new({label})).clicked() {{}}",
                    w.rect.w, w.rect.h
                )
            }
            WidgetKind::Label => match binding {
                Some(binding) => format!("        ui.label(&self.{binding});"),
                None => format!("        ui.label({label});"),
            },
            WidgetKind::TextInput => match binding {
                Some(binding) => format!(
                    "        ui.add_sized([{:.1}, {:.1}], egui::TextEdit::singleline(&mut self.{binding}));",
                    w.rect.w, w.rect.h
                ),
                None => format!("        // TextInput {label}: set a valid Binding"),
            },
            WidgetKind::Slider => match binding {
                Some(binding) => format!(
                    "        ui.add_sized([{:.1}, {:.1}], egui::Slider::new(&mut self.{binding}, {:.1}..={:.1}).text({label}));",
                    w.rect.w, w.rect.h, w.props.min, w.props.max
                ),
                None => format!("        // Slider {label}: set a valid Binding"),
            },
            WidgetKind::Checkbox => match binding {
                Some(binding) => format!(
                    "        ui.add_sized([{:.1}, {:.1}], egui::Checkbox::new(&mut self.{binding}, {label}));",
                    w.rect.w, w.rect.h
                ),
                None => format!("        // Checkbox {label}: set a valid Binding"),
            },
            WidgetKind::Frame => {
                format!("        ui.group(|ui| {{ ui.label({label}); }});")
            }
            WidgetKind::ComboBox => match binding {
                Some(binding) => format!(
                    "        egui::ComboBox::from_label({label}).selected_text(self.{binding}.as_str()).width({:.1}).show_ui(ui, |_ui| {{}});",
                    w.rect.w
                ),
                None => format!("        // ComboBox {label}: set a valid Binding"),
            },
            WidgetKind::RadioButton => match binding {
                Some(binding) => {
                    format!("        ui.radio_value(&mut self.{binding}, {label}.to_owned(), {label});")
                }
                None => format!("        // RadioButton {label}: set a valid Binding"),
            },
            WidgetKind::ProgressBar => match binding {
                Some(binding) => {
                    format!(
                        "        ui.add_sized([{:.1}, {:.1}], egui::ProgressBar::new(self.{binding}).text({label}));",
                        w.rect.w, w.rect.h
                    )
                }
                None => format!("        // ProgressBar {label}: set a valid Binding"),
            },
        };
        lines.push((Some(w.id), line));
        lines.push((Some(w.id), "    });".to_owned()));
    }
    lines
}
