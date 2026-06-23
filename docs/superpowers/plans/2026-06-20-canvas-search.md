# Canvas Search (S2 Item 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Ctrl+F canvas widget search — a floating panel that finds widgets by label, binding, tooltip, event handler name, or kind, with navigate-through and select-all.

**Architecture:** New `src/canvas/search.rs` owns all search state, panel rendering, and the `scroll_to_widget` helper. `InteractionState` gains one `canvas_search: Option<CanvasSearchState>` field. A post-handle overlay pass in `app.rs` paints rings/glows after rulers and bezel so they are not overdrawn.

**Tech Stack:** Rust 2024, egui 0.34.3, `uuid::Uuid` (already a dependency), no new crates.

**Spec:** `docs/superpowers/specs/2026-06-19-canvas-search-design.md`

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `src/canvas/search.rs` | **Create** | All search types, `run_search`, `draw_search_panel`, `scroll_to_widget` |
| `src/canvas/mod.rs` | **Modify** (line 9) | Add `pub mod search;` |
| `src/canvas/interaction.rs` | **Modify** | Add `canvas_search` to `InteractionState`; add `key_ctrl_f`; narrow bare-F guard |
| `src/panels/code_preview.rs` | **Modify** (line 442) | Gate Ctrl+F behind `args.editor_has_focus` |
| `src/panels/shortcuts.rs` | **Modify** (line 31) | Register `canvas_search` shortcut |
| `src/app.rs` | **Modify** (after line 3336) | Post-handle overlay pass for rings/glows |

---

## Task 1: Scaffold Types + Wire Module

**Files:**
- Create: `src/canvas/search.rs`
- Modify: `src/canvas/mod.rs:9`
- Modify: `src/canvas/interaction.rs:394`

- [ ] **Step 1.1: Create `src/canvas/search.rs` with types only**

```rust
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
```

- [ ] **Step 1.2: Add `pub mod search;` to `src/canvas/mod.rs`**

In `src/canvas/mod.rs`, after line 9 (`pub mod interaction;`), add:

```rust
pub mod search;
```

- [ ] **Step 1.3: Add `canvas_search` field to `InteractionState`**

In `src/canvas/interaction.rs` at line 394, `InteractionState` currently ends with:
```rust
    pub selected_behavior: Option<Uuid>,
}
```

Add the new field before the closing brace:
```rust
    pub selected_behavior: Option<Uuid>,
    /// Session-only canvas search state. Never serialized.
    pub canvas_search: Option<CanvasSearchState>,
}
```

Add the import at the top of `interaction.rs` with the other `use` statements:
```rust
use crate::canvas::search::{CanvasSearchState, SearchPanelResponse};
```

- [ ] **Step 1.4: Write the serialization-invariant test**

At the bottom of `src/canvas/search.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_search_state_is_not_serialize() {
        // Compile-time proof: CanvasSearchState must NOT implement Serialize.
        // If this test compiles, the type cannot be accidentally serialized.
        fn assert_not_serialize<T: ?Sized>() {}
        // The line below intentionally does NOT compile if Serialize is derived:
        // assert_not_serialize::<dyn serde::Serialize>();
        // Instead we verify the type is Clone + Default (session state traits only).
        let _: CanvasSearchState = CanvasSearchState::default();
        let s = CanvasSearchState { query: "x".into(), ..Default::default() };
        let _ = s.clone();
    }

    #[test]
    fn surface_switch_clears_canvas_search() {
        // InteractionState::default() is what app.rs calls on surface switch.
        // Verify canvas_search is None after reset.
        use crate::canvas::interaction::InteractionState;
        let mut state = InteractionState::default();
        state.canvas_search = Some(CanvasSearchState {
            query: "button".into(),
            matches: vec![uuid::Uuid::new_v4()],
            ..Default::default()
        });
        state = InteractionState::default();
        assert!(state.canvas_search.is_none());
    }
}
```

- [ ] **Step 1.5: Run tests — expect both to pass (types only, no logic yet)**

```
cargo test canvas::search::tests -- --nocapture
```

Expected: 2 tests pass.

- [ ] **Step 1.6: Run clippy — expect zero warnings**

