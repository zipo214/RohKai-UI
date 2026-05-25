---
name: svg-zero-dep
description: Use when changing SVG import, SVG image preview, SVG rasterization, or generated/exported SVG behavior. Enforces zero-new-dependency, security, fidelity, and no-hollow-output rules.
---

# SVG Zero-Dependency Work

## First Principles

- Do not add `resvg`, `usvg`, `tiny-skia`, or any substitute SVG renderer/parser crate.
- Do not treat "pure Rust crate" as approval. SVG work is a zero-new-crate zone unless the user explicitly reverses this policy by name.
- Preserve the original `.svg` / `svg_source` as the source of truth whenever fidelity is partial.
- Never downgrade a visible SVG/Image path to a label, colored bounding box, comment, or inert frame and call the feature complete.

## Required Output Forms

Before calling SVG/Image work done, verify all applicable output forms:

- Canvas preview renders a real visual image form.
- Properties expose the relevant source/options without corrupting project data.
- Live codegen states the same supported behavior as the canvas.
- Export either renders the image equivalently or explicitly marks the feature unavailable with tests and docs.
- Save/load preserves `svg_source` and import metadata.
- Tests cover supported behavior and unsupported diagnostics.

## Security And Limits

SVG parsing/rendering must be bounded and deterministic:

- cap file bytes, tag count, attributes, nesting, path commands, placeholders, and data URI bytes;
- reject external references, script execution, custom entities, and unbounded recursion;
- never panic on malformed XML/path/style input;
- emit structured diagnostics for skipped or approximated features.

## Fidelity Discipline

If a feature is only approximated, say so in diagnostics and docs. Gradients,
patterns, masks, clips, filters, animation, text layout, and `<use>` expansion
are separate capabilities; a parser seeing them is not the same as supporting
them.

## Verification

Run:

```powershell
cargo fmt --check
cargo check
cargo test
cargo clippy -- -D warnings
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-dependency-policy.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1
```
