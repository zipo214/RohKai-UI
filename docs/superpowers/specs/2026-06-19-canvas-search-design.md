# Canvas Search (S2 Item 1) — Design Spec

Date: 2026-06-19 (adversarial review applied 2026-06-20)
Stage: S2 — Canvas UX Depth
Trigger: Ctrl+F on canvas (distinct from code-panel Ctrl+F)

---

## Summary

A floating search panel anchored to the top-right of the canvas that lets users find widgets by name, kind, or property value. Supports navigate-through (↑↓/Enter) and select-all. Zero new color vocabulary — uses established kind-accent colors at reduced opacity. Six compile-blocking defects identified in adversarial review are corrected in this revision.

---

## Architecture

**New file:** `src/canvas/search.rs`

Responsible for:
- `CanvasSearchState` struct — query, last_query (debounce gate), match list, current index, open flag
- `SearchFieldRegistry` — extension point for searchable fields per widget kind (see Extensibility Notes for HashMap key constraint)
- `run_search(tree: &UiTree, registry: &SearchFieldRegistry, query: &str) -> Vec<WidgetId>` — pure function, no side effects; results sorted by canvas-space rect top-to-bottom then left-to-right (rect.y then rect.x)
- `draw_search_panel(ui: &mut egui::Ui, state: &mut CanvasSearchState, canvas_rect: egui::Rect) -> SearchPanelResponse` — immediate-mode floating panel; requires `canvas_rect` to position the `egui::Area` at top-right offset
- `scroll_to_widget(id: Uuid, tree: &UiTree, settings: &mut CanvasSettings, viewport: egui::Rect)` — new helper that adjusts `settings.pan` (not a nonexistent `canvas_offset`) to bring the widget into view without changing zoom

**`SearchPanelResponse`** carries all side effects; no hidden mutations inside the panel function:
```rust
pub struct SearchPanelResponse {
    pub close_requested: bool,
    pub select_all_ids: Option<Vec<WidgetId>>,
    pub return_focus_to_canvas: bool,
    pub scroll_to: Option<WidgetId>,
}
```

**`InteractionState`** (src/canvas/interaction.rs line 394, `#[derive(Default)]`) gains one field:
```rust
pub canvas_search: Option<CanvasSearchState>,
```

> **Note:** The spec previously named this struct `CanvasInteraction` — that struct does not exist. The real type is `InteractionState`.

**Design decision — surface-switch reset:** `InteractionState` is reset to `InteractionState::default()` on every surface switch (app.rs lines 451, 772, 860). `canvas_search` is therefore cleared to `None` on surface switch. This is intentional — search is transient session state, not a persistent per-surface document property.

**Overlay render order:** The search ring/glow overlay must be emitted in a **post-handle pass in app.rs**, after `rulers::draw()` (line 3328) and `draw_bezel()` (line 3336), following the same pattern as Stage-11 overlays at lines 3344–3367. Painter commands emitted inside `interaction::handle()` are drawn before rulers and the bezel strip and are overdrawn by those passes.

**Serialization invariant:** `CanvasSearchState` and `SearchFieldRegistry` must not derive or implement `Serialize`/`Deserialize`. They must never appear in any struct that participates in `ProjectDocument` serialization. Add a regression test verifying no `canvas_search` or `search_registry` key appears in serialized JSON.

---

## Search Scope (medium, extensible)

| Field | Source | Rust Type | None/empty handling |
|---|---|---|---|
| Label text | `WidgetInstance.label` | `String` | skip if empty |
| State binding | `WidgetInstance.state_binding` | `Option<String>` | skip if `None` |
| Label binding | `WidgetInstance.label_binding` | `Option<String>` | skip if `None` |
| Tooltip | `WidgetInstance.tooltip` | `Option<String>` | skip if `None` |
| on_click | `WidgetInstance.on_click` | `String` | skip if empty |
| on_change | `WidgetInstance.on_change` | `String` | skip if empty |
| on_double_click | `WidgetInstance.on_double_click` | `String` | skip if empty |
| on_lost_focus | `WidgetInstance.on_lost_focus` | `String` | skip if empty |
| on_drag_stopped | `WidgetInstance.on_drag_stopped` | `String` | skip if empty |
| event_handler (legacy) | `WidgetInstance.event_handler` | `Option<String>` | skip if `None` |
| Widget kind | `WidgetKind::display_name()` for built-ins; `WidgetKind::Custom(name)` inner string AND `WidgetInstance.descriptor_name` (if `Some`) for custom kinds | `String` / `Option<String>` | for custom: match both strings, skip `descriptor_name` if `None` |

Adding a new searchable field = one entry in `SearchFieldRegistry`. No other changes required.

