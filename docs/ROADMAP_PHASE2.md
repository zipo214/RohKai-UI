# RohKai Phase 2 → Master Execution Backlog

Phase 2 starts after the v0.2.0 release. This document is the **single ordered
backlog** for everything RohKai has not yet finished. It collects every unchecked
roadmap item, every formerly-deferred recommendation, every "Later:" note, every
former non-goal, every design gap surfaced in feature evaluations, and every
loose idea — regardless of which document it originally lived in — and puts them
in **one mandatory execution order** that ends with the in-house renderer.

## De-Deferral Policy (read first)

**Deferral is no longer an option.** Every capability that ever had a chance of
being implemented, or was parked as "deferred", "later", "future", "out of
scope", or "non-goal", is now an ordered to-do in the sequence below. When a
piece of work is too big to finish in one pass, the response is to **refactor it
into smaller ordered to-dos in place** — not to defer it.

Two consequences:

1. There is no "Deferred" subsection anywhere in this file. If something is not
   done, it appears in the Master Execution Order with a stage number.
2. The order **spaces large projects intermittently**. Each ★ LARGE stage is
   flanked by smaller consolidation stages so that no agent (or group, or future
   session) hits two overwhelming projects back-to-back and is tempted to defer.
   Large projects are also refactored internally into ordered sub-todos.

### The only survivors: two architecture invariants (NOT deferrals)

These are not capabilities being postponed — they are permanent constraints from
`CLAUDE.md`. The *capabilities* they appear to block are delivered later by the
**in-house renderer (final stage)**, not by violating the constraint:

- **No external renderer dependency.** No `resvg`, `usvg`, `tiny-skia`, Skia,
  Cairo, librsvg, or browser embedding — ever. High-fidelity rendering parity is
  delivered by RohKai's own renderer (S22), not by adding a renderer crate.
- **No C FFI / no system-toolkit bindings.** "QAxWidget-style native platform
  controls" stay rejected *as an implementation technique*. The user-visible goal
  (native-quality controls and integrations) is delivered by the in-house
  renderer + pure-Rust platform layer, not by binding C.

If either invariant is ever to change, that is an explicit, separate user
decision — it is not implied by "de-defer everything."

### Depth legend

Each item is annotated with its **current** code depth, cross-referenced against
the source on 2026-06-12:

- `[x] DONE` — real output in canvas/preview/export + tests.
- `[~] SHALLOW` — shipped but partial; the gap is named. This is the most
  important state: shallow surfaces are the ones that look done but are not.
- `[ ] TODO` — not started.

---

## Master Execution Order

Run top to bottom. `★ LARGE` = multi-week; refactor into ordered sub-todos before
starting. `★★ FINAL` = the renderer, last by definition.

