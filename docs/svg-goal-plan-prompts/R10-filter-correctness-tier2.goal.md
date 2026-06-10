```text
/goal /caveman ultra
Read AGENTS.md/CLAUDE.md. Work only in D:\dev\rohkai.
Run: pwsh -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1
Read svg-zero-dep skill. Read: docs/SVG_RENDERER_ROADMAP.md (Post-R8 lanes = truth),
src/canvas/svg_rasterizer.rs (R7 FilterGraph/FilterKind/FilterPrimitive apply, gaussian_blur, color_matrix,
composite_premultiplied_over, offscreen pipeline, parse_filter), src/canvas/svg_golden.rs,
src/codegen/export.rs (single-crate:: contract). Precondition: extend R7 filters — do not fork the pipeline.

Goal: implement R10 filter correctness + tier-2 primitives + blend modes end to end, in BOTH in-app and
export-embedded rasterizers. No new crates. Bounded + deterministic + diagnosed.

Before coding, derive + REPORT from code:
1. R7 filter buffer format (premultiplied sRGB) and where color-interpolation must convert to linearRGB
2. current filter region (whole canvas) and how to compute the precise filter region rect
3. which primitives are passthrough-identity today (tier 2/3) and which become real in tier 2
4. how mix-blend-mode would compose a group offscreen; tests+goldens per path

Required:
1. color-interpolation-filters: default linearRGB — convert source to linearRGB before the primitive graph
   and back to sRGB at the boundary (premultiplied-aware); honor `color-interpolation-filters: sRGB` to
   skip. Existing goldens that change must be justified in DEVLOG.
2. Filter region: compute from filterUnits/primitiveUnits + filter x/y/width/height (default obbox
   -10%..110%); clip primitive output + feFlood/feTile extents to it. Bounded buffer pixels with limit.*.
3. Tier-2 primitives: feComposite (over/in/out/atop/xor/arithmetic), feBlend (normal/multiply/screen/darken/
   lighten), feComponentTransfer (table/linear/gamma), feMorphology (dilate/erode, bounded radius). Real
   result buffers in the graph. Tier-3 (turbulence/displacement/convolution/lighting/image) stay
   passthrough + filter.unsupported_primitive.
4. mix-blend-mode on group layers: composite the isolated offscreen with the selected blend (reuse R4
   offscreen); diagnose unsupported modes.
5. both embedded sources std-only; single-crate:: export contract still passes; honor existing caps.

Tests:
1. goldens: linearRGB blur vs sRGB toggle, precise filter-region clip, feComposite arithmetic, feBlend
   multiply, feComponentTransfer gamma, feMorphology dilate, mix-blend-mode multiply group
2. determinism + fidelity; security: filter-region/morphology radius bombs -> bounded fail+diagnostic
3. export parity: ignored all-built-in exported-project cargo check still passes
4. invariant: every R10 feature rendering in-app also renders in exported copy

Verify (zero warnings): cargo fmt --check; cargo check; cargo test; cargo clippy --all-targets -- -D warnings;
pwsh scripts\validate-svg-import.ps1; pwsh scripts\check-text-encoding.ps1; cargo run (blur/blend SVG previews).

Docs: flip R10 tasks to [x] in SVG_RENDERER_ROADMAP.md + update Filters/Compositing gap rows; update
SVG_IMPORT.md + feature-evaluation; append CODE_COOP.md handoff + DEVLOG.md entry (next=R11).

Final report: path matrix, paths changed, tests+goldens, verification numbers, justified golden changes,
gaps only if excluded. Success: linearRGB filters + precise regions + tier-2 primitives + blend modes render
visibly and identically in-app and exported, bounded + diagnosed, tests green, zero warnings. Next: R11.
```
