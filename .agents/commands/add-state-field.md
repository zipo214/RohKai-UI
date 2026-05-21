# /add-state-field

Add a persistent designer-level state field to `RohKaiApp`.

## Usage

`/add-state-field zoom_level: f32 = 1.0`
`/add-state-field show_grid: bool`

## Steps

1. Add the field to `RohKaiApp` in `src/app.rs`.
2. If the type does not implement `Default`, add a manual `Default` impl.
3. If the field maps to a widget state binding, update `state_emitter.rs`.
4. If this is a persistent concept, update `AGENTS.md`.
5. Run `cargo clippy -- -D warnings`.

Designer-level fields live in `RohKaiApp`. Widget-level state lives in `UiTree` and codegen.
Do not conflate the two.
