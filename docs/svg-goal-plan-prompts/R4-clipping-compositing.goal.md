```text
/goal /caveman ultra
Read AGENTS.md/CLAUDE.md. Work only in D:\dev\rohkai.
Run: pwsh -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1
Read svg-zero-dep skill. Read: docs/SVG_RENDERER_ROADMAP.md (R4=truth),
src/canvas/svg_rasterizer.rs (SvgScene/DisplayList IR, Style cascade, PaintSampler,
rasterize_coverage + fill/stroke coverage, viewport/preserveAspectRatio, expand_use),
src/svg_core.rs (Affine2D::inverse, viewBox), src/canvas/svg_golden.rs,
src/codegen/export.rs (embedded copy + single-crate:: contract test).

Goal: implement R4 end to end — clipPath clipping, nested <svg> overflow clipping,
premultiplied-alpha internal buffer, isolated group opacity/compositing. Every feature
renders visibly (not just diagnosed) in BOTH in-app and export-embedded rasterizers, or is
removed from any UI/diagnostic implying support before coding.

Before coding, derive + REPORT from code:
1. scene/IR types + coverage/compositing fns every painted pixel flows through
2. pixel-painting paths: in-app canvas preview AND export.rs embedded copy (both change identically)
3. how clip-path/overflow/opacity/isolation reach or are dropped by Style + DisplayList
4. how clipPath url(#id) resolves vs existing defs/use/symbol+paint resolver (reuse it, don't fork)
5. which clip/overflow/group-opacity diagnostics flip from diagnosed->rendered
6. tests+goldens per path
If any render/export path won't be fixed now, STOP and report. No in-app-only support.

Required:
1. clip stack in display-list traversal: clip = coverage mask (reuse rasterize_*_coverage),
   intersected (min) with active mask before compositing each primitive; clip-path resolves from
   presentation attr OR CSS via Style cascade; clip-rule nonzero/evenodd; nested transforms on clip
   children; clipPathUnits userSpaceOnUse+objectBoundingBox, diagnose rest
2. nested <svg> overflow clipping to viewport rect (the R1 gap), via existing viewport mapping
3. premultiplied-alpha internal buffer; output straight RGBA ColorImage unchanged at boundary;
   halo-free deterministic edges; keep goldens stable unless provably more correct (justify in devlog)
4. group opacity + isolation: <g opacity> (and group-forming elements) render to isolated offscreen, composited once at group
   opacity (overlapping children don't double-darken); bound offscreen count/size by existing caps;
   emit limit.* on truncation, never fail silently
5. honor all existing caps; add clip/offscreen memory caps; no new deps
6. both embedded sources std-only; single-crate:: export contract still passes

Tests:
1. goldens: rect clip, path clip nonzero+evenodd, transformed clip, obbox clip, nested-svg overflow,
   semi-transparent overlapping group (no double-darken), isolated-vs-flat diff
2. determinism + fidelity-score (clipped/composited = rendered not approximated)
3. security: clip cycle, missing-id clip, clip depth limit, oversized offscreen -> bounded fail+diagnostic
4. export parity: ignored all-built-in exported-project cargo check still passes
5. invariant: every R4 feature rendering in-app also renders in exported copy

Verify (zero warnings): cargo fmt --check; cargo check; cargo test; cargo clippy -- -D warnings;
pwsh scripts\validate-svg-import.ps1; pwsh scripts\check-text-encoding.ps1; cargo run (clip Image previews clipped).

Docs: flip only truly-done R4 tasks to [x] in SVG_RENDERER_ROADMAP.md + update its limits/gap rows;
update SVG_IMPORT.md + feature-evaluation/svg-import-renderer.md; append CODE_COOP.md handoff + DEVLOG.md entry (next=R5).

Final report: path matrix, paths changed, tests+goldens added, verification numbers, gaps only if excluded before editing.
Success: clipPath + nested overflow + premultiplied compositing + isolated group opacity render visibly and
identically in-app and exported, tests/determinism/security/export-parity green, zero warnings. Next: R5.
```
