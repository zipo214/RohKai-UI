//! Behavior recipes — the interaction matrix that turns a (source event,
//! sink state field) pair into suggested typed [`VisualAction`]s.
//!
//! This layer is a **smart constructor only**: accepting a recipe inserts a
//! plain `Behavior` into `AppProps.behaviors` (the saved source of truth).
//! Nothing here is persisted, no recipe IDs reach codegen, and no Rust syntax
//! strings are assembled — `codegen::behavior` keeps emitting from the graph.
//!
//! Invariant: every suggestion this module returns carries an action that
//! [`crate::codegen::behavior::action_statement`] emits as a real statement.
//! A sink type with no type-safe action (e.g. `Vec<f32>`) gets **no**
//! suggestions instead of a broken one.

use crate::project::schema::{ValueExpr, VisualAction, WidgetEvent, WidgetInstance};

/// What a wire is being dropped on: the bound state field plus the type and
/// numeric range derived from the canonical `kind_table::state_info`.
#[derive(Debug, Clone, PartialEq)]
pub struct SinkInfo {
    /// Raw bound field name (sanitized later by `codegen::behavior`).
    pub field: String,
    /// Rust type from `kind_table::state_info` (`"f32"`, `"bool"`, …).
    pub rust_type: &'static str,
    /// Numeric range from the target widget's props (f32 sinks only).
    pub min: f32,
    pub max: f32,
}

/// One suggested behavior for a source/sink pair, ready to insert.
#[derive(Debug, Clone, PartialEq)]
pub struct RecipeSuggestion {
    /// Short UI label, e.g. `"Add 0.1 (clamped)"`.
    pub label: &'static str,
    /// The fully-typed action the recipe constructs.
    pub action: VisualAction,
}

/// Describe `widget` as a behavior sink: requires a non-empty `state_binding`
/// and a kind that carries state per `kind_table::state_info` (never re-listed
/// per kind).  Returns `None` otherwise.
pub fn sink_info_for(widget: &WidgetInstance) -> Option<SinkInfo> {
    let binding = widget.state_binding.as_deref().map(str::trim)?;
    if binding.is_empty() {
        return None;
    }
    let info = crate::codegen::kind_table::state_info(&widget.kind)?;
    Some(SinkInfo {
        field: binding.to_owned(),
        rust_type: info.rust_type,
        min: widget.props.min.min(widget.props.max),
        max: widget.props.max.max(widget.props.min),
    })
}

/// The interaction matrix: suggested typed actions for connecting `event` to
/// `sink`, default first.  Empty when the sink type has no type-safe action.
///
/// `event` currently does not narrow the suggestion set — every wired event
/// fires the same family of state mutations — but it is part of the matrix
/// signature so event-specific recipes can be added without reshaping callers.
pub fn suggestions_for(_event: WidgetEvent, sink: &SinkInfo) -> Vec<RecipeSuggestion> {
    let field = || sink.field.clone();
    match sink.rust_type {
        "f32" => vec![
            RecipeSuggestion {
                label: "Add 0.1 (clamped)",
                action: VisualAction::Add {
                    field: field(),
                    amount: 0.1,
                    min: Some(sink.min),
                    max: Some(sink.max),
                },
            },
            RecipeSuggestion {
                label: "Subtract 0.1 (clamped)",
                action: VisualAction::Subtract {
                    field: field(),
                    amount: 0.1,
                    min: Some(sink.min),
                    max: Some(sink.max),
                },
            },
            RecipeSuggestion {
                label: "Set 1.0",
                action: VisualAction::Set {
                    field: field(),
                    value: ValueExpr::Number(1.0),
                },
            },
            RecipeSuggestion {
                label: "Reset 0.0",
                action: VisualAction::Set {
                    field: field(),
                    value: ValueExpr::Number(0.0),
                },
            },
        ],
        "bool" => vec![
            RecipeSuggestion {
                label: "Toggle",
                action: VisualAction::Toggle { field: field() },
            },
            RecipeSuggestion {
                label: "Set true",
                action: VisualAction::Set {
                    field: field(),
                    value: ValueExpr::Flag(true),
                },
            },
            RecipeSuggestion {
                label: "Set false",
                action: VisualAction::Set {
                    field: field(),
                    value: ValueExpr::Flag(false),
                },
            },
        ],
        "String" => vec![RecipeSuggestion {
            label: "Set text…",
            action: VisualAction::Set {
                field: field(),
                value: ValueExpr::Text(String::new()),
            },
        }],
        // No type-safe single action for collection types (Vec<f32> charts
        // etc.): show nothing rather than a broken suggestion.
        _ => Vec::new(),
    }
}

