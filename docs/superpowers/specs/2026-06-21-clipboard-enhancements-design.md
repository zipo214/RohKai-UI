# Clipboard Enhancements Design Spec

**Feature:** S2 — Clipboard enhancements: Copy, Paste-at-cursor, Paste-multiple, Duplicate  
**Date:** 2026-06-21  
**Status:** Approved — ready for implementation planning

---

## Summary

Implement a complete in-app clipboard for the RohKai canvas: `Ctrl+C` (copy), `Ctrl+V` (paste at cursor), `Ctrl+D` (duplicate in-place). Clipboard is a private, session-only, in-memory buffer — it does not touch the OS clipboard. Paste places a remapped deep clone of the copied selection centered on the canvas cursor position. All widget fields are preserved exactly; only UUIDs and bindings are remapped. The feature is gated behind the existing `canvas_keyboard_owned` guard and disabled when any text editor, drag, or resize owns input.

---

## Adversarial Review Basis

This spec was informed by a 5-attacker adversarial requirements elicitation (data integrity, behavior edge cases, coordinate math, surface parity, UX disambiguation) producing 25 requirements (CB-01 through CB-25) before any design was written. All BLOCKING and IMPORTANT requirements are incorporated below.

---

## Section 1: Architecture & Data Model

### New file: `src/canvas/clipboard.rs`

Mirrors the `src/canvas/search.rs` pattern: one module owns all clipboard types and public functions. `app.rs` calls into it; the module has no knowledge of egui panels or draw state beyond what it needs for coordinate math.

### Modified files

- `src/canvas/mod.rs` — add `pub mod clipboard;`
- `src/canvas/interaction.rs` — add clipboard fields to `InteractionState`
- `src/project/ui_tree.rs` — add `paste_batch()`
- `src/app.rs` — add key handlers (alongside delete handler ~line 3257)
- `src/panels/shortcuts.rs` — register new shortcuts

### State on `InteractionState` (session-only, never serialized)

```rust
pub clipboard: Vec<WidgetInstance>,  // deep value snapshot, canvas-space rects
pub paste_cascade: usize,            // increments per paste; resets on new Copy
pub paste_flash: Option<(Vec<Uuid>, f32)>, // (new root ids, fade timer in seconds)
```

`InteractionState` must never derive `Serialize` or `Deserialize`. A regression test enforces this.

### Clipboard payload rule

The payload is a full `Clone` of the copy-closed `Vec<WidgetInstance>` captured at copy time, holding **all** fields:

- `svg_source`, `descriptor_name`, `descriptor_accent`, `live_tpl`, `export_tpl`, `descriptor_props`, `descriptor_cargo_deps`, `descriptor_state_fields`
- All handlers: `on_click`, `on_change`, `on_double_click`, `on_lost_focus`, `on_drag_stopped`, `event_handler`, `async_handler`, `handler_result`
- `colors` (fg, bg, corner_radius, font_size, text_align), `tooltip`, `enabled`, `label_binding`
- `custom_props`, `import_metadata`
- `child_cross_align`, `child_flex`, `grid_col_span`, `grid_row_span`, `constraints`, `db_binding`, `expand_svg_inline`

Only `id`, `children[]`, and binding strings are transformed at paste time. Everything else is copied verbatim. Behavior graph edges (`app_props.behaviors`) are **not** included in the payload — see Non-goals.

### Copy-closure rule

Copying a container must transitively include all descendant widgets in the payload. Copying a child-only clears any link that would point outside the copied set (the child pastes as a free root). No link in the payload points outside the copied set.

---

## Section 2: `clipboard.rs` Public API & Key Behaviors

### `copy_selection(selected: &[Uuid], tree: &UiTree) -> Vec<WidgetInstance>`

Builds the copy-closed set (expands each selected ID to all descendants transitively), clones all `WidgetInstance` fields, clears any children-link that points outside the set. Returns the payload. No-op (returns empty vec) if `selected` is empty. **Never mutates the tree or `app_props`.**