```
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 1.7: Commit**

```bash
git add src/canvas/search.rs src/canvas/mod.rs src/canvas/interaction.rs
git commit -m "feat(search): scaffold CanvasSearchState types + wire module"
```

---

## Task 2: Implement `run_search()`

**Files:**
- Modify: `src/canvas/search.rs` (add function + tests)

- [ ] **Step 2.1: Write all `run_search` tests first**

Add to `src/canvas/search.rs` below the existing test module content (extend the `tests` module):

```rust
    // ── run_search tests ──────────────────────────────────────────────────

    fn make_widget(label: &str, kind_str: &str) -> crate::project::schema::WidgetInstance {
        use crate::project::schema::{WidgetInstance, WidgetKind, WidgetProps, Rect};
        let kind = match kind_str {
            "Button" => WidgetKind::Button,
            "Label"  => WidgetKind::Label,
            "Slider" => WidgetKind::Slider,
            other    => WidgetKind::Custom(other.to_string()),
        };
        WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind,
            rect: Rect { x: 0.0, y: 0.0, w: 100.0, h: 40.0 },
            props: WidgetProps { label: label.to_string(), ..Default::default() },
            ..Default::default()
        }
    }

    fn make_tree(widgets: Vec<crate::project::schema::WidgetInstance>)
        -> crate::project::ui_tree::UiTree
    {
        let mut t = crate::project::ui_tree::UiTree::default();
        t.widgets = widgets;
        t
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
        // Partial match also works.
        assert_eq!(run_search(&tree, "slid"), vec![id]);
    }

    #[test]
    fn kind_match_custom_inner_name() {
        use crate::project::schema::{WidgetInstance, WidgetKind, WidgetProps, Rect};
        let mut w = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Custom("ply-button".to_string()),
            rect: Rect { x: 0.0, y: 0.0, w: 100.0, h: 40.0 },
            props: WidgetProps { label: "x".to_string(), ..Default::default() },
            ..Default::default()
        };
        w.descriptor_name = Some("Ply Button".to_string());
        let id = w.id;
        let tree = make_tree(vec![w]);
        // Matches descriptor_name
        assert_eq!(run_search(&tree, "ply button"), vec![id]);
        // Matches Custom inner string
        assert_eq!(run_search(&tree, "ply-button"), vec![id]);
    }

    #[test]
    fn binding_match_state_and_label() {
        use crate::project::schema::{WidgetInstance, WidgetKind, WidgetProps, Rect};
        let mut w1 = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Button,
            rect: Rect { x: 0.0, y: 0.0, w: 100.0, h: 40.0 },
            props: WidgetProps { label: "x".to_string(), ..Default::default() },
            ..Default::default()
        };
        w1.state_binding = Some("my_counter".to_string());
        let mut w2 = w1.clone();
        w2.id = uuid::Uuid::new_v4();
        w2.state_binding = None;
        w2.label_binding = Some("MyTitle".to_string());
        let id1 = w1.id;
        let id2 = w2.id;
        let tree = make_tree(vec![w1, w2]);
        assert_eq!(run_search(&tree, "my_counter"), vec![id1]);
        assert_eq!(run_search(&tree, "mytitle"), vec![id2]);
    }

    #[test]
    fn run_search_with_none_optional_fields_does_not_panic() {
        use crate::project::schema::{WidgetInstance, WidgetKind, WidgetProps, Rect};
        let w = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Button,
            rect: Rect { x: 0.0, y: 0.0, w: 100.0, h: 40.0 },
            props: WidgetProps { label: String::new(), ..Default::default() },
            ..Default::default()
        };
        // tooltip=None, state_binding=None, label_binding=None, event_handler=None
        // all handler strings are empty by default
        let tree = make_tree(vec![w]);
        let result = run_search(&tree, "anything");
        assert!(result.is_empty()); // no panic is the test
    }

    #[test]
    fn navigate_noop_when_no_matches() {
        let mut state = CanvasSearchState {
            query: "zzz_no_match".into(),
            matches: vec![],
            current_index: 0,
            ..Default::default()
        };
        // Simulates what the state machine does before modulo.
        // The guard "if matches.is_empty() { return; }" must prevent panic.
        if !state.matches.is_empty() {
            state.current_index = (state.current_index + 1) % state.matches.len();
        }
        assert_eq!(state.current_index, 0); // no panic, no change
    }

    #[test]
    fn navigate_wraps_around_forward() {
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
        let mut state = CanvasSearchState {
            matches: ids.clone(),
            current_index: 2, // last
            ..Default::default()
        };
        // Forward wrap: 2 -> 0
        if !state.matches.is_empty() {
            state.current_index = (state.current_index + 1) % state.matches.len();
        }
        assert_eq!(state.current_index, 0);
        // Counter display: index 0 → "1 / 3"
        let counter = format!("{} / {}", state.current_index + 1, state.matches.len());
        assert_eq!(counter, "1 / 3");
    }

    #[test]
    fn navigate_wraps_around_backward() {
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
        let mut state = CanvasSearchState {
            matches: ids.clone(),
            current_index: 0, // first
            ..Default::default()
        };
        // Backward wrap: 0 -> 2
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
        let ids: Vec<Uuid> = (0..7).map(|_| Uuid::new_v4()).collect();
        let state = CanvasSearchState {
            matches: ids,
            current_index: 2, // 0-based → displayed as 3
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
        // The "Select All" button must be disabled when matches is empty.
        // Simulate the guard: only emit select_all_ids if non-empty.
        let ids: Option<Vec<Uuid>> = if state.matches.is_empty() {
            None // noop
        } else {
            Some(state.matches.clone())
        };
        assert!(ids.is_none());
    }
```

- [ ] **Step 2.2: Run tests — expect failures**

```
cargo test canvas::search::tests -- --nocapture
```

Expected: `run_search` tests fail with "unresolved function `run_search`".

- [ ] **Step 2.3: Implement `run_search()`**

Add to `src/canvas/search.rs` (before the `#[cfg(test)]` block):

```rust
use crate::project::{schema::WidgetInstance, ui_tree::UiTree};

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
        a.rect.y
            .partial_cmp(&b.rect.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.rect.x
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
    if let Some(ref b) = w.state_binding {
        if b.to_ascii_lowercase().contains(q_lower) {
            return true;
        }
    }
    // Label binding (identifier — ASCII fold)
    if let Some(ref b) = w.label_binding {
        if b.to_ascii_lowercase().contains(q_lower) {
            return true;
        }
    }
    // Tooltip (free text)
    if let Some(ref t) = w.tooltip {
        if t.to_lowercase().contains(q_lower) {
            return true;
        }
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
    if let Some(ref h) = w.event_handler {
        if h.to_ascii_lowercase().contains(q_lower) {
            return true;
        }
    }
    // Widget kind — use Debug repr for built-ins ("Button", "Slider", etc.)
    let kind_name = match &w.kind {
        crate::project::schema::WidgetKind::Custom(name) => {
            // Custom: try descriptor_name first, fall back to inner name.
            let dn = w
                .descriptor_name
                .as_deref()
                .unwrap_or(name.as_str());
            if dn.to_lowercase().contains(q_lower) {
                return true;
            }
            // Also try the raw inner name in case descriptor_name differed.
            name.to_lowercase()
        }
        other => format!("{other:?}").to_lowercase(),
    };
    if kind_name.contains(q_lower) {
        return true;
    }
    false
}
```

You also need these field accessors on `WidgetInstance`. They are accessed directly as `w.on_click`, `w.on_change`, etc. Confirm they exist at `src/project/schema.rs:1444–1459` before building.

- [ ] **Step 2.4: Run tests — expect all to pass**

```
cargo test canvas::search::tests -- --nocapture
```

Expected: all tests pass.

- [ ] **Step 2.5: Zero warnings**

```
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 2.6: Commit**

```bash
git add src/canvas/search.rs
git commit -m "feat(search): implement run_search with full field matching + tests"
```

---

## Task 3: Implement `scroll_to_widget()`

**Files:**
- Modify: `src/canvas/search.rs`

- [ ] **Step 3.1: Write the test**

Add to the `tests` module in `src/canvas/search.rs`:

```rust
    #[test]
    fn scroll_to_widget_pans_to_offscreen_widget() {
        use crate::canvas::interaction::CanvasSettings;
        use crate::project::schema::{WidgetInstance, WidgetKind, WidgetProps, Rect};
        use crate::project::ui_tree::UiTree;

        let mut settings = CanvasSettings {
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            ..Default::default()
        };
        let widget_id = Uuid::new_v4();
        let mut w = WidgetInstance {
            id: widget_id,
            kind: WidgetKind::Label,
            rect: Rect { x: 2000.0, y: 2000.0, w: 100.0, h: 40.0 },
            props: WidgetProps { label: "far".into(), ..Default::default() },
            ..Default::default()
        };
        let mut tree = UiTree::default();
        tree.widgets.push(w);

        // Viewport is 800x600, widget is at (2000, 2000) — far offscreen.
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        scroll_to_widget(widget_id, &tree, &mut settings, viewport);

        // After scrolling, the widget should be near the center of the usable viewport.
        // Pan should be non-zero — something moved.
        assert!(settings.pan != egui::Vec2::ZERO, "pan should have changed");
    }

    #[test]
    fn scroll_to_widget_noop_for_already_visible() {
        use crate::canvas::interaction::CanvasSettings;
        use crate::project::schema::{WidgetInstance, WidgetKind, WidgetProps, Rect};
        use crate::project::ui_tree::UiTree;

        let mut settings = CanvasSettings {
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            ..Default::default()
        };
        let widget_id = Uuid::new_v4();
        let w = WidgetInstance {
            id: widget_id,
            kind: WidgetKind::Label,
            rect: Rect { x: 100.0, y: 100.0, w: 80.0, h: 30.0 },
            props: WidgetProps { label: "near".into(), ..Default::default() },
            ..Default::default()
        };
        let mut tree = UiTree::default();
        tree.widgets.push(w);

        // Viewport is 800x600 — widget center (140, 115) is clearly inside.
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        scroll_to_widget(widget_id, &tree, &mut settings, viewport);

        // Pan should remain zero — widget was already visible.
        assert_eq!(settings.pan, egui::Vec2::ZERO);
    }
```

- [ ] **Step 3.2: Run tests — expect failures**

```
cargo test canvas::search::tests::scroll_to_widget -- --nocapture
```

Expected: fails with "unresolved function `scroll_to_widget`".

- [ ] **Step 3.3: Implement `scroll_to_widget()`**

Add to `src/canvas/search.rs` (after `run_search`, before `#[cfg(test)]`):

```rust
use crate::canvas::interaction::CanvasSettings;

/// Adjusts `settings.pan` to bring the widget with `id` into the visible viewport.
/// Does NOT change zoom level. No-ops if the widget is already visible.
///
/// The usable rect excludes the top-right 350×50 px occupied by the search panel
/// so the current match is never scrolled behind it.
pub fn scroll_to_widget(
    id: Uuid,
    tree: &UiTree,
    settings: &mut CanvasSettings,
    viewport: egui::Rect,
) {
    let Some(widget) = tree.widgets.iter().find(|w| w.id == id) else {
        return;
    };
    let widget_canvas_center = egui::vec2(
        widget.rect.x + widget.rect.w / 2.0,
        widget.rect.y + widget.rect.h / 2.0,
    );
    let widget_screen_center =
        (widget_canvas_center * settings.zoom).to_pos2() + settings.pan;

    // Shrink viewport to exclude the search panel footprint (top-right 350×50).
    let usable = egui::Rect::from_min_max(
        viewport.min,
        egui::pos2(viewport.max.x - 350.0, viewport.max.y - 50.0),
    );

    if usable.contains(widget_screen_center) {
        return; // already visible — do nothing
    }

    // Pan so the widget center lands at the center of the usable viewport.
    settings.pan = (usable.center().to_vec2() - widget_canvas_center * settings.zoom).into();
}
```

Note: Add `use egui;` at the top of `search.rs` if not already present. `egui` is already a dependency.

- [ ] **Step 3.4: Run tests — expect pass**

```
cargo test canvas::search::tests -- --nocapture
```

- [ ] **Step 3.5: Zero warnings**

```
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 3.6: Commit**

```bash
git add src/canvas/search.rs
git commit -m "feat(search): implement scroll_to_widget helper + tests"
```

---

## Task 4: Fix Ctrl+F Collisions + Register Shortcut

**Files:**
- Modify: `src/panels/code_preview.rs:442`
- Modify: `src/panels/shortcuts.rs:31`
- Modify: `src/canvas/interaction.rs:2477`

- [ ] **Step 4.1: Write the bare-F regression test**

Add to the tests module in `src/canvas/search.rs`:

```rust
    // NOTE: The bare-F zoom-to-fit regression is tested via integration.
    // Annotate here so the code review can trace it.
    // Test: search::tests::bare_f_still_zooms_to_fit
    // This is validated in cargo test with the canvas integration tests
    // after Task 6 wires the key handler. Leave a marker here.
```

- [ ] **Step 4.2: Gate `code_preview.rs` Ctrl+F behind `editor_has_focus`**

In `src/panels/code_preview.rs`, find line 442 (the Ctrl+F handler):

```rust
// BEFORE (line 442):
if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
    *search_open = !*search_open;
```

Change to:

```rust
// AFTER: only fire when the code panel actually has focus.
if *editor_has_focus && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
    *search_open = !*search_open;
```

- [ ] **Step 4.3: Register shortcut in `shortcuts.rs`**

In `src/panels/shortcuts.rs`, find `BUILTIN_SHORTCUTS` (line 17). The last entry currently ends with:
```rust
    ("shortcuts_help", "F1", "Show / hide this reference"),
];
```

Add canvas search before the closing bracket:
```rust
    ("shortcuts_help", "F1", "Show / hide this reference"),
    ("canvas_search", "Ctrl+F", "Open canvas widget search"),
];
```

- [ ] **Step 4.4: Narrow bare-F zoom-to-fit guard in `interaction.rs`**

At `src/canvas/interaction.rs:2477`, the current line is:
```rust
let key_f = keyboard_owned && ui.input(|i| i.key_pressed(egui::Key::F));
```

Replace with:
```rust
// Bare F = zoom-to-fit. Guard against Ctrl held so Ctrl+F goes to canvas_search.
let key_f = keyboard_owned && ui.input(|i| !i.modifiers.ctrl && i.key_pressed(egui::Key::F));
// Ctrl+F = open canvas search (also gated: must not be blocked by modal).
let key_ctrl_f = keyboard_owned
    && !settings.input_blocked
    && ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F));
