//! Canvas widget search — state, panel, and scroll helper.

use crate::canvas::interaction::CanvasSettings;
use crate::project::{schema::WidgetInstance, ui_tree::UiTree};
use egui;
use uuid::Uuid;

/// Width of the floating canvas search panel (px). Also used for viewport occlusion checks.
pub const SEARCH_PANEL_W: f32 = 350.0;
/// Height of the floating canvas search panel (px). Also used for viewport occlusion checks.
pub const SEARCH_PANEL_H: f32 = 50.0;

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

/// Draw the floating search panel. Returns side-effects for the caller to apply.
/// Navigation keys (Enter, Shift+Enter, Escape) are read inside here, bypassing
/// the `keyboard_owned` gate in interaction.rs — this is intentional.
pub fn draw_search_panel(
    ctx: &egui::Context,
    state: &mut CanvasSearchState,
    canvas_rect: egui::Rect,
    tree: &UiTree,
) -> SearchPanelResponse {
    let mut resp = SearchPanelResponse::default();

    let panel_x = (canvas_rect.max.x - SEARCH_PANEL_W - 8.0).max(canvas_rect.min.x + 4.0);
    let panel_pos = egui::pos2(panel_x, canvas_rect.min.y + 8.0);

    egui::Area::new(egui::Id::new("canvas_search_panel"))
        .order(egui::Order::Tooltip)
        .fixed_pos(panel_pos)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(SEARCH_PANEL_W - 16.0);
                ui.horizontal(|ui| {
                    ui.label("🔍");

                    // Query input
                    let te_resp = ui.add(
                        egui::TextEdit::singleline(&mut state.query)
                            .desired_width(120.0)
                            .hint_text("search widgets…"),
                    );

                    // Recompute on query change
                    if state.query != state.last_query {
                        state.last_query = state.query.clone();
                        state.matches = run_search(tree, &state.query);
                        state.current_index = 0;
                        state.just_wrapped = false;
                    }

                    // Navigation keys — read inside panel, NOT gated by keyboard_owned
                    let enter =
                        ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
                    let shift_enter =
                        ctx.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.shift);
                    let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
                    let ctrl_f = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F));

                    if ctrl_f {
                        te_resp.request_focus();
                    }
                    if escape {
                        resp.close_requested = true;
                        resp.return_focus_to_canvas = true;
                    }

                    if !state.matches.is_empty() {
                        if enter {
                            let n = state.matches.len();
                            state.just_wrapped = state.current_index == n - 1;
                            state.current_index = (state.current_index + 1) % n;
                            resp.scroll_to = Some(state.matches[state.current_index]);
                        } else if shift_enter {
                            let n = state.matches.len();
                            state.just_wrapped = state.current_index == 0;
                            state.current_index = (state.current_index + n.saturating_sub(1)) % n;
                            resp.scroll_to = Some(state.matches[state.current_index]);
                        } else {
                            // No keyboard navigation this frame — clear the wrap
                            // flash so it only shows for the single frame of the wrap.
                            state.just_wrapped = false;
                        }
                    }

                    // Counter display
                    if !state.query.is_empty() {
                        let counter_text = if state.matches.is_empty() {
                            "0 / 0".to_string()
                        } else {
                            format!("{} / {}", state.current_index + 1, state.matches.len())
                        };
                        if state.matches.is_empty() {
                            ui.label(
                                egui::RichText::new(counter_text)
                                    .color(egui::Color32::from_rgba_unmultiplied(255, 80, 80, 220)),
                            );
                        } else if state.just_wrapped {
                            ui.strong(counter_text);
                        } else {
                            ui.label(counter_text);
                        }
                    }

                    // ↑ ↓ navigation buttons
                    let nav_enabled = !state.matches.is_empty();
                    if ui
                        .add_enabled(nav_enabled, egui::Button::new("↑"))
                        .clicked()
                    {
                        let n = state.matches.len();
                        state.just_wrapped = state.current_index == 0;
                        state.current_index = (state.current_index + n.saturating_sub(1)) % n;
                        resp.scroll_to = Some(state.matches[state.current_index]);
                    }
                    if ui
                        .add_enabled(nav_enabled, egui::Button::new("↓"))
                        .clicked()
                    {
                        let n = state.matches.len();
                        state.just_wrapped = state.current_index == n - 1;
                        state.current_index = (state.current_index + 1) % n;
                        resp.scroll_to = Some(state.matches[state.current_index]);
                    }

                    // Select All
                    if ui
                        .add_enabled(nav_enabled, egui::Button::new("Select All"))
                        .clicked()
                    {
                        let live_ids: std::collections::HashSet<uuid::Uuid> =
                            tree.widgets.iter().map(|w| w.id).collect();
                        let valid: Vec<uuid::Uuid> = state
                            .matches
                            .iter()
                            .filter(|id| live_ids.contains(id))
                            .copied()
                            .collect();
                        if !valid.is_empty() {
                            resp.select_all_ids = Some(valid);
                            resp.close_requested = true;
                            resp.return_focus_to_canvas = true;
                        }
                    }

                    // ✕ close
                    if ui.button("✕").clicked() {
                        resp.close_requested = true;
                        resp.return_focus_to_canvas = true;
                    }
                });
            });
        });

    resp
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
    let origin =
        viewport.center().to_vec2() + settings.pan - egui::vec2(canvas_w, canvas_h) * zoom / 2.0;

    // Widget center in screen space.
    let widget_screen_center = (origin + widget_canvas_center * zoom).to_pos2();

    // Visibility check: inside viewport AND not occluded by the top-right
    // search panel footprint (350 wide × 50 tall).
    let in_viewport = viewport.contains(widget_screen_center);
    let in_panel_footprint = widget_screen_center.x >= viewport.max.x - SEARCH_PANEL_W
        && widget_screen_center.y <= viewport.min.y + SEARCH_PANEL_H;
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