**Matching strategy:**
- Free-text fields (label, tooltip): Unicode full case folding via `str::to_lowercase()` on both query and field value.
- Identifier fields (bindings, handler names): ASCII-only case folding via `to_ascii_lowercase()`, consistent with identifier validation in `codegen/rust.rs`.
- Kind search for `Custom` widgets: matches against both the `WidgetKind::Custom(name)` inner string and `WidgetInstance::descriptor_name.as_deref().unwrap_or_default()`. Descriptor name tried first.

---

## UX Behaviour

**Open:** `Ctrl+F` while canvas has focus AND `!modal_blocked` (`!settings.input_blocked`) opens panel, focuses input, does not steal canvas selection. Full condition: `ctrl_held && !modal_blocked && canvas_focused && key_F_pressed`.

**Ctrl+F while panel already open:** Focus the query input and select-all the existing query text (standard browser behavior). This transition fires from inside `draw_search_panel` via `ui.input()`, not from the `keyboard_owned`-gated block in `interaction.rs`.

**Close:** Escape or ✕ → clears `canvas_search` to `None`, removes all rings/glows, **sets `canvas_focused = true`** so the user can immediately use canvas keyboard shortcuts without a mouse click. `SearchPanelResponse::return_focus_to_canvas` must be `true` on these paths.

**Select All close path:** applies match IDs as active multi-selection, closes panel, **sets `canvas_focused = true`**.

**Empty query:** no matches, no rings, counter hidden. Select All disabled.

**No results:** counter shows `0 / 0`, input tinted red (`egui::Color32::from_rgba_unmultiplied(255, 80, 80, 180)` — the established smart-guide red). Counter always uses `N / M` format.

**Navigation:**
- Enter / ↓ button → next match, scroll canvas to bring it into view
- Shift+Enter / ↑ button → previous match
- Counter label: `"(current_index + 1) / matches.len()"` formatted as e.g. `"3 / 7"`
- Wrap-around: set `just_wrapped = true` for one frame to produce a visual counter flash

**Select All:**
- Applies all valid (live-tree) match IDs as the active multi-selection and closes the search panel
- If `matches.is_empty()`: no-op, button is disabled (greyed out, not in Tab order)
- Feeds directly into existing multi-select tools (alignment, property edit, delete)

**Escape key priority:** inline label edit cancellation (when `state.inline_edit` is `Some`) takes precedence over search panel close. Cancel the label edit first; leave search panel open.

**Navigation key routing:** `Enter`, `Shift+Enter`, and `Escape` must be read from `ui.input()` inside `draw_search_panel`, NOT from the `keyboard_owned`-gated block in `interaction.rs`. See Input Routing Contract.

---

## Visual Treatment

Rendered as a post-handle overlay pass in app.rs (after rulers and bezel). The overlay receives a `&[(WidgetId, egui::Rect)]` slice of pre-computed screen rects collected during the normal widget draw pass — it does not re-fetch widget data from the tree.

**Panel background:** `ui.visuals().window_fill` — use the egui style token so the panel matches the active theme. Do not hardcode `rgba(30,30,46,230)`.

**Current match (dark canvas):**
- `painter.rect_stroke(rect.expand(3.0), 4.0, Stroke::new(2.0, rgba(52,211,153,192)))`
- No fill change, no text color change, no interior modification

**Other matches (dark canvas):**
- Inner glow ring: `rgba(52, 211, 153, 46)` at 3px expand
- Outer haze: `rgba(52, 211, 153, 31)` at 6px expand
- Two `painter.rect` calls with zero stroke (egui has no native box-shadow)

**Light theme adaptation:** On `ui.visuals().dark_mode == false`, increase current-match ring to full alpha and add a 1px dark outline to maintain ≥ 3:1 contrast ratio (WCAG AA for non-text UI graphics). Glow rings use `alpha 115` and `alpha 77` respectively.

**Non-matches:** completely unchanged.

---

## Floating Panel Layout

Top-right of the canvas `Rect`, fixed offset 8px from edge. `egui::Area` with `Order::Tooltip` (one layer above `Foreground`) to guarantee the panel renders above the context menu Area which uses `Order::Foreground`.

Assign a stable Id: `egui::Id::new("canvas_search_panel")`.

**Positioning formula:**
```rust
let panel_pos = egui::pos2(
    canvas_rect.max.x - 340.0 - 8.0,
    canvas_rect.min.y + 8.0,
);
```

**Minimum canvas width guard:** If `canvas_rect.width() < 360.0`, cap the panel's left edge to `canvas_rect.min.x + 4.0`.

```
┌──────────────────────────────────────────┐
│ 🔍 [___query input___]  3 / 7  ↑ ↓  [Select All]  ✕ │
└──────────────────────────────────────────┘
```

