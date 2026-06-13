# Code CoOp

Short agent-to-agent handoff diary. This is not the devlog and not the roadmap.
Use it for the 3-4 sentence "what I am doing and what the next agent should
know" note at the start of a meaningful planning or coding session.

Keep entries newest-first. Be plain, specific, and honest about uncertainty.
Mention the branch, the immediate goal, touched areas, and any known hazards.

## 2026-06-15 — CodeRabbit PR #9 batch 2 + gh thread replies

Branch `dev`. Completed the second batch of CodeRabbit fixes (commit 4a14459): name_counter now reseeds on project open; sanitise_ident guards against leading-digit SQL identifiers; db_panel refresh_schema clears all dependent caches on both Ok and Err paths; added effective_field_binding() helper in codegen/rust.rs to unify binding resolution across all surfaces; properties.rs duplicate test renamed to state the actual invariant. Also added the DB_INTEGRATION_RESEARCH.md cross-reference to ENGINEERING_INVARIANTS.md Invariant 10, and the egui API maintenance reminder in feature-evaluation doc. Posted gh api replies on all 5 remaining open threads (workflow SHA, DB cross-ref, feature-eval reminder, sink-type validation deferred, db_panel stale cache). All 14 original CodeRabbit threads now have zipo214 replies; 554 unit + 17 fidelity + doctest green. Sink-type validation (comment 3417193356) is deferred — needs field-type metadata threading through UiTree into the behavior graph; acknowledged in thread. PR #9 is ready for final CI pass and merge decision.

## 2026-06-14 — CodeRabbit review triage on PR #9

Branch `dev`. PR #9 diffs against `main`, so CodeRabbit reviewed the whole
accumulated `dev` history (88 files), not just the recipe/rename change. Triaged
~30 findings: fixed 10 genuinely-valid low-risk bugs with a regression test for
the class (behavior clamp min>max panic guard + test; rusqlite version aligned to
the documented `0.40`; component_state StateMachine initial-state now escaped via
`string_literal`; shortcuts whitespace-only override trims to blank;
component_tray state delete now drops dangling initial_state/transitions; app.rs
timer interval clamped to >=1ms; formula emitter `unreachable!` → debug_assert +
benign literal; shaper trait doc reconciled with the fontless fallback;
check-surface-parity unused `$fidelity` removed; Invariant 10 scope clarified to
"designer binary"). Verified and SKIPPED the "Critical" CornerRadius claim — it is
a false positive: `egui::CornerRadius::from(f32)` compiles on egui 0.34.3 (probed
with a throwaway example). Skipped (with reasons) heavier/out-of-scope items:
behavior sink-type validation (needs emit-time field types), db_panel cache reset
(Stage 13), app.rs name_counter/timer-respawn lifecycle, rustybuzz glyph_id
scaffold, and assorted doc-churn. Full gate green (553 unit + 17 fidelity +
doctest, all-target clippy, fmt, encoding). Local commit; push to PR branch.

## 2026-06-14 — Behavior recipes UI surfacing + Global Rust Wiring rename

Branch `dev`. The recipe matrix (`codegen::behavior_recipes`) was already
committed (e79c787) and auto-applies the default suggestion on wire drop; this
session closed the two open goal gaps. The Behaviors panel
(`panels::behaviors`) now surfaces the full suggestion set for the wired
source→sink pair as one-click selectable chips (default pre-selected, params
still editable below, raw Action picker kept as advanced). User-facing "Rust
Wiring" is now "Global Rust Wiring" via a single-source `rust_wiring::PANEL_TITLE`
const (window + menu button), with a guard test; docs (ARCHITECTURE, CODE_INDEX)
reframed and the previously-unindexed behavior modules added. No new crates,
serde untouched, codegen still consumes graph actions not recipe IDs. Full gate
green (552 unit + 17 fidelity + doctest, all-target clippy, fmt, encoding).
Committed locally only — coop hazard (external agent state) still says don't push.

## 2026-06-13 - Project surfaces and modal dialogs

Branch `codex/project-surfaces-modal`, isolated from `dev`. I am replacing the
single-window project root with a versioned `ProjectDocument` containing a main
surface plus modal-dialog surfaces, then wiring authoring, behaviors, preview,
Lazare, export, and transactional modal state through that model. The main
hazard is accidental duplicated authority between project-global properties,
surface properties, and the old `UiTree.app_props`; the completed migration
must leave one canonical owner and preserve schema-v1 projects losslessly.

## 2026-06-12 - Rust 1.96 and dependency alignment audit

Branch `codex/toolchain-dependency-refresh`, isolated from `dev`. RohKai now
separates edition (2024), dependency-driven MSRV (1.92), and pinned/tested Rust
(1.96.0); every direct dependency is at its current crates.io release. Generated
projects remain edition 2021 for portability but share the 1.92 MSRV and
egui/eframe/rfd versions; alignment and networked freshness scripts guard drift.
Full tests, all-target Clippy, SVG validation, export Cargo fixtures, and native
launch smoke passed before the local commit.

## 2026-06-12 — Migrated to Rust edition 2024 (exports stay 2021)

Branch `dev`, commit `3329961`. `Cargo.toml` edition 2021 → 2024 (rustc 1.95).
`cargo fix` + `clippy --fix` (nested ifs → let-chains in designer-only sources) +
`cargo fmt` 2024 style across the tree. **Gotcha for future agents:**
`src/canvas/svg_rasterizer.rs` is embedded VERBATIM into edition-2021 exported
projects, so it must NOT use edition-2024 `if let` let-chains — they are a
compile error under 2021. It is kept as nested `if let` with
`#[allow(clippy::collapsible_if)]` on the module (`canvas/mod.rs`); the
`all_builtin_widgets_export_cargo_check` test is the guard (it caught the
regression when clippy --fix collapsed them). Same applies to any future
export-embedded source. Exports stay edition 2021 for portability — bumping them
to 2024 (raising user MSRV to 1.85) is a separate, unmade decision. Gate green:
clippy 0, 551 + 17 + 1 tests, fmt clean.

## 2026-06-12 — Behavior graph: visual event→state wiring shipped

Branch `dev`. Implemented the beginner-facing Behavior Graph: persisted
`AppProps.behaviors` (typed `VisualAction`s), canvas socket drag with
Visio-style wires (open circle = event source, closed = state target), a
Behaviors editor in the Properties tab, and one shared emitter
(`codegen::behavior`) used by live code and export — Button Click → Add 0.1
emits `self.state.progress = (self.state.progress + 0.1).clamp(0.0, 1.0);` in
nested Frame/V/H/Grid paths too. Also fixed a latent export bug: layout-child
combos with a Change handler compared `Option<()>` to `Some(true)` (now tracks
`changed` like frame-child combos). Suite: 544 unit + 17 fidelity + doctest,
clippy `--all-targets -D warnings` green, launch smoke OK. Known boundary:
live preview dispatches behaviors exactly where it dispatches handlers
(TextArea/FontComboBox change stays export-only — pre-existing parity line).

## 2026-06-12 — S1 closed: visual anchors, named Grid slots, recursive layouts

Branch `dev`, based on `1e7fd38`; Claude is out of usage. S1 is now complete:
canvas constraint handles write parent-relative anchors without moving the
widget, Grid slots have stable names and drag-to-slot, and nested layouts reflow,
draw, emit, export, and Lazare-round-trip recursively. The prior one-line Stage
15 roadmap edit was preserved in intent and rewritten into explicit bespoke-
foundation review milestones. Full suite: 523 unit + 17 fidelity + doctest,
zero ignored, warning-denied clippy green. Do not merge or push while external
agent state remains uncertain.

## 2026-06-12 — v0.2.0 audit remediation: export gates + widget-authoring UX

Branch `dev`. Remediation is complete and ready for a scoped local commit.
Generated-project checks now run normally with `-D warnings`; embedded SVG is
std-only; formula/database export state and dependencies are complete. All six
ignored tests were promoted, which exposed and fixed an infinite loop on
malformed path numbers after `Z`; the suite is now 515 + 17 + doctest, zero
ignored. Widget authoring is consolidated under Widgets: Create New Widget opens
the true Visual Widget Maker, with guided/advanced descriptor paths clearly
named; all three windows share viewport-safe bounds. Do not merge while Claude
is active/ambiguous. After this commit, resume S1 anchor-handle drag and nested-
layout Lazare round-trip, then reorder bespoke secure-code milestones before
Stage 15.

## 2026-06-12 — S1 finished: constraint-solver bug fix + 3 half-wired fields wired

Branch `dev`. Finished the S1 parity gaps the checker found. **Real latent bug
fixed:** `apply_constraints` ran every frame with `margin += `, so any margin
constraint walked the widget off screen and shrank it; it also anchored every
widget to the canvas, never its parent. Rewrote the solver to be **idempotent**
(margin folded into absolute alignment — safe per-frame, no save/load drift) and
**parent-relative** (parents-before-children, frame = parent's solved rect).
Surfaced `validate_constraints` in the Properties Constraints section (red
messages). Wired the two half-wired fields end-to-end: `text_align` (was dead
everywhere) → egui_emitter + export + preview; `child_cross_align` per-child
override → VLayout/HLayout codegen (dropped UI Stretch — no proven egui path).
Parity tests added. 499 lib + 17 integration + 1 doctest green, zero warnings,
export cargo-check fixture green. Margin semantics intentionally changed (insets
within the alignment anchor; no-op without one) — see `constraint_solver.rs`
header + the RCA's resolved follow-ups. Next: S1 remainder (anchor visual-handle
drag, nested-layout Lazare round-trip) or S2.

## 2026-06-12 — Roadmaps de-deferred into ordered master backlog; RCA + parity checker shipped

Branch `dev`, commits `1352975` (lib+bin) → `ca28d9e` (roadmaps) → this one.
Rewrote `ROADMAP_PHASE2.md` as the single ordered backlog S1–S22 (renderer S22,
LARGE projects spaced); removed all non-goal/deferral/strikethrough language from
the SVG/JPEG/text roadmaps and pointed them at the S-stages. The only surviving
"non-goals" are two CLAUDE.md architecture invariants — no external renderer dep,
no C FFI — whose capability is delivered by the S22 in-house renderer, not by
reversing the rule. **Flag for the user:** if they truly want C FFI / external
renderer crates, that is a separate explicit decision; I did NOT convert those two
to to-dos. Then RCA'd the recurring class (cross-surface parity drift) in
`docs/RCA-2026-06-12-surface-parity-drift.md`: root cause = enum variants get
exhaustive-match forcing but struct fields / roadmap claims / unsurfaced pub APIs
do not. Built the guard: `scripts/check-surface-parity.ps1` (advisory; flags
field→codegen gaps, roadmap↔code drift, dead-code pub) + extended
`fidelity_audit.rs` with a `ALL_KINDS` codegen-completeness test (13 tests now).
Checker surfaced real S1 follow-ups: `text_align`/`child_cross_align`/
`constraints` have no `src/codegen/` reference; `validate_constraints` unsurfaced.
All green: 495 lib + 13 integration + 1 doctest, zero clippy warnings.