/// The matrix's default pick for a source/sink pair (first suggestion).
pub fn default_suggestion(event: WidgetEvent, sink: &SinkInfo) -> Option<RecipeSuggestion> {
    suggestions_for(event, sink).into_iter().next()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{Behavior, WidgetKind, WidgetProps};
    use crate::project::ui_tree::UiTree;
    use uuid::Uuid;

    fn f32_sink() -> SinkInfo {
        SinkInfo {
            field: "progress".to_owned(),
            rust_type: "f32",
            min: 0.0,
            max: 1.0,
        }
    }

    #[test]
    fn button_click_f32_default_is_add_clamped() {
        let s = default_suggestion(WidgetEvent::Click, &f32_sink()).expect("default");
        assert_eq!(s.label, "Add 0.1 (clamped)");
        assert_eq!(
            s.action,
            VisualAction::Add {
                field: "progress".to_owned(),
                amount: 0.1,
                min: Some(0.0),
                max: Some(1.0),
            }
        );
    }

    #[test]
    fn f32_matrix_offers_set_and_reset_alternates() {
        let labels: Vec<&str> = suggestions_for(WidgetEvent::Click, &f32_sink())
            .iter()
            .map(|s| s.label)
            .collect();
        assert_eq!(
            labels,
            vec![
                "Add 0.1 (clamped)",
                "Subtract 0.1 (clamped)",
                "Set 1.0",
                "Reset 0.0"
            ]
        );
    }

    #[test]
    fn bool_default_is_toggle_and_string_default_is_set_text() {
        let bool_sink = SinkInfo {
            field: "dark".to_owned(),
            rust_type: "bool",
            min: 0.0,
            max: 1.0,
        };
        let s = default_suggestion(WidgetEvent::Click, &bool_sink).expect("bool default");
        assert_eq!(
            s.action,
            VisualAction::Toggle {
                field: "dark".to_owned()
            }
        );

        let string_sink = SinkInfo {
            field: "name".to_owned(),
            rust_type: "String",
            min: 0.0,
            max: 1.0,
        };
        let s = default_suggestion(WidgetEvent::Click, &string_sink).expect("string default");
        assert!(matches!(
            s.action,
            VisualAction::Set {
                value: ValueExpr::Text(_),
                ..
            }
        ));
    }

    #[test]
    fn unsupported_sink_type_shows_no_suggestions() {
        let vec_sink = SinkInfo {
            field: "samples".to_owned(),
            rust_type: "Vec<f32>",
            min: 0.0,
            max: 1.0,
        };
        assert!(
            suggestions_for(WidgetEvent::Click, &vec_sink).is_empty(),
            "no type-safe action for Vec<f32> — must show nothing"
        );
    }

    #[test]
    fn every_shown_suggestion_emits_real_code() {
        // Invariant: a suggestion either constructs a supported typed action
        // (action_statement emits a real statement) or is not shown at all.
        // Walk the full matrix: every event × every state type kind_table can
        // produce, plus an unsupported type as the negative case.
        use crate::project::schema::WidgetEvent::*;
        let sinks = [
            ("f32", "progress"),
            ("bool", "dark"),
            ("String", "name"),
            ("Vec<f32>", "samples"),
        ];
        for event in [Click, DoubleClick, Change, LostFocus, DragStopped] {
            for (rust_type, field) in sinks {
                let sink = SinkInfo {
                    field: field.to_owned(),
                    rust_type,
                    min: 0.0,
                    max: 1.0,
                };
                for s in suggestions_for(event, &sink) {
                    let stmt = crate::codegen::behavior::action_statement(&s.action, "state.");
                    assert!(
                        stmt.ends_with(';') && !stmt.trim_start().starts_with("//"),
                        "{event:?} + {rust_type} suggestion {:?} must emit code, got: {stmt}",
                        s.label
                    );
                }
            }
        }
    }

    #[test]
    fn sink_info_derives_from_binding_and_kind_table() {
        let bar = WidgetInstance {
            kind: WidgetKind::ProgressBar,
            state_binding: Some("progress".to_owned()),
            props: WidgetProps {
                min: 0.0,
                max: 1.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let sink = sink_info_for(&bar).expect("bound ProgressBar is a sink");
        assert_eq!(sink.rust_type, "f32");
        assert_eq!(sink.field, "progress");

        let unbound = WidgetInstance {
            kind: WidgetKind::ProgressBar,
            ..Default::default()
        };
        assert!(sink_info_for(&unbound).is_none(), "no binding → no sink");

        let button = WidgetInstance {
            kind: WidgetKind::Button,
            state_binding: Some("x".to_owned()),
            ..Default::default()
        };
        assert!(sink_info_for(&button).is_none(), "stateless kind → no sink");
    }

    #[test]
    fn accepted_recipe_creates_graph_action_and_exports_mutation() {
        // Acceptance path: wire drop → default recipe → plain Behavior in the
        // graph → export emits the canonical progress mutation.  Codegen never
        // sees a recipe ID.
        let btn_id = Uuid::from_u128(0xC1);
        let bar = WidgetInstance {
            id: Uuid::from_u128(0xC2),
            kind: WidgetKind::ProgressBar,
            state_binding: Some("progress".to_owned()),
            ..Default::default()
        };
        let btn = WidgetInstance {
            id: btn_id,
            kind: WidgetKind::Button,
            props: WidgetProps {
                label: "More".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };

        let sink = sink_info_for(&bar).expect("sink");
        let suggestion = default_suggestion(WidgetEvent::Click, &sink).expect("default");

        let mut tree = UiTree {
            widgets: vec![btn, bar.clone()],
            ..Default::default()
        };
        tree.app_props.behaviors.push(Behavior::widget(
            Uuid::from_u128(0xC3),
            btn_id,
            WidgetEvent::Click,
            Some(bar.id),
            suggestion.action,
        ));

        let app_rs = crate::codegen::export::project_files(&tree)
            .into_iter()
            .find(|(name, _)| name == "src/app.rs")
            .map(|(_, content)| content)
            .expect("exported app.rs");
        assert!(
            app_rs.contains("self.state.progress = (self.state.progress + 0.1).clamp(0.0, 1.0);"),
            "accepted recipe must export the canonical mutation"
        );
    }
}
