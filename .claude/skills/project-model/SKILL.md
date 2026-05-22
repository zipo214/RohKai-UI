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

## WidgetInstance (`src/project/schema.rs`)

```rust
id:              Uuid
kind:            WidgetKind
rect:            Rect
props:           WidgetProps
state_binding:   Option<String>
children:        Vec<WidgetInstance>
import_metadata: Option<ImportMetadata>
tooltip:         Option<String>
enabled:         Option<bool>
fg_color:        Option<[u8; 3]>
corner_radius:   Option<f32>
label_binding:   Option<String>
custom_props:    Vec<CustomProp>
event_handler:   Option<String>
```

## WidgetProps

```rust
label:         String
min:           f32
max:           f32
default_value: f32
```

`default_value` is meaningful for sliders and is clamped during repair.

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
```

Adding a new kind requires schema, widget default constructor, palette coverage, canvas rendering, parser/codegen/export/state behavior, and tests or validation notes.

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
```

## Serialization

Project files are `.rohkai.json` and use the versioned `ProjectFile` envelope. Legacy bare `UiTree` files still load. Dirty checks, save, and open must all use `src/project/io.rs` serialization paths.
