use crate::codegen::rust::field_binding;
use crate::project::{schema::WidgetKind, ui_tree::UiTree};
use std::collections::HashSet;

/// Walk the UiTree and emit an AppState struct with the required fields.
pub fn emit(tree: &UiTree) -> String {
    let mut out = String::from("// Generated AppState — do not edit manually\nstruct AppState {\n");
    let mut seen = HashSet::new();

    for w in &tree.widgets {
        if let Some(binding) = field_binding(w.state_binding.as_deref()) {
            if !seen.insert(binding.to_owned()) {
                out.push_str(&format!("    // Duplicate binding skipped: {binding}\n"));
                continue;
            }
            let ty = match &w.kind {
                // No state binding needed for these kinds
                WidgetKind::Button | WidgetKind::Frame => continue,
                WidgetKind::Label
                | WidgetKind::TextInput
                | WidgetKind::ComboBox
                | WidgetKind::RadioButton => "String",
                WidgetKind::Slider | WidgetKind::ProgressBar => "f32",
                WidgetKind::Checkbox => "bool",
            };
            out.push_str(&format!("    {}: {},\n", binding, ty));
        } else if w.state_binding.is_some() {
            out.push_str("    // Invalid binding skipped.\n");
        }
    }

    out.push_str("}\n");
    out
}
