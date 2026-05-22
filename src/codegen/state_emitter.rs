use crate::codegen::{kind_table, rust::field_binding};
use crate::project::schema::{WidgetInstance, WidgetKind};
use crate::project::ui_tree::UiTree;
use std::collections::HashSet;

struct StateField {
    name: String,
    ty: &'static str,
    default_expr: String,
}

/// Walk the UiTree and emit an AppState struct with the required fields.
pub fn emit(tree: &UiTree) -> String {
    let mut fields = Vec::new();
    let mut comments = Vec::new();
    let mut seen = HashSet::new();

    for w in &tree.widgets {
        // Standard state binding
        if let Some(binding) = field_binding(w.state_binding.as_deref()) {
            if !seen.insert(binding.to_owned()) {
                comments.push(format!("// Duplicate binding skipped: {binding}"));
            } else {
                let Some(info) = kind_table::state_info(&w.kind) else {
                    continue; // Button / Frame carry no state
                };
                fields.push(StateField {
                    name: binding.to_owned(),
                    ty: info.rust_type,
                    default_expr: default_expr_for_widget(w),
                });
            }
        } else if w.state_binding.is_some() {
            comments.push("// Invalid binding skipped.".to_owned());
        }

        // Bound label field
        if let Some(ref lb) = w.label_binding {
            if let Some(b) = field_binding(Some(lb.as_str())) {
                if seen.insert(b.to_owned()) {
                    fields.push(StateField {
                        name: b.to_owned(),
                        ty: "String",
                        default_expr: "String::new()".to_owned(),
                    });
                }
            }
        }

        // Custom props
        for prop in &w.custom_props {
            if let Some(b) = field_binding(Some(prop.name.as_str())) {
                if seen.insert(b.to_owned()) {
                    fields.push(StateField {
                        name: b.to_owned(),
                        ty: prop.ty.rust_type(),
                        default_expr: prop.ty.default_expr().to_owned(),
                    });
                }
            }
        }
    }

    let mut out = String::from("// Generated AppState - do not edit manually\nstruct AppState {\n");
    for field in &fields {
        out.push_str(&format!("    {}: {},\n", field.name, field.ty));
    }
    for comment in &comments {
        out.push_str("    ");
        out.push_str(comment);
        out.push('\n');
    }
    out.push_str("}\n\n");
    out.push_str("impl Default for AppState {\n    fn default() -> Self {\n        Self {\n");
    for field in &fields {
        out.push_str(&format!(
            "            {}: {},\n",
            field.name, field.default_expr
        ));
    }
    out.push_str("        }\n    }\n}\n");
    out
}

fn default_expr_for_widget(w: &WidgetInstance) -> String {
    if w.kind == WidgetKind::Slider {
        format!("{:.3}", w.props.default_value)
    } else {
        kind_table::state_info(&w.kind)
            .map(|info| info.default_expr.to_owned())
            .unwrap_or_else(|| "()".to_owned())
    }
}
