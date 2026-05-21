# /new-widget

Scaffold a new widget type in RohKai. Work only in `D:\dev\rohkai`.

## Usage

`/new-widget <KindName>` where `KindName` is PascalCase, for example `Spinner`.

## Steps

1. `src/project/schema.rs`: add the variant to `WidgetKind`.
2. `src/widgets/<snake_name>.rs`: create `pub fn default_instance() -> WidgetInstance` with sensible `rect`, `props.label`, `min`, `max`, and `state_binding`.
3. `src/widgets/mod.rs`: add `pub mod <snake_name>;`, add an arm to `default_for()`, and add the variant to `ALL_KINDS`.
4. `src/canvas/interaction.rs`: add any needed accent, tag, and drawing behavior.
5. `src/codegen/egui_emitter.rs`: emit the correct live preview egui call.
6. `src/codegen/state_emitter.rs`: add the binding type, or group with stateless widgets.
7. `src/codegen/export.rs`: add generated app state fields and widget emission.
8. Run `cargo check` and resolve all warnings.

The palette panel discovers `ALL_KINDS`; do not add a separate palette list unless the app architecture changes.