| # | Stage | Size | Theme |
|---|---|---|---|
| S1 | Layout & Constraint Completion | M | finish the constraint/layout slice already shipped shallow |
| S2 | Canvas UX Depth | S | navigation, clipboard, multi-edit, state preview |
| **S3** | **Font Shaping Engine (P2-A)** | **★ LARGE** | HarfBuzz-class shaping + BIDI; unblocks real-font text everywhere |
| S4 | Code Panel & Codegen Depth | M | memoization, rename/dedup, diff, true command undo |
| S5 | Responsive Layout & Design Tokens | M | breakpoints, layout preview/templates, token system |
| **S6** | **Data-Bound Model Views** | **★ LARGE** | real model Table/List/Tree, virtual scroll, sort, filter |
| S7 | SVG Image Format Completion | M | progressive/CMYK/12-bit/lossless JPEG, ICC, IDCT speed, fuzz |
| **S8** | **Interactive Chart Engine** | **★ LARGE** | axes, series editor, legend, zoom/pan, data binding |
| S9 | SVG Real-Font Text | M | .ttf/.otf glyphs + shaping/BIDI via S3; designer font loading |
| **S10** | **Full CSS Engine** | **★ LARGE** | combinators, @media, @import, pseudo, attribute, custom props |
| S11 | Accessibility & Internationalisation | M | ARIA, RTL, locale externalisation, keyboard-only authoring |
| **S12** | **Database Depth** | **★ LARGE** | query builder, schema viewer, preview, multi-backend, pools |
| S13 | Component Runtime Depth | M | HTTP runtime, full FSM/timer dispatch, inspector scripting |
| **S14** | **SVG Animation** | **★ LARGE** | SMIL + CSS @keyframes/transition + clock/repaint loop |
| S15 | Platform Targets & Packaging | M | WASM depth, native installers, profiling overlay, annotations |
| **S16** | **foreignObject + External Resource Policy** | **★ LARGE** | sandboxed HTML/CSS sub-layout; opt-in external resources |
| S17 | Sharing & Ecosystem | M | template marketplace, component-library publishing, design diff |
| **S18** | **SVG Scripting Sandbox** | **★ LARGE** | opt-in, isolated ECMAScript execution (security-critical) |
| **S19** | **Multi-Document & Windowing** | **★ LARGE** | model item-views, Dock, MDI, multi-window |
| S20 | Code Intelligence | M | code-panel IntelliSense, smart layout suggestions |
| **S21** | **Cross-Framework Export & Collaboration** | **★ LARGE** | SwiftUI/Compose/RN stubs; multiplayer CRDT (far future) |
| **S22** | **In-House Renderer (Stage 15)** | **★★ FINAL** | replace egui; own layout+GPU rasterizer; S3 text pipeline |

The remainder of this file is the per-stage detail. Cross items into stage
milestone notes in `docs/DEVLOG.md` as they are scheduled.

### Bespoke Foundation Milestones

RohKai-owned, security-sensitive foundations stay above S22 in the execution
order so the final renderer consumes proven subsystems instead of inventing
them inside a renderer rewrite. These are review checkpoints, not permission to
skip unfinished stages:

- **M1 after S3:** review the owned font shaping/BIDI API and whether its
  renderer-facing contracts are stable.
- **M2 after S7/S9/S10:** review image decoding, real-font SVG text, CSS, and
  bounded document parsing as one secure rendering-input stack.
- **M3 after S14/S16/S18:** review animation, external-resource sandboxing,
  foreignObject, and scripting threat models before any general renderer accepts
  active content.
- **M4 after S19:** review layout, model/view, docking, and multi-window
  requirements against the proposed renderer architecture.

At each milestone the user may direct a new architecture study or reorder later
work. S22 implementation still requires an explicit user go-ahead and a
ratified architecture decision.

---

## Priority Starter Threads (context)

### P2-A — Font Shaping Engine → scheduled as **S3**

Pure-Rust, zero-C HarfBuzz-class shaper in `src/canvas/shaper/` under the
single-`crate::` embedded contract. Trait scaffolding (`ShaperEngine` +
`RustyBuzzShaper` + `HersheyShaper`) already exists; `rustybuzz` is the approved
interim engine for the **main-app canvas only** and must never enter the
export-embedded `svg_rasterizer.rs` (std-only). Full detail in S3.

### P2-B — Database Integration Research → **[x] DONE**

`rusqlite = { version = "0.40", features = ["bundled"] }` approved (2026-06-11)
and in `Cargo.toml`. The threat model (no `format!()` SQL, `params![]` only) is
Invariant 10. Remaining DB *implementation* depth is S12.

---

## S1 — Layout & Constraint Completion ✅

Closed 2026-06-12. The constraint and layout slice now has parent-relative,
idempotent solving, visual authoring, recursive canvas/code/export behavior, and
Lazare hierarchy round-trip.

- [x] DONE — Constraint solving is idempotent and ordered parents-before-
      descendants; nested widgets resolve against their actual parent frame.
- [x] DONE — `validate_constraints` errors render in Properties.
- [x] DONE — Four draggable canvas anchor handles target parent
      leading/center/trailing and top/center/bottom anchors, preserve current
      geometry by deriving margins, and render persistent connector lines.