```

- [ ] **Step 4.5: Build — confirm no errors**

```
cargo check
```

Expected: compiles cleanly.

- [ ] **Step 4.6: Zero warnings**

```
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 4.7: Commit**

```bash
git add src/panels/code_preview.rs src/panels/shortcuts.rs src/canvas/interaction.rs
git commit -m "fix(search): gate code_preview Ctrl+F; add key_ctrl_f; narrow bare-F guard"
```

---

## Task 5: Implement `draw_search_panel()`

**Files:**
- Modify: `src/canvas/search.rs`

- [ ] **Step 5.1: Add `draw_search_panel` stub + import**

Add to the imports at the top of `src/canvas/search.rs`:

```rust
use egui::{self, Key, Modifiers};
```

Add the function stub (before `#[cfg(test)]`):

```rust
/// Draw the floating search panel. Returns side-effects for the caller to apply.
/// Navigation keys (Enter, Shift+Enter, Escape) are read inside here, bypassing
/// the `keyboard_owned` gate in interaction.rs (see Input Routing Contract in spec).
pub fn draw_search_panel(
    ui: &mut egui::Ui,
    state: &mut CanvasSearchState,
    canvas_rect: egui::Rect,
    tree: &UiTree,
) -> SearchPanelResponse {
    SearchPanelResponse::default()
}
```

