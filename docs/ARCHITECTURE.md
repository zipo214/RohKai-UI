# RohKai Architecture

Pure-Rust WYSIWYG egui UI designer. Zero dependencies on web runtimes.
Single binary; no platform-specific dialog layer beyond `rfd` for file pickers.

---

## Data model — `UiTree` as single source of truth

```
src/project/
  schema.rs      — WidgetKind, WidgetProps, Rect, WidgetInstance
  ui_tree.rs     — UiTree (owns Vec<WidgetInstance>)
  io.rs          — save/load (serde_json ↔ .rohkai.json)
```

`UiTree` is the **only** authoritative state for what is on the canvas.
All panels read from it; the canvas mutates it in-place through `tree.get_mut(id)`.

### Schema types

| Type | Purpose |
|---|---|
| `WidgetKind` | Enum: `Button`, `Label`, `TextInput`, `Slider`, `Checkbox` |
| `WidgetProps` | `label: String`, `min: f32`, `max: f32` (shared across kinds) |
| `Rect` | `{ x, y, w, h }` in canvas-local pixels; default `(20, 20, 120, 32)` |
| `WidgetInstance` | `{ id: Uuid, kind, rect, props, state_binding: Option<String> }` |
| `UiTree` | `{ widgets: Vec<WidgetInstance>, canvas_width, canvas_height }` |

`state_binding` is the Rust field name emitted into generated code. `None` → placeholder
comment `/* TODO: set binding */` in output.

### Persistence

`project::io::save` serializes `UiTree` to pretty JSON and returns the string.
`RohKaiApp` stores this string as `saved_json` to compute dirty state
(`is_dirty` = current serialization ≠ `saved_json`).

---

## Canvas render pipeline

```
app.rs: RohKaiApp::update()
  └─ egui::CentralPanel → canvas::interaction::handle(ui, tree, state, selected, settings)
       │
       ├─ allocate_painter (Sense::click_and_drag)
       ├─ draw background + dotted canvas boundary
       ├─ draw 20px grid lines
       ├─ for each widget in tree.widgets:
       │    canvas_rect(widget, origin)   ← widget_instance::canvas_rect adds origin offset
       │    rect_filled  (kind_fill tint)
       │    rect_stroke  (accent if selected, gray otherwise)
       │    text label   (center)
       │    kind tag     (bottom-right, 9px muted)
       ├─ if selected: draw 8 resize handles (8×8 px squares)
       └─ process mouse/keyboard → mutate tree in-place
```

`canvas::widget_instance::canvas_rect` is the only place that converts schema coords
to screen coords. Every other subsystem works in schema space.

Visual style per kind (accent color):

| Kind | Accent |
|---|---|
| Button | `#34D399` (green) |
| Label | `#A3A7AF` (gray) |
| TextInput | `#60A5FA` (blue) |
| Slider | `#FB923C` (orange) |
| Checkbox | `#A78BFA` (purple) |

Fill is the accent darkened to ~15% (`r*3/20, g*3/20, b*3/20`).

---

## Codegen pipeline

```
src/codegen/
  egui_emitter.rs   — emits egui update() body
  state_emitter.rs  — emits AppState struct
  mod.rs            — re-exports
```

Both emitters are **pure functions**: `emit(tree: &UiTree) -> String`.
They are called every frame in `panels::code_preview::show` and their output is
displayed in the right panel. No caching; strings are regenerated each repaint.

### `egui_emitter::emit`

Wraps output in `egui::CentralPanel::default().show(ctx, |ui| { ... })`.
Iterates `tree.widgets` in order.

### Widget kind → generated code

| Kind | Generated line |
|---|---|
| `Button` | `if ui.button("label").clicked() { }` |
| `Label` | `ui.label(&self.binding);` |
| `TextInput` | `ui.text_edit_singleline(&mut self.binding);` |
| `Slider` | `ui.add(egui::Slider::new(&mut self.binding, min..=max).text("label"));` |
| `Checkbox` | `ui.checkbox(&mut self.binding, "label");` |

`Button` ignores `state_binding` (buttons have no state field).
All others use `state_binding` or emit `/* TODO: set binding */`.

### `state_emitter::emit`

Emits `struct AppState { ... }`.
Iterates `tree.widgets`; skips `Button` (no state).
Field types:

| Kind | Rust type |
|---|---|
| `Label` / `TextInput` | `String` |
| `Slider` | `f32` |
| `Checkbox` | `bool` |

Only widgets with a non-`None` `state_binding` appear in the struct.

---

## App structure (`src/app.rs`)

`RohKaiApp` owns:

| Field | Type | Purpose |
|---|---|---|
| `ui_tree` | `UiTree` | Single source of truth |
| `interaction` | `InteractionState` | Drag/resize transient state |
| `selected_id` | `Option<Uuid>` | Currently selected widget |
| `current_file` | `Option<PathBuf>` | Open file path |
| `saved_json` | `Option<String>` | Snapshot for dirty detection |
| `last_error` | `Option<String>` | Displayed in menu bar |
| `canvas_settings` | `CanvasSettings` | Snap toggle, step, canvas size |

Layout per frame:

```
TopBottomPanel("menu_bar")   — File menu, file name, dirty indicator, error
SidePanel::right("code_output") — Generated code (egui_emitter + state_emitter)
SidePanel::left("left_panel")   — Palette, Properties, canvas size + snap controls
CentralPanel                    — Canvas (interaction::handle)
```

Keyboard shortcuts: `Ctrl+N` new, `Ctrl+O` open, `Ctrl+S` save, `Ctrl+Shift+S` save-as,
`Delete` remove selected widget, `G` toggle grid snap, `Arrow keys` nudge.

---

## Platform layer

No custom platform layer. File dialogs use `rfd::FileDialog` (cross-platform).
`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` suppresses the
console window in release builds on Windows.

The application icon is **generated at runtime** in `main.rs`: `generate_icon()` rasterises
the Greek letters ρϗ into a 256×256 RGBA buffer using `ab_glyph` with the embedded
NotoSans font. No separate icon asset file.
