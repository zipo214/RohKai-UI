```text
/goal /caveman ultra
Read AGENTS.md/CLAUDE.md. Work only in D:\dev\rohkai.
Run: pwsh -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1
Read svg-zero-dep skill + docs/TEXT_IMPORT_PLAN.md. Read: docs/SVG_RENDERER_ROADMAP.md (Post-R8 lanes = truth),
src/svg_import.rs (R6 TextChunk model), src/canvas/svg_rasterizer.rs (DisplayList, render path, R4 clip/
offscreen, coverage_scan, UnsupportedText), src/canvas/svg_golden.rs, src/codegen/export.rs (single-crate::).
Precondition: editable-first text (R6) stays intact; this adds an OPTIONAL raster snapshot path. No new crates,
no external font/shaping crate.

Goal: implement R11 raster text + textPath as an opt-in vector-outline snapshot, rendering in BOTH in-app and
export-embedded rasterizers. Deterministic, bounded, diagnosed. Editable import unchanged.

Before coding, derive + REPORT from code:
1. how <text>/<tspan> currently reach the rasterizer (UnsupportedText skip) and the R6 chunk model
2. zero-dep glyph source options: embed a compact public-domain stroked/Hershey-style vector font as data
   (no font file loading, no shaping crate); document the chosen glyph set + its coverage/limits
3. how a glyph outline/stroke path lowers into the existing coverage/stroke pipeline + R4 clip
4. textPath: sampling a path for glyph placement; tests+goldens per path

Required:
1. Bundled zero-dep vector glyph set (e.g. Hershey simplex, public domain) embedded as in-repo data; ASCII +
   common Latin coverage. Unknown glyphs -> a tofu box + diagnostic. Document coverage honestly.
2. Raster text render: lay out chunk runs (x/y/dx/dy, anchor, font-size scale) and stroke/fill each glyph
   path through the existing coverage pipeline + R4 clip/opacity; deterministic placeholder metrics from the
   glyph set. Flip <text> from UnsupportedText -> rendered when snapshot mode is on.
3. textPath: place glyphs along a referenced path by arc-length sampling; bounded glyph count; diagnose
   bidi/complex-shaping as unsupported (explicit warning), never silently wrong.
4. Mode: keep editable component import default; raster snapshot is opt-in (per-image/source-backed) and
   clearly the visual-fidelity fallback, source preserved. Document the editable-vs-snapshot choice.
5. both embedded sources std-only; single-crate:: export contract still passes; honor caps (glyph/seg limits).

Tests:
1. goldens: a short word rendered (snapshot), anchored text, textPath along a curve
2. determinism + fidelity; unknown-glyph tofu + diagnostic; bidi/shaping deferred-diagnostic test
3. editable-first regression: component import still produces grouped labels (R6 unchanged)
4. export parity: ignored all-built-in exported-project cargo check still passes

Verify (zero warnings): cargo fmt --check; cargo check; cargo test; cargo clippy --all-targets -- -D warnings;
pwsh scripts\validate-svg-import.ps1; pwsh scripts\check-text-encoding.ps1; cargo run (text snapshot previews).

Docs: flip R11 tasks to [x] in SVG_RENDERER_ROADMAP.md + update Text gap row; update TEXT_IMPORT_PLAN.md
(phase 3 status), SVG_IMPORT.md, feature-evaluation; append CODE_COOP.md handoff + DEVLOG.md entry (next=R12).

Final report: glyph-set choice + coverage limits, path matrix, tests+goldens, verification numbers, gaps only
if excluded. Success: opt-in raster text + textPath render via a bundled zero-dep vector font, editable import
intact, bounded + diagnosed, tests green, zero warnings. Next: R12.
```
