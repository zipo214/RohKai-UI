```text
/goal /caveman ultra
Read AGENTS.md/CLAUDE.md. Work only in D:\dev\rohkai.
Run: pwsh -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1
Read svg-zero-dep skill. Read: docs/SVG_RENDERER_ROADMAP.md (R6=truth),
docs/TEXT_IMPORT_PLAN.md (phase 1 = authoritative model: TextRun/TextChunk/TextLayout/TextProvenance),
src/svg_import.rs (text/tspan import -> Label, Style text fields, provenance/warnings),
src/canvas/svg_rasterizer.rs (DisplayList, Style cascade, R4 compositing/clip),
src/codegen/export.rs (embedded copy + single-crate:: contract).

Goal: implement R6 text import phase 1 (and optional visual snapshot) end to end. Text stays editable
FIRST; original SVG remains source of truth. Defer any owned shaping engine. Build the
docs/TEXT_IMPORT_PLAN.md phase-1 model and surface it across importer output paths + diagnostics.

Before coding, derive + REPORT from code:
1. how <text>/<tspan> import today (single Label flatten) and which span cases are lost/diagnosed
2. importer output paths: component (editable widgets) AND image-mode raster preview
3. how positioned/styled spans map to TextRun/TextChunk/TextLayout vs collapse to one label
4. anchors (text-anchor) + baselines (dominant/alignment-baseline, baseline-shift) handling/diagnostics
5. which text diagnostics flip from "flattened" to "modeled/grouped" vs stay deferred (textPath/bidi/shaping)
6. tests+goldens/snapshots per path

If a required path (e.g. grouped multi-label import) won't be done now, STOP and report. Do not call
single-label flatten "text done".

Required:
1. parse SVG text into TextRun/TextChunk/TextLayout/TextProvenance before creating widgets
2. robust tspan runs: x/y/dx/dy lists, style runs, source order, per-run provenance + warning flags
3. grouped multi-label import for positioned/styled spans (not one misleading label); deterministic
   placeholder bounds documented as approximate
4. anchors + baseline diagnostics: apply what is editable, warn explicitly where not
5. optional vector-outline snapshot mode ONLY after editable text + source preservation stay intact;
   if implemented, render through R4 compositing/clip; if deferred, diagnose clearly
6. textPath, bidi, full shaping/kerning/ligatures remain deferred with explicit diagnostics
7. both embedded sources std-only; single-crate:: export contract still passes

Tests:
1. multi-chunk positioned text -> multiple grouped labels with correct provenance/order
2. anchor + baseline cases produce correct placement or explicit diagnostic
3. determinism + fidelity-score (modeled vs flattened); malformed/empty text safe
4. snapshot mode (if shipped): golden + determinism; else deferred-diagnostic test
5. export parity: ignored all-built-in exported-project cargo check still passes
6. invariant over text cases across component + image paths

Verify (zero warnings): cargo fmt --check; cargo check; cargo test; cargo clippy -- -D warnings;
pwsh scripts\validate-svg-import.ps1; pwsh scripts\check-text-encoding.ps1; cargo run (positioned-span SVG imports as grouped labels).

Docs: flip only truly-done R6 tasks to [x] in SVG_RENDERER_ROADMAP.md + update limits/gap rows; update
TEXT_IMPORT_PLAN.md status; update SVG_IMPORT.md + feature-evaluation; append CODE_COOP.md handoff + DEVLOG.md entry (next=R7).

Final report: path matrix, paths changed, tests/snapshots added, verification numbers, gaps only if excluded before editing.
Success: positioned/styled text imports as editable grouped labels with provenance + honest anchor/baseline
diagnostics (optional snapshot rendered through R4), no misleading single-label collapse, tests green, zero warnings. Next: R7.
```
