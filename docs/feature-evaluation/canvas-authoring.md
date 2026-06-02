# Canvas Authoring Evaluation

## Scope

This covers the design canvas, selection, movement, resize, z-order, snap, smart
guides, rulers, preview visuals, hierarchy awareness, and authoring ergonomics.

## Top-Class Expectation

The canvas should feel like a precise design surface and a truthful egui preview.
Users should understand size, position, hierarchy, alignment, and generated code
impact while editing. Competitor-depth means Qt Designer layout precision plus
Figma-like manipulation polish, without breaking immediate-mode egui semantics.

## Current State

| Feature | Current Depth | What It Does Now | Gap |
|---|---:|---|---|
| Selection | 3 | Click, multi-select, rubber-band, selection list. | Needs selection breadcrumbs and locked/hidden states. |
| Move/resize | 3 | Drag, nudge, handles, aspect lock, resize cursor feedback. | Needs constraints, numeric transform panel polish, and handle accessibility. |
| Z-order/layers | 3 | Context menu and Layers tab; drag reorder through `UiTree`. | Needs grouping hierarchy operations and visibility/lock toggles. |
| Snap/grid | 3 | Grid snap, step setting, guide snapping. | Needs per-document units, snap profiles, and object spacing controls. |
| Smart guides | 3 | Edge/center alignment and equidistant guides during drag/resize. | Needs persistent measurement overlays and multi-object distribution UI. |
| Rulers/guides | 3 | Pixel rulers, persistent guide lines, Ctrl+R. | Needs guide manager, named guides, and unit/preset support. |
| Canvas preview visuals | 3 | Widgets look close to egui output; Image widgets rasterize SVG subset. | Needs real style parity for all widget states and nested layout preview. |
| Preview mode | 3 | F5 renders actual egui widgets with preview state. | Needs bidirectional preview/debug controls and event simulation history. |

## Utility

- Authoring utility: very high. This is the main work surface.
- Inspection utility: high. Rulers, guides, layers, and overlays explain layout.
- Runtime utility: medium-high. Canvas geometry drives export.

## Ideal State

| Capability | Ideal Behavior |
|---|---|
| Layout-aware editing | Users can switch between absolute placement and egui layout containers, with clear constraints. |
| Hierarchy-first canvas | Containers, children, slots, overlays, and non-visual components are visible without clutter. |
| Measurement system | Rulers, distances, spacing, alignment, and canvas/window presets are inspectable and editable. |
| Component isolation | Double-click enters component/container editing context; breadcrumbs return to parent. |
| State visualization | Bound fields, ownership, error flow, and preview state can be overlaid without blocking edits. |
| Large-project performance | 500+ widgets remain responsive with incremental codegen/canvas invalidation. |

## Depth Measurements

| Measure | Target For Top-Class |
|---|---|
| Geometry correctness | Canvas rects round-trip through save/load/codegen with no drift. |
| Manipulation latency | Drag/resize under 16 ms/frame for 500 widgets on target hardware. |
| Snap accuracy | Snap/guide math exact within 0.5 canvas px at all zoom levels. |
| Visual fidelity | Canvas and preview mode differ only by documented design-time affordances. |
| Hierarchy clarity | User can identify parent/container and z-order for any selected widget in under 2 seconds. |

## Recommended Next Work

1. Add visibility/lock toggles to Layers.
2. Add guide manager and measurement inspector.
3. Add layout-container editing affordances: slots, padding, spacing, child order.
4. Add canvas performance instrumentation for widget count and frame time.
5. Add visual regression screenshots for common canvas operations.

