use crate::codegen::{kind_table, rust::field_binding};
use crate::project::schema::{WidgetInstance, WidgetKind};
use crate::project::ui_tree::UiTree;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// One AppState field derived from the UiTree.
#[derive(Debug, Clone, PartialEq)]
pub struct AppStateField {
    pub name: String,
    pub ty: String,
    pub default_expr: String,
}

/// Result of a full-tree field collection pass.
pub struct CollectedFields {
    /// Deduplicated fields in stable insertion order.
    pub fields: Vec<AppStateField>,
    /// Human-readable warnings: duplicates, type collisions, invalid bindings.
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Collection
// ---------------------------------------------------------------------------

/// Walk `tree` and collect every AppState field exactly once.
///
/// Deduplication rules:
/// - Same name + same type   → silently deduplicate; first occurrence wins.
/// - Same name + different type → keep first, append a warning.
/// - Invalid binding string  → skip, append a warning.
pub fn collect(tree: &UiTree) -> CollectedFields {
    let mut fields: Vec<AppStateField> = Vec::new();
    // Maps field name → index in `fields`, for collision detection.
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut warnings: Vec<String> = Vec::new();

    for w in &tree.widgets {
        collect_one(w, &mut fields, &mut seen, &mut warnings);
    }

    // Behavior-graph actions mutate AppState fields; declare any the widgets
    // themselves did not, so generated mutations always compile.
    collect_behavior_fields(tree, &mut fields, &mut seen, &mut warnings);

    CollectedFields { fields, warnings }
}

fn collect_behavior_fields(
    tree: &UiTree,
    fields: &mut Vec<AppStateField>,
    seen: &mut HashMap<String, usize>,
    warnings: &mut Vec<String>,
) {
    use crate::project::schema::{ValueExpr, VisualAction};
    for b in &tree.app_props.behaviors {
        let Some(raw) = b.action.field() else {
            continue; // CallHandler — no state field
        };
        let effective = crate::codegen::rust::effective_binding(raw.trim());
        let Some(name) = field_binding(Some(effective.as_str())) else {
            warnings.push(format!("Invalid behavior field {:?} skipped.", raw));
            continue;
        };
        let (ty, default_expr) = match &b.action {
            VisualAction::Add { .. } | VisualAction::Subtract { .. } => ("f32", "0.0"),
            VisualAction::Toggle { .. } => ("bool", "false"),
            VisualAction::Set { value, .. } => match value {
                ValueExpr::Number(_) => ("f32", "0.0"),
                ValueExpr::Text(_) => ("String", "String::new()"),
                ValueExpr::Flag(_) => ("bool", "false"),
            },
            VisualAction::CallHandler { .. } => continue,
        };
        push_field(
            AppStateField {
                name: name.to_owned(),
                ty: ty.to_owned(),
                default_expr: default_expr.to_owned(),
            },
            fields,
            seen,
            warnings,
        );
    }
}

fn collect_one(
    w: &WidgetInstance,
    fields: &mut Vec<AppStateField>,
    seen: &mut HashMap<String, usize>,
    warnings: &mut Vec<String>,
) {
    // ---- Standard state binding ----
    if let Some(raw) = w.state_binding.as_deref() {
        let effective = crate::codegen::rust::effective_binding(raw);
        if let Some(name) = field_binding(Some(effective.as_str())) {
            if let Some(info) = kind_table::state_info(&w.kind) {
                push_field(
                    AppStateField {
                        name: name.to_owned(),
                        ty: info.rust_type.to_owned(),
                        default_expr: default_expr_for_widget(w),
                    },
                    fields,
                    seen,
                    warnings,
                );
            }
            // No `state_info` → widget carries no state (Button, Frame…).
        } else {
            warnings.push(format!("Invalid state binding {:?} skipped.", raw));
        }
    }

    // ---- Bound label field ----
    if let Some(ref lb) = w.label_binding {
        if let Some(b) = field_binding(Some(lb.as_str())) {
            push_field(
                AppStateField {
                    name: b.to_owned(),
                    ty: "String".to_owned(),
                    default_expr: "String::new()".to_owned(),
                },
                fields,
                seen,
                warnings,
            );
        }
    }

    // ---- Data source binding (Table / ListView / TreeView model) ----
    if let Some(ref src) = w.props.data_source_binding {
        if let Some(b) = field_binding(Some(src.as_str())) {
            let (ty, default_expr) = match &w.kind {
                WidgetKind::Table => ("Vec<Vec<String>>".to_owned(), "Vec::new()".to_owned()),
                _ => ("Vec<String>".to_owned(), "Vec::new()".to_owned()),
            };
            push_field(
                AppStateField {
                    name: b.to_owned(),
                    ty,
                    default_expr,
                },
                fields,
                seen,
                warnings,
            );
        }
    }

    // ---- Formula dependencies (MathLabel expression variables) ----
    if w.kind == WidgetKind::MathLabel && !w.props.formula_expr.trim().is_empty() {
        match crate::codegen::formula::parse_formula(&w.props.formula_expr) {
            Ok(node) => {
                for raw in crate::codegen::formula::collect_variables(&node) {
                    let effective = crate::codegen::rust::effective_binding(&raw);
                    if let Some(name) = field_binding(Some(effective.as_str())) {
                        push_field(
                            AppStateField {
                                name: name.to_owned(),
                                ty: "f32".to_owned(),
                                default_expr: "0.0".to_owned(),
                            },
                            fields,
                            seen,
                            warnings,
                        );
                    } else {
                        warnings.push(format!("Invalid formula variable {:?} skipped.", raw));
                    }
                }
            }
            Err(error) => warnings.push(format!(
                "Formula for widget {} is invalid and was skipped: {error}",
                w.id
            )),
        }
    }

    // ---- Database binding fallback field ----
    // A DB-bound widget without an explicit Binding still needs a concrete
    // AppState destination for the generated load_from_db() assignment.
    if w.db_binding.is_some()
        && w.state_binding
            .as_deref()
            .is_none_or(|binding| binding.trim().is_empty())
    {
        let (ty, default_expr) = kind_table::state_info(&w.kind)
            .map(|info| (info.rust_type.to_owned(), default_expr_for_widget(w)))
            .unwrap_or_else(|| ("String".to_owned(), "String::new()".to_owned()));
        push_field(
            AppStateField {
                name: crate::codegen::state_emitter::db_field_name(w),
                ty,
                default_expr,
            },
            fields,
            seen,
            warnings,
        );
    }

    // ---- Custom props ----
    for prop in &w.custom_props {
        if let Some(b) = field_binding(Some(prop.name.as_str())) {
            push_field(
                AppStateField {
                    name: b.to_owned(),
                    ty: prop.ty.rust_type().to_owned(),
                    default_expr: prop.ty.default_expr().to_owned(),
                },
                fields,
                seen,
                warnings,
            );
        }
    }

    // ---- Descriptor state fields (Custom widgets) ----
    for [key, rust_type, default_expr] in &w.descriptor_state_fields {
        if let Some(b) = field_binding(Some(key.as_str())) {
            push_field(
                AppStateField {
                    name: b.to_owned(),
                    ty: rust_type.clone(),
                    default_expr: default_expr.clone(),
                },
                fields,
                seen,
                warnings,
            );
        }
    }
}

/// Insert `f` into `fields`, deduplicating by name.  Emits a warning on type
/// collision (same name, different type).
fn push_field(
    f: AppStateField,
    fields: &mut Vec<AppStateField>,
    seen: &mut HashMap<String, usize>,
    warnings: &mut Vec<String>,
) {
    if let Some(&idx) = seen.get(&f.name) {
        if fields[idx].ty != f.ty {
            warnings.push(format!(
                "Field {:?} bound to type {:?} and {:?}; keeping {:?}.",
                f.name, fields[idx].ty, f.ty, fields[idx].ty
            ));
        }
        // duplicate or collision → first occurrence wins, drop `f`
    } else {
        seen.insert(f.name.clone(), fields.len());
        fields.push(f);
    }
}

// ---------------------------------------------------------------------------
// Default expression helpers
// ---------------------------------------------------------------------------

/// Compute the Rust default-value expression for a widget's state binding.
pub fn default_expr_for_widget(w: &WidgetInstance) -> String {
    use crate::codegen::rust::string_literal;
    match w.kind {
        WidgetKind::Slider => format!("{:.3}", w.props.default_value),
        WidgetKind::ComboBox => {
            string_literal(
                w.props
                    .options
                    .iter()
                    .find(|o| !o.trim().is_empty())
                    .map(|o| o.trim())
                    .unwrap_or("Option A"),
            ) + ".to_owned()"
        }
        _ => kind_table::state_info(&w.kind)
            .map(|info| info.default_expr.to_owned())
            .unwrap_or_else(|| "()".to_owned()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{WidgetKind, WidgetProps};
    use crate::project::ui_tree::UiTree;

    fn make_tree(widgets: Vec<WidgetInstance>) -> UiTree {
        UiTree {
            widgets,
            ..Default::default()
        }
    }

    fn simple_widget(kind: WidgetKind, binding: &str) -> WidgetInstance {
        WidgetInstance {
            kind,
            props: WidgetProps {
                label: "x".to_owned(),
                ..Default::default()
            },
            state_binding: Some(binding.to_owned()),
            ..Default::default()
        }
    }

    #[test]
    fn collects_checkbox_field() {
        let tree = make_tree(vec![simple_widget(WidgetKind::Checkbox, "my_check")]);
        let r = collect(&tree);
        assert_eq!(r.fields.len(), 1);
        assert_eq!(r.fields[0].name, "my_check");
        assert_eq!(r.fields[0].ty, "bool");
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn deduplicates_same_name_same_type() {
        let tree = make_tree(vec![
            simple_widget(WidgetKind::Checkbox, "flag"),
            simple_widget(WidgetKind::Checkbox, "flag"),
        ]);
        let r = collect(&tree);
        assert_eq!(r.fields.len(), 1, "duplicate same-type silently deduped");
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn warns_on_type_collision() {
        let mut w1 = simple_widget(WidgetKind::Checkbox, "val");
        let mut w2 = simple_widget(WidgetKind::Slider, "val");
        w2.state_binding = Some("val".to_owned());
        w1.state_binding = Some("val".to_owned());
        let tree = make_tree(vec![w1, w2]);
        let r = collect(&tree);
        // Checkbox → bool, Slider → f32: type collision
        assert_eq!(r.fields.len(), 1, "first occurrence kept");
        assert_eq!(r.fields[0].ty, "bool");
        assert_eq!(r.warnings.len(), 1, "one collision warning");
        assert!(r.warnings[0].contains("bool") && r.warnings[0].contains("f32"));
    }

    #[test]
    fn skips_button_no_state_info() {
        let tree = make_tree(vec![simple_widget(WidgetKind::Button, "btn")]);
        let r = collect(&tree);
        // Button has no state_info → no fields
        assert!(r.fields.is_empty());
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn collects_formula_variables_as_f32_fields() {
        let widget = WidgetInstance {
            kind: WidgetKind::MathLabel,
            props: WidgetProps {
                formula_expr: "width * height + type".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = collect(&make_tree(vec![widget]));
        let names: Vec<_> = result
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect();
        assert_eq!(names, vec!["height", "type_value", "width"]);
        assert!(result.fields.iter().all(|field| field.ty == "f32"));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn invalid_formula_adds_warning_without_fields() {
        let widget = WidgetInstance {
            kind: WidgetKind::MathLabel,
            props: WidgetProps {
                formula_expr: "unknown(value)".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = collect(&make_tree(vec![widget]));
        assert!(result.fields.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("unsupported formula function"));
    }

    #[test]
    fn invalid_binding_warns_not_panics() {
        let tree = make_tree(vec![WidgetInstance {
            kind: WidgetKind::Checkbox,
            state_binding: Some("123invalid".to_owned()),
            ..Default::default()
        }]);
        let r = collect(&tree);
        assert!(r.fields.is_empty());
        assert_eq!(r.warnings.len(), 1);
        assert!(r.warnings[0].contains("123invalid"));
    }

    #[test]
    fn descriptor_state_fields_collected() {
        let mut w = WidgetInstance {
            kind: WidgetKind::Button, // kind doesn't matter for descriptor fields
            descriptor_state_fields: vec![
                ["counter".to_owned(), "u32".to_owned(), "0".to_owned()],
                [
                    "label_text".to_owned(),
                    "String".to_owned(),
                    "String::new()".to_owned(),
                ],
            ],
            ..Default::default()
        };
        w.state_binding = None;
        let tree = make_tree(vec![w]);
        let r = collect(&tree);
        assert_eq!(r.fields.len(), 2);
        assert_eq!(r.fields[0].name, "counter");
        assert_eq!(r.fields[0].ty, "u32");
        assert_eq!(r.fields[1].name, "label_text");
    }

    #[test]
    fn behavior_field_declared_when_no_widget_binds_it() {
        use crate::project::schema::{Behavior, VisualAction, WidgetEvent};
        let btn = simple_widget(WidgetKind::Button, "ignored");
        let source_id = btn.id;
        let mut tree = make_tree(vec![btn]);
        tree.app_props.behaviors = vec![Behavior {
            id: uuid::Uuid::from_u128(0x30),
            source_widget: source_id,
            event: WidgetEvent::Click,
            target_widget: None,
            action: VisualAction::Add {
                field: "progress".to_owned(),
                amount: 0.1,
                min: Some(0.0),
                max: Some(1.0),
            },
        }];
        let r = collect(&tree);
        let f = r
            .fields
            .iter()
            .find(|f| f.name == "progress")
            .expect("behavior field must be declared");
        assert_eq!(f.ty, "f32");
        assert_eq!(f.default_expr, "0.0");
    }

    #[test]
    fn behavior_field_dedupes_against_widget_binding() {
        use crate::project::schema::{Behavior, VisualAction, WidgetEvent};
        let bar = simple_widget(WidgetKind::ProgressBar, "progress");
        let source_id = bar.id;
        let mut tree = make_tree(vec![bar]);
        tree.app_props.behaviors = vec![Behavior {
            id: uuid::Uuid::from_u128(0x31),
            source_widget: source_id,
            event: WidgetEvent::Click,
            target_widget: None,
            action: VisualAction::Add {
                field: "progress".to_owned(),
                amount: 0.1,
                min: None,
                max: None,
            },
        }];
        let r = collect(&tree);
        assert_eq!(
            r.fields.iter().filter(|f| f.name == "progress").count(),
            1,
            "behavior field must dedupe against the widget's own binding"
        );
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn keyword_binding_is_sanitized_not_skipped() {
        let tree = make_tree(vec![simple_widget(WidgetKind::Checkbox, "type")]);
        let r = collect(&tree);
        // Should generate a field "type_value", not skip it entirely
        assert_eq!(
            r.fields.len(),
            1,
            "keyword binding must be sanitized, not dropped"
        );
        assert_eq!(r.fields[0].name, "type_value");
        assert!(
            r.warnings.is_empty(),
            "sanitized keyword should not emit warning"
        );
    }
}
