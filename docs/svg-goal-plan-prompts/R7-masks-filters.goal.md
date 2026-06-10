```text
/goal /caveman ultra
Read AGENTS.md/CLAUDE.md. Work only in D:\dev\rohkai.
Run: pwsh -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1
Read svg-zero-dep skill. Read: docs/SVG_RENDERER_ROADMAP.md (R7=truth),
src/canvas/svg_rasterizer.rs (R4 offscreen/premultiplied buffer, clip stack, DisplayList, PaintSampler, caps),
src/svg_core.rs (Affine2D, lengths/units), src/canvas/svg_golden.rs,
src/codegen/export.rs (embedded copy + single-crate:: contract).
Precondition: R4 complete — masks/filters MUST reuse the R4 offscreen + premultiplied pipeline, not a new one.

Goal: implement R7 masks + filters tier 1 end to end, rendering visibly in BOTH in-app and export-embedded
rasterizers, on top of the R4 offscreen pipeline. Bound filter regions hard (no memory bombs). Unsupported
primitives show partial-output diagnostics, never silent drop.

Before coding, derive + REPORT from code:
1. R4 offscreen/premultiplied buffer + clip API masks/filters will reuse
2. pixel paths: in-app preview AND export.rs embedded copy (both change identically)
3. how mask/filter url(#id) resolves vs existing defs/use/clip/paint resolver (reuse it, don't fork)
4. filter region computation + buffer precision/caps; which primitives are tier 1 vs deferred
5. which mask/filter diagnostics flip from diagnosed->rendered vs stay unsupported
6. tests+goldens per path

If any tier-1 primitive or mask mode won't be done now, STOP and report. Do not call a partial set "filters done".

Required:
1. masks: maskUnits + maskContentUnits, alpha AND luminance modes, offscreen mask buffer intersected with
   coverage before composite (reuse R4); nested transforms; bounded buffer size
2. filter region: compute from filterUnits/primitiveUnits + bounds; clamp buffer pixels via caps
3. filters tier 1: feGaussianBlur, feOffset, feFlood, feMerge, feColorMatrix, feDropShadow — primitive graph
   with result buffers, executed in premultiplied space, output straight RGBA at boundary
4. unsupported primitives (tier 2/3: feComposite/feBlend/feTile/feTurbulence/etc) -> partial output + explicit diagnostic
5. honor existing caps; add filter/mask buffer memory caps; emit limit.* on truncation; never panic
6. both embedded sources std-only; single-crate:: export contract still passes

Tests:
1. goldens: luminance mask, alpha mask, feGaussianBlur, feOffset, feFlood+feMerge, feColorMatrix, feDropShadow
2. determinism + fidelity-score (rendered vs approximated/unsupported)
3. security: filter region bomb -> bounded fail+diagnostic; mask cycle; missing-id mask/filter
4. export parity: ignored all-built-in exported-project cargo check still passes
5. invariant: every R7 feature rendering in-app also renders in exported copy

Verify (zero warnings): cargo fmt --check; cargo check; cargo test; cargo clippy -- -D warnings;
pwsh scripts\validate-svg-import.ps1; pwsh scripts\check-text-encoding.ps1; cargo run (masked/blurred SVG previews correctly).

Docs: flip only truly-done R7 tasks to [x] in SVG_RENDERER_ROADMAP.md + update limits/gap rows; update
SVG_IMPORT.md + feature-evaluation; append CODE_COOP.md handoff + DEVLOG.md entry (next=R8).

Final report: path matrix, paths changed, tests+goldens, verification numbers, gaps only if excluded before editing.
Success: alpha+luminance masks and filters tier 1 render visibly and identically in-app and exported on the R4
pipeline, region/buffer caps prevent bombs, unsupported primitives diagnosed, tests green, zero warnings. Next: R8.
```
