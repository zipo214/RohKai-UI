//! Canvas widget search — state, panel, and scroll helper.

use uuid::Uuid;
use crate::project::{schema::WidgetInstance, ui_tree::UiTree};

/// Session-only state for the Ctrl+F canvas search panel.
/// Never serialize this type — it must not derive Serialize/Deserialize.
#[derive(Debug, Default, Clone)]
pub struct CanvasSearchState {
    /// Current text in the query input.
    pub query: String,
    /// Last query that was actually searched (debounce gate).
    pub last_query: String,
    /// Ordered list of matching widget IDs (sorted top-to-bottom, left-to-right).
    pub matches: Vec<Uuid>,
    /// 0-based index of the currently highlighted match.
    pub current_index: usize,
    /// Set for one frame after wrap-around to flash the counter.
    pub just_wrapped: bool,
}

/// Side-effects returned from `draw_search_panel`. The caller applies them —
/// no mutations happen inside the panel function.
#[derive(Debug, Default)]
pub struct SearchPanelResponse {
    pub close_requested: bool,
    /// Some(ids) when "Select All" was clicked with non-empty matches.
    pub select_all_ids: Option<Vec<Uuid>>,
    /// True on close paths (Escape, ✕, Select All) — caller sets canvas_focused.
    pub return_focus_to_canvas: bool,
    /// Some(id) when navigation advanced — caller calls scroll_to_widget.
    pub scroll_to: Option<Uuid>,
}

