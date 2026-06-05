---
name: project-model
description: Use when reading or writing UiTree, WidgetInstance, settings, or project schema types. This is the source-of-truth data model; all agents must speak this language before mutating canvas/codegen state.
---

# Project Data Model

## Rule

`UiTree` is the single project source of truth. The canvas renders it, the code panel emits from it, the parser writes back into it, and project save/load serializes it. User preferences live separately in `UserSettings` and must not dirty `.rohkai.json` files.

## UiTree (`src/project/ui_tree.rs`)

The project root:

```rust
widgets: Vec<WidgetInstance>
app_props: AppProps
```

Use `UiTree::add`, `remove`, `get_mut`, `group`, `ungroup`, and `validate_and_repair` instead of scattering mutation rules in panels.

Never push to or splice `UiTree.widgets` directly. Direct `Vec` access bypasses validation, dirty tracking, and repair logic.

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
layout_spacing:f32
grid_columns:  usize
```

`default_value` is meaningful for sliders and is clamped during repair.
`options` is meaningful for ComboBox, defaults to `["Option A", "Option B", "Option C"]`, and is repaired to a non-empty default for ComboBox widgets.
`inner_margin` and `layout_spacing` drive Frame/layout container reflow. `grid_columns` drives GridLayout row-major cell assignment and generated `ui.end_row()` boundaries.

## WidgetKind

Current built-ins:

```text
Button
Label
TextInput
Slider
Checkbox
Frame
ComboBox
RadioButton
ProgressBar
Image
```

Adding a new kind requires schema, widget default constructor, palette coverage, canvas rendering, parser/codegen/export/state behavior, and tests or validation notes.

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

Project files are `.rohkai.json` and use the versioned `ProjectFile` envelope. Legacy bare `UiTree` files still load. Dirty checks, save, and open must all use `src/project/io.rs` serialization paths.