Caller stores the result in `interaction.clipboard` and resets `interaction.paste_cascade` to 0.

### `paste_payload(clipboard: &[WidgetInstance], cursor_canvas: Option<Pos2>, cascade: usize, tree: &mut UiTree, pan: Vec2, zoom: f32, panel_rect: Rect, canvas_size: Vec2) -> PasteResult`

1. Computes bounding-box center of payload rects in canvas space as the **anchor**
2. Computes **target**: `cursor_canvas` if `Some`; otherwise visible-canvas-viewport center as fallback (derived from `pan`, `zoom`, `panel_rect` — never `Pos2::ZERO`, never raw screen coords)
3. Computes `delta = target - anchor + Vec2::splat(cascade as f32 * PASTE_CASCADE_STEP)`
4. Calls `UiTree::paste_batch(staged_widgets, delta, target_container)`
5. Returns `PasteResult { new_root_ids, had_behaviors: bool }`

**Cursor-to-canvas conversion** uses the shared helper exclusively:

```
canvas_origin = crate::canvas::rulers::canvas_origin(canvas_size, zoom, pan, panel_rect)
cursor_canvas = (cursor_screen - canvas_origin) / zoom
```

Never re-derives this formula inline. `cursor_canvas` is `None` when pointer is off the canvas rect or `ctx.input(|i| i.pointer.interact_pos())` is `None`.

**`PASTE_CASCADE_STEP = 16.0` canvas units** — zoom-stable (applied before `/zoom` scaling). Cascade counter resets on any new `copy_selection` or `cut_selection`; does not reset on cursor movement.

### `duplicate_in_place(selected: &[Uuid], tree: &mut UiTree) -> PasteResult`

Copy-closes the selection, calls `paste_batch` with `delta = Vec2::splat(PASTE_CASCADE_STEP)`. Does **not** touch `interaction.clipboard` or `interaction.paste_cascade`. Stateless relative to the clipboard.

### `cut_selection(selected: &[Uuid], tree: &mut UiTree) -> Vec<WidgetInstance>`

Calls `copy_selection`, then removes the copy-closed set from the tree immediately (not lazy). Returns the payload. Caller stores it on `interaction.clipboard` and resets `paste_cascade` to 0. Cut and paste are two independent undo steps (each captured at a frame boundary by the existing undo recorder).

---

## Section 3: Key Bindings, UX & Visual Feedback

### Keyboard handlers

Located in `app.rs` alongside the delete handler (~line 3257).

**Gate condition — all four actions require ALL of:**
- `canvas_keyboard_owned`
- `!interaction.drag.is_active()`
- `!interaction.resize.is_active()`
- `!editor_has_focus` (code panel, properties fields, canvas search, rename inputs)

Text-field copy/paste (`Ctrl+C`/`Ctrl+V` in a focused text box) remains normal OS/editor behavior, completely unaffected.

| Shortcut key ID | Display | Action |
|---|---|---|
| `canvas_copy` | `Ctrl+C` | `copy_selection` → store on clipboard, reset cascade |
| `canvas_paste` | `Ctrl+V` | `paste_payload` → add to tree, select new roots, increment cascade |
| `canvas_duplicate` | `Ctrl+D` | `duplicate_in_place` → add to tree, select new roots |

All three registered in `BUILTIN_SHORTCUTS`. Shortcut detection resolves through `UserSettings` override lookup — not hardcoded `Key::C` checks — so user rebinds stay consistent with the reference panel.

`Ctrl+X` (Cut) is **out of scope for v1** — see Non-goals.

### Context menu enable/disable state

Add `Copy` and `Paste` entries above the existing Group/Z-order items in the canvas right-click context menu:

| Entry | Enabled when |
|---|---|
| `Copy` | `!selected.is_empty()` |
| `Duplicate` | `!selected.is_empty()` |
| `Paste` | `!clipboard.is_empty() && active_surface_exists` |
| All four | Disabled/hidden when drag, resize, or text editor is active |

Menu-driven paste uses the menu-open cursor position as `cursor_canvas` (not the live mouse at time of selection).