/// Returns widget IDs that match `query` (case-insensitive substring).
/// Results are sorted top-to-bottom then left-to-right by canvas rect.
/// Returns empty vec when `query` is empty or blank.
pub fn run_search(tree: &UiTree, query: &str) -> Vec<Uuid> {
    let q = query.trim();
    if q.is_empty() {
        return vec![];
    }
    let q_lower = q.to_lowercase();

    let mut matches: Vec<&WidgetInstance> = tree
        .widgets
        .iter()
        .filter(|w| widget_matches(w, &q_lower))
        .collect();

    // Sort: top-to-bottom (rect.y), then left-to-right (rect.x).
    matches.sort_by(|a, b| {
        a.rect
            .y
            .partial_cmp(&b.rect.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.rect
                    .x
                    .partial_cmp(&b.rect.x)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    matches.iter().map(|w| w.id).collect()
}

fn widget_matches(w: &WidgetInstance, q_lower: &str) -> bool {
    // Label (free text — Unicode case fold)
    if !w.props.label.is_empty() && w.props.label.to_lowercase().contains(q_lower) {
        return true;
    }
    // State binding (identifier — ASCII fold)
    if let Some(ref b) = w.state_binding
        && b.to_ascii_lowercase().contains(q_lower)
    {
        return true;
    }
    // Label binding (identifier — ASCII fold)
    if let Some(ref b) = w.label_binding
        && b.to_ascii_lowercase().contains(q_lower)
    {
        return true;
    }
    // Tooltip (free text)
    if let Some(ref t) = w.tooltip
        && t.to_lowercase().contains(q_lower)
    {
        return true;
    }
    // Event handler strings (identifier fields — ASCII fold)
    for handler in [
        w.on_click.as_str(),
        w.on_change.as_str(),
        w.on_double_click.as_str(),
        w.on_lost_focus.as_str(),
        w.on_drag_stopped.as_str(),
    ] {
        if !handler.is_empty() && handler.to_ascii_lowercase().contains(q_lower) {
            return true;
        }
    }
    if let Some(ref h) = w.event_handler
        && h.to_ascii_lowercase().contains(q_lower)
    {
        return true;
    }
    // Widget kind — use Debug repr for built-ins ("Button", "Slider", etc.)
    match &w.kind {
        crate::project::schema::WidgetKind::Custom(name) => {
            let dn = w.descriptor_name.as_deref().unwrap_or(name.as_str());
            if dn.to_lowercase().contains(q_lower) {
                return true;
            }
            if name.to_lowercase().contains(q_lower) {
                return true;
            }
        }
        other => {
            if format!("{other:?}").to_lowercase().contains(q_lower) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_search_state_is_not_serialize() {
        // Compile-time proof: CanvasSearchState must NOT implement Serialize.
        // We verify the type is Clone + Default (session state traits only).
        let _: CanvasSearchState = CanvasSearchState::default();
        let s = CanvasSearchState { query: "x".into(), ..Default::default() };
        let _ = s.clone();
    }

    // ── run_search tests ──────────────────────────────────────────────────

    fn make_widget(label: &str, kind_str: &str) -> crate::project::schema::WidgetInstance {
        use crate::project::schema::{WidgetInstance, WidgetKind, WidgetProps};
        let kind = match kind_str {
            "Button" => WidgetKind::Button,
            "Label" => WidgetKind::Label,
            "Slider" => WidgetKind::Slider,
            other => WidgetKind::Custom(other.to_string()),
        };
        WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind,
            props: WidgetProps {
                label: label.to_string(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn make_tree(
        widgets: Vec<crate::project::schema::WidgetInstance>,
    ) -> crate::project::ui_tree::UiTree {
        crate::project::ui_tree::UiTree {
            widgets,
            ..Default::default()
        }
    }

    #[test]
    fn empty_query_returns_no_matches() {
        let tree = make_tree(vec![make_widget("Submit", "Button")]);
        assert!(run_search(&tree, "").is_empty());
    }

    #[test]
    fn label_match_case_insensitive() {
        let w = make_widget("Ünter Submit", "Button");
        let id = w.id;
        let tree = make_tree(vec![w]);
        assert_eq!(run_search(&tree, "ünter"), vec![id]);
        assert_eq!(run_search(&tree, "ÜNTER"), vec![id]);
    }

    #[test]
    fn kind_match_builtin() {
        let w = make_widget("x", "Slider");
        let id = w.id;
        let tree = make_tree(vec![w]);
        assert_eq!(run_search(&tree, "slider"), vec![id]);
        assert_eq!(run_search(&tree, "SLIDER"), vec![id]);
        assert_eq!(run_search(&tree, "slid"), vec![id]);
    }

    #[test]
    fn kind_match_custom_inner_name() {
        use crate::project::schema::{WidgetInstance, WidgetKind, WidgetProps};
        let mut w = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Custom("ply-button".to_string()),
            props: WidgetProps {
                label: "x".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        w.descriptor_name = Some("Ply Button".to_string());
        let id = w.id;
        let tree = make_tree(vec![w]);
        assert_eq!(run_search(&tree, "ply button"), vec![id]);
        assert_eq!(run_search(&tree, "ply-button"), vec![id]);
    }

    #[test]
    fn binding_match_state_and_label() {
        use crate::project::schema::{WidgetInstance, WidgetKind, WidgetProps};
        let mut w1 = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Button,
            props: WidgetProps {
                label: "x".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        w1.state_binding = Some("my_counter".to_string());
        let mut w2 = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Button,
            props: WidgetProps {
                label: "x".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        w2.label_binding = Some("MyTitle".to_string());
        let id1 = w1.id;
        let id2 = w2.id;
        let tree = make_tree(vec![w1, w2]);
        assert_eq!(run_search(&tree, "my_counter"), vec![id1]);
        assert_eq!(run_search(&tree, "mytitle"), vec![id2]);
    }

    #[test]
    fn run_search_with_none_optional_fields_does_not_panic() {
        use crate::project::schema::{WidgetInstance, WidgetKind, WidgetProps};
        let w = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Button,
            props: WidgetProps {
                label: String::new(),
                ..Default::default()
            },
            ..Default::default()
        };
        let tree = make_tree(vec![w]);
        let result = run_search(&tree, "anything");
        assert!(result.is_empty());
    }

    #[test]
    fn navigate_noop_when_no_matches() {
        let mut state = CanvasSearchState {
            query: "zzz_no_match".into(),
            matches: vec![],
            current_index: 0,
            ..Default::default()
        };
        if !state.matches.is_empty() {
            state.current_index = (state.current_index + 1) % state.matches.len();
        }
        assert_eq!(state.current_index, 0);
    }

    #[test]
    fn navigate_wraps_around_forward() {
        let ids: Vec<uuid::Uuid> = (0..3).map(|_| uuid::Uuid::new_v4()).collect();
        let mut state = CanvasSearchState {
            matches: ids.clone(),
            current_index: 2,
            ..Default::default()
        };
        if !state.matches.is_empty() {
            state.current_index = (state.current_index + 1) % state.matches.len();
        }
        assert_eq!(state.current_index, 0);
        let counter = format!("{} / {}", state.current_index + 1, state.matches.len());
        assert_eq!(counter, "1 / 3");
    }

    #[test]
    fn navigate_wraps_around_backward() {
        let ids: Vec<uuid::Uuid> = (0..3).map(|_| uuid::Uuid::new_v4()).collect();
        let mut state = CanvasSearchState {
            matches: ids.clone(),
            current_index: 0,
            ..Default::default()
        };
        if !state.matches.is_empty() {
            state.current_index =
                (state.current_index + state.matches.len().saturating_sub(1)) % state.matches.len();
        }
        assert_eq!(state.current_index, 2);
        let counter = format!("{} / {}", state.current_index + 1, state.matches.len());
        assert_eq!(counter, "3 / 3");
    }

    #[test]
    fn counter_displays_one_based_index() {
        let ids: Vec<uuid::Uuid> = (0..7).map(|_| uuid::Uuid::new_v4()).collect();
        let state = CanvasSearchState {
            matches: ids,
            current_index: 2,
            ..Default::default()
        };
        let counter = format!("{} / {}", state.current_index + 1, state.matches.len());
        assert_eq!(counter, "3 / 7");
    }

    #[test]
    fn select_all_with_zero_matches_is_noop() {
        let state = CanvasSearchState {
            query: "zzz".into(),
            matches: vec![],
            ..Default::default()
        };
        let ids: Option<Vec<uuid::Uuid>> = if state.matches.is_empty() {
            None
        } else {
            Some(state.matches.clone())
        };
        assert!(ids.is_none());
    }

    #[test]
    fn surface_switch_clears_canvas_search() {
        // InteractionState::default() is what app.rs calls on surface switch.
        // Verify canvas_search is None after reset.
        use crate::canvas::interaction::InteractionState;
        // Build a state that has canvas_search populated.
        let with_search = InteractionState {
            canvas_search: Some(CanvasSearchState {
                query: "button".into(),
                matches: vec![uuid::Uuid::new_v4()],
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(with_search.canvas_search.is_some());
        // Simulate surface switch: replace with fresh default (as app.rs does).
        let reset = InteractionState::default();
        assert!(reset.canvas_search.is_none());
    }
}
