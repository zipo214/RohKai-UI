---
name: canvas-patterns
description: Use when working on canvas hit-testing, widget selection, resize handles,
drag state, or grid snap in interaction.rs. Describes the actual interaction model.
---

# Canvas Interaction Model

Source of truth: `src/canvas/interaction.rs`.

## Coordinate system

All widget positions are stored in **canvas-local space** (`schema::Rect { x, y, w, h }`).
`canvas_rect()` (`src/canvas/widget_instance.rs`) converts to screen space by adding `origin`
(the top-left of the egui response rect):

```rust
egui::Rect::from_min_size(origin + egui::vec2(w.rect.x, w.rect.y), egui::vec2(w.rect.w, w.rect.h))
```

## Hit testing — widget selection

Widgets are tested in **reverse draw order** (last-on-top wins) so the visually topmost
widget is selected first:

```rust
for widget in tree.widgets.iter().rev() {
    let rect = egui::Rect::from_min_size(origin + egui::vec2(widget.rect.x, widget.rect.y), ...);
    if rect.contains(pos) { /* select + start drag */ break; }
}
```

Handles are checked **before** body hit-testing so a handle click on the selected widget
always wins, even if it visually overlaps a different widget.

## Resize handles — 8-point layout

`ResizeHandle` enum has 8 variants: `TopLeft`, `Top`, `TopRight`, `Left`, `Right`,
`BottomLeft`, `Bottom`, `BottomRight`.

Each handle's **anchor point** is one of the 8 cardinal/corner positions on the widget rect:

| Handle | Anchor |
|---|---|
| TopLeft | `rect.left_top()` |
| Top | `rect.center_top()` |
| TopRight | `rect.right_top()` |
| Left | `rect.left_center()` |
| Right | `rect.right_center()` |
| BottomLeft | `rect.left_bottom()` |
| Bottom | `rect.center_bottom()` |
| BottomRight | `rect.right_bottom()` |

**Hit rect**: `8×8` px square centered on the anchor (`HANDLE_HALF = 4.0`):

```rust
egui::Rect::from_center_size(self.anchor(rect), egui::vec2(8.0, 8.0))
```

Cursor icons: `ResizeNwSe` (TL/BR), `ResizeNeSw` (TR/BL), `ResizeVertical` (T/B),
`ResizeHorizontal` (L/R).

## Drag state machine

```
idle
 │ mouse-down on handle of selected widget
 ▼
resizing  (InteractionState::resize = Some(ResizeState { id, handle, start_rect, start_pos }))
           InteractionState::dragging = None
 │ mouse-up
 ▼
idle

idle
 │ mouse-down on widget body
 ▼
dragging  (InteractionState::dragging = Some(id), drag_offset recorded)
           InteractionState::resize = None
 │ mouse-up
 ▼
idle
```

Both states clear on `!is_down` (primary button released):

```rust
if !is_down { state.dragging = None; state.resize = None; }
```

## Drift-free drag delta approach

### Move drag

`drag_offset` is the pointer position **relative to the widget's top-left** at the moment
of mouse-down. Each frame, the new position is computed from the **current** absolute pointer,
not accumulated increments:

```rust
let raw = pos - origin - state.drag_offset;
w.rect.x = raw.x.max(0.0);
w.rect.y = raw.y.max(0.0);
```

This means no floating-point drift — the widget corner always tracks back to the same
cursor-relative offset regardless of how many frames have elapsed.

### Resize drag

`start_rect` captures the widget rect at mouse-down. `start_pos` captures the pointer.
Each frame, `delta = pos - start_pos` is applied to `start_rect` via `apply_delta()`:

```rust
let delta = pos - rs.start_pos;
let new_rect = rs.handle.apply_delta(&rs.start_rect, delta);
```

`apply_delta` constrains each handle to move only its relevant edges, clamping to
`MIN_SIZE = 20.0` and `x/y >= 0.0`.

## Grid snap

`CanvasSettings { snap_enabled: bool, snap_step: f32 }`. Toggle with **G** key.

```rust
fn snap(val: f32, step: f32) -> f32 { (val / step).round() * step }
```

Applied **after** computing the raw rect in both move and resize paths:

- **Move**: `snap(raw.x, step)`, `snap(raw.y, step)` before writing to `w.rect`.
- **Resize**: `snap_rect()` snaps all four fields (`x`, `y`, `w`, `h`) after `apply_delta`.

`snap_rect` also enforces `x/y >= 0` and `w/h >= MIN_SIZE`.

## Keyboard nudge

Arrow keys move the selected widget by `1.0 px` (snap off) or `snap_step` (snap on).
Clamped to `x/y >= 0`. Does not use the drag state machine.