### 1 — Menu availability (summarised above)

Disabled states described per entry above.

### 2 — Immediate confirmation (toast/status, auto-expires ~1.5 s, no modal)

- `"Copied Button"` — single widget, uses `WidgetKind` display name
- `"Copied 4 widgets"` — multiple
- `"Cut 2 widgets"`
- `"Pasted 4 widgets"` / `"Duplicated 3 widgets"`
- `"Pasted N widgets — behavior wires not copied"` — when copied widgets had associated behaviors
- `"Can't paste while resizing"` / `"Can't paste while dragging"` — when shortcut fires during blocked state
- **No message** on no-op (empty selection copy, empty clipboard paste)

### 3 — Canvas confirmation

- Every newly created widget (paste or duplicate) becomes the new `session.selected`
- Selection handles render immediately — multi-paste shows all new widgets selected together
- Optional: brief teal ring/fade around pasted bounding box, using the same visual language as canvas search but implemented as a **separate** `paste_flash` state on `InteractionState` — never touches `CanvasSearchState`

### 4 — Viewport behavior

- After paste/duplicate, if any part of the pasted bounding box is outside the visible canvas viewport, pan the canvas to bring the full bounding box into view (or as much as possible for very large groups)
- Never silently paste at `(0, 0)` — fallback anchor is always visible-canvas-viewport center (CB-07)
- No pan if the pasted result is already fully visible

---

## Section 4: `UiTree::paste_batch`

```
UiTree::paste_batch(staged_widgets: Vec<WidgetInstance>, anchor_delta: Vec2, target_container: Option<Uuid>) -> PasteResult
```

**All steps execute against a staged copy — no tree mutation until step 9.**

1. Build one `old→new` UUID map over the entire copied closure
2. Derive pasted roots: widgets whose old ID is **not** listed in any other copied widget's `children[]`
3. Deep-clone every `WidgetInstance` field
4. Rewrite `widget.id` and every `children[]` entry through the `old→new` map
5. Apply the same canvas-local `anchor_delta` to **every** copied widget's `rect.x`/`rect.y` equally — no special-casing for children (all rects are canvas-local, not relative to parent)
6. Remap internal references:
   - `LayoutConstraints.equal_width_to` / `equal_height_to`: remap when both endpoints are in the copied set; clear otherwise
   - `state_binding`: renamed once per unique binding string across the whole set and shared consistently — never per-widget via `make_binding_unique()` in a loop, which would break intentionally-shared group bindings
   - References to widgets outside the copied set: cleared