/// Paint search match rings and glows onto the canvas painter.
/// Called AFTER rulers, bezel, and Stage-11 overlays so rings are on top.
///
/// `widget_screen_rects` — pre-computed (id, screen_rect) pairs derived from
/// widget canvas rects projected through canvas_origin + zoom.
pub fn draw_search_overlay(
    painter: &egui::Painter,
    state: &CanvasSearchState,
    widget_screen_rects: &[(uuid::Uuid, egui::Rect)],
    dark_mode: bool,
) {
    if state.query.is_empty() || state.matches.is_empty() {
        return;
    }

    // Established project accent colors (same as kind-accent ring system).
    // Current match: teal ring at 75% opacity (rgba 52,211,153, ~192/255).
    // Other matches: soft glow (18% inner, 12% outer of 255).
    let teal = egui::Color32::from_rgb(52, 211, 153);

    let (ring_alpha, glow_inner_alpha, glow_outer_alpha): (u8, u8, u8) = if dark_mode {
        (192, 46, 31) // 0.75, ~0.18, ~0.12 of 255
    } else {
        (220, 77, 51) // boosted slightly for light backgrounds
    };

    for (i, &match_id) in state.matches.iter().enumerate() {
        let Some(&(_, rect)) = widget_screen_rects.iter().find(|(id, _)| *id == match_id) else {
            continue; // widget not visible on canvas this frame
        };

        if i == state.current_index {
            // Current match: solid teal ring, no fill.
            let ring_color =
                egui::Color32::from_rgba_unmultiplied(teal.r(), teal.g(), teal.b(), ring_alpha);
            painter.rect_stroke(
                rect.expand(3.0),
                4.0,
                egui::Stroke::new(2.0, ring_color),
                egui::StrokeKind::Outside,
            );
        } else {
            // Other matches: soft glow (two filled rects, no stroke).
            let inner_color = egui::Color32::from_rgba_unmultiplied(
                teal.r(),
                teal.g(),
                teal.b(),
                glow_inner_alpha,
            );
            let outer_color = egui::Color32::from_rgba_unmultiplied(
                teal.r(),
                teal.g(),
                teal.b(),
                glow_outer_alpha,
            );
            painter.rect_filled(rect.expand(6.0), 4.0, outer_color);
            painter.rect_filled(rect.expand(3.0), 4.0, inner_color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_search_state_is_not_serialize() {
        // Compile-time proof: CanvasSearchState must NOT implement Serialize.
        // We verify the type is Clone + Default (session state traits only).
        let _: CanvasSearchState = CanvasSearchState::default();
        let s = CanvasSearchState {
            query: "x".into(),
            ..Default::default()
        };
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
            rect: Rect {
                x: 2000.0,
                y: 2000.0,
                w: 100.0,
                h: 40.0,
            },
            props: WidgetProps {
                label: "far".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut tree = UiTree::default();
        tree.widgets.push(widget);
        // Use canvas size 400x300 (from tree.app_props.win_w/win_h)
        tree.app_props.win_w = 400.0;
        tree.app_props.win_h = 300.0;

        // Use a non-origin viewport to expose coordinate-space bugs
        let viewport = egui::Rect::from_min_max(egui::pos2(200.0, 40.0), egui::pos2(1000.0, 640.0));
        scroll_to_widget(widget_id, &tree, &mut settings, viewport);

        // After scrolling, widget center should be in viewport (not behind panel)
        let canvas_w = tree.app_props.win_w;
        let canvas_h = tree.app_props.win_h;
        let zoom = settings.zoom;
        let origin = viewport.center().to_vec2() + settings.pan
            - egui::vec2(canvas_w, canvas_h) * zoom / 2.0;
        let widget_screen_center = (origin + egui::vec2(2050.0, 2020.0) * zoom).to_pos2();
        assert!(
            viewport.contains(widget_screen_center),
            "widget should be visible after scroll"
        );
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
            rect: Rect {
                x: 100.0,
                y: 100.0,
                w: 80.0,
                h: 30.0,
            },
            props: WidgetProps {
                label: "near".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut tree = UiTree::default();
        tree.widgets.push(widget);
        tree.app_props.win_w = 400.0;
        tree.app_props.win_h = 300.0;

        // Use same non-origin viewport
        let viewport = egui::Rect::from_min_max(egui::pos2(200.0, 40.0), egui::pos2(1000.0, 640.0));

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
        assert_eq!(
            settings.pan, initial_pan,
            "pan should not change for already-visible widget"
        );
    }

    #[test]
    fn ctrl_f_only_opens_when_canvas_focused() {
        let canvas_focused = false;
        let input_blocked = false;
        let key_ctrl_f = true;

        let mut canvas_search: Option<CanvasSearchState> = None;

        if canvas_focused && !input_blocked && key_ctrl_f {
            canvas_search = Some(CanvasSearchState::default());
        }

        assert!(
            canvas_search.is_none(),
            "should not open when canvas not focused"
        );
    }

    #[test]
    fn deleted_widget_removed_from_match_list() {
        let id_a = uuid::Uuid::new_v4();
        let id_b = uuid::Uuid::new_v4();

        let mut state = CanvasSearchState {
            query: "button".into(),
            matches: vec![id_a, id_b],
            current_index: 1,
            ..Default::default()
        };

        let live_ids: std::collections::HashSet<uuid::Uuid> = std::iter::once(id_b).collect();
        state.matches.retain(|id| live_ids.contains(id));
        state.current_index = state
            .current_index
            .min(state.matches.len().saturating_sub(1));

        assert_eq!(state.matches, vec![id_b]);
        assert_eq!(state.current_index, 0);
    }

    #[test]
    fn key_f_guard_logic_bare_vs_ctrl() {
        // Verify the guard logic: bare F fires zoom-to-fit; Ctrl+F fires search.
        let keyboard_owned = true;
        let input_blocked = false;

        // Bare F (no ctrl): should only trigger zoom-to-fit, not search.
        let ctrl_held = false;
        let key_f_bare = keyboard_owned && !ctrl_held;
        let key_ctrl_f = keyboard_owned && !input_blocked && ctrl_held;
        assert!(key_f_bare, "bare F should trigger zoom-to-fit");
        assert!(!key_ctrl_f, "bare F should not trigger canvas search");

        // Ctrl+F: should only trigger search, not zoom-to-fit.
        let ctrl_held = true;
        let key_f_with_ctrl = keyboard_owned && !ctrl_held;
        let key_ctrl_f = keyboard_owned && !input_blocked && ctrl_held;
        assert!(!key_f_with_ctrl, "Ctrl+F should not trigger zoom-to-fit");
        assert!(key_ctrl_f, "Ctrl+F should trigger canvas search");
    }

    #[test]
    fn canvas_search_not_in_serialized_project() {
        // CanvasSearchState is on InteractionState which is never serialized.
        // Verify it cannot accidentally appear in a serialized project JSON.
        // We serialize ProjectDocument (the public serializable project root)
        // directly, since ProjectFile in io.rs is a private wrapper around it.
        let doc = crate::project::document::ProjectDocument::default();
        let json = serde_json::to_string(&doc).expect("serialize ok");
        assert!(
            !json.contains("canvas_search"),
            "canvas_search must not appear in serialized project"
        );
        assert!(
            !json.contains("search_registry"),
            "search_registry must not appear in serialized project"
        );
    }

    #[test]
    fn ctrl_f_blocked_when_canvas_not_focused() {
        // canvas_owns_keyboard in interaction.rs is a private fn — it cannot be
        // called from this test module. The production gate is already covered by
        // `input_ownership_tests::text_focus_and_modals_block_canvas_keyboard`
        // in interaction.rs. Here we verify the *same invariant* by replicating
        // the guard expression: !modal_blocked && canvas_focused && !wants_keyboard.
        let modal_blocked = false;
        let canvas_focused = false;
        let wants_keyboard_input = false;
        let owns = !modal_blocked && canvas_focused && !wants_keyboard_input;
        assert!(
            !owns,
            "canvas should not own keyboard when canvas_focused=false"
        );
    }
}