- Width: ~340px fixed
- Single row
- Rounded corners: 5px

**Tab order:** `input → ↑ button → ↓ button → Select All → ✕` (looping). Call `TextEdit::request_focus()` at panel open time.

**↑↓ navigation controls:** must be `egui::Button` widgets (not painted glyphs) so they are keyboard-focusable. Bare arrow keys are NOT intercepted by the search panel — arrow keys in canvas context nudge selected widgets as normal.

---

## State Machine

```
Closed
  → Ctrl+F (canvas_focused && !modal_blocked)
      → Open(query="", matches=[], index=0)
         build search results from active tree; request_focus on TextEdit

Open
  → Ctrl+F
      → request_focus on TextEdit; select-all query text (no close, no reset)
  → type query
      → if query != last_query: recompute matches (run_search), reset index to 0
         update last_query = query
  → Enter / ↓
      → if matches.is_empty() { return; }   ← REQUIRED: prevents usize % 0 panic
         index = (index + 1) % matches.len()
         scroll to current match (scroll_to_widget)
  → Shift+Enter / ↑
      → if matches.is_empty() { return; }
         index = (index + matches.len().saturating_sub(1)) % matches.len()
         scroll to current match (scroll_to_widget)
         [set just_wrapped=true for one-frame counter highlight if index wrapped to end]
  → Select All
      → if matches.is_empty() { noop — button is disabled }
         else: validate match IDs against live tree, set canvas selection = valid_ids
               → Closed; return_focus_to_canvas = true
  → Escape / ✕
      → [if inline_edit is Some: cancel inline_edit first; leave panel open]
         → Closed; return_focus_to_canvas = true
  → canvas drag/click
      → does NOT close search
  → surface switch
      → canvas_search reset to None (via InteractionState::default() at app.rs line 451)
  → widget deleted while open
      → re-validate matches against live tree on next frame; clamp index to matches.len().saturating_sub(1)
```

**Navigation key handling:** `Enter`, `Shift+Enter`, `Escape` are read from `ui.input()` inside `draw_search_panel`, NOT from the `keyboard_owned`-gated block. See Input Routing Contract.

**Counter display:** always `"(current_index + 1) / matches.len()"` as `"N / M"`. Zero-results: `"0 / 0"`. Empty-query: counter hidden.

---

## Input Routing Contract

`canvas_owns_keyboard()` (interaction.rs line 2289–2294) returns `false` when `ui.ctx().egui_wants_keyboard_input()` is `true`. Once the search panel's `TextEdit` has focus, this condition is permanently true for the panel's lifetime.

**Keys that MUST be read inside `draw_search_panel` (bypass `keyboard_owned`):**
- `Enter` / ↓ button — navigate to next match
- `Shift+Enter` / ↑ button — navigate to previous match
- `Escape` — close panel (lower priority than `inline_edit` cancellation)
- `Ctrl+F` while open — refocus input, select-all query text

**Keys that MUST remain in the `keyboard_owned`-gated block:**
- `Ctrl+F` open trigger — requires `canvas_focused && !modal_blocked`

**Keys that must NOT be intercepted by the search panel:**
- Bare arrow keys — `TextEdit` consumes them when focused; the panel must not separately bind arrow keys for navigation when `TextEdit` is active, as this would swallow widget-nudge shortcuts. Use dedicated ↑/↓ `Button` widgets for mouse navigation instead.

---

## Scroll-to-Match

No scroll-into-view utility exists in the codebase for this purpose. Rubber-band selection (interaction.rs lines 3737–3765) collects IDs but does not scroll. `compute_fit_rect` (line 99) changes zoom — incorrect here. The field is `CanvasSettings::pan`, not `canvas_offset`.

Implement a new helper in `search.rs`:

```rust
pub fn scroll_to_widget(
    id: Uuid,
    tree: &UiTree,
    settings: &mut CanvasSettings,
    viewport: egui::Rect,
) {
    let Some(widget) = tree.widgets.iter().find(|w| w.id == id) else { return; };
    let widget_canvas_center = egui::vec2(
        widget.rect.x + widget.rect.w / 2.0,
        widget.rect.y + widget.rect.h / 2.0,
    );
    let widget_screen_center = widget_canvas_center * settings.zoom + settings.pan;
    // Already-visible guard: excludes top-right panel footprint
    let usable = viewport.shrink2(egui::vec2(350.0, 50.0));
    if usable.contains(widget_screen_center.to_pos2()) { return; }
    settings.pan = usable.center().to_vec2() - widget_canvas_center * settings.zoom;
}
```

The `usable` viewport excludes the top-right 350×50 area so `scroll_to_widget` never pans the current match under the search panel.

---

## Extensibility Notes

