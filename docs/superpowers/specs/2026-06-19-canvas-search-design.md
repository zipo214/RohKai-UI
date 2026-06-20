# Canvas Search (S2 Item 1) — Design Spec

Date: 2026-06-19  
Stage: S2 — Canvas UX Depth  
Trigger: Ctrl+F on canvas (distinct from code-panel Ctrl+F)

---

## Summary

A floating search panel anchored to the top-right of the canvas that lets users find widgets by name, kind, or property value. Supports navigate-through (↑↓) and select-all. Zero new color vocabulary — uses established kind-accent colors at reduced opacity.

---

## Architecture

**New file:** `src/canvas/search.rs`

Responsible for:
- `CanvasSearchState` struct (query, match list, current index)
- `SearchFieldRegistry` — maps `WidgetKind → Vec<SearchField>` (extensible)
- `run_search(tree, query) -> Vec<WidgetId>` — pure function, no side effects
- `draw_search_panel(ui, state) -> SearchPanelResponse` — immediate-mode panel

**`CanvasInteraction`** gains one field:
```rust
pub canvas_search: Option<CanvasSearchState>,
```

**`interaction.rs` rendering pass** reads `canvas_search` to apply ring/glow overlays after the normal widget draw pass. No other file changes needed.

---

## Search Scope (medium, extensible)

Initial searchable fields per widget:

| Field | Source |
|---|---|
| Label text | `WidgetInstance.label` |
| Binding name | `WidgetInstance.binding` |
| Tooltip | `WidgetInstance.tooltip` |
| Event handler name | `WidgetInstance.on_click` / `on_change` |

Adding a new field = one entry in `SearchFieldRegistry`. No other changes required.

Matching: case-insensitive substring. Kind search matches the display name (e.g. "button", "slider").

---

## UX Behaviour

**Open:** Ctrl+F while canvas has focus → opens panel, focuses input, does not steal canvas selection.  
**Close:** Escape, or ✕ button → clears search state, removes all rings/glows, restores canvas to normal.  
**Empty query:** no matches, no rings, counter hidden.  
**No results:** counter shows "0 results", input tinted red (use `egui::Color32::from_rgba_unmultiplied(255, 80, 80, 180)` — the established smart-guide red).

**Navigation:**
- Enter / ↓ button → advance to next match, scroll canvas to bring it into view
- Shift+Enter / ↑ button → previous match
- Counter label: `"3 / 7"`

**Select All:**
- Clicking "Select All" applies all match IDs as the active multi-selection and closes the search panel
- Feeds directly into existing multi-select tools (alignment, property edit, delete)

---

## Visual Treatment

Rendered as a second overlay pass after normal widget drawing.

**Current match:**
- `outline: 2px, rgba(52, 211, 153, 0.75), offset 3px`
- No fill change, no text color change, no interior modification

**Other matches:**
- Inner glow ring: `rgba(52, 211, 153, 0.18)` at 3px spread
- Outer haze: `rgba(52, 211, 153, 0.12)` at 10px spread
- No outline, no interior change

**Non-matches:** completely unchanged.

In egui terms: after widget rects are drawn, iterate `search.matches` and paint an overlay pass. Current match: `painter.rect_stroke(rect.expand(3.0), 4.0, Stroke::new(2.0, rgba(52,211,153,192)))`. Other matches: two `painter.rect` calls with expanded rects at alpha 46 and 31 respectively, simulating a CSS-style glow spread (egui has no native box-shadow).

---

## Floating Panel Layout

Top-right of the canvas `Rect`, fixed offset 8px from edge. `egui::Area` with `Order::Foreground`.

```
┌─────────────────────────────────────┐
│ 🔍 [___query input___]  3/7  ↑ ↓  [Select All]  ✕ │
└─────────────────────────────────────┘
```

- Width: ~340px fixed
- Single row, no wrapping
- Panel background: `egui::Color32::from_rgba_unmultiplied(30, 30, 46, 230)` (matches canvas dark)
- Rounded corners: 5px

---

## State Machine

```
Closed
  → Ctrl+F → Open(query="", matches=[], index=0)

Open
  → type query   → recompute matches, reset index to 0
  → Enter / ↓    → index = (index + 1) % matches.len(); scroll to current
  → Shift+Enter  → index = (index + matches.len() - 1) % matches.len(); scroll
  → Select All   → set canvas selection = matches; → Closed
  → Escape / ✕  → → Closed
  → canvas drag/click → does NOT close search (search persists across canvas interaction)
```

---

## Scroll-to-Match

When the current match changes, compute the widget's canvas-space rect and call the existing pan/zoom scroll-into-view utility (already used for rubber-band selection). If no such utility exists, compute the offset delta to center the widget and apply it to `canvas_offset`.

---

## Extensibility Notes

`SearchFieldRegistry` is the extension point. Future additions (S11 accessibility, S20 code intelligence, S6 data binding) add entries here without touching search logic or rendering. The registry is built once at startup and borrowed immutably during search.

---

## Out of Scope

- Regex or fuzzy matching (substring only for S2)
- Search history / saved queries
- Highlighting the matched substring inside the widget label (text is not editable in search mode)
- Cross-surface search (searches active surface only)

---

## Files Touched

| File | Change |
|---|---|
| `src/canvas/search.rs` | New — all search logic and panel |
| `src/canvas/interaction.rs` | Add `canvas_search` field; Ctrl+F handler; overlay render pass |
| `src/canvas/mod.rs` | `pub mod search;` |

No other files require changes.

---

## Tests

- `search::tests::empty_query_returns_no_matches`
- `search::tests::label_match_case_insensitive`
- `search::tests::kind_match`
- `search::tests::binding_match`
- `search::tests::navigate_wraps_around`
- `search::tests::select_all_returns_all_match_ids`
- `search::tests::extensible_registry_custom_field`
