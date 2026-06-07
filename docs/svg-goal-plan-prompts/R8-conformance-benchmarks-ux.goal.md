```text
/goal /caveman ultra
Read AGENTS.md/CLAUDE.md. Work only in D:\dev\rohkai.
Run: pwsh -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1
Read svg-zero-dep skill. Read: docs/SVG_RENDERER_ROADMAP.md (R8=truth),
src/canvas/svg_rasterizer.rs (SvgRenderReport/warning/unsupported/fidelity, rasterize_with_report),
src/svg_import.rs (SvgImportOutput report, provenance/source spans), src/app.rs + src/panels/properties.rs
(WidgetKind::Image / show_image at ~L828, svg_source), src/canvas/svg_golden.rs.
Note: SvgRenderReport is NOT yet surfaced in app UI — that gap is the core of R8.

Goal: implement R8 conformance + benchmarks + editor UX end to end. Make renderer claims measurable and
make the existing report/provenance VISIBLE in RohKai. Reference/oracle tools stay developer/CI-only, never
runtime deps. No new crates.

Before coding, derive + REPORT from code:
1. every field already in SvgRenderReport + SvgImportOutput report (fidelity, warnings, unsupported, node ids/spans)
2. where SVG Image widgets render in app/panels and where a report panel + source viewer would attach
3. which supported features lack a golden today; enumerate the golden-corpus gaps by phase (R0-R7)
4. how to run a dev-only reference comparison + benchmark without adding runtime deps
5. what "rendered vs editable approximation" toggle needs from existing state
6. tests per path

If report-UI or source-viewer won't be done now, STOP and report. Diagnostics existing in data but invisible
to the user is the exact R8 failure to avoid.

Required:
1. SVG report UI panel: fidelity score, warnings, unsupported features, per-node source ids/spans, wired to
   the selected SVG Image widget (reuse SvgRenderReport/SvgImportOutput; no new report computation)
2. source viewer + "rendered vs editable approximation" toggle for SVG Image widgets
3. golden-image corpus: fill the per-feature gaps across geometry/paint/clip/mask/filter/text/image/malicious
   (ASCII golden harness in svg_golden.rs); every supported feature has a visual test
4. renderer benchmark suite (#[ignore] if slow): parse time, scene build, raster time, peak allocations;
   document budgets like the existing 512px smoke
5. dev-only reference comparison harness (browser/librsvg/resvg as optional external oracles or CI artifacts,
   gated/ignored; NEVER a runtime/Cargo dependency)
6. both embedded sources std-only; single-crate:: export contract still passes

Tests:
1. report-UI logic test: report fields map to displayed rows for representative SVGs
2. source-viewer/toggle state test
3. golden-corpus determinism across the expanded set
4. benchmark smoke runs (ignored heavy) + an always-run fast matrix smoke
5. export parity: ignored all-built-in exported-project cargo check still passes

Verify (zero warnings): cargo fmt --check; cargo check; cargo test; cargo clippy -- -D warnings;
pwsh scripts\validate-svg-import.ps1; pwsh scripts\check-text-encoding.ps1; cargo run (select SVG Image -> report panel + source viewer + toggle visible).

Docs: flip only truly-done R8 tasks to [x] in SVG_RENDERER_ROADMAP.md + reconcile its gap matrix against tests;
update SVG_IMPORT.md + feature-evaluation; append CODE_COOP.md handoff + DEVLOG.md entry (SVG roadmap R0-R8 closed).

Final report: report-UI/path matrix, corpus + benchmarks added, paths changed, verification numbers, remaining
deferred lanes (animation/scripting/external loads stay out of scope), gaps only if excluded before editing.
Success: every supported feature has a visual test, report+provenance are visible in-app with a rendered/editable
toggle, benchmarks+dev-only oracle exist with no runtime deps, claims match tests, zero warnings. SVG roadmap done.
```
