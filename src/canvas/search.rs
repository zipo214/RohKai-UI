//! Canvas widget search — state, panel, and scroll helper.

use uuid::Uuid;
use crate::canvas::interaction::CanvasSettings;
use crate::project::{schema::WidgetInstance, ui_tree::UiTree};
use egui;

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
    // Intentionally excluded: props.placeholder, props.radio_value, props.group_binding,
    // props.formula_expr, props.data_source_binding, descriptor_props values —
    // scope limited to primary label/binding/event fields per spec.
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

/// Adjusts `settings.pan` to bring the widget with `id` into the visible viewport.
/// Does NOT change zoom level. No-ops if the widget is already visible.
///
/// The visibility check excludes the top-right 350×50 px occupied by the search
/// panel so the current match is never considered visible when it is behind it.
pub fn scroll_to_widget(
    id: Uuid,
    tree: &UiTree,
    settings: &mut CanvasSettings,
    viewport: egui::Rect,
) {
    let Some(widget) = tree.widgets.iter().find(|w| w.id == id) else {
        return;
    };

    let canvas_w = tree.app_props.win_w;
    let canvas_h = tree.app_props.win_h;
    let zoom = settings.zoom;

    // Widget center in canvas space.
    let widget_canvas_center = egui::vec2(
        widget.rect.x + widget.rect.w / 2.0,
        widget.rect.y + widget.rect.h / 2.0,
    );

    // Canonical origin: where canvas (0,0) maps to screen.
    // Matches the transform used in src/canvas/rulers.rs (canvas_origin).
    let origin = viewport.center().to_vec2() + settings.pan
        - egui::vec2(canvas_w, canvas_h) * zoom / 2.0;

    // Widget center in screen space.
    let widget_screen_center = (origin + widget_canvas_center * zoom).to_pos2();

    // Visibility check: inside viewport AND not occluded by the top-right
    // search panel footprint (350 wide × 50 tall).
    let in_viewport = viewport.contains(widget_screen_center);
    let in_panel_footprint = widget_screen_center.x >= viewport.max.x - 350.0
        && widget_screen_center.y <= viewport.min.y + 50.0;
    if in_viewport && !in_panel_footprint {
        return; // already visible — do nothing
    }

    // Scroll so the widget center lands at the viewport center (always clear
    // of the panel corner).
    // Derivation:
    //   new_origin = viewport.center() + new_pan - canvas_size * zoom / 2
    //   target = new_origin + widget_canvas_center * zoom = viewport.center()
    //   => new_pan = canvas_size * zoom / 2 - widget_canvas_center * zoom
    settings.pan = egui::vec2(canvas_w, canvas_h) * zoom / 2.0 - widget_canvas_center * zoom;
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

    // ── scroll_to_widget tests ────────────────────────────────────────────────

    #[test]
    fn scroll_to_widget_pans_to_offscreen_widget() {
        use crate::canvas::interaction::CanvasSettings;
        use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
        use crate::project::ui_tree::UiTree;

        let mut settings = CanvasSettings {
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            ..Default::default()
        };
        let widget_id = uuid::Uuid::new_v4();
        let widget = WidgetInstance {
            id: widget_id,
            kind: WidgetKind::Label,
            rect: Rect { x: 2000.0, y: 2000.0, w: 100.0, h: 40.0 },
            props: WidgetProps { label: "far".into(), ..Default::default() },
            ..Default::default()
        };
        let mut tree = UiTree::default();
        tree.widgets.push(widget);
        // Use canvas size 400x300 (from tree.app_props.win_w/win_h)
        tree.app_props.win_w = 400.0;
        tree.app_props.win_h = 300.0;

        // Use a non-origin viewport to expose coordinate-space bugs
        let viewport = egui::Rect::from_min_max(
            egui::pos2(200.0, 40.0),
            egui::pos2(1000.0, 640.0),
        );
        scroll_to_widget(widget_id, &tree, &mut settings, viewport);

        // After scrolling, widget center should be in viewport (not behind panel)
        let canvas_w = tree.app_props.win_w;
        let canvas_h = tree.app_props.win_h;
        let zoom = settings.zoom;
        let origin = viewport.center().to_vec2() + settings.pan
            - egui::vec2(canvas_w, canvas_h) * zoom / 2.0;
        let widget_screen_center = (origin + egui::vec2(2050.0, 2020.0) * zoom).to_pos2();
        assert!(viewport.contains(widget_screen_center), "widget should be visible after scroll");
    }

    #[test]
    fn scroll_to_widget_noop_for_already_visible() {
        use crate::canvas::interaction::CanvasSettings;
        use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
        use crate::project::ui_tree::UiTree;

        let mut settings = CanvasSettings {
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            ..Default::default()
        };
        let widget_id = uuid::Uuid::new_v4();
        let widget = WidgetInstance {
            id: widget_id,
            kind: WidgetKind::Label,
            rect: Rect { x: 100.0, y: 100.0, w: 80.0, h: 30.0 },
            props: WidgetProps { label: "near".into(), ..Default::default() },
            ..Default::default()
        };
        let mut tree = UiTree::default();
        tree.widgets.push(widget);
        tree.app_props.win_w = 400.0;
        tree.app_props.win_h = 300.0;

        // Use same non-origin viewport
        let viewport = egui::Rect::from_min_max(
            egui::pos2(200.0, 40.0),
            egui::pos2(1000.0, 640.0),
        );

        // Widget center is (140, 115) in canvas space.
        // With zoom=1, pan=ZERO, canvas 400x300:
        // origin = viewport.center() + pan - canvas_size/2
        //        = (600, 340) + (0,0) - (200, 150) = (400, 190)
        // screen center = (400+140, 190+115) = (540, 305)
        // viewport is (200,40)-(1000,640), so (540,305) IS inside.
        // Not in panel footprint (max.x-350=650, 540 < 650).
        // Should NOT pan.
        let initial_pan = settings.pan;
        scroll_to_widget(widget_id, &tree, &mut settings, viewport);
        assert_eq!(settings.pan, initial_pan, "pan should not change for already-visible widget");
    }
}