## 2026-06-12 — Crate promoted to lib+bin; fidelity_audit harness green; de-defer pass starting

Branch `dev`. Closed the in-flight fidelity thread: the uncommitted P2.4 codegen
parity (child_flex/grid spans in `egui_emitter`), P2.5 `effective_shortcut()`
wiring, and the CSS-at-rule diagnostic (`parse_css_stylesheet.atrule_count`)
were all real but the new `tests/fidelity_audit.rs` cross-surface harness could
not link — RohKai was a **binary-only crate**. Added `src/lib.rs` as the crate
root (`pub mod` for all 9 modules) and slimmed `src/main.rs` to a shell over
`rohkai::app`; `tests/` can now import the public API. Fixed one clippy error the
untracked harness carried (`.last()`→`.next_back()`). Now **495 lib + 11
integration + 1 doctest pass, zero clippy warnings**. Next: per user directive,
de-defer EVERY roadmap (remove all non-goal/deferral/strikethrough language,
convert to ordered to-dos), re-enumerate one master backlog ending in the
in-house renderer with large projects spaced — the ONLY surviving non-goal is the
zero-external-renderer-dependency architecture invariant (CLAUDE.md). Known
shallow surfaces found: `constraint_solver::apply_constraints` iterates flat
`tree.widgets` (no recursion into layout children); `validate_constraints` is
`#[allow(dead_code)]` (no properties-panel surface).

## 2026-06-11 — P2.3/P2.5/P2.6 merged into dev; 489 tests green, zero warnings

Branch `dev`. Merged three of the five background agents: P2.3 (constraint-based layout — LayoutConstraints, HAlign/VAlign, constraint solver, show_constraints panel), P2.5 (formula depth, timer wiring via mpsc, state machine schema + editor, shortcut customization, .rkwb ZIP bundle), and P2.6 (Stage 13 DB integration — DatabaseEngine trait, SqliteEngine, db_panel, DbBinding on WidgetInstance, Invariant 10 in ENGINEERING_INVARIANTS.md, rusqlite added to Cargo.toml). P2.4 was merged in the prior session. P2.7 (SVG R9) had nothing new to merge (R9 was already done in v0.2.0). All conflicts were struct-field additions resolved by keeping both sides; 489 tests pass. Next work: run cargo run smoke, then decide whether to start P2.8 or address open items from the caveman review.

## 2026-06-11 — P2.1-C/D merged + 5 agents launched for P2.3–P2.7

Branch `dev`, pushed `6de4a08`. Completed all P2.1 deferred items via three worktree agents (B=style tokens+hit regions+event zones, C=state variants, D=layout groups+slots). VWM now has: StyleTokens doc-level variables, HitRegion primitive, PrimState/PrimVariants per-primitive hover/pressed/disabled/checked overrides, HGroup/VGroup/Grid/Stack group kinds with two-pass codegen (claimed children skipped at top level), SlotDef slots on WidgetMakerDoc with slot comment emission, state variant UI in the primitive inspector. 457 tests, zero warnings. Also added rusqlite = { version = "0.40", features = ["bundled"] } as approved dependency (written to CLAUDE.md). Five background worktree agents now running for P2.3 (constraint-based layout), P2.4 (layout UX depth), P2.5 (formula/timer/state-machine/shortcuts/.rkwb), P2.6 (DB integration via rusqlite + DatabaseEngine trait), P2.7 (SVG R9 markers + pattern tiling). When agents complete, merge each in order and run full verification before pushing. Watch for merge conflicts in schema.rs (P2.3 and P2.4 both touch WidgetInstance) and in properties.rs (P2.3, P2.4, P2.5, P2.6 all add panels). Invariant 10 (no format!() SQL) must be in ENGINEERING_INVARIANTS.md before P2.6 merges.

## 2026-06-11 — P2.1 + P2.2 merged: VWM codegen preview + canvas UX depth

Branch `dev`, pushed `de4f7e5`. Two parallel worktree agents landed and merged clean. P2.1: codegen preview tab in Widget Maker right panel (live `gen_live_preview`/`gen_export_template` display), primitive z-order ↑↓ buttons in layer list, `PrimAnchor` enum + `min_w`/`min_h` constraints on `MakerPrimitive` (serde-default for backward compat), `doc_from_descriptor()` round-trip for VWM-originated descriptors. P2.2: zoom-to-selection (`F` key, fits selected or all widgets with 10% padding + min zoom clamp), property reset (right-click context menu on label/text fields + geometry DragValues), error highlighting (red 2px outline on canvas for `DuplicateId` / `InvalidHandler` / `MissingBinding`), auto widget naming (`button_1`, `button_2`, … — `name_counter` cleared on New). 440 tests, zero warnings. Deferred from P2.1: hit regions, layout groups, state variants, slots, event zones, style tokens. Deferred from P2.2: canvas Ctrl+F search, clipboard enhancements, minimap, multi-select property editing, context tooltips. Next: CodeQL false-positive dismissal on GitHub Security tab (rust_wiring.rs L89/172/228); add Invariant 10 (SQL injection guard) to ENGINEERING_INVARIANTS.md before any Stage 13 work; rusqlite crate awaits explicit user approval before Cargo.toml addition.

## 2026-06-11 — v0.2.0 PR #6 review fixes + CI gate repair

Branch `dev`. Fixed 4 Qodo review bugs + 1 arch violation from PR #6 review. (1) Arch: moved `prim_to_egui_lines` / `gen_export_template` / `gen_live_preview` from `canvas/widget_maker.rs` into new `src/codegen/widget_maker_emit.rs` — restores CLAUDE.md invariant (no Rust syntax strings outside `src/codegen/`); also fixes unescaped text bug by using `string_literal()`. (2) Bug: added `create_dir_all` before `.rkwd` write in Widget Maker save — fresh-install won't silently fail. (3) Bug: added `gen_app_rs_wasm` that clones tree + replaces FilePicker→Label so WASM export never contains `rfd::` references; added test. (4) Bug: unique per-process temp dir for WASM preview (appends PID). (5) Previously: fixed CI `cargo fmt --check` failure (16 files formatted), added `permissions: contents: read` to workflow, bumped `Cargo.toml` to v0.2.0. 416 tests, zero warnings. PR #6 CI + Qodo re-review pending. Next agent: wait for CI green + Qodo re-review, then merge PR #6 → main and tag v0.2.0. After merge: RustyBuzz integration (ShaperEngine trait in `src/canvas/shaper/`, `rustybuzz` approved dependency), dismiss 3 CodeQL false positives on GitHub Security tab (`rust_wiring.rs` L89/172/228 are egui widget IDs, not crypto values).

## 2026-06-11 — Good-citizen pass: Invariant 7, module docs, handler extraction, tests, resize, browser preview, doc staleness

Branch `dev`. Implemented all 7 items from the `/goal` task list. (1) Fixed Invariant 7 filename sanitizer — `sanitize_widget_id_to_filename` now whitelists `[A-Za-z0-9_-]` with 6 tests. (2) Added `//!` module docs to canvas/panels/widgets mod.rs plus widget_maker.rs and widget_maker_panel.rs. (3) Extracted `resolve_click_handler`/`resolve_change_handler` from egui_emitter + export into shared `src/codegen/handlers.rs`. (4) Added 12 UiTree unit tests (bring_to_front, send_to_back, group, ungroup, remove cascade, validate_and_repair x2) and 4 canvas pure-logic tests (snap, resize handle hit/delta/min-size). (5) Wired Widget Maker corner handles interactively — `corner_hit()` + `apply_corner_resize()` using `resize_corner: Option<u8>` on WidgetMakerDoc. (6) Added "Preview in Browser…" File menu item — PATH-checks trunk, exports WASM to temp dir, spawns `trunk serve`. (7) Updated all stale feature-eval docs, ARCHITECTURE.md field tables, ROADMAP Stage 7.x checkboxes (first 6 items now [x]), Stage 12 browser preview item marked [x]. 412 tests pass, zero clippy warnings. Next: the user wants to understand the full VWM vision vs MVP gap — `docs/VISUAL_WIDGET_MAKER.md` is the canonical plan (created 2026-05-28); the "Later Capabilities" section (state variants, event zones, slots, layout groups, style tokens, round-trip) is entirely unimplemented.

## 2026-06-11 — All deferred depth-gate items implemented (pre-release gate closed)

On `dev`. Six depth-gate features shipped in sequence, all passing 394 tests /
zero clippy warnings: (1) **Data model groundwork** — `DataColumnType`/`DataColumn`
schema, `data_source_binding` on `WidgetProps`, bound Table/ListView/TreeView
emit iteration code; (2) **True layout ownership** — `SizePolicy` {Fixed/FillWidth/Fill}
per-child + `grid_row_height` on GridLayout + export `layout_cross_align` parity;
(3) **Lazare IDE depth** — Ctrl+F code search (match count + Prev/Next), Symbol list
(widget/handler navigation), clickable diagnostic navigation; (4) **Visual Widget
Maker** — `WidgetMakerDoc`+`MakerPrimitive` model, mini-canvas, Rect/Outline/
Ellipse/Text primitives, Save→.rkwd pipeline; (5) **Object Inspector depth** —
`describe_kind()`, "design-time stub" badge, sectioned config, inline generated-code
preview; (6) **ROADMAP** — Stage 12 WASM, formula depth, runtime stubs, data model,
layout ownership, Lazare depth, Widget Maker, Inspector all checked off. Remaining:
Stage 13 DB (blocked — needs crate approval), font shaping (blocked — needs
`rustybuzz` approval), Stage 15 deferred. Also added rayon parallelism, formula
parser, WASM export in the same session.

## 2026-06-10 — SVG R12 complete: namespace + recovery + a11y (post-R8 lanes ALL done)