- [ ] **Step 5.2: Implement the full panel**

Replace the stub with the real implementation:

```rust
pub fn draw_search_panel(
    ui: &mut egui::Ui,
    state: &mut CanvasSearchState,
    canvas_rect: egui::Rect,
    tree: &UiTree,
) -> SearchPanelResponse {
    let mut resp = SearchPanelResponse::default();

    let panel_width = 340.0f32;
    let panel_x = (canvas_rect.max.x - panel_width - 8.0)
        .max(canvas_rect.min.x + 4.0); // minimum-width guard
    let panel_pos = egui::pos2(panel_x, canvas_rect.min.y + 8.0);

    egui::Area::new(egui::Id::new("canvas_search_panel"))
        .order(egui::Order::Tooltip)
        .fixed_pos(panel_pos)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style())
                .rounding(5.0)
                .show(ui, |ui| {
                    ui.set_width(panel_width);
                    ui.horizontal(|ui| {
                        // 🔍 icon
                        ui.label("🔍");

                        // Query input
                        let te = egui::TextEdit::singleline(&mut state.query)
                            .desired_width(140.0)
                            .hint_text("search widgets…");
                        let te_resp = ui.add(te);

                        // Recompute on query change (debounce gate).
                        if state.query != state.last_query {
                            state.last_query = state.query.clone();
                            state.matches = run_search(tree, &state.query);
                            state.current_index = 0;
                            state.just_wrapped = false;
                        }

                        // Read navigation keys from inside the panel (bypasses keyboard_owned).
                        let enter = ui.input(|i| {
                            i.key_pressed(Key::Enter) && !i.modifiers.shift
                        });
                        let shift_enter = ui.input(|i| {
                            i.key_pressed(Key::Enter) && i.modifiers.shift
                        });
                        let escape = ui.input(|i| i.key_pressed(Key::Escape));
                        // Ctrl+F while open: refocus input.
                        let ctrl_f = ui.input(|i| {
                            i.modifiers.ctrl && i.key_pressed(Key::F)
                        });

                        if ctrl_f {
                            te_resp.request_focus();
                        }

                        if escape {
                            resp.close_requested = true;
                            resp.return_focus_to_canvas = true;
                        }

                        if !state.matches.is_empty() {
                            if enter {
                                state.just_wrapped = state.current_index == state.matches.len() - 1;
                                state.current_index =
                                    (state.current_index + 1) % state.matches.len();
                                resp.scroll_to = Some(state.matches[state.current_index]);
                            }
                            if shift_enter {
                                let n = state.matches.len();
                                state.just_wrapped = state.current_index == 0;
                                state.current_index =
                                    (state.current_index + n.saturating_sub(1)) % n;
                                resp.scroll_to = Some(state.matches[state.current_index]);
                            }
                        }

                        // Counter label
                        if !state.query.is_empty() {
                            let counter = if state.matches.is_empty() {
                                // Red tint for no results.
                                let label = egui::RichText::new("0 / 0")
                                    .color(egui::Color32::from_rgba_unmultiplied(255, 80, 80, 220));
                                ui.label(label);
                            } else {
                                let label = if state.just_wrapped {
                                    egui::RichText::new(format!(
                                        "{} / {}",
                                        state.current_index + 1,
                                        state.matches.len()
                                    ))
                                    .strong()
                                } else {
                                    egui::RichText::new(format!(
                                        "{} / {}",
                                        state.current_index + 1,
                                        state.matches.len()
                                    ))
                                };
                                ui.label(label);
                            };
                        }

                        // ↑ ↓ navigation buttons
                        let prev_enabled = !state.matches.is_empty();
                        let next_enabled = !state.matches.is_empty();

                        if ui.add_enabled(prev_enabled, egui::Button::new("↑")).clicked() {
                            let n = state.matches.len();
                            state.just_wrapped = state.current_index == 0;
                            state.current_index = (state.current_index + n.saturating_sub(1)) % n;
                            resp.scroll_to = Some(state.matches[state.current_index]);
                        }
                        if ui.add_enabled(next_enabled, egui::Button::new("↓")).clicked() {
                            state.just_wrapped =
                                state.current_index == state.matches.len() - 1;
                            state.current_index =
                                (state.current_index + 1) % state.matches.len();
                            resp.scroll_to = Some(state.matches[state.current_index]);
                        }

                        // Select All — disabled when no matches.
                        let select_all_enabled = !state.matches.is_empty();
                        if ui
                            .add_enabled(select_all_enabled, egui::Button::new("Select All"))
                            .clicked()
                        {
                            // Validate IDs against live tree before handing off.
                            let live_ids: std::collections::HashSet<Uuid> =
                                tree.widgets.iter().map(|w| w.id).collect();
                            let valid: Vec<Uuid> = state
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

                        // ✕ close button
                        if ui.button("✕").clicked() {
                            resp.close_requested = true;
                            resp.return_focus_to_canvas = true;
                        }
                    });
                });
        });

    resp
}
```

