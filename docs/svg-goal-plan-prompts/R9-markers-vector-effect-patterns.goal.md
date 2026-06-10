```text
/goal /caveman ultra
Read AGENTS.md/CLAUDE.md. Work only in D:\dev\rohkai.
Run: pwsh -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1
Read svg-zero-dep skill. Read: docs/SVG_RENDERER_ROADMAP.md (Post-R8 lanes = truth),
src/canvas/svg_rasterizer.rs (DisplayList build/execute, stroke geometry/flatten_path_data, ClipDef/offscreen
from R4/R7, PaintServerTable, render_shape, unsupported_tag_feature, is_container_tag), src/svg_core.rs
(Affine2D, viewbox_transform), src/canvas/svg_golden.rs, src/codegen/export.rs (single-crate:: contract).
Precondition: reuse R1 stroke flattening, R3 paint, R4 clip/offscreen — do not fork them.
STATUS: vector-effect non-scaling-stroke is DONE + committed (61f3d66: VectorEffect enum,
effective_device_stroke, vector_effect.unsupported diag, golden r9_non_scaling_stroke + 2 tests).
Implement ONLY markers + pattern tiling; do not redo vector-effect.

Goal: implement R9 markers + pattern tiling end to end (vector-effect already done), rendering visibly
in BOTH in-app and export-embedded rasterizers. No new crates. Bounded + deterministic + diagnosed.

Before coding, derive + REPORT from code:
1. where flattened path vertices/tangents are available (for marker placement) and how strokes lower
2. how a referenced <marker>/<pattern> resolves vs existing defs/use/clip/paint resolver (reuse it)
3. how vector-effect would bypass the CTM scale on stroke width
4. which diagnostics flip from unsupported->rendered (marker, pattern); tests+goldens per path

Required:
1. Markers: marker-start/mid/end resolve a <marker> def; place its content at path vertices with orient
   (angle | auto | auto-start-reverse from segment tangents), markerUnits (strokeWidth|userSpaceOnUse),
   marker viewBox/refX/refY/markerWidth/markerHeight + overflow clip. Bounded marker count (cap +
   limit.* diagnostic). Render through the display list.
2. [DONE in 61f3d66] vector-effect: non-scaling-stroke renders constant device-space width + diagnoses
   other values. Skip — retained here only for lane provenance.
3. Patterns: real tiling via the R7 offscreen — patternUnits/patternContentUnits/viewBox/patternTransform,
   nested content rendered once to a tile then repeated across the fill bbox; bounded tile count/pixels with
   limit.* on truncation. Flip pattern from diagnosed->rendered; keep diagnostics for unsupported sub-attrs.
4. Parser: retain <marker>/<pattern> (already in unsupported_tag_feature) + ensure container children parse
   (is_container_tag); skip the defs in scene build like clipPath/mask/filter.
5. both embedded sources std-only; single-crate:: export contract still passes; honor existing caps.

Tests:
1. goldens: arrowhead marker on a line (start/mid/end), auto-orient marker, non-scaling-stroke under scale,
   tiled pattern fill (userSpaceOnUse + objectBoundingBox)
2. determinism + fidelity (rendered not approximated); security: marker/pattern cycle, missing id, depth/tile
   caps -> bounded fail+diagnostic
3. export parity: ignored all-built-in exported-project cargo check still passes
4. invariant: every R9 feature rendering in-app also renders in exported copy

Verify (zero warnings): cargo fmt --check; cargo check; cargo test; cargo clippy --all-targets -- -D warnings;
pwsh scripts\validate-svg-import.ps1; pwsh scripts\check-text-encoding.ps1; cargo run (marker+pattern SVG previews).

Docs: flip R9 tasks to [x] in SVG_RENDERER_ROADMAP.md + update gap matrix (Markers/Patterns/vector-effect
rows); update SVG_IMPORT.md + feature-evaluation; append CODE_COOP.md handoff + DEVLOG.md entry (next=R10).

Final report: path matrix, paths changed, tests+goldens, verification numbers, gaps only if excluded.
Success: markers, non-scaling-stroke, and pattern tiling render visibly and identically in-app and exported,
bounded + diagnosed, tests green, zero warnings, no new deps. Next: R10.
```
