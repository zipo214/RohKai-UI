//! Behavior graph codegen — turns `AppProps.behaviors` wires into Rust
//! state-mutation statements.
//!
//! This is the beginner-facing counterpart to `rust_wiring` (the advanced
//! escape hatch).  Both the live code panel (`egui_emitter`, fields accessed as
//! `self.{field}`) and export (`export`, fields accessed as
//! `self.state.{field}`) call [`statements_for_event`]; the field prefix is the
//! only difference between the two surfaces, so canvas/preview/export cannot
//! drift (Invariant 1).
//!
//! Invariant: **every** [`VisualAction`] variant emits a real statement from
//! [`action_statement`] — no UI-only action variants.  The exhaustive match
//! plus `every_visual_action_variant_emits_code` enforce this at compile time
//! and in CI.

use crate::project::schema::{Behavior, ValueExpr, VisualAction, WidgetEvent};
use crate::project::ui_tree::UiTree;
use uuid::Uuid;

/// All behaviors fired by `(source_widget, event)`, in declaration order.
pub fn behaviors_for_event(
    tree: &UiTree,
    source_widget: Uuid,
    event: WidgetEvent,
) -> Vec<&Behavior> {
    tree.app_props
        .behaviors
        .iter()
        .filter(|b| b.source_widget == source_widget && b.event == event)
        .collect()
}

/// True when `(source_widget, event)` has at least one behavior wired.
pub fn has_behaviors_for_event(tree: &UiTree, source_widget: Uuid, event: WidgetEvent) -> bool {
    tree.app_props
        .behaviors
        .iter()
        .any(|b| b.source_widget == source_widget && b.event == event)
}

/// Handler names referenced by `CallHandler` actions anywhere in the graph.
/// Export collects these alongside per-widget event handlers so a stub is
/// always generated for them.
pub fn call_handler_names(tree: &UiTree) -> Vec<&str> {
    tree.app_props
        .behaviors
        .iter()
        .filter_map(|b| match &b.action {
            VisualAction::CallHandler { handler } => {
                let h = handler.trim();
                (!h.is_empty()).then_some(h)
            }
            _ => None,
        })
        .collect()
}

/// One Rust statement for `action`.  `field_prefix` is what sits between
/// `self.` and the field name: `""` for the live code panel (`self.progress`),
/// `"state."` for export (`self.state.progress`).
///
/// Always returns a statement: valid actions emit the mutation, invalid field
/// or handler names emit an explanatory comment (still real output — the code
/// panel shows the user why nothing runs).
pub fn action_statement(action: &VisualAction, field_prefix: &str) -> String {
    // Exhaustive match on purpose: adding a VisualAction variant will not
    // compile until its emission is written here.
    match action {
        VisualAction::Set { field, value } => match sanitized_field(field) {
            Some(f) => format!("self.{field_prefix}{f} = {};", value_expr_literal(value)),
            None => invalid_field_comment(field),
        },
        VisualAction::Add {
            field,
            amount,
            min,
            max,
        } => arith_statement(field, *amount, *min, *max, '+', field_prefix),
        VisualAction::Subtract {
            field,
            amount,
            min,
            max,
        } => arith_statement(field, *amount, *min, *max, '-', field_prefix),
        VisualAction::Toggle { field } => match sanitized_field(field) {
            Some(f) => format!("self.{field_prefix}{f} = !self.{field_prefix}{f};"),
            None => invalid_field_comment(field),
        },
        VisualAction::CallHandler { handler } => {
            let h = handler.trim();
            if crate::codegen::rust::is_valid_identifier(h) {
                format!("self.{h}();")
            } else {
                format!("// behavior: invalid handler name {h:?} — not emitted")
            }
        }
    }
}