`SearchFieldRegistry` is the extension point. Future additions (S11 accessibility, S20 code intelligence, S6 data binding) add entries here without touching `run_search` or rendering.

**HashMap key constraint:** `WidgetKind` does not currently derive `Hash` or `Eq` (schema.rs line 560). A `HashMap<WidgetKind, Vec<SearchField>>` requires `Hash + Eq`. Two options:
- (a) Add `#[derive(Hash, Eq)]` to `WidgetKind` — confirm all variants are `Eq`-safe; `Custom(String)` is fine.
- (b) Key the registry by a `WidgetKindDiscriminant` enum (no payload) and handle `Custom` variants via a separate dynamic lookup path.

**Chosen approach must be documented before implementation begins.**

**Hot-reload lifecycle:** `cmd_reload_descriptors()` (app.rs line 1056) replaces the descriptor list at runtime. The registry must be marked dirty and rebuilt after any descriptor reload. The "built once at startup" claim applies only to built-in `WidgetKind` entries. `Custom` widget fields are discovered dynamically by iterating `WidgetInstance::descriptor_props` at search time — this eliminates the HashMap key problem for `Custom` variants and ensures hot-reloaded descriptors are always searchable.

---

## Out of Scope

- Regex or fuzzy matching (substring only for S2)
- Search history / saved queries
- Highlighting the matched substring inside the widget label
- Cross-surface search (searches active surface only)
- Bare arrow key navigation within the search panel (conflicts with canvas nudge)

---

## Files Touched

| File | Change |
|---|---|
| `src/canvas/search.rs` | New — `CanvasSearchState`, `SearchFieldRegistry`, `run_search`, `draw_search_panel`, `scroll_to_widget`, `SearchPanelResponse` |
| `src/canvas/interaction.rs` | Add `canvas_search: Option<CanvasSearchState>` to `InteractionState` (line 394); add `key_ctrl_f` binding; narrow existing bare-F zoom-to-fit guard at line 2477 with `!ctrl_held` |
| `src/canvas/mod.rs` | `pub mod search;` |
| `src/panels/code_preview.rs` | Gate existing Ctrl+F handler at line 442 behind `args.editor_has_focus` — prevents stealing keypress from canvas when code panel does not have focus |
| `src/panels/shortcuts.rs` | Register `("Ctrl+F", "Open canvas widget search")` in `BUILTIN_SHORTCUTS` (lines 16–32) |
| `src/app.rs` | Insert post-handle search overlay pass after `rulers::draw()` and `draw_bezel()` (after line 3336), following Stage-11 overlay pattern at lines 3344–3367 |

---

## Tests

**Existing (updated):**
- `search::tests::empty_query_returns_no_matches`
- `search::tests::label_match_case_insensitive` — include at least one non-ASCII char to lock in Unicode full case-folding
- `search::tests::kind_match` — cover built-in display name AND `Custom` inner string AND `descriptor_name`
- `search::tests::binding_match` — exercise both `state_binding` and `label_binding` (both `Option<String>`); `WidgetInstance.binding` does not exist
- `search::tests::navigate_wraps_around` — assert counter string `"1 / N"` after forward wrap AND `"N / N"` after backward wrap, not just index value
- `search::tests::select_all_returns_all_match_ids`
- `search::tests::extensible_registry_custom_field`

**Required new:**
- `search::tests::navigate_noop_when_no_matches` — `matches=[]`; fire Enter and Shift+Enter; assert no panic, index remains 0 (covers usize % 0 panic)
- `search::tests::ctrl_f_only_opens_when_canvas_focused` — assert `canvas_search` remains `None` when `canvas_focused=false`
- `search::tests::deleted_widget_removed_from_match_list` — open search, delete a matched widget, assert ghost ID not fed to Select All
- `search::tests::select_all_with_zero_matches_is_noop` — assert no selection change and no panel close when `matches.is_empty()`
- `search::tests::canvas_search_not_in_serialized_project` — serialize a `ProjectDocument`; assert JSON contains no `canvas_search` or `search_registry` key
- `search::tests::bare_f_still_zooms_to_fit` — assert bare `F` still triggers zoom-to-fit after adding the `Ctrl+F` search handler (regression for `!ctrl_held` guard)
- `search::tests::run_search_with_none_optional_fields_does_not_panic` — widget with `tooltip=None`, `state_binding=None`, `label_binding=None`, `event_handler=None`; call `run_search`; assert no panic
- `search::tests::counter_displays_one_based_index` — assert displayed counter is `"3 / 7"` when `current_index=2` and `matches.len()=7`
- `search::tests::surface_switch_clears_canvas_search` — simulate surface switch via `InteractionState::default()`; assert `canvas_search` is `None`
