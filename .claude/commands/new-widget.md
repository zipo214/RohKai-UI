# /new-widget

Scaffold a new widget type in RohKai. Work only in `D:\dev\rohkai`.

## Usage

`/new-widget <KindName>` — KindName must be PascalCase, e.g. `Spinner`.

## Steps (in order — all match arms are exhaustive, so cargo check will guide you)

### 1. `src/project/schema.rs`
Add the variant to the `WidgetKind` enum under the "Stage 5 additions" comment block.

### 2. `src/widgets/<snake_name>.rs` (new file)
Create `pub fn default_instance() -> WidgetInstance` with sensible `rect`, `props.label`, `min`, `max`, and `state_binding`.

### 3. `src/widgets/mod.rs`
- Add `pub mod <snake_name>;`
- Add arm to `default_for()` match
- Add variant to `ALL_KINDS` slice

### 4. `src/canvas/interaction.rs`
- `kind_accent()` — add `Color32::from_rgb(r, g, b)` not already used by another kind
- `kind_tag()` — 3–4 char lowercase tag
- `draw_widget()` match arm — use painter calls to give the widget a distinct visual

### 5. `src/codegen/egui_emitter.rs` — inside `emit_indexed()`
Add a match arm emitting the correct egui call string.

### 6. `src/codegen/state_emitter.rs` — inside `emit()`
Add a match arm with the Rust binding type, or group with `Button | Frame => continue` if stateless.

### 7. `src/codegen/export.rs` — inside `gen_app_rs()`
- Add to the `fields` filter_map: binding type + default expression (or `return None` if stateless)
- Add to the widget emit match arm

### 8. `cargo check` — zero warnings required

## Notes
- The palette panel auto-discovers all `ALL_KINDS` entries — no palette change needed
- Accent colours in use (avoid duplicating): teal, gray, blue, orange, violet, white, amber, rose, cyan
- `draw_widget` already handles the kind tag and selection stroke via the shared default arm — only add a custom arm if the widget needs a distinct visual shape