- [x] DONE — Per-child alignment/flex/size policy and grid row/span controls are
      exposed and drive live reflow/codegen.
- [x] DONE — Grid slots have persistent row-major names, Properties editing,
      canvas labels, generated-code/export comments, arrow reorder, and direct
      canvas drag-to-slot with cell feedback.
- [x] DONE — Nested V/H/Grid layouts reflow in ownership-depth order, render
      recursively on canvas, emit recursively in live/export code, and round-
      trip through Lazare using explicit parent markers. Empty layout closures
      intentionally clear prior child ownership.

## S2 — Canvas UX Depth

From the Stage 8.5 comparative analysis. All small, all independent.

- [ ] TODO — Search in canvas: Ctrl+F to find widgets by name, kind, or property
      value (distinct from the code-panel search).
- [ ] TODO — Clipboard enhancements: paste-at-cursor and paste-multiple with full
      property preservation.
- [ ] TODO — Multi-select property editing: edit one property across all selected
      widgets at once.
- [ ] TODO — Context tooltips: hover any designer UI element to see its purpose.
- [ ] TODO — Minimap: corner overview of the whole canvas (retained off-screen
      render pass).
- [ ] TODO — Visual state preview: click into hover/pressed/disabled/checked in
      the designer without running the app.
- [ ] TODO — Canvas ruler-guide persistence across save/load (named guides exist;
      persist them in the project file).
- [ ] TODO — Dark/light canvas background independent of the app theme.

## S3 — ★ LARGE — Font Shaping Engine (P2-A)

**Goal:** a pure-Rust, zero-C HarfBuzz-class shaper passing **2252 of 2252**
Unicode shaping tests, living in `src/canvas/shaper/` behind `ShaperEngine`.
Unblocks real-font SVG text (S9) and the renderer text pipeline (S22).

Refactored into ordered sub-todos:

- [~] SHALLOW — `ShaperEngine` trait + `RustyBuzzShaper` (main-app canvas) +
      `HersheyShaper` fallback exist. Interim only; not the owned port.
- [ ] TODO — OpenType GSUB (single, multiple, alternate, ligature, contextual,
      chained-context substitution).
- [ ] TODO — OpenType GPOS (single, pair/kerning, cursive, mark-to-base,
      mark-to-ligature, mark-to-mark, contextual positioning).
- [ ] TODO — Arabic/Syriac joining + right-to-left shaping.
- [ ] TODO — Indic reordering (Devanagari, Bengali, Tamil): matra/virama,
      reph, syllable clustering.
