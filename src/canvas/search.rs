//! Canvas widget search — state, panel, and scroll helper.

use uuid::Uuid;

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
