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
| `WidgetKind` | Enum: Button, Label, TextInput, Slider, Checkbox, Frame, ComboBox, RadioButton, ProgressBar, Image, Custom(String) |
| `WidgetProps` | Per-widget content and behaviour knobs (see table below) |
| `Rect` | `{ x, y, w, h }` in canvas-local pixels; default `(20, 20, 120, 32)` |
| `WidgetInstance` | Full widget node (see table below) |
| `UiTree` | `{ widgets: Vec<WidgetInstance>, app_props: AppProps }` |

`Custom(String)` holds the descriptor id (e.g. `"ply.button"`).
`Image` holds SVG source in `WidgetInstance.svg_source`.

#### WidgetProps fields

| Field | Type | Used by |
|---|---|---|
| `label` | `String` | all |
| `min`, `max`, `default_value` | `f32` | Slider, ProgressBar |
| `options` | `Vec<String>` | ComboBox |
| `step` | `Option<f32>` | Slider |
| `show_value` | `bool` | Slider |
| `orientation` | `Orientation` | Slider |
| `placeholder` | `String` | TextInput |
| `password_mode` | `bool` | TextInput |
| `max_length` | `Option<usize>` | TextInput |
| `radio_value` | `String` | RadioButton |
| `group_binding` | `String` | RadioButton |
| `show_percentage` | `bool` | ProgressBar |
| `animated` | `bool` | ProgressBar |
| `inner_margin` | `f32` | Frame |
| `stroke_color` | `Option<[u8; 3]>` | Frame |
| `stroke_width` | `f32` | Frame |

#### WidgetInstance fields (beyond id/kind/rect/props)

| Field | Type | Purpose |
|---|---|---|
| `state_binding` | `Option<String>` | Rust AppState field name; `None` → placeholder comment |
| `children` | `Vec<Uuid>` | Frame child widget IDs |
| `import_metadata` | `Option<SvgImportMetadata>` | SVG import provenance |
| `tooltip` | `Option<String>` | `.on_hover_text(...)` codegen |
| `enabled` | `Option<bool>` | `ui.set_enabled(false)` codegen |
| `fg_color` | `Option<[u8; 3]>` | Foreground/text color |
| `bg_color` | `Option<[u8; 3]>` | Background/fill color override |
| `corner_radius` | `Option<f32>` | Widget corner rounding |
| `font_size` | `Option<f32>` | Override font size (pt) |
| `text_align` | `Option<TextAlign>` | Left / Center / Right |
| `label_binding` | `Option<String>` | Bound-mode label from AppState |
| `custom_props` | `Vec<CustomProp>` | User-added state fields |
| `on_click` | `String` | Click handler name (Button) |
| `on_change` | `String` | Change handler name (interactive widgets) |
| `svg_source` | `Option<String>` | Raw SVG text for Image widgets |
| `expand_svg_inline` | `bool` | Embed full SVG in live code panel |
| `descriptor_*` | various | Snapshotted `.rkwd` metadata for Custom widgets |

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
  egui_emitter.rs      — emits egui update() body (live preview)
  export.rs            — emits complete standalone Rust project
  state_emitter.rs     — emits AppState struct
  field_collector.rs   — shared AppState field collection for live preview,
                         export, and descriptor state fields (single source of truth)
  parser.rs            — Lazare reverse path: parses edited code back into UiTree
  kind_table.rs        — field types and widget metadata for codegen (binding types,
                         event applicability, state field rules per WidgetKind)
  widget_descriptor.rs — .rkwd descriptor types, loader, template engine, validation
  widget_bundle.rs     — .rkwb bundle (multi-descriptor JSON envelope)
  rust.rs              — Rust string/binding helpers (string_literal, field_binding)
  mod.rs               — re-exports
```

Both emitters are **pure functions**: `emit(tree: &UiTree) -> String`.
They are called every frame in `panels::code_preview::show` and their output is
displayed in the right panel. No caching; strings are regenerated each repaint.

`field_collector.rs` is the single source of truth for which widgets contribute
AppState fields, their Rust types, and their default expressions. Both
`state_emitter` and `export` call it instead of duplicating logic.

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

`RohKaiApp` is decomposed into focused sub-structs:

| Sub-struct | Key fields | Purpose |
|---|---|---|
| `ProjectState` | `ui_tree`, `current_file`, `saved_json` | Persistent document — single source of truth + file path + dirty snapshot |
| `SessionState` | `interaction`, `selected`, `canvas_settings`, `dragging_guide`, `hovered_guide`, `lock_aspect_ratio` | Per-session canvas interaction and view state (not persisted) |
| `MessageState` | `last_error`, `export_message`, `template_message` | One-frame status messages for the status bar |
| `PreferencesState` | `user_settings`, `draft`, `settings_path` | Live + draft user preferences, persistence path |
| `CodePanelState` | `buffer`, `status`, `last_generated`, `split_ratio` | Lazare code panel — editable buffer and live/pending/error status |
| `DescriptorState` | `widgets`, `errors`, `editor`, `builder` | Loaded `.rkwd` descriptors, load errors, in-app editor state, beginner builder state |

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

## SVG subsystem

```
src/
  svg_import.rs         — SVG → WidgetInstance template importer (zero new dependencies)
  svg_core.rs           — shared SVG microsyntax module: CSS color parsing, numeric-list
                          parsing (e.g. viewBox/points/d tokens), affine transform
                          decomposition, path command tokenization. Used by both
                          svg_import.rs and canvas/svg_rasterizer.rs to avoid duplication.
  canvas/
    svg_rasterizer.rs   — zero-dependency SVG rasterizer for canvas Image preview;
                          includes internal SvgSceneItem flattening boundary with
                          accumulated transforms and resolved inherited style.
                          Supported subset — not full resvg/usvg equivalence.
```

SVG work is a **zero-new-crate zone**: no `resvg`, `usvg`, `tiny-skia`, or substitute
renderer chains. All SVG behavior is implemented in RohKai source files.

---

## Platform layer

No custom platform layer. File dialogs use `rfd::FileDialog` (cross-platform).
`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` suppresses the
console window in release builds on Windows.

The application icon is **generated at runtime** in `main.rs`: `generate_icon()` rasterises
the Greek letters ρϗ into a 256×256 RGBA buffer using `ab_glyph` with the embedded
NotoSans font. No separate icon asset file.

### Planned: `src/platform/` (Stage 15 prep)

`src/platform/thread_pool.rs` is reserved for Stage 15's own renderer scheduler.
Until then, parallel work uses `rayon` (approved dependency). Do not create
`src/platform/` before Stage 15 scope is active.