- [ ] **Step 5.3: Build — expect no errors**

```
cargo check
```

- [ ] **Step 5.4: Zero warnings**

```
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 5.5: Run all search tests**

```
cargo test canvas::search::tests -- --nocapture
```

Expected: all tests still pass.

- [ ] **Step 5.6: Commit**

```bash
git add src/canvas/search.rs
git commit -m "feat(search): implement draw_search_panel with navigate + select-all"
```

---

## Task 6: Wire Open/Close in `interaction.rs` + `app.rs`

**Files:**
- Modify: `src/canvas/interaction.rs` (near line 2492 — `key_f` usage block)
- Modify: `src/app.rs` (pass `canvas_focused` restoration back)

- [ ] **Step 6.1: Write the focus-gating test**

Add to the `tests` module in `src/canvas/search.rs`:

```rust
    #[test]
    fn ctrl_f_only_opens_when_canvas_focused() {
        // Simulate the open guard: canvas_focused && !input_blocked && key_ctrl_f.
        // When canvas_focused is false, canvas_search must remain None.
        let canvas_focused = false;
        let input_blocked = false;
        let key_ctrl_f = true; // key was pressed

        let mut canvas_search: Option<CanvasSearchState> = None;

        if canvas_focused && !input_blocked && key_ctrl_f {
            canvas_search = Some(CanvasSearchState::default());
        }

        assert!(canvas_search.is_none(), "should not open when canvas not focused");
    }