On `dev`. **R12 done** (commit a0b563f) — final post-R8 lane; **the SVG renderer
roadmap (R0–R12 + R8.1) is now complete.** All in `svg_rasterizer.rs`
(export-embedded). (1) Namespace: `XmlParser.ns_stack` of `NsFrame`; `apply_xmlns`
reads xmlns/xmlns:prefix from raw open-tag headers (note: `parse_attr` strips
prefixes, so xmlns must be scanned from the raw header slice, not raw_attrs);
qualified names → `Namespace::{Svg,Xlink,Foreign}`; foreign elements skip their
balanced subtree + `namespace.foreign_element`. Key fix: prefix-stripping turned
`<custom:rect>` into a rendered `rect` — now foreign. (2) Recovery:
`consume_close_tag` counts mismatched/unclosed; → `recovery.malformed_markup` +
`recovered_error_count`, never ParseFailed/panic; security gates stay hard-fail.
(3) a11y: `<title>`/`<desc>` captured inline during parse (NOT as SvgNode, so
they don't render as R11 glyphs) + root aria-label fallback, bounded
`MAX_A11Y_TEXT`, on `SvgRenderReport` + report-panel rows + export-preserved.
`SvgRenderReport` gained `title`/`desc`/`recovered_error_count` (svg_report test
literal + panel updated). Gate green: 335 tests / fmt / clippy --all-targets /
validate-svg-import / check-text-encoding. No new deps. **Next:** no open SVG
renderer lanes remain — remaining gaps are explicit out-of-profile non-goals
(real font files+shaping, tier-3 filters, progressive JPEG, ICC, foreignObject).
Pick from `docs/ROADMAP.md` open stages (12 Platform Targets / 13 Data / 15
Renderer) or the deferred Stage 9 parallel-processing items.

## 2026-06-10 — SVG R11 complete: raster text + textPath (bundled Hershey font)

On `dev`. **R11 done** (commit e8eae08). Image-mode rasterizer now renders
`<text>`/`<tspan>`/`<textPath>` as a vector-outline snapshot; editable component
import (R6, svg_import.rs) untouched. Key pieces, all in `svg_rasterizer.rs`
(export-embedded verbatim): `HERSHEY_SIMPLEX` (public-domain stroked font, ASCII
32..=126, 30 units/em, `^` simplified); parser now captures `<text>` inner
markup to the MATCHING close tag (`SvgNode::Text.content` — old code cut at
first `</`); `scan_text_runs` (one tspan level x/y/dx/dy, deeper flattened +
diags); `lower_text_command` lays out runs in user space and emits ONE stroked
`DrawCommand::Shape` (PathData of glyph polylines) → full reuse of stroke
pipeline/clips/masks/filters/gradients; `ArcLengthPath` places textPath glyphs
by arc length with midpoint-tangent rotation. Style gained inherited `font_size`
+ `text_anchor`. Honesty: every text render emits `text.raster_snapshot`
(→ Medium fidelity, intentional); tofu + diags for non-ASCII/bidi/combining;
`MAX_TEXT_GLYPHS` cap. `DrawCommand::UnsupportedText` removed; the one test
asserting text-unsupported was flipped (justified in DEVLOG). **Test-writing
heads-up:** the 0.67px glyph stroke AA-splits across pixel columns when a stem
straddles a boundary — assert alpha>50, not >100/200. Gate green: 327 tests /
fmt / clippy --all-targets / scripts / launch smoke. **Next: R12** (namespace
model + malformed recovery + a11y metadata — final open lane); read
`docs/svg-goal-plan-prompts/R12-*.goal.md` first.

## 2026-06-10 — SVG R10 complete: filter correctness + tier-2 + blend modes

On `dev`. **R10 done**, shipped as four verified+committed increments (each ends
green; tree always shippable): (1) tier-2 primitives feComposite/feBlend/
feComponentTransfer/feMorphology — real buffers on the R7 premultiplied pipeline,
`<feComponentTransfer>` added to `is_container_tag` so feFunc* children parse,
feMorphology radius capped (`MAX_MORPH_RADIUS`); (2) `mix-blend-mode` —
`BlendMode` threaded LayerRaw→ResolvedLayer→Offscreen, composited via new
`composite_offscreen_blended` (Normal path byte-identical, so no existing group/
mask/filter golden moved); (3) linearRGB `color-interpolation-filters` default
(srgb↔linear premult-aware convert at the graph boundary, `sRGB` opts out,
flood/dropshadow colours linearised) — existing goldens use pure 0/255 so none
moved, gamma proven by a pixel-exact unit test; (4) precise filter region
(`filterUnits`+x/y/w/h, default obbox −10%..110% via source alpha extent;
userSpaceOnUse exact via CTM) clipping the result. **Heads-up for R11+:** region
clipping is spec-correct and clips filter output beyond the element bbox, so
feOffset/feFlood/feMorphology/feDropShadow/feGaussianBlur fixtures+tests now carry
explicit filter regions (documented inline). Gate green:
fmt/check/test 321/clippy --all-targets/validate-svg-import/check-text-encoding/
cargo run. No new deps. **Next: R11** (raster text & textPath) — read
`docs/svg-goal-plan-prompts/R11-*.goal.md` first; it is heavy, gate on real need.

## 2026-06-09 — Engineering invariants doc (process hardening from PR #4 review)

On `dev`. Triaged the PR #4 CodeRabbit batch (32 comments): all substantive
findings are already fixed in current code (verified the one without a ✅ reply —
`refresh_preview_state()` now re-seeds preview after every tree swap in
`app.rs`). Distilled the recurring **bug classes** into a new read-on-demand
reference `docs/ENGINEERING_INVARIANTS.md`: surface parity (canvas/preview/
export), single-source-of-truth classification, input-ownership gating,
reset/default restoration, generated-identifier safety + codegen module
boundary, string byte-slicing, filename sanitizing, conservative shipped
defaults, doc-contradiction reconciliation — each with an invariant + cheap
guard, plus the systemic-fix workflow. Wired terse pointers into `CLAUDE.md`,
`AGENTS.md`, and preflight, and bumped the documented gate to `cargo clippy
--all-targets -- -D warnings` (plain clippy skipped the `examples/`/`tests/`
lints the reviewer flagged). No code behavior changed. Next coding lane is still
**R10** (filters).

## 2026-06-09 — SVG R9 complete: markers + pattern tiling

On `dev`. **R9 is now fully done** (vector-effect shipped earlier in 61f3d66;
this session added markers + pattern tiling). All in `src/canvas/svg_rasterizer.rs`
(embedded verbatim into exports, so in-app == export; new
`embedded_rasterizer_includes_r9_render_paths` test enforces parity). **Patterns:**
`PaintServerTable` grew a `patterns: HashMap<String, PatternDef>` built in a second
pass (`build_pattern_def`, href-merge + `reference.pattern_cycle` guard);
`PaintSampler::Pattern` renders one tile lazily per fill via `build_pattern_sampler`
(reuses the `<mask>` subtree-render trick through the new shared
`render_content_items`), repeating across the bbox with `rem_euclid` wrap; tile
pixels capped by `MAX_PATTERN_TILE_PIXELS`; content self-reference is broken by
rendering the tile with the pattern removed from a cloned table. **Markers:**
`build_markers` (called in `DisplayList::build`, stored on `DrawCommand::Shape`)
resolves `marker-start/mid/end` (+`marker`), extracts vertices+tangents from
line/poly/path geometry, places content with orient `auto`/`auto-start-reverse`/
angle, `markerUnits`, `viewBox`/`refX`/`refY` + overflow clip; drawn in
`execute()` after the shape, bounded by `MAX_MARKER_PLACEMENTS`. Both `<marker>`
and `<pattern>` def nodes are now skipped in scene build (like clipPath/mask) so
they no longer emit unsupported diags. 4 new goldens + 7 new unit tests; updated 3
older tests that asserted patterns-unsupported. **Gate green:** fmt/check/clippy
`--all-targets -D warnings`/test (313 pass, 6 ignored)/validate-svg-import/
check-text-encoding/`cargo run` launch smoke. No new deps. **Next: R10**
(filter linearRGB color-interpolation + precise filter regions + tier-2
feComposite/feBlend/mix-blend-mode) — read
`docs/svg-goal-plan-prompts/R10-*.goal.md` first.

## 2026-06-07 — SVG R9 part 1: vector-effect non-scaling-stroke

On `dev`. Started R9 (markers/vector-effect/patterns). **Shipped the vector-effect
pillar fully**: new `VectorEffect` enum + `parse_vector_effect`, non-inherited
`Style.vector_effect` field (reset in `inherit_parts` like opacity), parsed in
`apply_declaration`. `effective_device_stroke(style, xform, length_bases)` divides
user-space stroke width + dash metrics by `affine_max_scale(ctm)` so the stroke
stays constant in device space (the mesh is built in local space then CTM-scaled,
so dividing first restores the requested px width). Unsupported `vector-effect`
values are diagnosed (`vector_effect.unsupported`) at shape lowering. Golden
`r9_non_scaling_stroke` + 2 unit tests. Gate green (fmt/test 304/clippy/scripts).
**R9 NOT complete** — markers and pattern tiling remain (the larger def-subtree
placement/tiling parts; reuse the `<mask>`/`resolve_clip` machinery:
`scene.references.by_xml_id` → render child subtree via `render_shape` into a
buffer). Next: implement markers, then patterns, before flipping R9 in the roadmap.

## 2026-06-07 — SVG R8.1 Conformance + Security Hardening (post-R8 lane 1/5)

On `dev`. Post-R8 lanes now have paste-ready goal prompts in
`docs/svg-goal-plan-prompts/` (R8.1, R9–R12, each ≤4000 chars) and an auto-read
protocol in CLAUDE.md/AGENTS.md (read the lane prompt before starting it).
**R8.1 shipped**: in-repo deterministic fuzz harness in
`src/canvas/svg_rasterizer.rs` test module (`fuzz_rng`/`fuzz_mutate`/`fuzz_drive`/
`fuzz_run` + `fuzz_smoke_decoders_never_panic` always-run and
`fuzz_decoders_no_panic_bounded` ignored 8k sweep), seed corpus in
`tests/fixtures/svg_fuzz/` (seed.svg, seed_path.txt); 9 new `w3c_*` ASCII goldens
filling feature gaps (currentColor, rgb(), fill-opacity, use, nested transform,
polyline, circle, ellipse, alpha mask) in `src/canvas/svg_golden.rs`; memory-cap
regressions (oversized canvas/document, path-token flood, inflate ceiling); new
`docs/SVG_PRECISION_AND_BENCH.md` precision+benchmark policy (flags sRGB-vs-
linearRGB filter boundary as the R10 gap). No new deps; both embedded sources
stay std-only. Next lane: **R9** (markers/vector-effect/patterns) — read
`docs/svg-goal-plan-prompts/R9-markers-vector-effect-patterns.goal.md` first.

## 2026-06-06 — SVG R8 Conformance + Report UI (roadmap R0–R8 closed)

On `dev`, SVG R8 closes the renderer roadmap. New `src/panels/svg_report.rs`:
`report_summary(&SvgRenderReport)` is a pure, unit-tested mapping to display rows
(fidelity / rendered / skipped / warnings / unsupported + per-diagnostic lines
with byte-span provenance); `show_report(ui, src)` renders it with a
rendered-report / SVG-source toggle (egui temp memory) and a read-only source
viewer. Wired into `panels::properties::show_image` for the selected SVG widget
(computes `rasterize_with_report` at a fixed 256px — no new report logic).
Added a polygon-geometry golden, an `#[ignore]` benchmark
(`raster_benchmark_complex_scene_within_budget`, measures parse+scene+raster of a
200-rect gradient/clip/stroke scene), and a dev-only
`reference_oracle_scene_is_deterministic` (`#[ignore]`) — external reference
renderers stay CI-artifact/dev-only, never runtime deps (note: don't write the
banned crate names in `src/`; `validate-svg-import.ps1` greps for them). Tests:
297 passed / 5 ignored; clippy --all-targets, fmt, validate-svg, encoding,
ignored exported-project compile all clean. Roadmap R0–R8 marked complete;
deferred lanes (progressive JPEG, R6 raster-text snapshot, filter tier 2/3) stay
tracked + runtime-diagnosed.

## 2026-06-06 — SVG R7 Masks + Filters Tier-1

On `dev`, SVG R7 is done in `src/canvas/svg_rasterizer.rs`, all on the R4
offscreen pipeline. Masks: alpha + luminance (`mask-type`), rendered by lowering
the `<mask>` subtree through the existing `render_shape`/`PaintSampler` into a
premultiplied buffer, reduced to a coverage alpha, then multiplied into the
masked element's isolated offscreen. Filters tier-1: a primitive graph
(`feGaussianBlur` = separable triple box-blur radius-capped, `feOffset`,
`feFlood`, `feMerge`/`feMergeNode`, `feColorMatrix` matrix/saturate/
luminanceToAlpha, `feDropShadow`) run in premultiplied space, with
`in`/`SourceGraphic`/`SourceAlpha`/named results. `LayerRaw`/`ResolvedLayer`
gained `mask_ref`/`filter_ref`; `LayerFrame` borrows the `&ResolvedLayer` so
`EndLayer` applies filter→mask before `composite_offscreen`. Shapes with
mask/filter now get a synthetic layer (`shape_layer`) in `build_items`. Parser:
`fe*` primitives + `mask`/`filter` defs retained (skipped in scene build);
`femerge` added to `is_container_tag`. mask/filter attrs no longer diagnosed as
unsupported (the dead `PendingDiagnostic::Unsupported` variant was removed).
Tier 2/3 primitives pass through with `filter.unsupported_primitive`; blur radius
+ offscreen caps bound everything (huge stdDeviation completes). Tests: 3 new
goldens (luminance mask, feOffset, feFlood+feMerge) + 8 unit tests; full suite
293 passed, ignored export compile + clippy/fmt/validate-svg/encoding clean.
Next: R8 conformance/benchmarks/report UI.

## 2026-06-06 — SVG R6 Text Import (chunked multi-label, phase 1-2)

On `dev`, SVG R6 editable text import is done (TEXT_IMPORT_PLAN phases 1-2) in
`src/svg_import.rs`. `<text>`/`<tspan>` now parse into a `TextChunk` model that
splits at every absolutely-positioned span (`x`/`y`); each non-empty chunk
imports as its own editable `Label` (was: one collapsed label). Sibling chunks
share a new `SvgImportMetadata::text_group` id (schema field, `#[serde(default)]`,
backward-compatible). Relative/styled spans flatten into the surrounding chunk
with `text.tspan_adjust`/`text.tspan_style` diagnostics; per-chunk `text-anchor`
and `dominant-baseline` apply with `text.baseline` for approximated baselines;
`text.missing_font` flags placeholder metrics. `text_widget` → `text_widgets`
(returns `Vec`); `flatten_text` → `tspan_text` (warning-free concat) +
`build_text_label`. Raster text rendering (vector snapshot), textPath, bidi, and
shaping stay deferred (phase 3+); rasterizer still buckets `<text>` as
unsupported. Tests: 6 new R6 unit tests + updated `tspan_text` fixture (now 2
labels); full suite 285 passed, ignored export compile + clippy/fmt/validate-svg/
encoding clean. Next: R7 masks/filters.

## 2026-06-06 — SVG R5 JPEG Decoder (baseline JPEG now rendered)

On `dev`, the R5 baseline JPEG follow-on is done in
`src/canvas/svg_rasterizer.rs`: a from-scratch decoder for baseline /
extended-sequential Huffman JPEG (SOF0/SOF1), 8-bit, 1 or 3 components
(grayscale / YCbCr), arbitrary integer chroma subsampling (4:4:4/4:2:2/4:2:0 …)
with restart markers and `0xFF00` de-stuffing. Pipeline: marker parse → quant +
Huffman tables → entropy decode (DC diff + AC RLE, zigzag) → dequantize →
separable 8×8 float IDCT → chroma upsample → YCbCr→RGB, drawn through the same R4
clip/premultiplied image path as PNG (`decode_image_href` now routes `FF D8`
to `decode_jpeg`). Progressive/arithmetic/lossless/12-bit/CMYK →
`image.unsupported_jpeg`; malformed → `image.decode_failed`. std-only, embedded
verbatim by export (single `crate::` import preserved), so in-app and exported
rasterizers decode JPEG identically. 6 JPEG tests (ffmpeg-minted 4:4:4 / 4:2:0
fixtures + a hand-encoded 1-component grayscale fixture + progressive/malformed
guards); full suite 279 passed, ignored export compile + clippy/fmt/validate-svg/
encoding all clean. Deferred JPEG follow-ups: progressive, integer/AAN IDCT,
broader corpus. Next: R6 text import.

## 2026-06-06 — SVG R5 PNG Embedded Images (PNG done, JPEG deferred)

On `dev`, SVG R5 embedded raster images: a zero-dependency PNG `data:` decoder
lands in `src/canvas/svg_rasterizer.rs` (base64 + from-scratch zlib/DEFLATE
inflate with stored/fixed/dynamic Huffman + scanline unfilter + RGBA8 expansion
for color types 0/2/3/4/6 at 8/16-bit; interlace and sub-byte depths diagnosed).
`<image>` lowers to `DrawCommand::Image`/`ImageSkipped` in `DisplayList::build`
and draws through the R4 clip/premultiplied pipeline with
`svg_core::viewbox_transform` `preserveAspectRatio` placement (slice trimmed to
the dest rect), element opacity, `clip-path`, and deterministic nearest-neighbour
sampling. Decode is bounded by pixel/inflate caps; external refs stay fail-closed
at the existing document gate; baseline JPEG is detected and reported
`image.jpeg_unsupported` as a tracked deferred follow-on. Decoder is std-only and
embedded verbatim by export (single `crate::` import preserved), so in-app and
exported rasterizers render PNG identically. 14 R5 tests (real zlib fixtures via
python) + ignored export compile pass; full suite 274 passed; clippy/fmt/
validate-svg/encoding clean. Next: R6 text import, or the JPEG follow-on.

## 2026-06-06 — SVG R4 Complete (clip / overflow / compositing / group opacity)

On `dev`, SVG R4 is done end-to-end in `src/canvas/svg_rasterizer.rs` (single
source embedded verbatim by `src/codegen/export.rs`, so in-app and exported
rasterizers render identically). Added: a layer stack threaded through
`DisplayList` via `BeginLayer`/`EndLayer` markers (scene flattening emits them
for groups/nested-`<svg>` that need clip/opacity/overflow); `clipPath` rendering
(clip-rule nonzero/evenodd, transformed children, both clipPathUnits with shape
bbox for objectBoundingBox, nested clip intersection, reuse of the first-id-wins
reference table with cycle/depth caps); nested-`<svg>` overflow clipping;
premultiplied-alpha offscreen compositing (`blend_pixel_premultiplied` +
`composite_offscreen`, straight-RGBA base/output unchanged); isolated group
opacity (no double-darken) bounded by offscreen depth/byte caps. `opacity` is now
non-inherited (reset in `Style::inherit_parts`) — the key correctness fix; root
opacity no longer cascades to children (minor, documented). Coverage was
refactored into a shared `coverage_scan` used by fills, strokes, and clip masks.
Goldens: clip golden flipped from diagnosed→rendered (justified in DEVLOG) plus
new clip/overflow/group goldens. Hazard: clip resolution happens in
`DisplayList::build` (needs `view_xform`); overflow rect uses the pre-viewport
transform captured in `LayerRaw`. All gates green, zero warnings. Next: R5
embedded raster images.

## 2026-06-06 — Codex SVG R2/R3 Complete

On `dev`, R2 shared style/reference resolution and R3 linear/radial paint
servers are complete for the documented subset. Importer and rasterizer now
share bounded tier-1 CSS/currentColor semantics; raster mode expands guarded
local use/symbol references and renders deterministic gradient fills/strokes
with units, transforms, spread, href inheritance, malformed-value diagnostics,
goldens, and export-embedding coverage. Patterns remain explicit transparent
unsupported paint servers; R4 clipping and compositing is next after the final
whole-repo gate recorded in the devlog.

## 2026-06-06 — Codex SVG R2/R3 Execution

On `dev`, I am continuing directly from the verified dirty R1 tree into R2
shared style/reference semantics and then R3 paint servers. R2 must close
selector specificity/order, currentColor, duplicate IDs, and bounded local
`defs`/`symbol`/`use` expansion before R3 consumes those references for
gradients. The renderer and `svg_core` remain embedded export sources, so all
new code stays std-only, bounded, deterministic, and covered by the existing
embedding/export compile contracts.

## 2026-06-06 — Codex SVG R1 Stroke Execution

On `dev`, SVG R1 stroke execution and final whole-repo gates are complete.
The renderer now retains path semantics, tessellates local-space caps/joins,
supports dashes and `pathLength`, uses separate fill winding/parity and
stroke-union 8x8 coverage, and reports bounded-work truncation. Export embedding
is enforced by a source-contract test; the next SVG work is R2 shared
style/reference resolution, not more R1 geometry. Full tests, strict clippy,
SVG validation, dependency/encoding policy, export compile, performance smoke,
and launch smoke all passed.

## 2026-06-06 — Codex SVG R1 Stroke Plan

On `dev`, the next R1 work is decomposed into a local-space stroke mesh,
cap/join/miter semantics, dash runs, antialiasing coverage, then exact bounds
and transform torture tests. The current renderer transforms centerline points
before applying an untransformed device-space width, so scaled strokes and
translucent overlapping segment quads are the first correctness hazards to
remove. R2 should begin only after these R1 geometry invariants and goldens are
closed.

## 2026-06-06 — Codex SVG R1 Fill Rules

On `dev`, inherited SVG `fill-rule` semantics now reach the owned display list
and raster backend. The renderer correctly defaults to nonzero, supports
explicit evenodd, respects inherited and inline-style precedence, and warns on
invalid final declarations without discarding the inherited rule. Analytical
winding tests and golden fixtures protect the distinction; stroke tessellation
is the next R1 slice.

## 2026-06-06 — Codex SVG R1 Viewport Semantics

On `dev`, R1 now has shared full `preserveAspectRatio` semantics for root and
nested SVG viewports: `none`, all nine alignments, `meet`/`slice`, and
per-viewport percentage bases. Analytical alpha-bound and pixel tests caught
and removed a leaked 20px designer-placeholder minimum from importer viewport
math and corrected raster fill to pixel-center coverage. Nested viewport
overflow clipping remains R4; the next R1 slice is nonzero/evenodd fill rules.

## 2026-06-06 — Codex SVG R0 Closure

On `dev`, SVG R0 is now closed: importer and rasterizer share strict
length/unit parsing; raster nodes have stable preorder IDs and exact byte spans;
local IDs/reference uses have independent caps and first-id-wins behavior; and
non-local structured references are rejected. `DisplayList` now owns lowered
shape/path geometry, resolved style/transform state, diagnostics, and
provenance, so the scene can be dropped before raster execution. The next SVG
work is R1 geometry quality, not more metadata or another parser split.

## 2026-06-06 — Codex SVG Roadmap Consolidation

On `dev`, I consolidated the scattered Stage 7.x, Stage 9, renderer R0-R8, and
Stage 15 SVG language without deleting historical context.
`docs/SVG_RENDERER_ROADMAP.md` is now the sole detailed SVG execution authority:
R6 owns text/tspan work, R8 owns report/source-viewer UX, and Stage 15 remains
a separate deferred general-renderer decision. The next implementation target
is R0 closure: stable source-spanned node IDs, a bounded reference table,
scene/display-list-only traversal, and shared SVG length parsing.

## 2026-06-06 — Codex Lazare Editor Stabilization

On `dev`, the Lazare stabilization pass now replaces heuristic UUID searches
with exact generated/parser source ranges and paints canvas-authoritative
selection outlines in a dedicated editor gutter. Invalid edits remain visible
without mutating `UiTree`; empty code clears widgets; duplicate paste repairs
UUIDs, offsets placement, regenerates canonical code, and selects the new
widgets. Canvas input now derives from response/layer/focus ownership instead
of an expanding utility-window list; remaining IDE-depth work is search,
symbols, diagnostic navigation, diff view, and generated/user-region ownership.

## 2026-06-05 — Codex Code Highlight Polish + Window Input Isolation

On `dev`, I am tightening the generated-code selection outline after visual
inspection showed the border either clipping at the panel edge or crossing the
selected code text. The next fix is to add real editor padding and draw a
subtle IDE-style outline outside the glyph bounds, while keeping the selected
code readable. I am also checking modal/window input isolation because the
shortcut window scroll wheel and Rust Wiring drag are leaking events to the
canvas behind them; View also needs a direct Preview Mode menu item in addition
to F5.

## 2026-06-05 — Codex Code Highlight Outline + Launcher Trace

On `dev`, I am fixing the generated-code selection highlight so it no longer
uses green TextEdit span backgrounds or copied preview blocks. The new approach
draws a foreground outline from `TextEditOutput` galley rows, so it maps to the
actual wrapped/scrolled text layout. I also confirmed the user's `rohkai`
PowerShell 7 shortcut currently runs `cargo run` from `D:\dev\rohkai`, and I am
making `scripts/run.ps1` print branch/commit/dirty-state so future launches prove
which source version is running.

## 2026-06-05 — Codex Layout-Aware Spacers

On `dev`, I reviewed the recent layout passes and am implementing the next
depth step: spacers that behave as layout items instead of only standalone
markers. The intended source-of-truth rule is conservative: `VerticalSpacer`
flexes inside `VLayout`, `HorizontalSpacer` flexes inside `HLayout`, and other
widgets keep their current fixed size while layouts assign absolute canvas rects.
This updated canvas reflow, layout properties, parser/codegen/export tests, and
docs without adding a separate layout model. First-slice parser hierarchy
round-trip, container-level stretch, group/ungroup ownership, and grid child
reorder are included; still open are per-child policies, richer slot editing,
and multi-level layout semantics.

## 2026-06-04 — Codex Layout Properties + Outline Hierarchy

On `dev`, I am adding the first real layout-ergonomics pass after the V/H/Grid
ownership slices. The intended scope is concrete and source-of-truth-driven:
`WidgetProps` gains layout spacing and GridLayout column count, `UiTree` reflow
uses those values, live/export codegen mirror the same grid column boundaries,
and the canvas grid preview reflects the selected column count. I am also
changing Layers/Outline from "flat list with indented children wherever they
happen to be in draw order" to an explicit parent/child row model so owned
layout children read as hierarchy; deeper slot editing and parser round-trip
remain future work.

## 2026-06-04 — Codex GridLayout Ownership Slice

On `dev`, I am continuing the layout-depth pass by extending the shared
layout-ownership path from VLayout/HLayout to GridLayout. The bounded target is
a real first slice: GridLayout owns direct children, reflows them row-major into
a default 3-column grid, and live/export codegen nests them inside an
`egui::Grid` with `ui.end_row()` boundaries. This does not add editable
row/column/stretch properties yet; those remain a follow-up once the ownership
semantics are proven by tests and the generated-project compile fixture.

## 2026-06-04 — Codex HLayout Ownership Slice

On `dev`, I extended the VLayout ownership work into a shared stack-layout path
for both `VLayout` and `HLayout`. `UiTree::attach_to_stack_layout_at()` and
`UiTree::reflow_stack_layouts()` now own the model behavior; VLayout stacks
children vertically and HLayout divides direct children horizontally with the
same default margin/spacing assumptions. Canvas drop/release/resize, live
codegen, export, unit tests, and the generated-project compile fixture now cover
HLayout-owned children too. Still open: GridLayout cell ownership, layout
policies/properties, layout-aware spacers, parser round-trip, and richer
Layers/Outline operations.

## 2026-06-04 — Codex VLayout Ownership Slice

On `dev`, I started the layout-depth pass with only `VLayout`, using the existing
`WidgetInstance.children` source-of-truth relation rather than adding a parallel
layout model. `UiTree::attach_to_vlayout_at()` attaches/detaches children based
on final canvas drop center, and `UiTree::reflow_vlayouts()` gives direct
children absolute canvas rects inside the parent so existing hit-testing,
selection, save/load, and child rendering still work. Canvas palette/template
drops, drag release, and VLayout resize now call that model path; live codegen
and export emit direct children inside `ui.vertical(|ui| { ... })`, and the real
generated-project compile fixture includes a VLayout-owned child button. Still
open: HLayout/GridLayout ownership, spacing/alignment/stretch properties,
layout-aware spacers, parser round-trip for layout hierarchy, and deeper outline
semantics.

## 2026-06-04 — Codex Pre-Release Reliability + SVG Maturity

On `dev`, I moved the roadmap toward depth-first release closure and verified the
all-built-in-widget export fixture instead of trusting string-level tests. The
real ignored generated-crate `cargo check` exposed and fixed an SVG Image export
bug: generated apps now embed both `svg_core` and `rohkai_svg`, use
`rohkai_svg::rasterize_or_fallback()`, and compile with every built-in widget
plus FilePicker/rfd and SVG Image in one project. SVG validation now runs the
golden renderer harness, and `src/canvas/svg_golden.rs` covers supported buckets
plus unsupported gradient, unsupported clip, opacity, stroke/path, and unsafe
external href behavior. Roadmap truth was reconciled so display-list split and
golden harness are not listed as both done and undone; source spans/reference
tables/text/layout remain future SVG work, not complete.

## 2026-06-03 — Codex Lazare/QoL Release Hardening

On `dev`, fixed the visible code-panel/Lazare QoL issues and then corrected the
first pass after user feedback. Selected-widget code highlighting is now native
to the editable TextEdit layouter as a subtle span background, not underline and
not a hand-painted overlay rectangle; do not reintroduce estimated
char-width/line-height geometry because it drifts from egui wrapping/spacing.
Highlight ranges still exclude the `CentralPanel` preamble and follow the full
canvas selection set, so multi-select highlights every selected widget block and
deselecting clears them. Canvas rubber-band selection now previews candidate
widgets while dragging and requests repaint after release so multi-selection is
visible immediately. Deleting all code clears canvas widgets through
`UiTree::clear_widgets()` and resyncs to canonical empty generated code.
Paste is more editor-like:
orphan widget constructor lines create widgets, and duplicate pasted generated
blocks with the same `widget_<uuid>` create fresh-offset instances with new UUIDs,
then the buffer canonicalizes immediately so it stays instant/stable. The left
panel keeps tabs + stable scrolling, the unnecessary hard width cap was relaxed
back so users can widen it, and a new `Stack` toggle lets Palette/Properties/
Layers/Components/Templates appear together as collapsible sections in the same
left panel. Layers is currently an outline/draw-order panel, not a separate
Photoshop-like layer creation system; UI copy now says "Layers / Outline" and
points users to Palette/Templates to add items. The roadmap now explicitly says
VLayout/HLayout/GridLayout are layout-intent MVPs, not true Qt/Lazarus-style
layout managers, and lists the real closure criteria: child ownership, canvas
reflow, spacing/alignment/stretch properties, layout-aware spacers, hierarchy-aware
hit testing/outline/delete, and nested codegen/export/parser parity. I also
expanded the generated-export cargo check
fixture to cover FilePicker/rfd, channels, iterator method export, simple local
trait binding, and state bindings; this uncovered and fixed the invalid
`fn name(&self) -> Vec<_>` iterator signature. Verified: fmt, check, 164 tests +
1 ignored, clippy, launch smoke, ignored compile fixture previously, encoding.
Did not touch SVG/renderer.

## 2026-06-03 — Claude Generated-Export Compile Proof

On `dev`, closed the long-standing "string-level proof only" gap for event/async
export. Added a real `cargo check` fixture in `codegen/export.rs` tests:
`export_compile_fixture_cargo_check` (`#[ignore]`, ~30s with cached deps) writes a
generated project to a unique std-only temp dir (`env::temp_dir()` + pid/nanos),
runs `std::process::Command` `cargo check` with a shared `CARGO_TARGET_DIR`, and
panics with stderr on non-zero exit. The fixture tree covers the full surface:
top-level Button Click + DoubleClick, two async buttons (Plain + Result), and a
Frame with TextInput (LostFocus) + Slider (DragStopped) children, plus `name: String`
and `vol: f32` bindings. A fast always-run smoke
(`export_compile_fixture_generates_required_files_and_matrix`) proves generatability
+ matrix markers without compiling. Verified the ignored test actually PASSES
(generated crate compiles in 29.87s). Also added
`button_click_and_double_click_both_emitted_no_suppression` to lock the ordering
decision (both fire, egui-native; Click not suppressed). No new crates (pure std +
the cargo toolchain). 159 tests + 1 ignored, gate clippy clean, fmt + encoding clean.
Updated `docs/PROMPT_CONTRACT.md` with a "compile proof vs string proof" section.
Did not touch SVG/WASM/DB/renderer/widget-maker.

## 2026-06-03 — Claude Nested/Frame-Child Event Export Parity

On `dev`, closed the last event-export path: nested/frame children. `export_child_line`
previously rendered Frame children with NO event handlers (Button child emitted an
empty `.clicked() {}`; combos were dead `Label` placeholders). Now it routes every
`supported_events()` event through `rust_wiring::handler_call()` + the central
registry, via new `export_child_event_dispatch` (binds `let child_response = ui.put(…)`
then `if child_response.<method>() { … }` per wired event) and `export_child_combo`
(renders a REAL interactive `egui::ComboBox` via `allocate_ui_at_rect`, gated on
`child_combo.inner == Some(true)`). Threaded `handler_registry` into `export_child_line`
+ its Frame call site. Handler collection already iterated all `tree.widgets` (children
included), so registry/conflict/async-fields already covered children — only the call
site was missing; a nested conflict test proves top-level↔child detection. Event
ordering decided + documented: Button Click+DoubleClick both fire (egui native; Click
not suppressed). Tests +9: nested invariant over every `(kind,event)` pair, 6 focused
nested (Button Click/DoubleClick, TextInput LostFocus, Slider/SpinBox DragStopped,
Checkbox Change), interactive-combo-child, top-level↔nested conflict. 157 tests, gate
`cargo clippy -- -D warnings` clean, fmt + encoding clean. Both top-level and nested
export now have full parity — no Properties event row is ignored. Remaining proof gap:
no `cargo build` fixture (string-level only). Did not touch SVG/WASM/DB/renderer/widget-maker.

## 2026-06-03 — Codex Prompt Contract Standard

On `dev`, added `docs/PROMPT_CONTRACT.md` as the standard skeleton for Codex-to-Claude
and Claude-to-Codex implementation goals. This came from the async/event parity
misses: prompts that said "full parity" still let agents stop at top-level or
primary-event paths. The new contract requires deriving the source-of-truth set,
enumerating all runtime/export/nested/custom paths before edits, stopping if any
required path is excluded, and adding invariant tests so hidden output paths cannot
drift quietly. `AGENTS.md`, `CLAUDE.md`, and `CODE_INDEX.md` now point at it.

## 2026-06-02 — Claude FULL Event Export Parity (primary + secondary)

On `dev`, completed event export parity: the prior patch wired only PRIMARY events
(Click/Change); secondary events (Button DoubleClick, TextInput/TextArea LostFocus,
Slider/SpinBox DragStopped) were still exposed in Properties but dropped by export.
Now export collects a handler from EVERY event field per widget and emits, per
widget, one bound `let evt_response = …;` plus an `if evt_response.<method>() { … }`
for each wired event — all routed through `rust_wiring::handler_call()` + the
central registry. New `event_field_handler`/`event_egui_method`/`event_dispatch_block`
in `codegen/export.rs`; Button/TextInput/TextArea/Slider/SpinBox/Checkbox/RadioButton
arms now call the shared dispatcher (ComboBox/FontComboBox keep bespoke combo gates,
Change-only). Handler collection loop iterates `kind.supported_events()` so conflict
detection covers all event fields. egui 0.29 methods `double_clicked`/`lost_focus`/
`drag_stopped` verified against the live `egui_emitter` path. `primary_event` is now
`#[cfg(test)]` (export no longer needs a "primary" notion). Tests: invariant rewritten
to iterate every `(kind, event)` pair (Result-mode `if let Err` proof + per-event gate
method); +5 focused secondary tests; +1 primary+secondary-on-one-widget; +1
across-event-field conflict test. 148 tests, zero warnings (gate `cargo clippy -- -D
warnings` clean; the 3 `--all-targets` lints are pre-existing: hello_button,
field_collector, templates.rs). Honest remaining gap: container-child export
(`export_child_line`) wires no events — separate pre-existing path. Did not touch
SVG/WASM/DB/renderer/widget-maker.

## 2026-06-02 — Claude Properties/Export Event Parity (Codex Review)

On `dev`, closed the Codex-flagged parity gap: Properties exposed `On Change` for
TextArea, SpinBox, and FontComboBox but export silently ignored those handlers.
Root cause was two independent `match w.kind` statements (Properties vs export)
that drifted. Fix introduces a single source of truth — `WidgetKind::supported_events()`
in `project::schema` (exhaustive, wildcard-free match; new kinds won't compile
until classified) plus `primary_event()`/`is_event_capable()`. Properties'
`show_event_handler` now derives its row list from it via `event_ui_meta`; export's
handler collection uses `primary_event()` to pick Click vs Change and to skip
non-event kinds. Wired the three missing export arms (TextArea/SpinBox mirror
TextInput's `.changed()` + registry `handler_call()`; FontComboBox gates on a new
inner `changed` bool → `font_combo.inner == Some(true)`). Added a top-of-`app.rs`
`!! HANDLER CONFLICTS DETECTED` summary block (in addition to the near-handler
comment). Tests: +9 (4 schema capability, 1 export invariant over all event-capable
kinds proving Result-mode routing, 3 focused TextArea/SpinBox/FontComboBox, 1
FontComboBox no-handler no-dangling-binding). 141 tests, zero warnings, fmt + encoding
clean. Known remaining gap: secondary events (double-click/lost-focus/drag-stopped)
exposed in Properties but not yet exported; no cargo compile fixture. Did not touch
SVG/WASM/DB/renderer/widget-maker.

## 2026-06-02 — Claude Async Wiring Gap Fixes (Codex Review)

On `dev`, fixed four async-wiring gaps from the Codex review. (1) **Repaint gap**:
`rust_wiring::async_repaint_block` now emits `ctx.request_repaint_after(16ms)`
after the drain block whenever any task is in flight, so exported apps repaint
without user input. (2) **Handler contract consistency**: TextInput, Slider,
Checkbox, ComboBox, and RadioButton call sites now route through
`rust_wiring::handler_call()` (previously emitted bare `self.h()` bypassing
async/result semantics). (3) **Conflict detection**: handler collection upgraded
from `HashSet` to `HashMap<name→index>`; if multiple widgets share a handler name
with different async/result modes, the generated code emits a `// CODEGEN CONFLICT`
comment and all call sites are normalized to the first-registered mode. (4) **Test
suite**: 7 new tests (3 in `rust_wiring`, 4 in `export`): repaint block shape, non-button
async launcher routing, conflict warning + call-site normalization, combined 3-widget
async coherence fixture. 132 tests, zero warnings, encoding clean. Remaining gap:
no full `cargo build` compile fixture on generated output (documented in eval doc).

## 2026-06-02 — Claude Async Task Wiring: Overclaim Resolved

On `dev`, replaced the Stage 11 async placeholder (a `thread::spawn` body that was
TODO-only) with a real std-only generated task contract in `codegen/rust_wiring.rs`
+ `codegen/export.rs`. Per async handler the export now emits: `{h}_rx:
Option<Receiver<MSG>>` / `{h}_running: bool` / `{h}_error: Option<String>` (Result)
fields + Default init; a launcher `fn {h}(&mut self)` that guards double-launch,
spawns a thread, and `mpsc::send`s `{h}_worker()`; a free-fn `{h}_worker() -> MSG`
with NO `&mut self` (honest UI/worker split); and a borrow-safe `try_recv` drain at
the top of `update()` that records running/error. MSG = `()`/`Result<(),String>`/
`Option<()>`. Handler collection moved above the ExportedApp struct so async fields
land in struct + Default. 9 new tests (rust_wiring + export). Reclassified async to
Functional MVP in the eval doc + ROADMAP — NOT top-class (worker body is a user
stub; no status-widget binding/cancellation/compile-fixture yet). 125 tests, zero
warnings, encoding clean. Preserved all uncommitted Codex work (e.g. FilePicker rfd
dep, Chart painter upgrade) — did not touch SVG/WASM/DB/renderer/widget-maker.

## 2026-06-02 — Codex Remaining Roadmap Evaluation

On `dev`, I added `docs/feature-evaluation/remaining-roadmap-items.md` to cover
unchecked roadmap work with anti-misread closure contracts. It distinguishes
nearby MVPs from actual closure for `.rkwb` bundles, Visual Widget Maker, SVG
text/import maturity, inline SVG expansion, parallelism, Form Layout, Formula
Widget, WASM, DB/data integration, Own Renderer, and high-risk widgets. I also
called out duplicate/stale SVG renderer checklist entries that need roadmap
reconciliation. Docs-only change layered on top of the existing uncommitted
Stage 10 remediation and feature-evaluation work.

## 2026-06-02 — Codex Stage 11 Evaluation

On `dev`, I audited Claude's Stage 11 Rust-centric implementation and added
`docs/feature-evaluation/rust-centric-visual-features.md`. The doc separates
real vertical slices from overclaims: ownership overlay and error-mode signatures
are the strongest; channels/iterator pipelines are functional MVP generators;
trait binding and macro palette are raw text/power-user surfaces; async task
wiring is currently a design-time spawn TODO, not a working async pipeline. This
is documentation/evaluation only and does not change Stage 11 code.

## 2026-06-02 — Codex Feature Evaluation Docs

On `dev`, I added `docs/feature-evaluation/` as the canonical product-depth
evaluation set. It defines the shared depth scale, then audits app shell,
canvas, widgets/components, codegen/Lazare/export, SVG, custom widgets, project
infrastructure, preferences/platform, and testing quality. These docs are meant
to answer "what would top-class look like, what do we actually have, and how do
we measure the gap" without bloating normal preflight context. I only touched
docs in this pass; existing Stage 10 remediation code remains in the worktree.

## 2026-06-02 — Codex Stage 10 Depth Remediation

On `dev`, I remediated the user-flagged Stage 10 depth gaps without touching SVG.
FilePicker export now adds `rfd = "0.14"` to generated Cargo.toml, MathLabel labels
are escaped as data rather than spliced into format strings, and Chart now emits a
real minimal `Vec<f32>` egui painter bar chart instead of a comment. The left rail is
now tabbed and width-capped (Palette/Props/Layers/Components/Templates), and outline
reorder goes through `UiTree::move_to_index` rather than direct `widgets.swap`.
Docs now classify Stage 10 features as Full / Functional MVP / Design-time MVP /
Planned so no agent accidentally overclaims Qt/Lazarus-level depth.

## 2026-05-28 — Claude Stage 11 COMPLETE (Rust-Centric Visual Features)

On `dev`, Stage 11 done after writing `docs/STAGE11_PLAN.md` (full design:
function/depth/UX/impact per feature). All 7 features: (1) Ownership overlay +
(4) Error-flow overlay → `canvas/overlays.rs`, read-only, View-menu toggles,
driven by field_collector + per-widget handler annotations. (2) Async wiring +
(3) Channels + (5) Iterator pipelines + (6) Trait impls → schema
(`WidgetInstance.async_handler`/`handler_result`, `AppProps.rust_wiring`) +
`codegen/rust_wiring.rs` (std-only, NO tokio — uses std::thread+mpsc as the
roadmap's "or similar") + export integration + `panels/rust_wiring.rs` editor
window. (7) Macro palette → `panels/macro_palette.rs`, appends snippets to the
Lazare code buffer. Properties panel gained async checkbox + error-mode dropdown.
108/108 tests (14 new), zero warnings, cargo run smoke OK. Key decision logged in
ROADMAP: tokio deliberately avoided per architecture rules. Remaining open:
Stage 12 (Platform/WASM), Stage 13 (Data/Integration), Stage 15 (Own Renderer).

## 2026-05-28 — Claude Stage 14 COMPLETE (out of order, per user goal)

On `dev`, executed Stage 14 (Project Infrastructure) ahead of 11–13 by user
request. Three items were already satisfied by Stage 8.5 (Help system →
shortcuts.rs, Interactive sandbox → preview.rs F5, Widget hierarchy → outline.rs
Ctrl+L) and are now ticked. New work this session: (1) Undo/redo —
`src/project/undo.rs`, serialized UiTree snapshots, 50-step cap, Ctrl+Z/Ctrl+Y/
Ctrl+Shift+Z, Edit menu, commit boundaries recorded each frame when pointer is up
(coalesces drags); `io::deserialize` extracted for restore. (2) Project tree +
file viewer — `src/panels/project_tree.rs`, File → Project Files…, reads
`export::project_files()` (newly extracted as the single source of truth for disk
export AND viewer). (3) Asset registry — `AppProps.assets: Vec<AssetEntry>`,
`AssetKind`, rfd picker add/remove, `assets/MANIFEST.txt` on export. 96/96 tests,
zero warnings. NOTE: Stages 11 (Rust-Centric Visual), 12 (Platform/WASM), 13
(Data/Integration) still open — user said "we will come back to the others."

## 2026-05-28 — Claude Stage 10 COMPLETE

On `dev`, Stage 10 (Technical & Computational Widgets) done. 11 new WidgetKinds:
ToolButton, CommandLinkButton, DialogButtonBox (button family); MathLabel,
FilePicker, Chart; Table (merges data-table + table-view), ListView, TreeView
(new "Data" palette category); StackedWidget, ToolBox. Plus 2 new ComponentKinds:
StateMachine (usize state field), HttpRequest (String response field) — Timer was
already done in Stage 9. Each widget wired through the full pipeline (schema →
kind_table → widgets/ → canvas render → preview → egui_emitter top+child → export
top+child → palette → properties). New widget files: buttons_ext.rs,
computational.rs, data_views.rs, containers_ext.rs. Notable codegen: FilePicker
emits rfd::FileDialog, Table/ListView emit Grid/ScrollArea from options, TreeView
emits CollapsingHeader. 6 new emitter tests. 89/89 tests, zero warnings
(`cargo clippy -- -D warnings`). Next: Stage 11 (Rust-Centric Visual Features).

## 2026-05-28 — Claude Stage 9 COMPLETE

On `dev`, Stage 9 (Widget Depth & Lazarus Completeness) is fully done. All items
across multiple commits: (1) 11 new widget kinds — TextArea, SpinBox, FontComboBox,
H/V Spacer, GroupBox, VLayout, HLayout, ScrollArea, GridLayout, TabWidget — each wired
through schema → canvas → codegen → export → palette → properties; (2) Properties
schema audit — `text_wrap`, TextInput/TextArea polish, ProgressBar `.fill()`; (3) Full
event list — `on_double_click`/`on_lost_focus`/`on_drag_stopped` with dynamic per-kind
panel; (4) Object Inspector bidirectionality + pending-code warning; (5) Design-time
component tray (`component_tray.rs`) — Timer/DataSource/Lifecycle chips + config +
codegen; (6) SVG scene/display-list IR split (`DisplayList`/`DrawCommand` in
svg_rasterizer.rs); (7) Golden fixture harness (`svg_golden.rs`, #[cfg(test)], ASCII
signatures, 5 fixtures). 83/83 tests, zero warnings. Next: Stage 10 (Technical &
Computational Widgets). Note: `cargo clippy --all-targets` flags 3 PRE-EXISTING lints
(examples/hello_button, field_collector test helper, templates.rs) — not from Stage 9;
the project gate `cargo clippy -- -D warnings` is clean.

## 2026-05-28 — Claude Stage 8.5 Complete

On `dev`, closed the final Stage 8.5 item: keyboard shortcut reference
(`src/panels/shortcuts.rs`). Floating window, F1 or `?` button in menu bar,
categorised shortcut table (File / Canvas / Selection / Grid Snap / Grouping /
Help). `shortcuts_open: bool` added to `SessionState`. All three Stage 8.5
items now ticked. 75/75 tests, zero warnings. Active scope moves to Stage 9.

## 2026-05-28 — Claude Stage 8.5: Outline Panel + Preview Mode

On `dev`, implemented Stage 8.5 — two features landed: (1) Document outline
panel (`src/panels/outline.rs`) — Ctrl+L toggle, "Layers" section in left
panel, accent-dot rows in draw order, click-select, Ctrl+click multi-select,
double-click canvas-center, drag-to-reorder z-order, Frame children indented,
read-only in preview mode; (2) Preview mode (`src/canvas/preview.rs`) — F5
toggle, actual egui widget rendering at 1:1 zoom, `PreviewState` holds live
runtime values keyed by binding, code panel hidden, status bar indicator,
PREVIEW badge + exit button overlaid. `allocate_ui_at_rect` replaced with
`allocate_new_ui` (deprecated in egui 0.29). 75/75 tests, zero warnings.

## 2026-05-25 - Codex Widget Maker Taxonomy Docs

On `dev`, I clarified that `src/panels/widget_builder.rs` is a Guided
Descriptor Builder, not the true Visual Widget Maker the product still needs.
The roadmap now treats Advanced Descriptor Editor, Guided Descriptor Builder,
and future Visual Widget Maker as separate layers, and
`docs/VISUAL_WIDGET_MAKER.md` sketches the future mini-canvas/primitive
composition path. This was a docs-only pass; no Rust behavior changed.

## 2026-05-25 - Codex SVG Path Tokenizer Core

On `dev`, I continued the SVG shared-core cleanup by moving SVG path data
tokenization into `src/svg_core.rs`. The importer now uses the shared lexer for
placeholder bounds and diagnostics, while the rasterizer uses the same tokens
and still keeps its own pixel-flattening semantics. One hazard was broader
unknown-command recognition: rasterizer parsing now explicitly skips unsupported
command payloads so it cannot stall when shared tokenization emits an unknown
letter. Verification passed: format check, cargo check, 75/75 tests, clippy,
SVG validation, and text-encoding guard.

## 2026-05-25 - Codex SVG Transform Core

On `dev`, I continued the SVG reduce/reuse/recycle cleanup by moving affine
transform math and transform-list parsing into `src/svg_core.rs` as
`Affine2D`. `src/svg_import.rs` now aliases its old `Matrix` to the shared
type, and `src/canvas/svg_rasterizer.rs` now aliases its old `Transform` to the
same shared type with `apply_f32` for pixel geometry. This removes another
duplicated parser/math surface while preserving importer bounds behavior and
rasterizer pixels. Verification passed: 69/69 tests, clippy, and
`scripts/validate-svg-import.ps1`.

## 2026-05-25 - Codex SVG Core Extraction

On `dev`, I started the reduce/reuse/recycle SVG cleanup by adding
`src/svg_core.rs` for shared zero-dependency SVG microsyntax. The first wired
slice moves shared color parsing and SVG number-list parsing under one tested
module, then uses it from both `src/svg_import.rs` and
`src/canvas/svg_rasterizer.rs`. This removes duplicated color tables/number
scanners without changing the public import/render API. Verification passed:
67/67 tests, clippy, and `scripts/validate-svg-import.ps1`.

## 2026-05-25 - Codex SVG Scene Boundary

On `dev`, I added the first internal `SvgScene` boundary in
`src/canvas/svg_rasterizer.rs`: XML nodes now flatten into scene items with
accumulated transforms, resolved inherited style, and unsupported-subtree flags
before raster output. This is not the full display-list/source-span/reference
IR yet, but it is a real step toward it and it fixes shape-level `transform`
attributes rendering. Verification passed with 65/65 tests, clippy, and
`scripts/validate-svg-import.ps1`; I avoided Widget Builder files except for
rustfmt touching already-dirty files.

## 2026-05-25 — Claude Beginner Widget Builder

On `dev`, added `src/panels/widget_builder.rs` — a guided beginner-friendly
entry point for creating `.rkwd` descriptors. Split inspector (name, id
auto-derive, Label/Button/RawTemplate type, label default, click handler) +
live canvas preview. "Advanced Descriptor…" closes the builder atomically and
hands the current draft to the full editor via `DescriptorEditorState::from_descriptor`.
Four `descriptor_editor.rs` helpers promoted to `pub(crate)`. 8 new tests.
63/63 tests, zero warnings. Entry points: File → Create Custom Widget…, Widgets menu.

## 2026-05-25 - Codex SVG Renderer Diagnostics Tightening

On `dev`, I continued the SVG renderer R0 track by moving unsupported-feature
diagnostics away from raw source scanning and toward parsed node/attribute
reporting in `src/canvas/svg_rasterizer.rs`. New tests prove comments no longer
create fake unsupported diagnostics and unsupported definition children count as
skipped. I also ran `cargo fmt`, which mechanically formatted recent
uncommitted guide/bezel files from the current working tree; behavior was not
changed there.

## 2026-05-24 — Claude Guide Drag Fixes

On `dev`, fixed two ruler-guide bugs: (1) guides created from ruler click were
not immediately draggable — fixed by setting `*dragging = Some(id)` right after
`guides.push()` in `rulers::handle_interaction`; (2) dragging a guide also
fired rubber-band selection — fixed by adding `guide_drag_active: bool` to
`CanvasSettings`, set each frame from `session.dragging_guide.is_some()`, which
gates the `just_pressed` block and clears `rubber_band`/`drag` while a guide is
held. Also fixed descriptor editor window stretching by replacing all
`desired_width(ui.available_width())` with explicit computed widths + centered
`default_pos`. 53/53 tests, zero warnings.

## 2026-05-24 — Claude Stage 8 Close-out: Guide Snap, Lock Ratio, Canvas Bezel

On `dev`, closed the remaining three Stage 8 items: (1) Guide snapping —
`interaction.rs` drag loop now checks `tree.app_props.guides` after static widget
alignment; widget edges/center snap to vertical/horizontal guide positions within
`snap_thr` and highlight the snapped guide span; (2) Lock aspect ratio — `lock_aspect_ratio:
bool` on `SessionState`, 🔒/🔓 button in status bar, ratio enforced after DragValue
edits by comparing prev/cur W×H; (3) Canvas bezel — `draw_bezel()` in `rulers.rs`,
View → Show/Hide Canvas Bezel toggle, `show_bezel: bool` on `AppProps`, draws mock
22px macOS-style title bar chrome (three traffic-light dots + centered title) above
canvas rect when enabled. 53/53 tests, zero warnings. Stage 8 fully complete.

## 2026-05-24 — Claude Stage 8: Rulers, Presets, Theming

On `dev`, implemented all Stage 8 clusters (`3885ed1`): (1) Pixel rulers —
`src/canvas/rulers.rs` (new), Ctrl+R toggle via View menu, horizontal +
vertical ruler strips with zoom-aware ticks, click-to-create guides,
drag-to-move, Delete-to-remove, `GuideRule` persisted in `AppProps.guides`; (2)
Document presets — "▾ Preset" dropdown in status bar (9 presets desktop +
mobile), `AppProps` gains `resizable`/`min_size`/`max_size`, export uses them;
(3) Theming — `ThemeSettings` in `AppProps` (dark/light, accent RGB, font size,
corner radius, spacing), View → Theme… floating window, live `apply_theme` each
frame, `.rktheme` save/load, export injects `ctx.set_visuals(...)` when
non-default. 53/53 tests, zero warnings. All existing saves load cleanly
(serde defaults). Remaining Stage 8: guide snapping, canvas bezel.

## 2026-05-24 — Claude .rkwb Bundle + SVG Inline Toggle

On `dev`, closed remaining Stage 7.x non-SVG-maturity items (`338ee65`):
(1) `.rkwb` widget bundle — JSON envelope of multiple `WidgetDescriptor`s,
no new crate; Widgets menu gains "Export Bundle…" + "Import Bundle…";
(2) "Expand SVG inline" toggle per Image widget — checkbox in Properties,
`expand_svg_inline: bool` on schema, `svg_source_arg` helper in emitter
switches between compact `[SVG: N bytes]` and full raw string literal; export
path unchanged (already embeds full SVG). 53/53 tests, zero warnings.
Remaining open 7.x: SVG Import Maturity (Codex track). Next stage is Stage 8.

## 2026-05-24 — Claude Descriptor Editor UI Fixes + Widgets Menu

On `dev`, fixed two bugs in the in-app descriptor editor (`8b3932d`): (1)
`desired_width(f32::INFINITY)` in TextEdit fields caused window to expand to
full RohKai width — replaced with `desired_width(ui.available_width())`; (2)
save-message was cleared the same frame it was set due to eager
`cmd_reload_descriptors()` call — fixed with transition detection (snapshot
`was_saved` before `show()`, reload only on false→true edge). Also added a
"Widgets" top-bar dropdown menu (New Descriptor, Import Definition, Reload,
per-descriptor Edit entries) so descriptors no longer require navigating the
File menu. 53/53 tests, zero warnings, fmt clean. Remaining 7.x: `.rkwb`
bundle; SVG import maturity is Codex's track.

## 2026-05-24 — Claude In-app .rkwd Editor

On `dev`, implemented the in-app descriptor editor (`1104547`): split-pane
`egui::Window` — left = full descriptor form (all fields, collapsible props,
add/remove rows), right = live canvas preview + expanded template TextEdits
updating every frame. Entry: File → New Widget Descriptor… or "Edit descriptor"
button in Custom widget properties panel. Save writes `.rkwd` to `widgets/` and
auto-reloads palette. 53/53 tests, zero warnings. Remaining 7.x: `.rkwb` bundle;
SVG import maturity is Codex's track.

## 2026-05-24 — Claude SVG Source Viewer

On `dev`, added SVG source viewer popup (`76b770e`): properties panel for Image
widgets now shows a "View source" button when svg_source is loaded; clicking
opens a read-only egui::Window with the full SVG text, byte count, and "Copy all"
button. `SessionState.svg_viewer_id` tracks open state; `PropertiesAction::ShowSvgSource`
routes the event. 53/53 tests, zero warnings, fmt clean. 7.x SVG Source Viewing
item ✅. Remaining 7.x open: in-app .rkwd editor, .rkwb bundle, SVG import
maturity (Codex domain).

## 2026-05-24 — Claude Stage 7.x Complete

On `dev`, finished all three Stage 7.x items: (1) handler calling-convention
unification (`egui_emitter.rs` + `code_preview.rs`); (2) descriptor hot-reload
and Import Widget Definition dialog (`app.rs`); (3) Lazare Custom round-trip
(`parser.rs` fallback extracts label/binding from template-expanded lines,
guarded so constructor line wins over handler calls). 53/53 tests, zero warnings,
`cargo fmt` clean. Remaining known gap: `descriptor_props` (`{{prop.KEY}}`
substitutions) don't feed back into Lazare sync — deferred.

## 2026-05-24 — Codex SVG Renderer R0

On `dev`, I am implementing the first SVG renderer roadmap slice: structured
render output/reporting, renderer diagnostics, and tests that prove current
behavior is deterministic and honest. I will preserve the stable `rasterize()`
and `rasterize_or_fallback()` wrappers and avoid touching Claude's recent
handler/hot-reload source work. The key hazard is overclaiming: this pass should
make renderer limits visible, not pretend gradients/text/clips are done.

## 2026-05-24 — Claude Track B + Track A

On `dev`, completed handler calling-convention unification (Track B): live
preview now emits `self.h();` and Tracé stubs use `fn h(&mut self)`, matching
export.rs — `egui_emitter.rs` and `code_preview.rs` touched. Also added
descriptor hot-reload (Track A partial): File → Reload Widget Descriptors
rescans `widgets/` without restart — `app.rs` touched. Remaining Track A work:
Import Widget Definition dialog and Lazare Custom round-trip (label/binding
needing descriptor template awareness). 47/47 tests, zero warnings, two commits.

## 2026-05-24 — Codex Low-Token Docs Consolidation

Working on `dev`, I am consolidating agent prep so future sessions do not burn
context by reading every guidance document by default. The procedural source is
`scripts/preflight-context.ps1`; AGENTS/CLAUDE hold policy; Code CoOp is the
normal short handoff; DEVLOG becomes history-on-demand. I am not changing app
behavior in this pass. Watch for stale older entries that are historically
useful but no longer part of default preflight.

## 2026-05-24 — Codex PowerShell 7 UTF-8 Standardization

Working on `dev`, I installed PowerShell 7 and am updating repo scripts and
agent guidance to prefer `pwsh` with explicit UTF-8 handling. The immediate
hazard is recurring mojibake from legacy Windows PowerShell 5.1 text paths, so
this pass adds a text encoding guard and fixes the known corrupted lines. Next
agents should use `pwsh -NoProfile -ExecutionPolicy Bypass -File ...` for repo
scripts and avoid shell text writers unless `-Encoding utf8` is explicit.

## 2026-05-23 — Claude Stage 7 Gap Fixes + SVG Code Contraction

Three confirmed gaps from the Stage 7 implementation are now fixed: (1)
`descriptor_state_fields` is snapshotted onto `WidgetInstance` and both
`state_emitter` and `export` emit them into AppState; (2) `apply_parsed` guards
against overwriting `WidgetKind::Custom` with a parser-inferred built-in; (3)
`image_preview_line` emits a compact `"[SVG: N bytes]"` placeholder instead of
the full raw SVG, keeping the code buffer readable. Export still embeds full SVG.
Previous DEVLOG claim that `descriptor_props` don't drive live codegen was wrong
and corrected. Roadmap updated with SVG source viewer and descriptor maturity
items. 30/30 tests, zero clippy warnings.

## 2026-05-23 — Claude Stage 7: .rkwd Widget Descriptor Format

Stage 7 is implemented and verified (30/30 tests, zero clippy warnings, clean
build). New `WidgetKind::Custom(String)` variant is live; descriptors load from
`<binary_dir>/widgets/*.rkwd` at startup; palette renders custom categories;
properties panel shows typed descriptor fields; codegen snapshots templates onto
instances so `egui_emitter` and `export` work without threading descriptors
everywhere. `widgets/ply-button.rkwd` is the worked example. Two known gaps
for 7.x: `state_emitter` does not yet emit `state_fields` from descriptors, and
`descriptor_props` changes don't drive live Lazare codegen sync.

## 2026-05-23 - Codex SVG/Image Parity Push

I am continuing from the clean baseline to remove the hollow SVG/Image paths.
The immediate target is live codegen/export parity first, then renderer safety
and semantics, with base verification after each feature set. I will preserve
the no-new-dependency rule and avoid claiming full SVG equivalence unless the
implemented output forms actually prove it. Watch `src/codegen/export.rs`,
`src/codegen/egui_emitter.rs`, and `src/canvas/svg_rasterizer.rs`.

## 2026-05-23 - Codex Baseline Stabilization

I am stabilizing the current dirty worktree without changing SVG behavior. The
first goal is formatting and verification so Codex and Claude have a clean
baseline before deeper renderer/export work. Known hazard: the SVG rasterizer is
real but incomplete and should not be overclaimed as equivalent yet. I will only
make narrow compile/clippy fixes if the verification suite requires them.

## 2026-05-23 - Codex

I am tightening coordination rules rather than changing app behavior. The main
goal is harmony between Codex, Claude, and future agents: `docs/CODE_INDEX.md`
is the map, this file is the short handoff diary, and preflight now points to
both. The current SVG rasterizer work is real but incomplete, especially export
parity and full SVG feature support, so do not overclaim it. The worktree is
already dirty from earlier SVG/Stage 7 changes; avoid broad formatting or
unrelated rewrites unless the user asks.

## 2026-05-26 — Cline Comprehensive Code Review

Performed a full codebase review and produced 9 recommendations across 3 groups
in `docs/CLINE_REVIEW_AND_RECOMMENDATIONS.md` and group-specific files. Added
`rayon = "1"` to `Cargo.toml` as core dependency for app-wide parallelism and
updated `docs/ROADMAP.md` with parallelism foundation tasks for Stage 9. No app
behavior changed. Verification: `cargo check` passes, zero warnings. Key findings:
overall 9/10 — excellent architecture (UiTree single source of truth), zero
clippy warnings, 75 tests passing, strong security practices. Main improvement
areas: codegen memoization, module-level docs, integration tests, and parallel
SVG rasterization (now enabled by rayon).

## 2026-05-24 - Claude

Completed the full 5-patch Rust-ness Remediation Plan. All patches verified
(47 tests, zero clippy warnings, clean fmt) and pushed to PR #3 on dev branch.
Key structural changes: `RohKaiApp` is now decomposed into sub-structs
(`project`, `session`, `messages`, `prefs`, `code`, `descriptors`); all field
accesses in `app.rs` updated. The SVG rasterizer has a typed `Result` API with
`rasterize_or_fallback` as the canvas path. `field_collector` is the single
source of truth for AppState field collection. Next session: `export.rs` handler
parity, or Stage 7 descriptor UX (descriptor hot-reload, palette custom section).
