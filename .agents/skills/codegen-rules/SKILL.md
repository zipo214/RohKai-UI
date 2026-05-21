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

## Module boundary
`src/codegen/` is the only place allowed to build Rust syntax strings.
Panels may call `egui_emitter::emit(&tree)` but must not build strings themselves.
