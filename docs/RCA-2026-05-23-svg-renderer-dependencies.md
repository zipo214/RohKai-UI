# RCA - SVG Renderer Dependencies Bypassed Zero-New-Dependency Rule

## Incident

Local Stage 7 SVG Image work added direct dependencies in `Cargo.toml`:

- `resvg`
- `usvg`
- `tiny-skia`

This violated the active SVG import direction: keep importer work zero-new-dependency and preserve RohKai's own source-of-truth importer model.

Note: `tiny-skia` can still appear as an existing target-specific transitive of
the eframe/winit stack through `sctk-adwaita`. That is not SVG importer work.
The breach was adding SVG-renderer crates directly and using them in RohKai's
SVG import/canvas path.

## How It Bypassed Instructions

- The implementation treated "pure Rust, no C deps" as sufficient, but the stricter requirement was "no new crates / no new transitive dependencies" for SVG importer work.
- The feature was implemented as rasterization because that matched the desired visual output, but it skipped the repo's established SVG importer contract: editable/source-backed placeholders, original `.svg` preserved, diagnostics explicit.
- No automated dependency-policy check existed in preflight or validation, so `cargo check`, tests, and clippy could pass while the dependency policy was still broken.
- Codegen/export initially degraded `WidgetKind::Image` to comments, which made the feature partly hollow outside the canvas.

## Fix

- Removed direct `resvg`, `usvg`, and `tiny-skia` dependency additions.
- Replaced dependency-backed rasterization with novel in-repo Rust behavior:
  - `WidgetKind::Image` stores raw SVG source.
  - Canvas preview reuses RohKai's hardened zero-dependency SVG importer.
  - Imported preview geometry is fitted and painted inside the Image widget bounds.
  - Original SVG remains the source of truth.
- Replaced comment-only Image codegen/export output with visible egui source-backed preview frames.
- Added tests verifying Image mode output form:
  - one `WidgetKind::Image`
  - source preserved
  - dimensions derived from `width`/`height` or `viewBox`
  - deterministic ID
  - live codegen/export emit visible preview UI, not comments
- Added `scripts/check-dependency-policy.ps1`.
- Wired the dependency check into `scripts/validate-svg-import.ps1`.

## Prevention

- Run `scripts/check-dependency-policy.ps1` before accepting SVG importer changes.
- Keep zero-new-dependency requirements explicit in plans and test plans when SVG import work is involved.
- Treat "pure Rust crate" and "allowed dependency" as different decisions.
- Do not expose a feature path unless canvas, properties, code panel, export, tests, and docs all have a real output form.

## 2026-05-23 Follow-Up Audit

The later zero-dependency rasterizer pass is real code, but it is not yet a
practically indistinguishable replacement for the previous `resvg` / `usvg` /
`tiny-skia` behavior.

Observed gaps:

- `src/canvas/svg_rasterizer.rs` skips SVG text rendering.
- `defs`, `symbol`, `clipPath`, and `mask` need stricter non-rendering /
  application semantics; parsing a container is not equivalent support.
- Gradients, patterns, masks, clips, filters, animation, and external refs are
  still not fully rendered.
- Live codegen and export still emit a gray `egui::Frame` placeholder for
  `WidgetKind::Image`, so the output form is not equivalent outside the canvas.
- `cargo fmt --check` currently reports formatting changes in the rasterizer and
  Image codegen helpers, so the "zero warnings / clean" claim is incomplete until
  formatting is applied and the suite is rerun.

Resolved since this audit:

- Live codegen now emits SVG preview helper calls instead of a gray Frame.
- Export now embeds the RohKai-owned zero-dependency rasterizer module when Image
  widgets exist and renders preserved `svg_source` into egui textures.
- Rasterizer guardrails now reject unsafe raw `svg_source`, cap raster/input/path
  sizes, respect `display:none` / hidden visibility, and avoid rendering
  paint-server URLs as black boxes.
- Formatting, check, tests, and clippy passed after these changes.

Remedy direction:

1. Unify SVG import and raster preview around one bounded parser/IR so importer,
   canvas preview, diagnostics, and export cannot drift.
2. Add a real no-dependency SVG display list: nodes, styles, paint servers,
   transforms, paths, clips/masks, text runs, provenance, and diagnostics.
3. Add a no-dependency raster backend: bounded pixel buffer, alpha compositing,
   fill rules, anti-aliasing, stroke joins/caps/dashes, gradients, patterns,
   masks/clips, and filter subset only where implemented.
4. Fix `WidgetKind::Image` output parity: canvas, live codegen, export, tests,
   save/load, and docs must all either render equivalent output or explicitly
   document and test an unavailable path.
5. Add golden/fixture tests for each supported SVG feature and negative tests for
   unsupported/security-sensitive features. Tests must verify output form, not
   merely that code compiles.