/// All behavior statements for `(source_widget, event)`, one per line, each
/// line prefixed with `indent`.  Empty string when nothing is wired.
pub fn statements_for_event(
    tree: &UiTree,
    source_widget: Uuid,
    event: WidgetEvent,
    field_prefix: &str,
    indent: &str,
) -> String {
    behaviors_for_event(tree, source_widget, event)
        .iter()
        .map(|b| format!("{indent}{}\n", action_statement(&b.action, field_prefix)))
        .collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn arith_statement(
    field: &str,
    amount: f32,
    min: Option<f32>,
    max: Option<f32>,
    op: char,
    field_prefix: &str,
) -> String {
    let Some(f) = sanitized_field(field) else {
        return invalid_field_comment(field);
    };
    let lhs = format!("self.{field_prefix}{f}");
    let amount_lit = f32_literal(amount);
    let expr = format!("({lhs} {op} {amount_lit})");
    let bounded = match (min, max) {
        (Some(lo), Some(hi)) => {
            format!("{expr}.clamp({}, {})", f32_literal(lo), f32_literal(hi))
        }
        (Some(lo), None) => format!("{expr}.max({})", f32_literal(lo)),
        (None, Some(hi)) => format!("{expr}.min({})", f32_literal(hi)),
        (None, None) => expr,
    };
    format!("{lhs} = {bounded};")
}

/// Sanitize a behavior field name the same way state bindings are sanitized
/// (keyword remap via `effective_binding`, then identifier validation), so the
/// mutation always targets the exact field `field_collector` declares.
fn sanitized_field(raw: &str) -> Option<String> {
    let effective = crate::codegen::rust::effective_binding(raw.trim());
    crate::codegen::rust::field_binding(Some(effective.as_str())).map(str::to_owned)
}

fn invalid_field_comment(field: &str) -> String {
    format!("// behavior: invalid state field {field:?} — not emitted")
}

/// An `f32` literal that always carries a decimal point so generated
/// arithmetic type-checks (`1` would be an integer literal).
pub(crate) fn f32_literal(v: f32) -> String {
    let s = format!("{v}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

fn value_expr_literal(value: &ValueExpr) -> String {
    match value {
        ValueExpr::Number(n) => f32_literal(*n),
        ValueExpr::Text(t) => format!("{}.to_owned()", crate::codegen::rust::string_literal(t)),
        ValueExpr::Flag(b) => format!("{b}"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::Behavior;

    fn add_progress() -> VisualAction {
        VisualAction::Add {
            field: "progress".to_owned(),
            amount: 0.1,
            min: Some(0.0),
            max: Some(1.0),
        }
    }

    #[test]
    fn add_emits_clamped_mutation_for_both_surfaces() {
        assert_eq!(
            action_statement(&add_progress(), "state."),
            "self.state.progress = (self.state.progress + 0.1).clamp(0.0, 1.0);"
        );
        assert_eq!(
            action_statement(&add_progress(), ""),
            "self.progress = (self.progress + 0.1).clamp(0.0, 1.0);"
        );
    }

    #[test]
    fn every_visual_action_variant_emits_code() {
        // Invariant: no UI-only action variants.  One sample per variant; each
        // must emit a real statement (not a comment).
        let samples = vec![
            VisualAction::Set {
                field: "name".to_owned(),
                value: ValueExpr::Text("hi".to_owned()),
            },
            VisualAction::Set {
                field: "vol".to_owned(),
                value: ValueExpr::Number(2.0),
            },
            VisualAction::Set {
                field: "on".to_owned(),
                value: ValueExpr::Flag(true),
            },
            add_progress(),
            VisualAction::Subtract {
                field: "progress".to_owned(),
                amount: 0.1,
                min: Some(0.0),
                max: Some(1.0),
            },
            VisualAction::Toggle {
                field: "dark".to_owned(),
            },
            VisualAction::CallHandler {
                handler: "on_go".to_owned(),
            },
        ];
        for action in samples {
            let stmt = action_statement(&action, "state.");
            assert!(
                !stmt.trim_start().starts_with("//"),
                "{action:?} must emit a statement, got comment: {stmt}"
            );
            assert!(stmt.ends_with(';'), "{action:?} must emit a statement");
        }
    }

    #[test]
    fn set_variants_emit_typed_literals() {
        let text = VisualAction::Set {
            field: "name".to_owned(),
            value: ValueExpr::Text("hi".to_owned()),
        };
        assert_eq!(
            action_statement(&text, "state."),
            "self.state.name = \"hi\".to_owned();"
        );
        let num = VisualAction::Set {
            field: "vol".to_owned(),
            value: ValueExpr::Number(2.0),
        };
        assert_eq!(action_statement(&num, "state."), "self.state.vol = 2.0;");
        let flag = VisualAction::Set {
            field: "on".to_owned(),
            value: ValueExpr::Flag(false),
        };
        assert_eq!(action_statement(&flag, "state."), "self.state.on = false;");
    }

    #[test]
    fn subtract_and_partial_bounds() {
        let only_min = VisualAction::Subtract {
            field: "p".to_owned(),
            amount: 1.0,
            min: Some(0.0),
            max: None,
        };
        assert_eq!(
            action_statement(&only_min, ""),
            "self.p = (self.p - 1.0).max(0.0);"
        );
        let unbounded = VisualAction::Add {
            field: "p".to_owned(),
            amount: 0.5,
            min: None,
            max: None,
        };
        assert_eq!(action_statement(&unbounded, ""), "self.p = (self.p + 0.5);");
    }

    #[test]
    fn toggle_and_call_handler() {
        let t = VisualAction::Toggle {
            field: "dark".to_owned(),
        };
        assert_eq!(
            action_statement(&t, "state."),
            "self.state.dark = !self.state.dark;"
        );
        let c = VisualAction::CallHandler {
            handler: "on_go".to_owned(),
        };
        assert_eq!(action_statement(&c, "state."), "self.on_go();");
    }

    #[test]
    fn keyword_field_is_sanitized_like_state_bindings() {
        let t = VisualAction::Toggle {
            field: "type".to_owned(),
        };
        assert_eq!(
            action_statement(&t, "state."),
            "self.state.type_value = !self.state.type_value;"
        );
    }

    #[test]
    fn invalid_field_emits_diagnostic_comment_not_code() {
        let bad = VisualAction::Toggle {
            field: "123 bad".to_owned(),
        };
        let stmt = action_statement(&bad, "state.");
        assert!(stmt.starts_with("//"), "invalid field must not emit code");
    }

    #[test]
    fn statements_for_event_filters_by_source_and_event() {
        let btn = Uuid::from_u128(0x1);
        let other = Uuid::from_u128(0x2);
        let mut tree = UiTree::default();
        tree.app_props.behaviors = vec![
            Behavior {
                id: Uuid::from_u128(0x10),
                source_widget: btn,
                event: WidgetEvent::Click,
                target_widget: None,
                action: add_progress(),
            },
            Behavior {
                id: Uuid::from_u128(0x11),
                source_widget: btn,
                event: WidgetEvent::DoubleClick,
                target_widget: None,
                action: VisualAction::Toggle {
                    field: "dark".to_owned(),
                },
            },
            Behavior {
                id: Uuid::from_u128(0x12),
                source_widget: other,
                event: WidgetEvent::Click,
                target_widget: None,
                action: VisualAction::CallHandler {
                    handler: "nope".to_owned(),
                },
            },
        ];

        let s = statements_for_event(&tree, btn, WidgetEvent::Click, "state.", "    ");
        assert_eq!(
            s,
            "    self.state.progress = (self.state.progress + 0.1).clamp(0.0, 1.0);\n"
        );
        assert!(has_behaviors_for_event(
            &tree,
            btn,
            WidgetEvent::DoubleClick
        ));
        assert!(!has_behaviors_for_event(&tree, other, WidgetEvent::Change));
        assert_eq!(call_handler_names(&tree), vec!["nope"]);
    }

    #[test]
    fn f32_literal_always_type_checks_as_f32() {
        assert_eq!(f32_literal(0.1), "0.1");
        assert_eq!(f32_literal(1.0), "1.0");
        assert_eq!(f32_literal(-2.0), "-2.0");
        assert_eq!(f32_literal(2.5), "2.5");
    }
}
