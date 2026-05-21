---
name: canvas-patterns
description: Use when working on canvas hit-testing, widget selection, resize handles, drag state, or grid snap in interaction.rs. Describes the actual interaction model.
---

# Canvas Interaction Model

Source of truth: `src/canvas/interaction.rs`.

## Coordinate System

All widget positions are stored in canvas-local space with `schema::Rect { x, y, w, h }`.
`canvas_rect()` in `src/canvas/widget_instance.rs` converts to screen space by adding
`origin`, the top-left of the egui response rect.

## Hit Testing

Widgets are tested in reverse draw order so the visually topmost widget is selected first.
Resize handles are checked before body hit-testing so a handle click on the selected widget
always wins, even if it visually overlaps a different widget.

## Resize Handles

`ResizeHandle` has 8 variants:

- `TopLeft`
- `Top`
- `TopRight`
- `Left`
- `Right`
- `BottomLeft`
- `Bottom`
- `BottomRight`

Each handle is an 8x8 px hit rect centered on its cardinal or corner anchor.

Cursor icons:

- `ResizeNwSe` for top-left and bottom-right
- `ResizeNeSw` for top-right and bottom-left
- `ResizeVertical` for top and bottom
- `ResizeHorizontal` for left and right

## Drag State Machine

Idle becomes resizing when the primary pointer goes down on a handle of the selected widget.
Idle becomes dragging when the primary pointer goes down on a widget body.
Both states clear when the primary pointer is released.

Only one of `InteractionState::dragging` and `InteractionState::resize` should be active.

## Drift-Free Movement

Move drag stores `drag_offset`, the pointer position relative to the widget top-left at
mouse-down. Each frame computes position from the current absolute pointer and the original
offset. Do not accumulate per-frame deltas for movement.

Resize drag stores `start_rect` and `start_pos`. Each frame computes `delta = pos - start_pos`
and applies that to `start_rect`. Do not accumulate per-frame resize deltas.

## Grid Snap

`CanvasSettings` owns `snap_enabled` and `snap_step`. Snapping applies after raw movement or
resize math. Clamp snap step to a positive value before dividing by it.

Snap all four rect fields in `snap_rect()`, and always enforce:

- `x >= 0`
- `y >= 0`
- `w >= MIN_SIZE`
- `h >= MIN_SIZE`

## Keyboard Nudge

Arrow keys move the selected widget by 1 px when snap is off, or by `snap_step` when snap is
on. Clamp to `x/y >= 0`. Keyboard nudge does not use the drag state machine.
