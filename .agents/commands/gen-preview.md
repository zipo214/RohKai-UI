# /gen-preview

Regenerate and review the code preview output for the current UiTree state.
Use when the code panel looks stale or after schema/codegen edits.

## Steps

1. Read `src/codegen/egui_emitter.rs` and `src/codegen/state_emitter.rs`.
2. Verify every `WidgetKind` variant has the expected match behavior in both emitters.
3. If any arm is missing, add it using `.agents/skills/codegen-rules/SKILL.md`.
4. Trace through `emit()` for a representative `UiTree` and confirm output is valid Rust.
5. Run `cargo clippy --quiet` to verify no regressions.
