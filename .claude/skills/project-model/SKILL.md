---
name: project-model
description: Use when reading or writing UiTree, WidgetInstance, settings, or project schema types. This is the source-of-truth data model; all agents must speak this language before mutating canvas/codegen state.
---

# Project Data Model

## Rule

`ProjectDocument` is the project source of truth. Each `UiSurface.tree` is the
single source of truth for that surface: the canvas renders it, the code panel
emits from it, and the parser writes back into it. `ActiveDocument` exposes the
active tree without creating another owner. Project save/load serializes the
whole document. User preferences live separately in `UserSettings` and must not
dirty `.rohkai.json` files.

## ProjectDocument (`src/project/document.rs`)

```rust
props: ProjectProps
root_surface: Uuid
surfaces: Vec<UiSurface>
```

Use `add_modal_surface`, `duplicate_surface`, `rename_surface`,
`remove_surface`, `move_surface`, and `validate_and_repair` rather than editing
`surfaces` directly. The root surface cannot be deleted. Global behaviors,
components, assets, theme, and Rust wiring belong to `ProjectProps`; title,
size, guides, constraints, and modal policy belong to each surface.

## UiTree (`src/project/ui_tree.rs`)

The surface-local widget tree:

```rust
widgets: Vec<WidgetInstance>
app_props: AppProps
```

Use `UiTree::add`, `remove`, `get_mut`, `group`, `ungroup`, and `validate_and_repair` instead of scattering mutation rules in panels.

Never push to or splice `UiTree.widgets` directly. Direct `Vec` access bypasses validation, dirty tracking, and repair logic.

`WidgetInstance.children` is an ownership tree, not a loose reference list. A
child may have at most one parent and cycles are invalid. Use UiTree helpers for
attach/move/remove/repair; if a feature reads or writes `children`, derive and
test the full topology matrix before coding: top-level, Frame-owned,
layout-owned, layout-inside-layout, Frame-owned-layout, moved child,
empty/cleared container parse, duplicate-parent repair, and cycle repair.

## WidgetInstance (`src/project/schema.rs`)

```rust
id:              Uuid
kind:            WidgetKind
rect:            Rect
props:           WidgetProps
state_binding:   Option<String>
children:        Vec<Uuid>
import_metadata: Option<ImportMetadata>
tooltip:         Option<String>
enabled:         Option<bool>
fg_color:        Option<[u8; 3]>
corner_radius:   Option<f32>
label_binding:   Option<String>
custom_props:    Vec<CustomProp>
event_handler:   Option<String>
svg_source:      Option<String>
```

## WidgetProps

```rust
label:         String
min:           f32
max:           f32
default_value: f32
options:       Vec<String>
inner_margin:  f32
layout_spacing: f32
layout_stretch: bool
grid_columns:  usize
```

`default_value` is meaningful for sliders and is clamped during repair.
`options` is meaningful for ComboBox, defaults to `["Option A", "Option B", "Option C"]`, and is repaired to a non-empty default for ComboBox widgets.
`inner_margin` and `layout_spacing` drive Frame/layout container reflow.
`layout_stretch` controls first-slice fill/stretch behavior in layout containers.
`grid_columns` drives GridLayout row-major cell assignment and generated
`ui.end_row()` boundaries.

## WidgetKind

Use the canonical `WidgetKind`/`ALL_KINDS` APIs in source before relying on any
handwritten list. Adding a new kind requires schema, widget default constructor,
palette coverage, canvas rendering, parser/codegen/export/state behavior, and
tests or validation notes.

`WidgetKind::Image` is backed by `svg_source` and must have a real output form
everywhere it appears: canvas rendering, properties, live codegen, export,
tests, docs. Do not replace it with comments or labels and call that complete.

## AppState field types

Use `src/codegen/kind_table.rs` as the first stop. Current defaults:

```text
Button       -> no state unless custom/event handler
Label        -> String when bound
TextInput    -> String
Slider       -> f32
Checkbox     -> bool
Frame        -> no state
ComboBox     -> String
RadioButton  -> bool
ProgressBar  -> f32
Custom props -> declared type
Image        -> no AppState field
```

## Dependency And Output Rules

SVG import, SVG image preview, and SVG renderer work are zero-new-crate zones.
Do not add `resvg`, `usvg`, `tiny-skia`, a substitute SVG renderer crate, or a
new dependency chain. "Pure Rust crate" is not permission.

No hollow features: if a widget kind or property is visible in RohKai, it must
have a real behavior in the canvas, code panel, export, save/load, tests, and
docs, or it must be explicitly marked unavailable.

## Serialization

Project files are `.rohkai.json` and use schema-v2 `ProjectDocument` envelopes.
Legacy bare `UiTree` and schema-v1 files still load into one root surface. Dirty
checks, save, open, and undo must all use `src/project/io.rs` document
serialization paths.