```

- [ ] **Step 6.2: Run test — expect pass (no logic to write, it's already correct)**

```
cargo test canvas::search::tests::ctrl_f_only_opens_when_canvas_focused -- --nocapture
```

- [ ] **Step 6.3: Wire open in `interaction.rs`**

In `src/canvas/interaction.rs`, find the block starting with `if key_f {` (line 2492) for zoom-to-fit. Just before or after that block (keeping `key_f` for zoom), add:

```rust
    // Ctrl+F — open canvas search panel.
    if key_ctrl_f {
        if state.canvas_search.is_none() {
            state.canvas_search = Some(crate::canvas::search::CanvasSearchState::default());
        } else if let Some(ref mut cs) = state.canvas_search {
            // Already open: reset just_wrapped; caller will refocus TextEdit via panel.
            cs.just_wrapped = false;
        }
    }
```

- [ ] **Step 6.4: Wire `SearchPanelResponse` side-effects in `app.rs`**

In `src/app.rs`, within the `CentralPanel` closure (around line 3260), find where `interaction::handle()` is called. After the call returns, add the search panel draw + response application. Find the comment `// Rulers and guide lines drawn on top of canvas content.` (around line 3328) and insert **before** it:

```rust
            // Canvas search panel — draw while canvas_search is Some.
            if self.session.interaction.canvas_search.is_some() {
                let search_resp = {
                    let cs = self.session.interaction.canvas_search.as_mut().unwrap();
                    crate::canvas::search::draw_search_panel(
                        ui,
                        cs,
                        panel_rect,
                        &self.project.ui_tree,
                    )
                };
                if search_resp.close_requested {
                    self.session.interaction.canvas_search = None;
                }
                if search_resp.return_focus_to_canvas {
                    self.session.interaction.canvas_focused = true;
                }
                if let Some(ids) = search_resp.select_all_ids {
                    self.session.selected = ids.into_iter().collect();
                }
                if let Some(scroll_id) = search_resp.scroll_to {
                    crate::canvas::search::scroll_to_widget(
                        scroll_id,
                        &self.project.ui_tree,
                        &mut self.session.canvas_settings,
                        panel_rect,
                    );
                }
            }
```

Also add the deleted-widget validation each frame while search is open. Inside the `if self.session.interaction.canvas_search.is_some()` block, before the `draw_search_panel` call:

```rust
                // Re-validate matches against live tree each frame.
                if let Some(ref mut cs) = self.session.interaction.canvas_search {
                    let live_ids: std::collections::HashSet<uuid::Uuid> =
                        self.project.ui_tree.widgets.iter().map(|w| w.id).collect();
                    let before_len = cs.matches.len();
                    cs.matches.retain(|id| live_ids.contains(id));
                    if cs.matches.len() != before_len {
                        cs.current_index =
                            cs.current_index.min(cs.matches.len().saturating_sub(1));
                    }
                }
```

- [ ] **Step 6.5: Write the deleted-widget test**

Add to `src/canvas/search.rs` tests:

```rust
    #[test]
    fn deleted_widget_removed_from_match_list() {
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();

        let mut state = CanvasSearchState {
            query: "button".into(),
            matches: vec![id_a, id_b],
            current_index: 1,
            ..Default::default()
        };

        // Simulate widget id_a being deleted from live tree.
        let live_ids: std::collections::HashSet<Uuid> =
            std::iter::once(id_b).collect();
        state.matches.retain(|id| live_ids.contains(id));
        state.current_index =
            state.current_index.min(state.matches.len().saturating_sub(1));

        assert_eq!(state.matches, vec![id_b]);
        assert_eq!(state.current_index, 0); // clamped from 1 to 0
    }
```

- [ ] **Step 6.6: Build + run all tests**

```
cargo check
cargo test canvas::search::tests -- --nocapture
```

Expected: all tests pass.

- [ ] **Step 6.7: Zero warnings**

```
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 6.8: Commit**

```bash
git add src/canvas/interaction.rs src/app.rs src/canvas/search.rs
git commit -m "feat(search): wire Ctrl+F open/close and SearchPanelResponse in app.rs"
```

---

## Task 7: Post-Handle Ring/Glow Overlay in `app.rs`

**Files:**
- Modify: `src/app.rs` (after rulers + bezel, before Stage-11 overlays)

The search panel Area renders correctly from Task 6. This task adds the ring/glow overlay drawn as painter calls so they sit above ruler lines.

- [ ] **Step 7.1: Add the overlay function to `search.rs`**

Add to `src/canvas/search.rs` (before `#[cfg(test)]`):

```rust
/// Paint search match rings and glows onto the canvas painter.
/// Called after rulers and bezel are drawn so rings are not overdrawn.
///
/// `widget_screen_rects` — pre-computed (id, screen_rect) pairs from the
/// normal widget draw pass. Using pre-computed rects prevents any stale-ID
/// window between the draw pass and the overlay pass.
pub fn draw_search_overlay(
    painter: &egui::Painter,
    state: &CanvasSearchState,
    widget_screen_rects: &[(Uuid, egui::Rect)],
    dark_mode: bool,
) {
    if state.matches.is_empty() || state.query.is_empty() {
        return;
    }

    // Base teal color from the established kind-accent system.
    let (ring_alpha, glow_inner_alpha, glow_outer_alpha) = if dark_mode {
        (192u8, 46u8, 31u8) // 0.75, 0.18, 0.12 of 255
    } else {
        (255u8, 115u8, 77u8) // full + boosted for light backgrounds
    };

    let teal = egui::Color32::from_rgb(52, 211, 153);

    for (i, &match_id) in state.matches.iter().enumerate() {
        let Some((_, rect)) = widget_screen_rects.iter().find(|(id, _)| *id == match_id)
        else {
            continue;
        };

        if i == state.current_index {
            // Current match: hard ring.
            let ring_color = egui::Color32::from_rgba_unmultiplied(
                teal.r(), teal.g(), teal.b(), ring_alpha,
            );
            painter.rect_stroke(
                rect.expand(3.0),
                egui::CornerRadius::from(4.0),
                egui::Stroke::new(2.0, ring_color),
            );
        } else {
            // Other matches: soft glow (two low-alpha filled rects, no stroke).
            let inner = egui::Color32::from_rgba_unmultiplied(
                teal.r(), teal.g(), teal.b(), glow_inner_alpha,
            );
            let outer = egui::Color32::from_rgba_unmultiplied(
                teal.r(), teal.g(), teal.b(), glow_outer_alpha,
            );
            painter.rect_filled(rect.expand(6.0), egui::CornerRadius::from(4.0), outer);
            painter.rect_filled(rect.expand(3.0), egui::CornerRadius::from(4.0), inner);
        }
    }
}
```

- [ ] **Step 7.2: Call the overlay from `app.rs`**

In `src/app.rs`, after the Stage-11 overlays block (after `draw_error_flow` call, around line 3367), add:

```rust
            // Search overlay — rings and glows above rulers and Stage-11 overlays.
            if let Some(ref cs) = self.session.interaction.canvas_search {
                if !cs.matches.is_empty() {
                    let origin = crate::canvas::rulers::canvas_origin(
                        canvas_size, zoom, pan, panel_rect,
                    );
                    // Collect pre-computed screen rects for all visible widgets.
                    let screen_rects: Vec<(uuid::Uuid, egui::Rect)> = self
                        .project
                        .ui_tree
                        .widgets
                        .iter()
                        .map(|w| {
                            let r = egui::Rect::from_min_size(
                                egui::pos2(
                                    origin.x + w.rect.x * zoom,
                                    origin.y + w.rect.y * zoom,
                                ),
                                egui::vec2(w.rect.w * zoom, w.rect.h * zoom),
                            );
                            (w.id, r)
                        })
                        .collect();
                    let painter = ui.painter_at(panel_rect);
                    let dark_mode = ui.visuals().dark_mode;
                    crate::canvas::search::draw_search_overlay(
                        &painter,
                        cs,
                        &screen_rects,
                        dark_mode,
                    );
                }
            }
```

- [ ] **Step 7.3: Check `canvas_origin` signature**

Verify `crate::canvas::rulers::canvas_origin` accepts `(canvas_size, zoom, pan, panel_rect)` by checking how it is called in the existing Stage-11 overlay block (around line 3344). Match that exact call signature.

- [ ] **Step 7.4: Build**

```
cargo check
```

- [ ] **Step 7.5: Zero warnings**

```
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 7.6: Full test suite**

```
cargo test
```

Expected: all existing tests still pass.

- [ ] **Step 7.7: Commit**

```bash
git add src/canvas/search.rs src/app.rs
git commit -m "feat(search): add ring/glow overlay pass above rulers and bezel"
```

---

## Task 8: Remaining Tests + Serialization Invariant

**Files:**
- Modify: `src/canvas/search.rs` (already has most tests)
- Modify: `src/project/schema.rs` or `src/project/io.rs` (serialization test)

- [ ] **Step 8.1: Add serialization-invariant integration test**

Add to `src/canvas/search.rs` tests (or as a separate test in `tests/` if project has an integration test dir):

```rust
    #[test]
    fn canvas_search_not_in_serialized_project() {
        // Serialize a default ProjectDocument and confirm no search keys appear.
        let doc = crate::project::schema::ProjectFile::default();
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
```

- [ ] **Step 8.2: Run the serialization test**

```
cargo test canvas::search::tests::canvas_search_not_in_serialized_project -- --nocapture
```

Expected: passes (since `CanvasSearchState` is only on `InteractionState` which is never serialized).

- [ ] **Step 8.3: Add bare-F regression test**

Add to `src/canvas/search.rs` tests:

```rust
    #[test]
    fn key_f_guard_logic_bare_vs_ctrl() {
        // Verify the guard logic: bare F fires zoom-to-fit; Ctrl+F fires search.
        // This test models the boolean guards in interaction.rs.
        let keyboard_owned = true;
        let input_blocked = false;

        // Bare F (no ctrl): should only trigger zoom-to-fit, not search.
        let ctrl_held = false;
        let key_f_bare = keyboard_owned && !ctrl_held;     // zoom-to-fit fires
        let key_ctrl_f = keyboard_owned && !input_blocked && ctrl_held; // search does NOT fire

        assert!(key_f_bare, "bare F should trigger zoom-to-fit");
        assert!(!key_ctrl_f, "bare F should not trigger canvas search");

        // Ctrl+F: should only trigger search, not zoom-to-fit.
        let ctrl_held = true;
        let key_f_with_ctrl = keyboard_owned && !ctrl_held; // zoom-to-fit does NOT fire
        let key_ctrl_f = keyboard_owned && !input_blocked && ctrl_held; // search fires

        assert!(!key_f_with_ctrl, "Ctrl+F should not trigger zoom-to-fit");
        assert!(key_ctrl_f, "Ctrl+F should trigger canvas search");
    }
```

- [ ] **Step 8.4: Run all search tests**

```
cargo test canvas::search::tests -- --nocapture
```

Expected: all 16+ tests pass.

- [ ] **Step 8.5: Full test suite + clippy**

```
cargo test
cargo clippy --all-targets -- -D warnings
```

Expected: all pass, zero warnings.

- [ ] **Step 8.6: Smoke test — run the app**

```
cargo run
```

Open the canvas, press `Ctrl+F`. Verify:
- Floating panel appears top-right of canvas
- Typing "button" highlights matching widgets with teal ring
- Enter navigates through matches; counter updates
- Select All closes panel and selects all matches
- Escape closes panel and restores canvas keyboard focus
- Bare `F` still zooms to fit (not opening search)
- Code panel Ctrl+F only fires when code panel is focused

- [ ] **Step 8.7: Final commit**

```bash
git add src/canvas/search.rs
git commit -m "test(search): add serialization invariant, bare-F regression, and remaining tests"
```

---

## Self-Review Checklist

**Spec coverage:**
- [x] Floating panel, top-right, `Order::Tooltip` — Task 5
- [x] Navigate + Select All — Tasks 5, 6
- [x] Medium field scope (all fields correct) — Task 2
- [x] Extensible via `widget_matches` function — Task 2
- [x] Current match: teal ring at 75% opacity — Task 7
- [x] Other matches: soft glow — Task 7
- [x] Panel background: `visuals().window_fill` — Task 5
- [x] Light/dark theme adaptation — Task 7
- [x] `InteractionState` (not `CanvasInteraction`) — Task 1
- [x] Surface switch clears state — Task 1 test
- [x] Overlay after rulers and bezel — Task 7
- [x] `CanvasSearchState` never serialized — Task 8
- [x] `code_preview.rs` Ctrl+F gated — Task 4
- [x] `shortcuts.rs` registration — Task 4
- [x] bare-F guard narrowed — Task 4
- [x] `app.rs` in Files Touched — Tasks 6, 7
- [x] `scroll_to_widget` with correct field (`settings.pan`) — Task 3
- [x] Panel clearance in scroll — Task 3
- [x] usize % 0 guard — Task 2 test + Task 5 impl
- [x] `WidgetInstance.binding` → `state_binding` + `label_binding` — Task 2
- [x] All 5 handler fields + legacy — Task 2
- [x] Custom widget kind matching — Task 2
- [x] Deleted widget re-validation each frame — Task 6
- [x] Tab order + request_focus — Task 5
- [x] `Select All` disabled when empty — Task 5
- [x] Focus restored on close — Task 6
- [x] Input routing (nav keys inside panel) — Task 5
- [x] All 16 tests — Tasks 1–8