7. **Validate staged graph before any commit:**
   - No duplicate IDs
   - No stale `children[]` references
   - No duplicate-parent state (child listed in two parents' `children[]`)
   - No cycles (no widget is its own ancestor)
   - No self-child
8. If validation fails: abort entirely, **no partial insert**, show toast: `"Paste failed: invalid widget graph"`
9. Commit all staged widgets into the tree in one call (direct `tree.widgets.insert` inside `UiTree` — not the public `add()` per-widget loop)
10. If `target_container` exists, attach pasted roots through the normal `UiTree::attach_to_layout_at` / reflow API. **Note:** if pasted roots are attached to a layout container, final rects may be changed by the normal layout reflow path. Free-root paste preserves `anchor_delta` geometry exactly.
11. Return `PasteResult { new_root_ids: Vec<Uuid>, had_behaviors: bool }` for selection, undo boundary, and viewport reveal

### `app_props.behaviors` — explicit v1 limitation

- Behavior graph edges are **not included** in the clipboard payload in v1
- `copy_selection` **never mutates** existing `app_props.behaviors`
- Paste does not duplicate behavior graph edges
- Paste must not create dangling behavior references
- Toast when copied widgets had associated behaviors: `"Pasted N widgets — behavior wires not copied"`
- Regression tests:
  - Paste widgets with behaviors attached; assert original behaviors remain unchanged
  - Assert pasted widgets do not create dangling behavior references in the tree
  - Assert the diagnostic fires

---

## Section 5: Testing

### `clipboard.rs` unit tests (pure functions, no egui context required)

- `copy_selection` on empty selection → empty vec, no panic
- `copy_selection` on Frame+children → closure includes all descendants, no link points outside copied set
- `copy_selection` on child-only selection → root derivation correct; no link to uncopied parent
- Coordinate round-trip: call the **actual paste coordinate helper** (not algebra), assert screen↔canvas round-trip at sampled `(zoom, pan, panel_rect, screen_pos)` grid values
- `cursor_canvas = None` fallback → anchor is visible canvas viewport center; result coords are finite and within the visible canvas viewport
- Multi-widget paste: 3-widget fixture at `(100,100)`, `(200,100)`, `(300,200)` pasted at cursor `(500,400)` — assert pairwise distances preserved, sizes unchanged at zoom 0.25 and 4.0
- 5 consecutive pastes of same payload → positions monotonically cascade by `PASTE_CASCADE_STEP`; new `copy_selection` resets cascade to 0
- `duplicate_in_place` → does not touch `interaction.clipboard` or `paste_cascade`

### `UiTree::paste_batch` unit tests

- After paste, no `children[]` entry references any pre-paste UUID; pasted subtree shares no UUID with source
- Frame+children paste: all children present, all links consistent
- Child-only paste: pasted widget has no parent entry in any other widget's `children[]`
- Shared `state_binding` across a radio group: all pasted members share one consistently renamed binding
- `constraints.equal_width_to` pointing outside copied set → constraint cleared, no dangling ref
- Validation abort: construct staged cycle → `paste_batch` returns error, tree byte-identical to pre-paste
- Target container tests:
  - Paste into `Frame` → roots attached via `attach_to_layout_at`
  - Paste into `VLayout`, `HLayout`, `GridLayout` → roots attached and reflowed
  - Free-root paste (no container) → `anchor_delta` geometry preserved exactly
- Behavior regression:
  - Paste widget with associated `app_props.behavior`; assert original behavior unchanged
  - Assert pasted widgets do not create dangling behavior references
  - Assert `"behavior wires not copied"` diagnostic fires

### Integration / parity tests

- After paste, live egui emitter output includes pasted widget code (canvas/code parity, CB-11)
- After paste, export emitter output includes pasted widgets in the generated Rust project
- After save/load round-trip, pasted widgets and their children remain valid (all UUIDs resolve, no dangling refs)
- After paste, `session.selected` equals new root IDs and is disjoint from source IDs (CB-14)
- `paste_batch` + `undo()` restores the pre-paste `ProjectDocument` exactly (CB-15)
- Serialization invariant: `interaction.clipboard` payload and `paste_cascade` are session-only and absent from saved `.rohkai.json`

---

## Non-goals / Explicit v1 Limits

- **OS clipboard serialization is out of scope.** The clipboard is a private in-app buffer. Canvas copy does not write to the OS clipboard; OS clipboard contents cannot be pasted as widgets.
- **Behavior graph edges are not copied in v1.** `app_props.behaviors` are not included in the clipboard payload. Pasting never duplicates behavior wiring. A diagnostic fires when copied widgets had behaviors.
- **Cut (`Ctrl+X`) is out of scope for v1.** This release ships Copy (`Ctrl+C`), Paste (`Ctrl+V`), and Duplicate (`Ctrl+D`) only. Cut requires an explicit approval and its own undo/transaction design before implementation.
- **Clipboard state is session-only and never persisted.** The clipboard is cleared on app restart. `.rohkai.json` files never contain clipboard contents.
- **"Paste N copies" dialog is out of scope.** Repeat paste is handled by pressing `Ctrl+V` multiple times with the cascade offset.
- **`Ctrl+D` duplicate-in-place differs from `Ctrl+V` paste-at-cursor** and both ship. They are not redundant: `Ctrl+D` is a single-keystroke speed-iteration flow (in-place, fixed offset); `Ctrl+V` is explicit placement at the cursor.
