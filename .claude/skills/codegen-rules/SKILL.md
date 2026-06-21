---
name: codegen-rules
description: Use when writing or reviewing code in src/codegen/. Rules for emitting
clean, idiomatic Rust strings from the UiTree without introducing bugs.
---

# Codegen Rules

## Emitter contract
- Input:  `&UiTree` (read-only borrow)
- Output: `String` (valid Rust source, ready to paste into an eframe app)
- Never panic — handle missing bindings with a fallback or return an error

## Formatting
- 4-space indent inside the panel closure
- Each widget on its own line
- Emit a `// widget {id}` comment above each block for traceability

## Widget emission order
Emit in `UiTree.widgets` index order (0 first). No sorting.

## State field naming
Use `snake_case` of `widget.state_binding`. If `None`, skip AppState entry.

## Error cases
- Widget needs a binding but `state_binding` is `None` → emit `/* TODO: set binding */`
- Unknown WidgetKind variant → compiler error is preferable to a silent skip (use exhaustive match)

## No hollow output
- A visible `WidgetKind` must not emit a comment-only placeholder, diagnostic
  label, or inert frame and call that feature complete.
- Live codegen and export may differ in surrounding app structure, but they
  must expose the same user-visible behavior for supported widget kinds.
- If a feature is design-time-only, document it as unavailable in export and
  add an explicit test for that limitation. Do not silently degrade.
- `Image` / SVG work must not depend on `resvg`, `usvg`, `tiny-skia`, or any
  substitute renderer crate unless the user explicitly reverses the policy.

## Output path derivation
Before changing codegen, derive the source tree shape and every output path:
top-level widget, Frame child, layout child, nested layout, custom/template,
parser/Lazare, live codegen, export, and generated-project compile behavior.
Representative string tests are not enough; add invariant tests over the
derived set or explicitly hide/diagnose unsupported paths before editing.

## Module boundary
`src/codegen/` is the only place allowed to build Rust syntax strings.
Panels may call `egui_emitter::emit(&tree)` but must not build strings themselves.
