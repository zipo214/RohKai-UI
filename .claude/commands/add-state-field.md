# /add-state-field

Add a persistent designer-level state field to `RohKaiApp`.

## Usage
    /add-state-field zoom_level: f32 = 1.0
    /add-state-field show_grid: bool

## Steps
1. Add the field to `RohKaiApp` in `src/app.rs`
2. If the type doesn't implement `Default`, add a manual `Default` impl
3. If the field maps to a widget state binding, update `state_emitter.rs` output
4. If this is a persistent concept (not ephemeral per-session), update CLAUDE.md
5. Run `cargo clippy -- -D warnings`

## Note
Designer-level fields (zoom, grid, selected panel) live in `RohKaiApp`.
Widget-level state (what the generated app will have) lives in `UiTree` → codegen.
Don't conflate the two.
