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
src/settings.rs  — user-level Preferences, saved outside project files
```

`UiTree` is the **only** authoritative state for what is on the canvas.
All panels read from it; the canvas mutates it in-place through `tree.get_mut(id)`.

### Schema types

| Type | Purpose |
|---|---|
| `WidgetKind` | Enum for built-in widgets: Button, Label, TextInput, Slider, Checkbox, Frame, ComboBox, RadioButton, ProgressBar |
| `WidgetProps` | `label: String`, `min: f32`, `max: f32` (shared across kinds) |
| `Rect` | `{ x, y, w, h }` in canvas-local pixels; default `(20, 20, 120, 32)` |
| `WidgetInstance` | `{ id: Uuid, kind, rect, props, state_binding, children, import_metadata }` |
| `UiTree` | `{ widgets: Vec<WidgetInstance>, app_props }` |

`state_binding` is the Rust field name emitted into generated code. `None` → placeholder
comment `/* TODO: set binding */` in output.

### Persistence

`project::io::save` serializes `UiTree` to pretty JSON and returns the string.
`RohKaiApp` stores this string as `saved_json` to compute dirty state
(`is_dirty` = current serialization ≠ `saved_json`).

User Preferences are intentionally separate from project persistence. `settings.rs`
loads/saves `%APPDATA%\RohKai\settings.json` (or fallback path) for UI scale,
code font size, canvas text scale, and default snap step. These values must not
make `.rohkai.json` dirty.

---

## Canvas render pipeline

```
app.rs: RohKaiApp::update()
  └─ egui::CentralPanel → canvas::interaction::handle(ui, tree, state, selected, settings)
       │
       ├─ allocate_painter (Sense::click_and_drag)
       ├─ draw background + dotted canvas boundary
       ├─ draw configurable grid lines (1–256 px step, default 8 px)
       ├─ for each top-level widget in tree.widgets:
       │    canvas_rect(widget, origin, zoom) ← widget_instance::canvas_rect scales schema coords
       │    rect_filled  (kind_fill tint)
       │    rect_stroke  (accent if selected, gray otherwise)
       │    text label   (center)
       │    kind tag     (bottom-right, 9px muted)
       ├─ draw Frame children in a second pass using parent child IDs
       ├─ if selected: draw 8 resize handles (8×8 px squares)
       └─ process mouse/keyboard → mutate tree in-place
```

`canvas::widget_instance::canvas_rect` converts schema coords to screen coords by
applying the current canvas origin and zoom. Project data remains in schema space.

Visual style per kind (accent color):

| Kind | Accent |
|---|---|
| Button | `#34D399` (green) |
| Label | `#9CA3AF` (gray) |
| TextInput | `#60A5FA` (blue) |
| Slider | `#FB923C` (orange) |
| Checkbox | `#A78BFA` (purple) |
| Frame | `#C8C8C8` (light gray) |
| ComboBox | `#FBBF24` (amber) |
| RadioButton | `#FB7185` (rose) |
| ProgressBar | `#22D3EE` (cyan) |

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

`parser.rs` performs the reverse Lazare path for supported generated-code edits:
it parses widget markers and selected egui calls back into `UiTree`, reporting
diagnostics without mutating the canvas on invalid edits.

### `egui_emitter::emit`

Emits one `egui::Area::fixed_pos(...)` block per top-level widget so canvas
position is preserved in generated code. Frame children are emitted with marker
comments so supported edits can round-trip through the parser.

### Widget kind → generated code

| Kind | Generated line |
|---|---|
| `Button` | `ui.add_sized([w, h], egui::Button::new("label"))` |
| `Label` | `ui.label(&self.binding);` |
| `TextInput` | `ui.add_sized([w, h], egui::TextEdit::singleline(&mut self.binding))` |
| `Slider` | `ui.add_sized([w, h], egui::Slider::new(&mut self.binding, min..=max).text("label"))` |
| `Checkbox` | `ui.add_sized([w, h], egui::Checkbox::new(&mut self.binding, "label"))` |
| `Frame` | `egui::Frame::group(ui.style()).show(...)` |
| `ComboBox` | `egui::ComboBox::from_label("label")...` |
| `RadioButton` | `ui.radio_value(&mut self.binding, value, "label")` |
| `ProgressBar` | `ui.add_sized([w, h], egui::ProgressBar::new(self.binding))` |

`Button` ignores `state_binding` (buttons have no state field).
All others use `state_binding` or emit `/* TODO: set binding */`.

### `state_emitter::emit`

Emits `struct AppState { ... }`.
Iterates `tree.widgets`; skips `Button` (no state).
Field types:

| Kind | Rust type |
|---|---|
| `Label` / `TextInput` / `ComboBox` / `RadioButton` | `String` |
| `Slider` | `f32` |
| `Checkbox` | `bool` |
| `ProgressBar` | `f32` |

Only widgets with a non-`None` `state_binding` appear in the struct.

---

## App structure (`src/app.rs`)

`RohKaiApp` owns:

| Field | Type | Purpose |
|---|---|---|
| `ui_tree` | `UiTree` | Single source of truth |
| `interaction` | `InteractionState` | Drag/resize transient state |
| `selected` | `Vec<Uuid>` | Current selection, ordered with last as primary/key object |
| `current_file` | `Option<PathBuf>` | Open file path |
| `saved_json` | `Option<String>` | Snapshot for dirty detection |
| `last_error` | `Option<String>` | Displayed in menu bar |
| `canvas_settings` | `CanvasSettings` | Snap toggle, snap step, pan, zoom |
| `user_settings` | `UserSettings` | Persisted user Preferences: UI scale, code font size, canvas text scale |
| `preferences_draft` | `UserSettings` | Modal draft edited by File -> Preferences before OK/Apply |

Layout per frame:

```
TopBottomPanel("menu_bar")   — File menu, Preferences, file name, dirty indicator, error
SidePanel::right("code_output") — Generated code (egui_emitter + state_emitter)
SidePanel::left("left_panel")   — Palette, Properties, Templates
TopBottomPanel("status_bar")    — Canvas size + grid snap controls
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