- [ ] TODO — Latin/Arabic ligatures (fi/fl, lam-alef) and required ligatures.
- [ ] TODO — Unicode bidirectional algorithm (UAX #9), full.
- [ ] TODO — CJK trivial shaping.
- [ ] TODO — Quality gate: `cargo test -- shaping` passes all 2252
      ([HarfBuzz corpus](https://github.com/harfbuzz/harfbuzz/tree/main/test/shaping/data/text-rendering-tests)).
- [ ] TODO — A std-only outline/metrics path usable by the export-embedded
      rasterizer (must not pull `rustybuzz` into `svg_rasterizer.rs`).

## S4 — Code Panel & Codegen Depth

Consolidation after S3. Mostly self-contained codegen work.

- [ ] TODO — Codegen memoization: `CodegenCache` keyed on `UiTree` hash; skip
      re-emit on unchanged frames. **Must not** suppress updates that reflect
      canvas mutations (Lazare sync).
- [ ] TODO — Handler rename: rename in the code panel and propagate to every
      widget that references the handler.
- [ ] TODO — Handler deduplication warning: two widgets sharing a handler name
      with different event kinds.
- [ ] TODO — Diff view: current generated code vs last saved/committed state.
- [ ] TODO — Undo/redo command pattern: replace snapshot undo with true
      `AppCommand` objects for granular steps and lower memory (design recorded
      in ROADMAP Cline Rec 9).
- [ ] TODO — Custom error types: manual `Display` impls now; `thiserror` only if
      a feature later justifies the dependency (explicit approval required).

## S5 — Responsive Layout & Design Tokens

- [ ] TODO — Responsive size-class breakpoints (compact / regular / custom).
- [ ] TODO — Layout preview: scrub the canvas size to watch the layout reflow.
- [ ] TODO — Layout templates: save and reuse constraint/layout presets.
- [ ] TODO — Design token system: named color/spacing/radius variables replacing
      hard-coded values across the tree; export as Rust constants.
- [ ] TODO — Color theme editor: visual `.rktheme` editor with live preview
      across all palette widgets.

## S6 — ★ LARGE — Data-Bound Model Views

`Table`/`ListView`/`TreeView` are static option-backed today. Make them
model-bound. Refactored:

- [~] SHALLOW — `DataColumn`/`DataColumnType` schema + `data_source_binding`
      exist; bound views emit iteration code. Backing model is static options.
- [ ] TODO — Real row model with typed columns and a `Vec<Row>` source binding.
- [ ] TODO — Virtual scroll for large datasets (100k+ rows).
- [ ] TODO — Column sort (stable, typed).
- [ ] TODO — Row filter / search predicate.
- [ ] TODO — Selection model + change events wired through codegen/export.
- [ ] TODO — Tree model: lazy child expansion, depth-bounded.

## S7 — SVG Image Format Completion

Baseline JPEG + PNG render. Finish the format matrix. (Formerly the JPEG
roadmap's "Defer Initially" list — now ordered.)

- [x] DONE — Baseline/extended-sequential JPEG (SOF0/SOF1), PNG types 0/2/3/4/6.
- [ ] TODO — Progressive JPEG (SOF2): spectral-selection + successive-approximation
      scan decode.
- [ ] TODO — Arithmetic-coded JPEG.
- [ ] TODO — CMYK / YCCK (4-component) JPEG → RGB.
- [ ] TODO — 12-bit and lossless JPEG.
- [ ] TODO — Integer/AAN IDCT replacing the float IDCT (speed).
- [ ] TODO — ICC colour-profile parse + map to sRGB (PNG `iCCP`, JPEG `APP2`).
- [ ] TODO — Sub-byte / interlaced (Adam7) PNG.
- [ ] TODO — `SvgRenderOptions` struct: caller-controlled budgets/options once the
      scene split needs them.
- [ ] TODO — R8.2 deep-fuzz hardening: structure-aware mutators over
      XML/path/PNG/JPEG/inflate; nightly + coverage workflow (still zero-dep).
      Prompt: `docs/svg-goal-plan-prompts/R8.2-deep-fuzz-ci-coverage.goal.md`.

## S8 — ★ LARGE — Interactive Chart Engine

`Chart` is a `Vec<f32>` bar painter. Build a real chart widget. Refactored:

- [~] SHALLOW — Bar `Chart` from a `Vec<f32>` binding (canvas + codegen).
- [ ] TODO — Chart kinds: line, bar, scatter, area, pie.
- [ ] TODO — Axes: ticks, labels, gridlines, auto/custom ranges.
- [ ] TODO — Series editor: multiple series, colors, legend.
- [ ] TODO — Interaction: zoom, pan, hover tooltips.
- [ ] TODO — Data-model binding to `Vec<Point>` / table source.
- [ ] TODO — Pure-Rust rendering only (no plotting crate unless user-approved).

## S9 — SVG Real-Font Text

Depends on S3. Replaces the Hershey-snapshot approximation with real glyphs.

- [x] DONE — Editable chunked text import (R6) + Hershey vector-outline snapshot
      with textPath (R11) + honest `text.raster_snapshot` diagnostics.
- [ ] TODO — Real font-file glyph rendering: parse `.ttf`/`.otf` (`glyf`/`CFF`
      outlines, `cmap`, `hmtx`) in the zero-dependency std-only profile.
- [ ] TODO — Full shaping + BIDI in the rasterizer via the S3 `ShaperEngine`
      (std-only outline path, no `rustybuzz` in the embedded source).
- [ ] TODO — Per-glyph position lists (x/y/dx/dy/rotate), textLength,
      lengthAdjust.
- [ ] TODO — Custom font loading in the designer: load a `.ttf`/`.otf` and preview
      text widgets immediately.

## S10 — ★ LARGE — Full CSS Engine

Today only tier-1 selectors (element/class/id/grouped) are supported. (Formerly
"Complete CSS" non-goal.) Refactored:

- [~] SHALLOW — Tier-1 selectors + inline/presentation cascade + currentColor.
- [ ] TODO — Combinators: descendant, child (`>`), sibling (`+`, `~`).
- [ ] TODO — Attribute selectors `[attr]`, `[attr=val]`, `[attr~=val]`, …
- [ ] TODO — Pseudo-classes/elements justified by real fixtures.
- [ ] TODO — `@media` (and bounded `@import` of `data:` stylesheets).
- [ ] TODO — CSS custom properties (`--var` / `var()`), `inherit`/`initial`/
      `unset`.
- [ ] TODO — Specificity/cascade parity proven against a curated fixture corpus.

## S11 — Accessibility & Internationalisation

- [ ] TODO — ARIA role annotations on exported egui widgets (screen-reader).
- [ ] TODO — RTL canvas mode: flip origin for right-to-left authoring.
- [ ] TODO — Locale string externalisation: export to `strings.toml` and generate
      `t!("key")` calls.
- [ ] TODO — Keyboard-only authoring: every canvas action without a mouse.
- [ ] TODO — Localization workflow: `.po` / `.ftl` / `.arb` export.

## S12 — ★ LARGE — Database Depth

SQLite slice ships; build out the rest. Multi-backend crates require explicit
user approval at stage start (Invariant 10 holds throughout: `params![]` only,
never `format!()` SQL). Refactored:

- [x] DONE — `DatabaseEngine` trait + `SqliteEngine`; `DbBinding`; `DbPanelState`
      window; `load_from_db()` codegen.
- [ ] TODO — Visual query builder: pick table, columns, WHERE filter.
- [ ] TODO — Schema viewer: tables and columns in a side panel.
- [ ] TODO — Design-time data preview: sample rows without runtime execution.
- [ ] TODO — Multi-backend behind `DatabaseEngine`: PostgreSQL, MySQL, Supabase
      (approved crate(s) only).
- [ ] TODO — Async query codegen + connection-pool AppState field.

## S13 — Component Runtime Depth

- [x] DONE — Timer (mpsc tick scheduler) + StateMachine (FSM schema + editor)
      runtime slice.
- [ ] TODO — HTTP request component: real runtime via an approved crate
      (`ureq`/`reqwest`); response parse; error UI.
- [ ] TODO — Full runtime component dispatch (timers fire FSM transitions; data
      sources push to bound widgets).
- [ ] TODO — Widget property-inspector scripting: a user `rhai`/`lua` script over
      the selected widget's props for batch edits (approved crate only).
- [ ] TODO — Smart layout suggestions: reflow proposals when widgets
      overlap/overflow.

## S14 — ★ LARGE — SVG Animation

Formerly the "Animation / SMIL / CSS animation" non-goals. Requires a clock +
repaint loop the static profile never had. Refactored:

- [x] DONE — `<animate>`/`animateTransform`/`animateMotion`/`set`/`mpath` and CSS
      at-rules are **diagnosed** (not silently dropped).
- [ ] TODO — Animation clock + bounded repaint loop in the renderer host.
- [ ] TODO — SMIL: `animate`, `animateTransform`, `animateMotion` (+ `mpath`),
      `set`; begin/end/dur/repeat timing.
- [ ] TODO — CSS `@keyframes` + `transition` execution.
- [ ] TODO — Determinism + bound tests (no unbounded animation work).

## S15 — Platform Targets & Packaging

- [x] DONE — WASM export, browser preview, Trunk config.
- [ ] TODO — WASM unsupported-widget diagnostic report.
- [ ] TODO — WASM in-app build status panel (Trunk output) + `.wasm` size budget.
- [ ] TODO — Native desktop packaging: `.deb` / `.rpm` / `.msi` / `.dmg`.
- [ ] TODO — Pixel-perfect grid snapping modes (isometric, 8pt, 4pt, custom).
- [ ] TODO — Canvas annotations: sticky notes, redline measurements.
- [ ] TODO — Performance profiling overlay: per-widget repaint cost in designer.

## S16 — ★ LARGE — foreignObject + External Resource Policy

Formerly "`foreignObject` content rendering" and "external network/file loading"
non-goals. Both need new infrastructure and a security model. Refactored:

- [x] DONE — `foreignObject` and external refs are **diagnosed / fail-closed**.
- [ ] TODO — Minimal HTML/CSS sub-layout engine to rasterize `foreignObject`
      content (bounded, deterministic, zero-dep).
- [ ] TODO — Opt-in external-resource policy: a sandbox that can load `file:`/
      `https:` images/fonts/stylesheets **only** under explicit, per-document
      user consent, with allowlist + size/time caps. Default stays fail-closed.

## S17 — Sharing & Ecosystem

- [ ] TODO — Template marketplace: upload/download `.rktp` packs with import
      validation + conflict resolution.
- [ ] TODO — Component-library sharing: publish a set of `.rkwd` descriptors as a
      named, version-pinned library.
- [ ] TODO — Design-to-code diff: import a Figma/Sketch design and highlight
      which canvas widgets diverge.

## S18 — ★ LARGE — SVG Scripting Sandbox

Formerly "Scripting (deliberately out of scope / security)". De-deferred but
placed late and behind a hard opt-in because executing untrusted SVG script is
the single most dangerous lane. Refactored:

- [x] DONE — `<script>` is **hard-rejected** by the security gate (default stays
      this way unless the user opts a document in).
- [ ] TODO — Isolated, capability-free pure-Rust ECMAScript interpreter
      (no DOM, no network, no FS) behind explicit per-document opt-in.
- [ ] TODO — A minimal scriptable DOM surface gated by the S16 resource policy.
- [ ] TODO — Execution budgets (instruction/time/memory caps) + determinism
      tests + a fuzz lane.

## S19 — ★ LARGE — Project Surfaces & Windowing

### S19A - Project Surfaces And Modal Dialogs
- [x] DONE - Schema-v2 `ProjectDocument`, lossless legacy/v1 migration, one
      protected root surface, multiple editable modal surfaces, CRUD/templates,
      per-surface workspace state, and project-wide undo/dirty persistence.
- [x] DONE - Typed surface lifecycle behaviors plus Open/Accept/Reject actions in
      F5 preview and native/WASM export.
- [x] DONE - Transactional modal drafts, semantic dialog button roles, nested
      top-only stack, default/Escape behavior, focus entry/restoration,
      diagnostics, aggregate state/handlers/dependencies, and warning-denied
      generated fixtures.
- [x] DONE - Migration, isolation, nested lifecycle, generated compile, and
      50-surface/10,000-widget stress fixtures.
- [~] VERIFY - Repeat narrow/normal/wide screenshot and accessibility review
      after the Windows Computer Use runtime packaging error is repaired.

### S19B - Modeless Secondary Windows
- [ ] TODO - Secondary eframe deferred viewports with synchronized state,
      geometry persistence, lifecycle/ownership, multiple instances, native
      multi-monitor/DPI behavior, and explicit web fallback.

### S19C - Main-Window Framework
- [ ] TODO - Toolbars, menus/actions, status areas, Dock Widget, split views,
      tab groups, and persisted workspace layouts.

### S19D - MDI And Advanced Windowing
- [ ] TODO - MDI Area, typed surface parameters/results, application/window
      modality, advanced instance management, and platform lifecycle.
- [ ] TODO - Model-based item views (MVC tree, virtual list 100k+) generalize S6
      into a reusable model/view framework; modal surfaces do not close this gap.

## S20 — Code Intelligence

- [ ] TODO — Code-panel IntelliSense: Rust keyword completion, local-binding
      suggestions, handler autocomplete.
- [ ] TODO — Smart layout suggestions promoted to AI-assisted reflow (if S13's
      heuristic version proves valuable).

## S21 — ★ LARGE — Cross-Framework Export & Collaboration

The far-future cluster. Each is independently large; grouped because they are the
last non-renderer work.

- [ ] TODO — Export to SwiftUI / Jetpack Compose / React Native stubs.
- [ ] TODO — Multiplayer / collaboration: two designers on one `.rohkai.json`,
      CRDT-based merge.

## S22 — ★★ FINAL — In-House Renderer (Stage 15)

Final implementation stage by default. It starts only when the user explicitly
directs execution, the applicable bespoke-foundation milestones above have been
reviewed, and a separate architecture decision is ratified. This is where the
two architecture invariants pay off: RohKai stops depending on egui's render
layer and gains full visual control **without** any external renderer dependency
or C FFI.

- [ ] TODO — Replace the egui rendering layer with a RohKai-owned pure-Rust
      renderer.
- [ ] TODO — The widget descriptor format drives the renderer's widget model
      directly.
- [ ] TODO — Zero transient C dependencies in the rendering stack.
- [ ] TODO — Custom layout engine with constraint + flex support (absorbs S1/S5).
- [ ] TODO — GPU-accelerated rasterizer (wgpu-based or an own path rasterizer).
- [ ] TODO — All previously constrained visual properties become available:
      arbitrary shapes, gradients, shadows, blend modes per widget.
- [ ] TODO — Text pipeline uses the S3 shaping engine end-to-end.
- [ ] TODO — Native-quality controls + platform integrations delivered here
      (the capability formerly mislabelled "QAxWidget"), in pure Rust.

---

## Completed in Phase 1 (v0.1.0 → v0.2.0)

Cross-referenced for context. Full history in `docs/DEVLOG.md` and git log.

| Area | Completion |
|---|---|
| Canvas: drag, select, resize, rubber-band, z-order, snap, smart guides, rulers | ✅ |
| Widgets: Button, Label, TextInput, Slider, Checkbox, Frame, ComboBox, RadioButton, ProgressBar, TextArea, SpinBox, FontComboBox, GroupBox, VLayout, HLayout, ScrollArea, GridLayout, TabWidget, ToolButton, CommandLinkButton, DialogButtonBox, MathLabel, FilePicker, Chart, Table, ListView, TreeView, StackedWidget, ToolBox, Image | ✅ |
| Custom widgets: `.rkwd` descriptor format, Advanced Editor, Guided Builder, Visual Maker MVP, `.rkwb` bundle | ✅ |
| Codegen: live egui Rust output, Lazare bidirectional sync, Ctrl+F search, symbol list, clickable diagnostics | ✅ |
| Export: complete compilable Rust project, WASM export, browser preview | ✅ |
| SVG renderer: R0–R12 complete + filter tier-3 (geometry, gradients, clip, masks, filters 1/2/3, patterns, markers, text, namespace recovery) | ✅ |
| Stage 14: snapshot undo/redo | ✅ |
| Stage 11: async task wiring, Rust wiring panel, iterator/trait snippets, Object Inspector | ✅ |
| P2.3 constraints, P2.4 layout UX, P2.5 formula/timer/FSM/shortcuts/.rkwb, P2.6 SQLite DB | ✅ (depth gaps tracked in S1/S6/S12) |
| Crate promoted to lib+bin; `fidelity_audit.rs` cross-surface parity harness | ✅ |
| Engineering invariants + 495 lib / 11 integration / 1 doctest, zero clippy warnings | ✅ |
