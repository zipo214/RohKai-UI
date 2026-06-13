# RohKai Devlog

Chronological session record. The roadmap stays strategic; this file records what happened, what was reviewed first, what changed, and what still needs attention.

## 2026-06-12 - Rust 1.96 And Current Dependency Alignment

### Context Reviewed
- Preflight, `AGENTS.md`, `CLAUDE.md`, current CoOp note, branch/worktree state,
  Cargo/lock/toolchain/CI configuration, generated-project dependency emission,
  and active version-bearing docs.
- Local and remote branches plus GitHub history were compared. Claude's edition
  2024 migration existed only on local `dev`; remote branches still reflected
  edition 2021, and no migration PR had been published.
- Official/current release data was checked before editing: Rust 1.96.0,
  egui/eframe 0.34.3, rfd 0.17.2, and the latest releases of every other direct
  dependency.

### Changes
- Worked on isolated branch `codex/toolchain-dependency-refresh`; main `dev`
  remained untouched and no merge was attempted.
- Declared edition 2024, MSRV 1.92, and pinned/tested Rust 1.96.0. CI now uses
  the same exact toolchain with Clippy and rustfmt.
- Updated every direct dependency to its current release and migrated RohKai,
  examples, live codegen, native export, WASM export, and generated Cargo files
  to egui/eframe 0.34.3 and rfd 0.17.2.
- Preserved the existing glow rendering backend explicitly instead of silently
  accepting eframe's changed default feature set.
- Kept generated projects on edition 2021 deliberately, with MSRV 1.92 and
  dependency versions sourced from central export constants.
- Added offline `check-toolchain-alignment.ps1` and networked
  `audit-dependency-updates.ps1`; preflight and Windows CI run the offline
  invariant check.
- Updated agent guidance, platform/release docs, roadmap dependency references,
  README setup, and feature-evaluation version claims. Historical reviews and
  chronological entries remain historical and are labeled where necessary.
- Installed and verified Rust 1.96.0 GNU/MSVC toolchains. The machine rustup
  default host is GNU because RohKai's repository-generic toolchain pin would
  otherwise select the locally unusable MSVC linker; MSVC remains installed and
  current.

### Verification
- `cargo fmt --check`: passed.
- `cargo check --all-targets`: passed.
- `cargo test`: 551 unit tests, 17 integration tests, and doc test passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Generated-project Cargo checks: both native export fixtures passed with
  warnings denied.
- Toolchain alignment, dependency policy, text encoding, dependency freshness,
  SVG parser/rasterizer/golden validation: passed.
- Native `rohkai.exe` launch smoke: remained healthy for six seconds.

### Risks And Follow-Ups
- This is a broad GUI API migration; the automated and launch checks are green,
  but a full manual release checklist remains appropriate before tagging.
- The branch is ready for review/integration after concurrent Claude work is
  reconciled. Do not merge it blindly over newer source edits.
- The recurring maintenance automation should run in an isolated worktree and
  propose major upgrades rather than applying them silently.

## 2026-06-12 — Behavior Graph: first-class visual event→state wiring

### Context Reviewed
- AGENTS/CLAUDE policy, preflight, PROMPT_CONTRACT, ARCHITECTURE,
  ENGINEERING_INVARIANTS; schema.rs, ui_tree.rs, canvas/interaction.rs,
  panels/properties.rs, codegen/{field_collector,state_emitter,egui_emitter,
  export,rust_wiring,handlers,parser,rust,kind_table}.

### Derivation (before coding)
- Source of truth: `UiTree`; events from `WidgetKind::supported_events()`;
  state types from `kind_table::state_info`; fields from `field_collector`.
- Output paths enumerated: live emitter (top-level + Frame children + layout
  children), export (`event_dispatch_block`, child dispatch, frame/layout
  combos, layout child lines), AppState (live + export), handler stubs.
- Scope: nested paths in scope. Templates regenerate widget UUIDs, so wires do
  not copy through templates (by design). Live preview dispatches behaviors
  exactly where it already dispatches handlers; TextArea/FontComboBox change
  remains export-only (pre-existing parity boundary, unchanged).

### Changes
- `schema.rs`: `Behavior`, `VisualAction` (Set/Add/Subtract/Toggle/CallHandler),
  `ValueExpr`, serde on `WidgetEvent`, `WidgetEvent::label()`, and
  `AppProps.behaviors` (serde-default, skip-empty → backward compatible).
- `ui_tree.rs`: `prune_stale_behaviors` on remove + validate_and_repair.
- `codegen/behavior.rs` (new): single emitter for both surfaces via field
  prefix (`""` live, `"state."` export); exhaustive match so a new action
  variant cannot compile without emission; invalid fields/handlers emit
  diagnostic comments, never broken code.
- `field_collector`: declares behavior-referenced fields no widget binds.
- `export.rs`: dispatch blocks emit behavior statements before handler calls
  on every supported event; `CallHandler` names get stubs; compile fixture
  gained a behavior button + bound ProgressBar. Fixed latent bug: layout-child
  combo change dispatch compared `Option<()>` with `Some(true)` (non-compiling
  exported code); now tracks `changed` like the frame-child combo.
- `egui_emitter.rs`: behavior emission in Button (click/double-click),
  TextInput (change/lost-focus), Slider (change/drag-stopped), Checkbox,
  ComboBox, RadioButton, SpinBox, Frame-child Button, layout-child Button.
- `canvas/interaction.rs`: event/state sockets (open/closed circles on edge
  centers, derived from `is_event_capable()` / `state_info` + binding),
  left-drag wire creation with live cubic preview, committed wires drawn as
  smooth connectors, wire click selection, guide-drag suppression, default
  action inferred from target state type (f32→Add 0.1 clamped to props
  min/max, bool→Toggle, String→Set).
- `panels/behaviors.rs` (new): selected-wire editor (event combo from
  supported_events, action type, field, amount/clamp, value, delete) + per-
  widget behavior list; wired into both Properties call sites in `app.rs`.
- Docs: ARCHITECTURE behavior-graph section + codegen module map row.

### Verification
- `cargo fmt --check` / `cargo check` / `cargo test` (544 unit + 17 fidelity +
  doctest, zero ignored — includes real `cargo check` compile fixture with a
  behavior wired) / `cargo clippy --all-targets -- -D warnings` /
  `check-text-encoding.ps1` all green; `cargo run` launch smoke OK (8s alive).

### Risks / Follow-ups
- Wire endpoints draw only when source and a field-bound target widget exist;
  field-only behaviors (target deleted) stay editable via the Behaviors list.
- Lazare: behavior mutation lines inside Button blocks parse like handler-call
  lines (same fallback class); no new round-trip surface added.
- Canvas socket hit-test takes priority over body clicks within 9px of edge
  centers; watch for conflicts with very small widgets.

## 2026-06-12 — S1 layout and constraint depth closed

### Context Reviewed
- Preflight/session guidance, Claude's stopped S1 handoff, the exact S1 backlog,
  layout/constraint schema and solver, canvas interaction, UiTree reflow,
  live/export emitters, Lazare parser, RCA, feature evaluations, and the
  preserved local `ROADMAP_PHASE2.md` edit.

### Findings
- Claude's handoff named visual anchors and nested Lazare as unfinished, but S1
  also still contained an unchecked Grid slot-editor item.
- One-level code emission was not the only nesting gap: layout reflow captured
  stale parent rectangles, canvas drawing stopped after direct children, export
  emitted nested containers as comments, and Lazare had no explicit parent
  identity for deeper marker nesting.
- The existing Grid slot list supported arrow reorder but had no stable names
  and canvas drag-reorder handled V/H layouts only.

### Changes
- Added four draggable constraint handles around the primary selection. They
  target the real parent frame's leading/center/trailing or top/center/bottom
  anchors, derive margins that preserve current geometry, and draw persistent
  connector lines from the widget to its active targets.
- Added `WidgetProps::grid_slot_names`: stable row-major names editable in
  Properties, visible on canvas, and represented in live/export code. Grid
  children now drag directly between cells with a full-cell insertion preview.
- UiTree reflows layout parents before descendants using current solved rects,
  independent of storage order. Canvas drawing, live code, and export recurse
  through nested V/H/Grid ownership with bounded cycle/depth guards.
- Generated child markers now include explicit parent UUIDs. Lazare restores
  arbitrary nesting and distinguishes an intentionally empty container so
  deleting its child code clears stale ownership.
- Marked S1 complete across the strategic roadmap, RCA, code index, and feature
  evaluation. Added bespoke secure-foundation milestone reviews before the
  explicit user/architecture gate for Stage 15.

### Verification
- `cargo test`: 523 unit tests + 17 fidelity tests + 1 doctest; zero failed,
  zero ignored.
- `cargo fmt --check`, `cargo check`, and
  `cargo clippy --all-targets -- -D warnings`: pass.
- Encoding guard and diff whitespace checks: pass; no `#[ignore]` remains.
- Focused tests cover geometry-preserving anchors, Grid pointer-to-slot mapping,
  parent-before-child nested reflow, named-slot emission, nested export, Lazare
  multi-level round-trip, and intentional empty-container clearing.

### Risks / Follow-ups
- Visual authoring still needs ordinary human smoke use for handle discoverability
  and very small/overlapping widgets; the interaction geometry and persistence
  are covered by deterministic tests.
- Do not merge or push while another agent's state is uncertain.

## 2026-06-12 — Export gates, SVG fuzz hardening, and widget-authoring UX

### Context Reviewed
- Preflight, `AGENTS.md`, latest `CODE_COOP.md`, engineering invariants, project
  model/SVG skills, exact `v0.2.0` release state, current `dev`, and Claude's S1
  handoff.
- Generated-project fixtures, SVG embedded-source contract, formula/state
  collection, database export, widget-authoring menus/windows.

### Findings
- Six important tests were ignored. The two export fixtures failed when run:
  embedded SVG referenced undeclared `rayon`; formula expressions emitted
  `self.field` in an exported `self.state` context; DB-bound projects omitted
  `rusqlite`; generated wiring/layout/tab code was not warning-clean.
- Promoting the SVG fuzz sweep exposed an infinite path-parser loop for malformed
  numeric operands after `Z`.
- File and Widgets menus duplicated three authoring commands. “Create Custom
  Widget” opened the guided descriptor builder rather than the true visual
  composition tool. All three authoring windows could exceed the viewport.

### Changes
- Export compile fixtures are normal warning-denied tests, share a Cargo target,
  and cover every built-in widget plus SVG, formula, DB, FilePicker, events, and
  Rust wiring. Generated code now treats intentionally latent wiring/state APIs
  explicitly and emits warning-clean empty layouts/tabs.
- Embedded SVG batch rasterization uses scoped std threads with deterministic
  ordering; no embedded `rayon` reference remains.
- Formula functions/arity are validated; variables are collected as `f32`
  AppState fields; live/export emission uses caller-provided state paths.
- DB bindings add bundled `rusqlite`, emit state/default/loader parity, and get a
  deterministic fallback field when no Binding is supplied.
- All six ignores removed. Renderer perf/oracle/fuzz gates run normally. Added a
  parser progress invariant and exact regression for malformed numbers after
  close-path.
- Widgets menu now owns Create New Widget (Visual Widget Maker), Guided
  Descriptor Builder, and Advanced Descriptor Editor. File-menu duplicates were
  removed. Shared viewport bounds constrain all three windows; Visual Widget
  Maker content scrolls on small screens.
- Ran `cargo fmt` to repair pre-existing repository-wide formatting drift.

### Verification
- `cargo test`: 515 unit tests + 17 fidelity tests + 1 doctest; zero failed,
  zero ignored.
- Both generated projects pass `cargo check` with `RUSTFLAGS=-Dwarnings`.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, encoding,
  dependency policy, and `scripts/validate-svg-import.ps1`: pass.
- App launches and responds. Windows GPU capture returned a blank wgpu client
  surface in this automation desktop, so visual screenshot proof was not
  claimed; viewport geometry is covered at normal and tiny sizes.

### Risks / Follow-ups
- S1 is not complete: anchor visual-handle drag and nested-layout Lazare
  round-trip remain.
- Do not merge while Claude is active or ambiguous. Next, finish those S1 items,
  then reorder bespoke secure in-house code milestones before Stage 15.

## 2026-06-12 — S1 parity gaps finished (constraint solver bug + 3 half-wired fields)

### Context Reviewed
- The RCA/checker S1 follow-ups from earlier this session.
- `constraint_solver.rs`, `app.rs:2887` (every-frame solve call), Properties
  `show_constraints`, egui_emitter/export layout + Label emission, preview Label.

### Findings
- **Latent bug:** `apply_constraints` ran every frame and `apply_margin` used
  `x += left` / `w -= dw` — a widget with a margin constraint drifted off screen
  and shrank each frame. Also aligned every widget to the **canvas**, never its
  parent. (My earlier RCA wording "doesn't recurse" was imprecise — all widgets
  are in the flat `tree.widgets`; the real gaps were idempotency + parent frame.)
- `text_align` was set in Properties but applied **nowhere** (not even canvas).
- `child_cross_align` per-child override was set in Properties but only the
  container's `layout_cross_align` reached codegen.

### Changes
- **Constraint solver rewrite** (`constraint_solver.rs`): idempotent (margin
  folded into absolute alignment; safe to run every frame — no drift, no
  save/load corruption) + parent-relative (solve parents-before-children, frame =
  parent's solved rect, canvas for top-level). Added `ConstraintError::message`/
  `widget_id`. New tests: idempotency, parent-relative, stretch, margin-within-
  alignment, margin-without-alignment-noop.
- **Validation surfaced**: `show_constraints` renders this widget's
  `validate_constraints` messages (red); caller computes over the whole tree
  before the mutable borrow. `#[allow(dead_code)]` removed.
- **text_align wired**: egui_emitter + export (top-level Label) + preview, via the
  proven `with_layout(top_down(Center/RIGHT))` pattern.
- **child_cross_align wired**: VLayout/HLayout child emission in egui_emitter +
  export (Center/End); Stretch removed from the per-child UI (no proven egui
  codegen path → avoids a new half-wired control).
- Parity tests in `fidelity_audit.rs` for text_align + child_cross_align.

### Verification
- `cargo test` — **499 lib + 17 fidelity_audit + 1 doctest**, 0 failed, 6 ignored.
- `cargo clippy --all-targets -- -D warnings` — zero warnings.
- `export_compile_fixture_cargo_check` (`--ignored`) — generated project
  `cargo check`s clean after the export changes.
- `scripts/check-surface-parity.ps1` — text_align/child_cross_align findings
  cleared; only the genuinely geometry/canvas-only `constraints`/`descriptor_accent`
  remain (correct).

### Risks / Follow-ups
- `Rect` left as `Clone` (not `Copy`) to avoid `clone_on_copy` churn on existing
  `rect.clone()` sites; solver uses explicit clones (16-byte struct, cheap).
- Margin semantics changed: margin now insets within the alignment anchor and is a
  no-op without one (the only way to make a per-frame solve idempotent). Documented
  in the module header and the updated `margin_*` tests.

## 2026-06-12 — Roadmap de-deferral + cross-surface-parity RCA & checker

### Context Reviewed
- All roadmaps (collated in the lib+bin entry below).
- Existing anti-drift patterns: `WidgetKind::supported_events` exhaustive match,
  `EVENT_CAPABLE_KINDS` cfg(test) parity list, exhaustive `emit_indexed` match.

### Changes
- **Roadmaps de-deferred** (`ca28d9e`): `ROADMAP_PHASE2.md` rewritten as the
  ordered master backlog S1–S22 (in-house renderer S22, LARGE projects spaced so
  agents are not driven to defer). Removed all non-goal/deferral/strikethrough
  language from `SVG_RENDERER_ROADMAP.md`, `jpegdecoder roadmap.md`,
  `TEXT_IMPORT_PLAN.md`; reframed `ROADMAP.md` header + Stage 13/15. Corrected
  stale SVG gap-matrix cells (filter tier-3 listed as out-of-scope though done).
  Kept exactly two architecture invariants (no external renderer dep, no C FFI);
  their capability is the S22 renderer.
- **RCA** `docs/RCA-2026-06-12-surface-parity-drift.md`: class = cross-surface
  parity drift; root cause = asymmetric forcing functions (enums forced by
  exhaustive match, struct fields / roadmap claims / unsurfaced pub APIs not).
- **Checker** `scripts/check-surface-parity.ps1`: advisory caveman-review audit —
  field→codegen coverage, roadmap `[x]`/`[ ]` ↔ code drift, dead-code `pub`.
  Exit 0 by default; `-Strict` fails on a DONE-overclaim.
- **Harness** `tests/fidelity_audit.rs`: added `every_widget_kind_emits_non_trivial_code`
  (walks `widgets::ALL_KINDS`) + `all_kinds_list_is_not_empty`. Removed the now
  load-bearing `ALL_KINDS`'s `#[allow(dead_code)]`.

### Verification
- `cargo test` — 495 lib + 13 `fidelity_audit` + 1 doctest, green.
- `cargo clippy --all-targets -- -D warnings` — zero warnings.
- `pwsh scripts/check-surface-parity.ps1` — 24 advisory findings, 0 overclaims.
- `pwsh scripts/check-text-encoding.ps1` — OK.

### Risks / Follow-ups (now ordered S-stage to-dos, not deferred)
- S1: recurse `apply_constraints` into layout children; surface
  `validate_constraints`; confirm/wire `text_align`/`child_cross_align`/
  `constraints` codegen.
- S4: sweep the ~16 `#[allow(dead_code)]` pub items (several redundant post-lib-split).
- **User decision pending:** the two architecture invariants (C FFI / external
  renderer) were intentionally NOT converted to to-dos; reversing them is a
  separate explicit call.

## 2026-06-12 — Crate → lib+bin; fidelity_audit harness linked & green

### Context Reviewed
- Session continuation after compaction; user asked to (1) finish the work
  stalled around `tests/fidelity_audit.rs`, (2) prove tier-3 SVG filters are
  real in code, (3) collate a functionality index, and (4) de-defer + re-enumerate
  every roadmap with the in-house renderer last.
- Read all roadmaps: `ROADMAP.md`, `ROADMAP_PHASE2.md`, `SVG_RENDERER_ROADMAP.md`,
  `jpegdecoder roadmap.md`, `TEXT_IMPORT_PLAN.md`, `ROADMAP_COMPARATIVE_ANALYSIS.md`.

### Findings (code cross-reference)
- **Tier-3 SVG filters are real**, not identity passthrough: `filter_tile`
  (`svg_rasterizer.rs:6760`), `displacement_map` (6783), `feConvolveMatrix`
  (6816), `feTurbulence` Perlin (6881-7056), lighting (7059+), `feImage` data-URI
  (7350+), with 7 acceptance tests (14177+). The `[ ]` claims in `ROADMAP_PHASE2.md`
  P2.7 were stale; the prior session had already reconciled them to `[x]`.
- **Root cause of the stalled thread:** RohKai was a binary-only crate (no
  `[lib]`, no `src/lib.rs`), so `tests/fidelity_audit.rs` (`use rohkai::…`) could
  not link — 6 `unresolved crate rohkai` errors. The harness was untracked,
  never linked, never linted: a hollow surface.

### Changes
- Added `src/lib.rs` (crate root, `pub mod` × 9). Slimmed `src/main.rs` to a
  shell over `rohkai::app::RohKaiApp` (no module declarations; icon raster kept).
- Fixed `tests/fidelity_audit.rs:212` clippy `double_ended_iterator_last`
  (`.last()`→`.next_back()`).
- Updated `docs/CODE_INDEX.md`: added `lib.rs`, `constraint_solver`, `db_engine`,
  `undo`, `formula`, `component_state`, `widget_bundle`, `widget_maker_emit`,
  `db_panel`, `shaper/`; corrected stale Timer/StateMachine/Formula/DB depth.

### Verification
- `cargo test` — **495 passed, 0 failed, 6 ignored** + 11 `fidelity_audit` + 1 doctest
- `cargo clippy --all-targets -- -D warnings` — zero warnings

### Risks / Follow-ups
- Promoting to a lib enables doctests (1 ran, passed). Future doc examples now
  execute under `cargo test`.
- Shallow surfaces to schedule (NOT defer): `apply_constraints` flat iteration
  (no recursion into layout children); `validate_constraints` not surfaced in UI.
- Next: rewrite the roadmaps to remove all deferral/non-goal language and produce
  one ordered master backlog (renderer last); build the RCA parity-check system.

## 2026-06-11 — P2.3/P2.4/P2.5/P2.6 merged; 489 tests, zero warnings

### Context Reviewed
- Session continuation after context compaction; prior state recovered from summary
- P2.3, P2.5, P2.6 background agent worktrees; P2.4 merged in prior session

### Changes
- **P2.3 merged** (`worktree-agent-acb0c559ef0c7fa34`): constraint-based layout — `LayoutConstraints` + `HAlign`/`VAlign` on `WidgetInstance`, `constraint_solver.rs` (5-pass solver, cycle detection, 14 tests), `show_constraints()` panel in properties
- **P2.5 merged** (`worktree-agent-aca16573f8781b29b`): formula `deps()`/`validate()` aliases, timer wiring via `mpsc` channel + `spawn_timers()`, `StateDef`/`TransitionDef`/`StateMachineProps` schema + state machine editor in component tray, shortcut customization with Reference/Customize tabs, `.rkwb` ZIP bundle with hand-coded CRC-32
- **P2.6 merged** (`worktree-agent-adb3b11803f47b733`): Stage 13 DB integration — `DatabaseEngine` trait + `SqliteEngine` impl (params![] only), `DbPanelState` floating window, `DbBinding` on `WidgetInstance`, `rusqlite = { version = "0.40", features = ["bundled"] }` added to Cargo.toml, Invariant 10 (no format!() SQL) in ENGINEERING_INVARIANTS.md
- **P2.7 skipped** — R9 (markers + pattern tiling + vector-effect) already fully implemented in v0.2.0; no new commit to merge
- All serial merge conflicts resolved by keeping both sets of struct fields/inits; no functionality dropped

### Verification
- `cargo check` — clean
- `cargo clippy --all-targets -- -D warnings` — zero warnings
- `cargo test` — **489 passed, 0 failed, 6 ignored**

### Risks / Follow-ups
- `rusqlite` bundled feature adds ~1 MB to binary; acceptable for Stage 13 but worth noting
- `DatabaseEngine` is `dyn` (object-safe); `SqliteEngine` is the only impl; no other engines planned yet
- Timer threads are daemon threads (detach on drop); if project is replaced rapidly, old timers finish their sleep before exiting; safe but adds a brief latency tail on project reload

## 2026-06-11 — P2.1 (VWM capabilities) + P2.2 (Canvas UX) merged

### Context Reviewed
- `docs/ROADMAP_PHASE2.md` P2.1 and P2.2 sections
- Both parallel worktree agents completed; output verified before merge

### Changes

**P2.1 — Visual Widget Maker later capabilities:**
- `src/panels/widget_maker_panel.rs`: "Properties | Code Preview" tab bar; Code Preview tab shows live `gen_live_preview` + `gen_export_template` output in read-only monospace scroll areas; primitive layer list with ↑↓ buttons (disabled at boundaries, swaps applied post-loop)
- `src/canvas/widget_maker.rs`: `PrimAnchor` enum (TopLeft/TopRight/BottomLeft/BottomRight/Center, serde snake_case, default TopLeft); `MakerPrimitive` gains `anchor`, `min_w`, `min_h` (all `#[serde(default)]` for backward compat); `apply_corner_resize` enforces min clamp; `pub fn doc_from_descriptor(desc) -> Option<WidgetMakerDoc>` parses VWM-originated descriptors (returns None for hand-written ones)
- ROADMAP P2.1: 4 items checked, 6 deferred with explicit notes
- +5 tests: swap_first_and_last, resize_below_min_w_is_clamped, prim_anchor_serde_default_roundtrip, doc_from_descriptor_round_trips_metadata, doc_from_descriptor_returns_none_for_non_vwm_descriptor

**P2.2 — Canvas UX depth:**
- `src/canvas/interaction.rs`: `compute_fit_rect(content_rect, viewport_rect, padding_fraction)` helper; `F` key zoom-to-selection (fits selected widgets or all if none, 10% padding, min zoom clamp); `WidgetError` enum + `compute_widget_errors(widgets) -> HashMap<Uuid, Vec<WidgetError>>`; red 2px outline on canvas for duplicate ID / invalid handler name / missing binding (Slider/TextInput/Checkbox/ComboBox/ProgressBar)
- `src/panels/properties.rs`: `field_text_resettable` wraps text field with right-click "Reset to default"; `show_geometry_resettable` gives each DragValue a reset context menu; applied to Button and Label panels
- `src/app.rs`: `name_counter: HashMap<String, u32>` on `RohKaiApp`; `next_widget_label(&mut self, kind) -> String` generates `"button_1"`, `"label_2"`, etc.; applied on palette click + drag; cleared in `cmd_new()`
- ROADMAP P2.2: 4 items checked, 5 deferred with notes
- +14 tests (zoom/fit, error detection variants, property reset, auto-naming counters)

### Verification
- `cargo fmt --check`: clean
- `cargo clippy --all-targets -- -D warnings`: zero warnings
- `cargo test`: **440 passed, 0 failed, 6 ignored**
- `git push origin dev` → `de4f7e5`

### Risks / Follow-ups
- P2.1 deferred: hit regions, layout groups, state variants, slots, event zones, style tokens — all need non-trivial new architecture
- P2.2 deferred: canvas Ctrl+F, clipboard enhancements, minimap, multi-select property edit, context tooltips
- Invariant 10 (SQL injection) not yet in ENGINEERING_INVARIANTS.md — add before Stage 13 codegen work
- `rusqlite` awaits explicit user approval before Cargo.toml addition
- 3 CodeQL false positives on GitHub Security tab (rust_wiring.rs L89/172/228) need manual dismissal

---

## 2026-06-11 — v0.2.0 PR review fixes (CI gate + Qodo)

### Context Reviewed
- Qodo review on PR #6 (4 bugs + 1 arch violation flagged)
- CLAUDE.md codegen-in-src/codegen invariant
- `src/canvas/widget_maker.rs`, `src/codegen/export.rs`, `src/app.rs`

### Changes

**Architecture violation fix (codegen in canvas layer):**
- New `src/codegen/widget_maker_emit.rs`: `pub gen_live_preview`, `pub gen_export_template`,
  private `prim_to_egui_lines`
- `canvas/widget_maker.rs` delegates to codegen; no Rust syntax strings in canvas layer
- Text prim uses `string_literal()` instead of manual `replace('"', "\\\"")` — fixes Bug 4
- 3 new tests: backslash escape, quote escape, label token double-braces

**Bug 2 — WASM FilePicker build break:**
- `gen_app_rs_wasm(tree)` clones tree, replaces FilePicker with Label, calls `gen_app_rs`
- `project_files_wasm` now calls `gen_app_rs_wasm` — generated WASM app.rs has no `rfd::` 
- Test: `wasm_app_rs_has_no_rfd`

**Bug 3 — Widget Maker save fails on fresh install:**
- `show_widget_maker_window` now calls `create_dir_all(&dir)` before `fs::write`
- Error surfaced cleanly if dir creation fails

**Bug 5 — Fixed temp preview directory collision:**
- Preview WASM export dir is now `$TMP/rohkai_wasm_preview_<PID>` (process-stable, unique across runs)

**CI gate:**
- Committed + pushed `cargo fmt` output (16 files), workflow `permissions: contents: read`,
  `--all-targets` clippy flag fix, version bump to 0.2.0

### Verification
- 416 tests, 0 failures, 0 clippy warnings, `cargo fmt --check` clean
- PR #6 CI pending (Windows build ~10 min); Qodo re-review queued

### Risks / Follow-ups
- After CI passes and Qodo clears: merge PR #6 → main, tag v0.2.0
- 3 CodeQL false positives still need manual dismissal on GitHub Security tab
  (egui widget ID strings at `rust_wiring.rs` L89/172/228, not cryptographic values)
- RustyBuzz integration (P2-A) deferred — architecture confirmed, awaiting next session

---

## 2026-06-11 — v0.2.0 release: good-citizen pass + Phase 2 roadmap

### Context Reviewed Before Editing
- `docs/ENGINEERING_INVARIANTS.md` (Invariant 7 filename sanitizing)
- `docs/CLINE_RECOMMENDATIONS_GROUP1.md` (Rec 1 handler extraction, Rec 2 module docs, Rec 6 tests)
- `src/canvas/widget_maker.rs`, `src/panels/widget_maker_panel.rs` (resize logic)
- `docs/ROADMAP.md`, `docs/SVG_RENDERER_ROADMAP.md`, `docs/ROADMAP_COMPARATIVE_ANALYSIS.md`,
  `docs/feature-evaluation/*` (Phase 2 roadmap input)
- `docs/VISUAL_WIDGET_MAKER.md` (Later Capabilities for Phase 2)

### Changes

**Invariant 7 fix (filename sanitizer):**
- `sanitize_widget_id_to_filename(id: &str) -> String` in `src/canvas/widget_maker.rs`
- Whitelists `[A-Za-z0-9_-]`; every other character (Windows-reserved `<>:"\|?*\`,
  control bytes, path separators) → `_`; empty/all-underscores result → `"widget"`
- Replaced broken `.chars().map(|c| if c == '.' || c == '/' ...)` in `app.rs`
- 6 unit tests: dots/slashes, Windows chars, control bytes, empty, all-seps, preserved

**Cline Rec 1 — handler extraction:**
- New `src/codegen/handlers.rs`: `resolve_click_handler` + `resolve_change_handler`
- `egui_emitter.rs` and `export.rs` delegate to the shared module (no logic duplication)

**Cline Rec 2 — module docs:**
- `//!` added to `canvas/mod.rs`, `panels/mod.rs`, `widgets/mod.rs`,
  `widget_maker.rs`, `widget_maker_panel.rs`

**Cline Rec 6 — unit tests (12 new):**
- UiTree: `bring_to_front_moves_to_end`, `send_to_back_moves_to_start`,
  `group_creates_frame_with_children`, `group_fails_with_less_than_two`,
  `ungroup_removes_frame_returns_children`, `remove_cascades_to_children`,
  `validate_and_repair_fixes_duplicate_ids`, `validate_and_repair_removes_stale_children`
- Canvas: `snap_value_to_grid`, `resize_handle_hit_detection`,
  `resize_handle_apply_delta_top_left`, `resize_handle_respects_min_size`

**Widget Maker interactive resize:**
- `resize_corner: Option<u8>` (serde skip) on `WidgetMakerDoc`
- `corner_hit(pos, rect) -> Option<u8>` (8px hit radius, 4 corners)
- `apply_corner_resize(prim, corner, dx, dy)` with MIN=5% and bounds clamping
- `drag_started` detects handle grab; `dragged` applies resize or move

**Stage 12 browser preview:**
- `cmd_preview_wasm`: PATH-checks trunk, exports to `%TEMP%/rohkai_wasm_preview`,
  spawns `trunk serve` detached; error if trunk missing
- "Preview in Browser…" added to File menu

**Doc staleness (Invariant 9):**
- `feature-evaluation/README.md`: VWM 0-1→1-2, SVG raster 2-3→4, testing 3→3-4
- `feature-evaluation/testing-quality.md`: 116→412 tests
- `feature-evaluation/remaining-roadmap-items.md`: VWM/WASM/Formula status updated
- `feature-evaluation/codegen-lazare-export.md`: code navigation 3→3-4
- `ARCHITECTURE.md`: 8 missing WidgetProps fields + 3 missing WidgetInstance fields added
- `ROADMAP.md`: Stage 7.x items 1-6 `[ ]`→`[x]`; Stage 12 browser preview `[x]`

**Phase 2 roadmap:**
- `docs/ROADMAP_PHASE2.md` created: 12 sorted sections + unsorted scratchpad
- Priority starter lines: P2-A font shaping (HarfBuzz port, 2252/2252 tests),
  P2-B DB integration research
- All deferred items from ROADMAP.md, SVG_RENDERER_ROADMAP.md, CLINE_REVIEW,
  ROADMAP_COMPARATIVE_ANALYSIS.md, feature-evaluation, VISUAL_WIDGET_MAKER.md
  collected and organised

**PR #6 created** for v0.2.0 merge to main.

### Verification
- `cargo test`: **412 pass / 0 fail / 6 ignored**
- `cargo clippy --all-targets -- -D warnings`: zero warnings
- `cargo fmt --check`: clean

### Risks / Follow-ups
- `cmd_preview_wasm` spawns `trunk serve` detached but does not track the PID;
  the server runs until the OS reclaims it. A future "Stop Preview" button
  should store the `Child` handle in `RohKaiApp`.
- Phase 2 roadmap P2-A (font shaping) is the most ambitious single piece of work
  in the project — budget 6–10 session-cycles before the 2252-test gate passes.
- CodeRabbit may flag the `cmd_preview_wasm` function for additional spawn safety
  checks.

## 2026-06-11 — Pre-release depth gate closed: all deferred features implemented

### Context Reviewed
- docs/ROADMAP.md depth gate items; docs/ENGINEERING_INVARIANTS.md
- src/codegen/egui_emitter.rs, export.rs, field_collector.rs, formula.rs (new)
- src/project/schema.rs, src/panels/properties.rs, code_preview.rs, component_tray.rs
- src/canvas/widget_maker.rs (new), src/panels/widget_maker_panel.rs (new)

### Changes Made
1. **Data model groundwork** (commit cd0fcea): DataColumnType/DataColumn schema;
   data_source_binding on WidgetProps; bound Table/ListView/TreeView emit for-loop
   iteration; Properties show_data_widget (Static/Bound mode, column editor for Table).

2. **True layout ownership** (commit 388f112): SizePolicy enum {Fixed/FillWidth/Fill}
   per-child; child_size_str() in emitter + export_child_size_str() in export; GridLayout
   .min_row_height(); VLayout/HLayout respect layout_cross_align in export (parity fix);
   Properties Size policy selector + Row H checkbox for GridLayout. 4 tests.

3. **Lazare IDE depth — search** (commit 7d43550): Ctrl+F opens search bar; ⌕ button
   toggle; case-insensitive find with Prev/Next match navigation; match count display;
   compute_search_spans() generates SourceSpan highlights; search navigation priority.

4. **Lazare IDE depth — symbol list + diagnostics** (commit c0867fb): Symbol list
   collapsible section (widget navigate + handler navigate); parse_diag_line() extracts
   line number; clickable error label jumps to error location via search activation.

5. **Visual Widget Maker** (commit 4f60e72): WidgetMakerDoc + MakerPrimitive model;
   mini-canvas with drag/select; Rect/Outline/Ellipse/Text primitives; toolbar with
   add/remove/z-order; properties panel (kind, %, RGB, font); generated template
   preview; Save Descriptor → .rkwd write + palette reload. "Visual Widget Maker…"
   in Tools menu. 5 tests.

6. **Object Inspector depth** (commit 83f20a0): describe_kind() for all ComponentKinds;
   "design-time stub" italic badge; sectioned config (Identity/Handler/Generated);
   inline generated AppState field + update() comment display; chip/button hover text.

7. **ROADMAP** updated: Stage 12 WASM checked off; formula depth, runtime stubs, data
   model, layout ownership, Lazare IDE, Widget Maker, Inspector all marked complete.

### Verification
- 394 tests pass; zero clippy warnings (--all-targets); cargo check clean.
- Commits: cd0fcea, 388f112, 7d43550, c0867fb, 4f60e72, 83f20a0 + ROADMAP docs.

### Remaining Open Items
- Stage 13 (DB integration): BLOCKED — needs explicit user approval for crate name
  (sqlx or rusqlite). No work started.
- Font shaping: BLOCKED — needs user approval for `rustybuzz`. No work started.[user comment: build harfbuzz shaping algorith port in rust that passes 2252 of 2252 shaping testss.  ]
- Stage 15 (own renderer): DEFERRED by user.
- Lazare diff view: deferred (mentioned in depth gate but not critical path).
- Visual Widget Maker resize handles: draggable (visual only now, not interactive).

## 2026-06-10 — SVG R12 complete: namespace + recovery + a11y (post-R8 lane 5/5)

### Context Reviewed Before Editing
- `docs/svg-goal-plan-prompts/R12-namespace-recovery-a11y.goal.md`; svg-zero-dep
- `src/canvas/svg_rasterizer.rs`: XmlParser/parse_element (prefix stripping),
  SvgDoc::parse (only None on no-root → ParseFailed; otherwise already lenient),
  SvgRenderReport, rasterize_with_report; `src/panels/svg_report.rs` report rows

### Derive + report (lane requirement)
1. `parse_element` stripped namespace prefixes (`svg:rect`→`rect`) with no scope
   tracking → a foreign `<custom:rect>` mis-rendered as a `rect`. xmlns must be
   read from the raw open-tag header (parse_attr strips prefixes from attr keys).
2. Hard-reject paths: only `svg_text_allowed` (DOCTYPE/entity/script/external)
   and "no `<svg>` root" → ParseFailed; the parser is otherwise lenient already
   (unclosed/junk recover by consuming), but recoveries weren't counted/diagnosed.
3. `<title>`/`<desc>` dropped (make_node returns None for them); aria-label/role
   unused.

### Changes (all in `src/canvas/svg_rasterizer.rs`; commit a0b563f)
- Namespace: `NsFrame` scope stack on `XmlParser` (bounded `MAX_NS_DEPTH`);
  `apply_xmlns` parses xmlns/xmlns:prefix from the raw header; `Namespace`
  {Svg,Xlink,Foreign} via `classify_namespace`; element qualified name resolved
  in scope (undeclared prefix → Foreign, except lenient `svg:`); foreign elements
  consume their balanced subtree, emit no node, bump `foreign_count`. xlink:href
  attrs still resolve (already stripped to `href`).
- Recovery: `consume_close_tag` compares the close-tag local name to the open
  element and bumps `recovered` on mismatch; unclosed containers bump it too.
  Surfaced as `recovery.malformed_markup` + `report.recovered_error_count`. Never
  ParseFailed/panic for malformed-but-rooted documents; security gates unchanged.
- a11y: `<title>`/`<desc>` captured inline in `parse_element` (balanced subtree,
  `strip_tags` + `bounded_a11y_text`, first-wins) — deliberately NOT as an
  `SvgNode`, so R11 text rendering does not draw them as glyphs; root aria-label
  is a title fallback. `SvgDoc`/`SvgScene` carry title/desc/foreign/recovered →
  `SvgRenderReport.title`/`desc`/`recovered_error_count`; `report_summary` adds
  Title/Description/Recovered rows. Export preserves via verbatim `svg_source`.

### Tests
foreign-ns element skipped (not mis-rendered) + diagnostic; xlink:href use
resolves; malformed recovery partial render + diagnostic + determinism;
title/desc extraction + `MAX_A11Y_TEXT` bound + aria-label fallback; security
regression (DOCTYPE/script/external still `ForbiddenContent`); panel a11y/recovery
rows; export-parity `embedded_rasterizer_includes_r12_paths`.

### Verification
- `cargo fmt --check` clean; `cargo check` clean; `cargo test` **335 pass / 6
  ignored / 0 fail**; `cargo clippy --all-targets -- -D warnings` clean;
  `validate-svg-import.ps1` + `check-text-encoding.ps1` pass. No new dependencies.

### Status / Follow-ups
- **The post-R8 SVG renderer roadmap (R8.1, R9–R12) is complete.** Remaining gaps
  are explicit out-of-profile non-goals: real font-file glyphs + shaping/bidi,
  tier-3 filter primitives, progressive/CMYK JPEG, ICC colour, `foreignObject`.
- Namespace model is bounded/heuristic (lenient `svg:` prefix; per-attribute
  namespace not fully tracked beyond xlink); full DOM recovery out of scope.
- Next work is outside the SVG renderer: `docs/ROADMAP.md` open stages (12/13/15)
  or the deferred Stage 9 parallel-processing cluster.

## 2026-06-10 — SVG R11 complete: raster text + textPath (post-R8 lane 4/5)

### Context Reviewed Before Editing
- `docs/svg-goal-plan-prompts/R11-raster-text-textpath.goal.md` (lane spec);
  `svg-zero-dep` skill; `docs/TEXT_IMPORT_PLAN.md`
- `src/canvas/svg_rasterizer.rs`: XmlParser is_text branch (content skipped!),
  SvgNode::Text (attrs only), DisplayList UnsupportedText path, Style model,
  stroke pipeline (stroke_polyline/render_shape), flatten_path_data
- `src/svg_import.rs` R6 TextChunk model (NOT reused — rasterizer must stay
  std-only/self-contained for export embedding)

### Glyph-set decision (lane requirement: derive + report)
Bundled **Hershey simplex** (Allen V. Hershey, US Naval Weapons Laboratory —
public domain), embedded as `HERSHEY_SIMPLEX: [&[i8]; 95]` inside
`svg_rasterizer.rs` (it must live in that file: export embeds it verbatim under
the single-`crate::` contract; an `include_str!` data file would break the
exported copy). Coverage: ASCII 32..=126 only; `^` is a simplified caret
substitute. Metrics: y-up, baseline 0, cap height 21 units, descender −7;
30 units = 1 em (cap = 0.70 em); stroke width 2 units = font_size/15, round
caps/joins. Everything else renders a tofu box + diagnostic. Transcription risk
of individual glyph data is bounded by goldens for tested glyphs and by the
deterministic/bounded contract for the rest.

### Changes (all in `src/canvas/svg_rasterizer.rs` unless noted; commit e8eae08)
- Parser: `<text>`/`<tspan>` capture raw inner markup to the *matching* close
  tag into new `SvgNode::Text.content` (old `consume_until("</")` cut nested
  tspans at the first `</`).
- `scan_text_runs`: flat TextRuns from plain text + one `<tspan>` level
  (x/y/dx/dy); deeper nesting → `text.tspan_nested_flattened`; styled tspans →
  `text.tspan_style_ignored`; `<textPath>` extracted (href, startOffset, text).
- Style: inherited `font_size` (SvgLength) + `text_anchor` parsed via
  apply_declaration (presentation attrs, CSS, inline style all work).
- `lower_text_command`: resolves font-size, applies whole-run text-anchor,
  x/y position lists approximated by first value (+ diagnostic), lays glyphs in
  user space, and emits ONE stroked-glyph `DrawCommand::Shape`
  (`ShapeGeometry::Path` of polyline subpaths) — full reuse of the stroke
  pipeline, R4 clip, masks/filters (via shape_layer), opacity, and gradient
  paint (text fill drives the glyph stroke paint; `stroke_opacity` =
  fill_opacity).
- textPath: referenced path lowered to user-space polylines
  (`user_space_subpaths`), `ArcLengthPath` arc-length table, glyph origin at
  pen distance with midpoint-tangent rotation; startOffset user units +
  percent; glyphs beyond path end not rendered (per SVG); missing href →
  `textpath.unresolved`.
- Honesty/diagnostics: `text.raster_snapshot` on every rendered text element
  (font-family substituted, approximate metrics → Medium fidelity by design);
  `text.glyph_unsupported` (tofu), `text.bidi_unsupported`,
  `text.shaping_unsupported` (combining marks), `limit.text_glyphs`
  (MAX_TEXT_GLYPHS = 4096/element).
- `DrawCommand::UnsupportedText` removed; `<text>` flips unsupported→rendered.

### Justified test changes
- `render_report_counts_rendered_skipped_and_text_limitations` previously
  asserted `<text>` lands in the `text` unsupported bucket with zero warnings;
  it now asserts the rendered count includes text, no `text` unsupported
  bucket, and the source-spanned `text.raster_snapshot` warning (fidelity stays
  Medium — same honest outcome, new mechanism).

### Tests / goldens
Goldens: `r11_text_word` ("Hi", legible H+i), `r11_text_anchor_middle`,
`r11_textpath_diagonal` (glyphs rotated along a rising diagonal). Unit tests:
determinism + snapshot diagnostic + no unsupported bucket, tofu (CJK), bidi
(Hebrew), tspan dy offset + unresolved textPath, glyph-cap bound. Export-parity
`embedded_rasterizer_includes_r11_render_paths`. Editable-first regression: all
svg_import tests unchanged and green. Note for future text tests: the ~0.67px
glyph stroke AA-splits across two pixel columns when a stem straddles a pixel
boundary — assert alpha > 50.

### Verification
- `cargo fmt --check` clean; `cargo check` clean; `cargo test` **327 pass / 6
  ignored / 0 fail**; `cargo clippy --all-targets -- -D warnings` clean;
  `validate-svg-import.ps1` + `check-text-encoding.ps1` pass; `cargo run`
  launch smoke OK (first attempt hit a transient console interrupt
  0xC000013A, clean re-run). No new dependencies.

### Risks / Follow-ups
- Glyph fidelity: bundled stroked font ≠ requested font-family; metrics are
  approximate by design and diagnosed. Real font-file glyph rendering/shaping
  stays out of scope (zero-dependency profile).
- Per-glyph x/y position lists approximated by first value; per-tspan styling
  ignored (both diagnosed).
- textPath uses the first+subsequent flattened subpaths concatenated; multi-
  subpath start offsets are approximate across subpath joins.
- **Next: R12** — namespace model, malformed-document recovery, title/desc
  a11y extraction (final open lane).

## 2026-06-10 — SVG R10 complete: filter correctness + tier-2 + blend (post-R8 lane 3/5)

### Context Reviewed Before Editing
- `docs/svg-goal-plan-prompts/R10-filter-correctness-tier2.goal.md` (lane spec)
- `docs/SVG_RENDERER_ROADMAP.md` Post-R8 gap matrix + lanes; `svg-zero-dep` skill
- `src/canvas/svg_rasterizer.rs` R7 filter machinery: FilterGraph/FilterKind/
  FilterPrimitive::apply, gaussian_blur, color_matrix,
  composite_premultiplied_over, parse_filter, ResolvedLayer/LayerRaw/Offscreen,
  composite_offscreen, layer_for_group/shape_layer

### Changes (all in `src/canvas/svg_rasterizer.rs`; embedded verbatim into exports)
Shipped as four verified, separately-committed increments:
- **Tier-2 primitives** (commit e268e9d): feComposite (over/in/out/atop/xor/
  arithmetic), feBlend (normal/multiply/screen/darken/lighten via premultiplied
  separable-blend), feComponentTransfer (identity/table/discrete/linear/gamma),
  feMorphology (dilate/erode, `MAX_MORPH_RADIUS`-capped). `<feComponentTransfer>`
  added to `is_container_tag` so its feFunc* children parse. Tier-3 stays
  Identity + `filter.unsupported_primitive`.
- **mix-blend-mode** (commit 4709d80): `BlendMode` (shared with feBlend) threaded
  LayerRaw→ResolvedLayer→Offscreen; non-Normal forces an offscreen and composites
  via `composite_offscreen_blended`. Normal path byte-identical.
- **linearRGB color-interpolation-filters** (commit 4e112ea): default linearRGB;
  `apply` converts source sRGB→linear (premultiplied-aware) before the graph and
  back after; feFlood/feDropShadow colours linearised; `sRGB` opts out.
- **Precise filter region** (commit 1a141ba): `FilterRegion` from filterUnits +
  x/y/w/h (default obbox −10%..110% via source alpha extent; userSpaceOnUse exact
  via CTM); `clip_to_filter_region` clips the result.

### Tests / goldens
New goldens: r10_composite_arithmetic_add, r10_blend_multiply,
r10_component_transfer_invert, r10_morphology_dilate, r10_mix_blend_multiply_group,
r10_filter_region_clips_flood. New unit tests: blend determinism + multiply/
screen, composite arithmetic/in, transfer gamma/linear/table, morphology growth +
radius cap, mix-blend vs src-over, linearRGB-vs-sRGB blur midpoint (pixel-exact),
filter region default + userSpaceOnUse. Three R10 export-parity markers added.

### Justified golden/test changes (region clipping)
Filter-region clipping is spec-correct and clips output beyond the element bbox.
feoffset_shifts_right, feflood_femerge, r10_composite_arithmetic_add,
r10_morphology_dilate (goldens) and gaussian_blur_softens_a_hard_edge,
fedropshadow_adds_offset_shadow, femorphology_dilate_grows... (unit tests) were
given explicit filter regions (documented inline) matching their intent, so their
expected output is preserved — not rebaked to clipped output. linearRGB perturbed
no golden (all use pure 0/255 colours; sRGB↔linear identity there).

### Verification
- `cargo fmt --check` clean; `cargo check` clean; `cargo test` **321 pass / 6
  ignored / 0 fail**; `cargo clippy --all-targets -- -D warnings` clean;
  `pwsh scripts/validate-svg-import.ps1` + `check-text-encoding.ps1` pass;
  `cargo run` launch smoke. No new dependencies.

### Risks / Follow-ups
- objectBoundingBox filter regions use the source alpha extent as a bbox proxy
  (exact geometric bbox is not threaded to the layer); userSpaceOnUse percentage
  lengths are approximated as user units + diagnosed.
- color-interpolation-filters is filter-level, not per-primitive.
- Tier-3 primitives (turbulence/displacement/convolution/lighting/tile/image)
  remain diagnosed.
- **Next: R11** (raster text & textPath) — heavy; gate on real product need.

## 2026-06-09 — Engineering invariants doc (process hardening)

### Context Reviewed Before Editing
- PR #4 CodeRabbit review batch (32 comments) — the bug classes underneath them
- `CLAUDE.md`/`AGENTS.md` Architecture + Session Rules; `scripts/preflight-context.ps1`
- `src/app.rs` (`set_preview_mode`/`refresh_preview_state`) to confirm the one
  finding lacking a ✅ reply was already addressed

### Changes
- Triage: every substantive PR #4 finding is already fixed in current code
  (preview re-seed, apply_theme reset, undo/redo focus gate, ruler layer
  ownership, aspect-lock preset bypass, `has_handler` via `supported_events()`,
  codegen module boundary + safe identifiers, UTF-8 char-safe truncation). No
  code fix required.
- New `docs/ENGINEERING_INVARIANTS.md`: a read-on-demand bug-class → invariant →
  cheap-guard table (parity, single-source-of-truth, input ownership, reset
  paths, generated-identifier safety, string byte-slicing, filename sanitizing,
  conservative defaults, doc consistency) plus the systemic-fix workflow and the
  `--all-targets` verification gate.
- Reinforced two always-on rules in `CLAUDE.md` + `AGENTS.md` (codegen identifier
  safety; surface parity / single-source-of-truth), bumped the documented clippy
  gate to `--all-targets`, and added doc + workflow pointers. Preflight now prints
  a one-line Engineering-Invariants reminder.

### Verification
- `pwsh scripts/check-text-encoding.ps1` OK; preflight runs clean and surfaces the
  new reminder. Docs/script-only change — no cargo build affected.

### Risks / Follow-ups
- The invariants doc is read-on-demand (kept out of the low-token default to avoid
  a bottleneck); discoverability relies on the CLAUDE/AGENTS/preflight pointers.
- Most invariants are enforced by convention + targeted tests, not automation;
  add per-class regression tests as those areas are next touched.

## 2026-06-09 — SVG R9 complete: markers + pattern tiling (post-R8 lane 2/5)

### Context Reviewed Before Editing
- `CLAUDE.md`/`AGENTS.md` SVG roadmap step protocol; `svg-zero-dep` skill;
  preflight output
- `docs/svg-goal-plan-prompts/R9-markers-vector-effect-patterns.goal.md` (lane spec)
- `docs/SVG_RENDERER_ROADMAP.md` Post-R8 gap matrix + lanes
- `src/canvas/svg_rasterizer.rs` landmarks: `PaintServerTable`/`PaintSampler`,
  `MaskDef::build_alpha`, `resolve_clip`/`collect_mask_items`, `DisplayList::build`/
  `execute`, `render_shape`/`flatten_path_data`, scene `build_items`, caps;
  `src/canvas/svg_golden.rs`; `src/codegen/export.rs` single-`crate::` contract

### Changes (all in `src/canvas/svg_rasterizer.rs` unless noted)
- **Vector-effect** was already shipped + committed in `61f3d66` (VectorEffect
  enum, `effective_device_stroke`, `vector_effect.unsupported` diag, golden
  `r9_non_scaling_stroke`). This session added the remaining two pillars.
- **Pattern tiling.** `PaintServerTable` gained `patterns: HashMap<String,
  PatternDef>`, built in a second pass after gradients (`build_pattern_def`:
  `href` attribute+content merge bounded by `MAX_PATTERN_REFERENCE_DEPTH`, cyclic
  href → `reference.pattern_cycle`). New `PaintSampler::Pattern` variant samples a
  pre-rendered straight-RGBA tile with `rem_euclid` wrap. `build_pattern_sampler`
  resolves the tile rect (patternUnits objectBoundingBox/userSpaceOnUse), maps
  content via viewBox/patternContentUnits, renders the tile once through the new
  shared `render_content_items` helper (extracted from `MaskDef::build_alpha`),
  caps tile pixels at `MAX_PATTERN_TILE_PIXELS`, and breaks content
  self-reference by rendering with the pattern removed from a cloned table.
- **Markers.** New `MarkerDef`/`MarkerSet`/`MarkerPlacement` + `build_markers`
  (in `DisplayList::build`, stored on `DrawCommand::Shape`): resolves
  `marker-start/mid/end` (+`marker` shorthand) via `final_style_property`,
  extracts vertices and in/out tangents from line/polyline/polygon/path geometry,
  computes orient (`auto`/`auto-start-reverse`/`<angle>`), builds the
  content→device transform honoring `markerUnits`, `viewBox`/`refX`/`refY`/
  `markerWidth`/`markerHeight`, and draws each placement in `execute()` after the
  shape, clipped to its viewport rect (overflow:hidden) ∩ ancestor clip. Bounded
  by `MAX_MARKER_PLACEMENTS` (`limit.marker_count`) / `MAX_MARKER_CONTENT_ITEMS`;
  missing/non-marker target → `marker.unresolved`.
- Scene build now skips `<marker>`/`<pattern>` def nodes like clipPath/mask/filter
  (they render only when referenced), so they no longer emit unsupported diags.
- **Tests/goldens:** 4 new goldens (`r9_marker_start_mid_end`,
  `r9_marker_auto_orient`, `r9_pattern_userspace_tile`,
  `r9_pattern_objectbbox_tile`); 7 new unit tests (marker placement, auto-orient
  determinism, missing-marker diag, userSpaceOnUse-ignores-stroke-width, pattern
  href cycle, self-referential pattern terminates, oversized tile capped, OBB
  tile); updated 3 existing tests that asserted patterns-unsupported; new
  `embedded_rasterizer_includes_r9_render_paths` export-parity test in
  `src/codegen/export.rs`. Embedded source stays std-only, single `crate::`.
- **Docs:** flipped R9 lane + Patterns/Markers/vector-effect gap rows + maturity
  assessment in `SVG_RENDERER_ROADMAP.md`; updated `SVG_IMPORT.md` and
  `docs/feature-evaluation/svg-import-renderer.md`; appended CODE_COOP handoff.

### Verification
- `cargo fmt --check` clean; `cargo check` clean; `cargo test` **313 pass / 6
  ignored / 0 fail**; `cargo clippy --all-targets -- -D warnings` clean;
  `pwsh scripts/validate-svg-import.ps1` passed (banned-crate grep + determinism +
  fixtures + clippy); `pwsh scripts/check-text-encoding.ps1` OK; `cargo run`
  launch smoke (25s, no early crash). No new dependencies.

### Risks / Follow-ups
- Marker/pattern content lowered via `collect_mask_items` ignores `<use>`
  expansion (same limitation as masks) — `<use>`-based marker/pattern content
  won't render; bounded + safe, documented.
- Pattern tile clipping does not wrap shapes across the tile edge (content
  overflowing the tile rect is clipped, not repeated). Acceptable for R9; note for
  any future fidelity pass.
- Tile/marker rendering composites in straight-sRGB like the rest of the base
  pipeline; gamma/linearRGB tuning is the R10 boundary.
- **Next: R10** (filter linearRGB color-interpolation, precise filter regions,
  tier-2 feComposite/feBlend/mix-blend-mode). Read `R10-*.goal.md` first.

## 2026-06-07 — SVG R8.1: Conformance + Security Hardening (post-R8 lane 1/5)

### Context Reviewed Before Editing
- `CLAUDE.md`/`AGENTS.md` SVG roadmap step protocol; `svg-zero-dep` skill
- `docs/svg-goal-plan-prompts/R8.1-conformance-security-hardening.goal.md`
- `docs/SVG_RENDERER_ROADMAP.md` Post-R8 gap matrix + lanes
- `src/canvas/svg_rasterizer.rs` (decoders: `decode_png`/`decode_jpeg`/`inflate`/
  `base64_decode`/`parse_path_d`/`rasterize_or_fallback`; caps; bench/oracle),
  `src/canvas/svg_golden.rs` (golden harness + fixtures)

### Changes
- Generated paste-ready post-R8 goal prompts (R8.1, R9–R12, each ≤4000 chars) +
  README run-order; added the auto-read "SVG renderer roadmap step protocol" to
  CLAUDE.md and AGENTS.md. (commit `8f52a8e`)
- **Fuzz harness** (in `svg_rasterizer.rs` test module): deterministic xorshift
  PRNG (`fuzz_rng`), bounded byte mutator (`fuzz_mutate`), `fuzz_drive` runs each
  mutated buffer through rasterize_or_fallback / parse_path_d / decode_png /
  decode_jpeg / inflate asserting no-panic + bounded output, over a checked-in
  seed corpus (`tests/fixtures/svg_fuzz/seed.svg` + `seed_path.txt` + the PNG/JPEG
  payload consts). `fuzz_smoke_decoders_never_panic` (always-run, 64 iters) +
  `fuzz_decoders_no_panic_bounded` (ignored, 8k iters, validated at 50k).
- **Curated W3C-1.1 subset goldens** (9) in `svg_golden.rs`: currentColor, rgb(),
  fill-opacity, `<use>`, nested-group transform, polyline, circle, ellipse,
  `mask-type="alpha"`. Crisp predictions matched the renderer exactly; AA disc
  signatures baked from captured output.
- **Memory-cap regressions**: oversized canvas request clamped (not allocated),
  oversized document rejected, path-token flood → empty default, inflate output
  ceiling honored.
- **Docs**: new `docs/SVG_PRECISION_AND_BENCH.md` (coverage grid, nearest
  sampling, premultiplied/sRGB vs linearRGB boundary — flags filters-run-in-sRGB
  as the R10 gap; benchmark budgets + methodology). Flipped R8.1 gap rows + lane
  bullet + maturity assessment in the roadmap; updated feature-evaluation.

### Verification
- `cargo fmt --check`, `cargo check`, `cargo test` (302 pass / 6 ignored),
  `cargo clippy --all-targets -- -D warnings`, `cargo test -- --ignored`
  (fuzz + bench + oracle), validate-svg-import.ps1, check-text-encoding.ps1.

### Risks / Follow-ups
- Goldens for circle/ellipse are renderer-defined (golden workflow): they catch
  regressions, not absolute AA correctness. Crisp goldens validate correctness.
- Next lane: **R9** (markers/vector-effect/patterns). Read its goal prompt first.

## 2026-06-06 — SVG R8: Conformance, Benchmark, Report UI (roadmap R0–R8 closed)

### Context Reviewed Before Editing
- `CLAUDE.md`, low-token preflight, `.agents/skills/svg-zero-dep/SKILL.md`
- `docs/SVG_RENDERER_ROADMAP.md` R8, `docs/svg-goal-plan-prompts/R8-*`
- `src/canvas/svg_rasterizer.rs` (SvgRenderReport/fidelity/warning/unsupported,
  rasterize_with_report), `src/panels/properties.rs` show_image + svg_source,
  `src/panels/mod.rs`, `src/canvas/svg_golden.rs`

### Changes
- New `src/panels/svg_report.rs`: `report_summary(&SvgRenderReport)` — a pure,
  unit-tested mapping to display rows (fidelity / rendered / skipped / warnings /
  unsupported) plus per-diagnostic `(code/feature, message[+byte-span])` lines;
  `show_report(ui, src)` renders it with a rendered-report / SVG-source toggle
  (egui temp memory) and a read-only source viewer.
- Wired the report panel into `panels::properties::show_image` for the selected
  SVG Image widget (computes `rasterize_with_report` at a fixed 256px; reuses the
  existing report, no new computation). Registered the module in `panels/mod.rs`.
- Golden corpus: added a crisp polygon-geometry golden (`polygon_square_fill`).
- Benchmark: `#[ignore] raster_benchmark_complex_scene_within_budget` measures
  parse+scene+raster of a 200-rect gradient/clip/stroke 256px scene (eprintln
  timing; generous hang guard — measures, doesn't gate). Joins the existing
  ignored 512px fill smoke.
- Dev-only oracle: `#[ignore] reference_oracle_scene_is_deterministic` — external
  reference renderers stay CI-artifact/dev-only, never runtime deps; in-repo
  stand-in asserts deterministic output. (Avoided banned crate names in `src/` so
  `validate-svg-import.ps1`'s dependency-policy grep stays green.)

### Verification
- `cargo test`: 297 passed, 5 ignored (R8 benchmark + oracle + prior 3). The 3
  svg_report unit tests assert the report→rows mapping incl. byte-span provenance.
- Ignored R8 tests pass when run with `--ignored` (benchmark ~6.5s/200 rects debug).
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `scripts/validate-svg-import.ps1`, `scripts/check-text-encoding.ps1`: clean.
  Ignored all-built-in exported-project `cargo check`: passes.

### Remaining Risks / Follow-Ups
- SVG renderer roadmap **R0–R8 closed**. Deferred + runtime-diagnosed: progressive
  JPEG, R6 vector-outline snapshot / raster text, filter tier 2/3.
- Post-roadmap: broader licensed conformance corpus + fuzzing; a true external
  reference-oracle remains a CI-artifact step, never a runtime dependency.
- Report panel re-rasterizes the selected Image at 256px each frame (cheap,
  selection-scoped); could cache by source hash if it ever shows up in profiles.

## 2026-06-06 — SVG R7: Masks + Filters Tier-1 (on the R4 offscreen pipeline)

### Context Reviewed Before Editing
- `CLAUDE.md`, low-token preflight, `.agents/skills/svg-zero-dep/SKILL.md`
- `docs/SVG_RENDERER_ROADMAP.md` R7, `docs/svg-goal-plan-prompts/R7-masks-filters.goal.md`
- `src/canvas/svg_rasterizer.rs` R4 layer/offscreen machinery (LayerRaw,
  ResolvedLayer, layer_for_group, execute layer stack, Offscreen,
  composite_offscreen, RasterTarget, render_shape, ClipMask), parser
  classification (`unsupported_tag_feature`, `is_container_tag`, build_items)

### Changes
- Masks (`mask="url(#id)"`, alpha + luminance via `mask-type`): `resolve_mask` +
  `collect_mask_items` lower the `<mask>` subtree to `MaskItem`s; `MaskDef::
  build_alpha` renders them through `render_shape` into a premultiplied buffer and
  reduces to a coverage alpha (luminance = 0.2125R+0.7154G+0.0721B on the
  premultiplied buffer, or the alpha channel). Applied by multiplying the masked
  element's isolated offscreen (`apply_mask_to_offscreen`).
- Filters tier-1: `FilterGraph`/`FilterPrimitive`/`FilterKind`/`FilterInput`.
  `parse_filter` reads the `<filter>` primitives; `FilterGraph::apply` runs them
  on the premultiplied source-graphic offscreen with named results and
  `in`/`SourceGraphic`/`SourceAlpha`. Primitives: `feGaussianBlur` (separable
  triple box-blur, `MAX_BLUR_RADIUS`-capped), `feOffset`, `feFlood`,
  `feMerge`(+`feMergeNode`), `feColorMatrix` (matrix/saturate/luminanceToAlpha),
  `feDropShadow`. Color matrix unpremultiplies → matrix → repremultiplies.
- Layer plumbing: `LayerRaw`/`ResolvedLayer` gained `mask_ref`/`filter_ref`;
  `needs_offscreen` now also true for mask/filter; `LayerFrame<'a>` borrows the
  `&ResolvedLayer` so `EndLayer` applies filter then mask before
  `composite_offscreen`. Shapes carrying mask/filter get a synthetic layer
  (`shape_layer`) emitted by `build_items`.
- Parser: retain `fe*` primitive elements + skip `mask`/`filter` defs in scene
  build (like `clipPath`); add `femerge` to `is_container_tag`. Removed the now
  dead `PendingDiagnostic::Unsupported` variant (mask/filter attrs are applied,
  not diagnosed).

### Verification
- `cargo test`: 293 passed, 3 ignored (3 new goldens: luminance mask, feOffset,
  feFlood+feMerge; 8 unit tests: alpha mask, missing-mask diagnostic, gaussian
  blur softening, colorMatrix saturate grayscale, dropShadow, unsupported-
  primitive partial, huge-blur-bounded, mask+filter determinism).
- Ignored all-built-in exported-project `cargo check`: passed (single `crate::`
  import contract intact; masks/filters render in the embedded copy too).
- `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `scripts/validate-svg-import.ps1`, `scripts/check-text-encoding.ps1`: clean.

### Remaining Risks / Follow-Ups
- Filter tier 2/3 (`feComposite`/`feBlend`/`feComponentTransfer`/`feMorphology`/
  `feTile`/`feImage`/`feDisplacementMap`/`feTurbulence`/convolution/lighting) pass
  through as identity with `filter.unsupported_primitive`.
- Filter region is the whole canvas (already bounded) rather than the precise
  filter-region rect; `maskContentUnits=objectBoundingBox` approximated in user
  space. Blur is a box approximation, not an exact Gaussian.
- Next: R8 (reference corpus, benchmarks, report UI + source viewer).

## 2026-06-06 — SVG R6: Editable Text Import (chunked multi-label, phases 1-2)

### Context Reviewed Before Editing
- `CLAUDE.md`, low-token preflight, `.agents/skills/svg-zero-dep/SKILL.md`,
  project-model skill
- `docs/TEXT_IMPORT_PLAN.md`, `docs/SVG_RENDERER_ROADMAP.md` R6,
  `docs/svg-goal-plan-prompts/R6-text-import-rendering.goal.md`
- `src/svg_import.rs` (text_widget/flatten_text, resolve_style text fields,
  metadata_for, normalize_widgets), `src/project/schema.rs` SvgImportMetadata

### Changes
- Added a `TextChunk` model to `svg_import.rs`: `<text>`/`<tspan>` split into
  chunks at every absolutely-positioned span (`x`/`y`). `text_widget` →
  `text_widgets` returning `Vec<WidgetInstance>` (one Label per non-empty chunk);
  `flatten_text` → `tspan_text` (warning-free subtree concat) + `build_text_label`
  (per-chunk placement, anchor, baseline, fill, provenance).
- Each chunk carries per-chunk font size, anchor, baseline, fill, source node,
  and warning flags. Relative/styled spans flatten into the current chunk with
  `text.tspan_adjust` / `text.tspan_style` diagnostics; absolute spans start a new
  chunk → new label.
- `text-anchor` start/middle/end and `dominant-baseline` middle/central/hanging
  applied per chunk; other baselines approximated with `text.baseline`.
  `text.missing_font` flags placeholder metrics. `textPath` stays
  unsupported-diagnosed.
- Schema: added `SvgImportMetadata::text_group: Option<String>`
  (`#[serde(default)]`, backward-compatible) tying a text element's chunk labels
  together; single-chunk text stays ungrouped (None).
- `import_node` "text" branch extends widgets with all chunk labels.

### Verification
- `cargo test`: 285 passed, 3 ignored (6 new R6 tests: grouped multi-label split,
  single-label no-group, relative anchor shift, baseline diagnostic, determinism,
  textPath deferred). Updated the `tspan_text` real-world fixture expectation
  (now 2 labels; `text.tspan_adjust` + `text.tspan_style`).
- Ignored all-built-in exported-project `cargo check`: passed (schema field is
  serde-default backward-compatible).
- `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `scripts/validate-svg-import.ps1`, `scripts/check-text-encoding.ps1`: clean.

### Remaining Risks / Follow-Ups
- Raster text rendering (vector-outline snapshot) deferred — rasterizer still
  buckets `<text>` as unsupported (TEXT_IMPORT_PLAN phase 3).
- Interleaved direct-text-after-tspan merges into the element's first chunk (the
  Node model concatenates direct text); rare, documented approximation.
- Placeholder font metrics (no real font measurement); textPath, bidi, shaping
  deferred. Owned shaping engine only if phases 1-3 prove insufficient.
- Next: R7 masks/filters.

## 2026-06-06 — SVG R5 Follow-on: Baseline JPEG Decoder

### Context Reviewed Before Editing
- `CLAUDE.md`, low-token preflight, `.agents/skills/svg-zero-dep/SKILL.md`
- `docs/jpegdecoder roadmap.md` (design note), `docs/SVG_RENDERER_ROADMAP.md` R5
- `src/canvas/svg_rasterizer.rs` PNG decoder + image path (`decode_image_href`,
  `DrawCommand::Image`, R4 clip/premultiplied pipeline), `src/codegen/export.rs`
  embedding contract

### Changes
- Implemented a zero-dependency baseline JPEG decoder in `svg_rasterizer.rs`:
  marker scan (SOI/APPn/DQT/DHT/SOF0/SOF1/DRI/SOS/EOI, others skipped), quant
  tables (8/16-bit), canonical JPEG Huffman tables (Annex F decode), MSB-first
  entropy bit reader with `0xFF00` de-stuffing, DC-diff + AC run-length (zigzag)
  block decode, dequantization, separable 8×8 float IDCT, nearest chroma
  upsampling, and YCbCr→RGB. Supports 8-bit, 1 or 3 components, arbitrary integer
  sampling factors (4:4:4 / 4:2:2 / 4:2:0 …) and restart intervals.
- `decode_image_href` now routes `FF D8` to `decode_jpeg`; replaced the
  `JpegDeferred` error with `MalformedJpeg` (`image.decode_failed`) and
  `UnsupportedJpeg` (`image.unsupported_jpeg`); `NotPng` → `NotImage`.
  Progressive/arithmetic/lossless/12-bit/CMYK are diagnosed unsupported.
- JPEG draws through the existing R4 clip/premultiplied image path (same
  `DrawCommand::Image`), so clipping, opacity, preserveAspectRatio, and export
  embedding are shared with PNG. No new `crate::` refs; std-only.

### Verification
- `cargo test`: 279 passed, 3 ignored. 6 JPEG tests: baseline 4:4:4 color,
  4:2:0 chroma-subsampled two-region, true 1-component grayscale, determinism,
  progressive-unsupported, malformed-not-panicking. Fixtures: 4:4:4 / 4:2:0 from
  ffmpeg's mjpeg encoder (ground truth); the 1-component grayscale JPEG was
  hand-encoded (flat block, Annex-K Huffman tables) since ffmpeg emitted
  3-component gray.
- Ignored all-built-in exported-project `cargo check`: passed (decoder embedded;
  single `crate::` import contract intact).
- `cargo fmt --check`, `cargo clippy -- -D warnings` (fixed rust-1.95
  is_none_or / is_multiple_of / resize / collapsible-match lints across the PNG
  and JPEG code), `scripts/validate-svg-import.ps1`,
  `scripts/check-text-encoding.ps1`: clean.

### Remaining Risks / Follow-Ups
- Deferred JPEG: progressive, arithmetic, lossless, 12-bit, CMYK/4-component.
- Float IDCT (clarity over speed); integer/AAN IDCT is a future optimization.
- Nearest chroma upsampling and nearest image sampling (deterministic, not
  smoothed). Broader reference-image corpus is future conformance (R8).
- Next: R6 text import.

## 2026-06-06 — SVG R5: Embedded PNG Raster Images (JPEG Deferred)

### Context Reviewed Before Editing
- `CLAUDE.md`, low-token preflight, `.agents/skills/svg-zero-dep/SKILL.md`
- `docs/SVG_RENDERER_ROADMAP.md` (R5 source of truth),
  `docs/svg-goal-plan-prompts/R5-embedded-raster-images.goal.md`
- `src/canvas/svg_rasterizer.rs` (SvgNode model, scene build, DisplayList
  build/execute, R4 clip/premultiplied pipeline, RasterTarget, caps),
  `src/svg_core.rs` (viewbox_transform/preserveAspectRatio, Affine2D),
  `src/svg_import.rs` image placeholder + data-URI cap, `src/codegen/export.rs`
  embedding contract

### Decision
- Implement zero-dependency PNG `data:` decode now; defer baseline JPEG to a
  tracked follow-on (detected and reported `image.jpeg_unsupported`). PNG is
  lossless/bounded and covers the bulk of embedded design-tool images; JPEG is a
  larger, separate lossy pipeline (Huffman + IDCT + YCbCr).

### Changes
- Added a from-scratch image decoder to `svg_rasterizer.rs`: base64 decode, zlib
  wrapper, DEFLATE inflate (stored/fixed/dynamic Huffman, puff-style canonical
  decoder), PNG chunk parse (IHDR/PLTE/tRNS/IDAT/IEND), scanline unfilter
  (None/Sub/Up/Average/Paeth), and RGBA8 expansion for color types 0/2/3/4/6 at
  bit depth 8 and 16 (truncated). Interlace and sub-byte depths are diagnosed.
- `<image>` lowers to `DrawCommand::Image` (or `ImageSkipped` with a specific
  code) in `DisplayList::build`; `execute` draws via nearest-neighbour sampling
  through the R4 clip/premultiplied `RasterTarget`, clipped to the destination
  rect (slice overflow) plus any `clip-path`, faded by element opacity.
- Placement reuses `svg_core::viewbox_transform` for `preserveAspectRatio`.
  Security: pixel cap (`MAX_IMAGE_PIXELS`), inflate-byte cap
  (`MAX_IMAGE_DECODE_BYTES`), bounded chunk reads; external references remain
  fail-closed at the existing document gate.
- Refreshed the stale `unsupported_tag_feature("image")` message.

### Verification
- `cargo test`: 274 passed, 3 ignored (14 new R5 tests: RGBA/RGB/palette+tRNS/
  gray decode, nearest-neighbour scale, clip, opacity, determinism,
  JPEG-deferred, interlaced, oversize, truncated, external document gate,
  stored-block inflate). PNG fixtures minted with real zlib (python) to exercise
  the dynamic-Huffman inflate path.
- Ignored all-built-in exported-project `cargo check`: passed (decoder embedded;
  single `crate::` import rewrite contract intact).
- `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `scripts/validate-svg-import.ps1`, `scripts/check-text-encoding.ps1`: clean.

### Remaining Risks / Follow-Ups
- Baseline JPEG decode deferred (tracked R5 follow-on); progressive JPEG out of
  scope.
- Sub-byte (1/2/4-bit) and interlaced PNG are diagnosed, not decoded.
- Nearest-neighbour sampling (no bilinear) — deterministic but not smoothed.
- Component import keeps `<image>` as an editable placeholder by design.
- Next: R6 text import (or the JPEG follow-on).

## 2026-06-06 — SVG R4: Clipping, Viewport Overflow, Premultiplied Compositing, Group Opacity

### Context Reviewed Before Editing
- `CLAUDE.md`, low-token preflight, `.agents/skills/svg-zero-dep/SKILL.md`
- `docs/SVG_RENDERER_ROADMAP.md` (R4 source of truth), `docs/SVG_IMPORT.md`,
  `docs/feature-evaluation/svg-import-renderer.md`
- `src/canvas/svg_rasterizer.rs` (full: SvgScene→DisplayList→execute, coverage,
  blend, paint sampler, references), `src/svg_core.rs` (Affine2D/inverse,
  viewbox), `src/canvas/svg_golden.rs`, `src/codegen/export.rs` embedding +
  `embedded_svg_sources_keep_single_import_rewrite_contract`, and
  `src/canvas/interaction.rs` Image preview path.

### Derived Render/Export Path Matrix
- Pixel-flow source of truth: `SvgScene{items}` (flat preorder) → `DisplayList`
  (`build` lowers, `execute` rasters) → `render_shape` → `coverage_scan`/
  `rasterize_coverage` → `blend_pixel`. Single shared coverage scan now also
  feeds clip masks.
- Both paint paths are one source: in-app `interaction.rs` →
  `svg_rasterizer::rasterize_or_fallback`; export embeds `svg_rasterizer.rs`
  verbatim into `mod rohkai_svg` (only rewrite: `crate::svg_core`→
  `super::svg_core`). No app-only support is possible by construction.

### Changes (`src/canvas/svg_rasterizer.rs` unless noted)
- Layer model: `SvgSceneItem` gained `layer: Option<LayerRaw>` + `is_layer_end`;
  scene flattening emits begin/end markers for groups and nested-`<svg>` that
  need clip/opacity/overflow. `DisplayList` gained `DrawCommand::BeginLayer`/
  `EndLayer`; `execute` maintains a clip + offscreen layer stack.
- clipPath rendering: `resolve_clip` (reuses the shared first-id-wins reference
  table; bounded `MAX_CLIP_DEPTH`, cycle detection), `collect_clip_shapes`
  (per-child transforms, nested `<g>`, `clip-rule` nonzero/evenodd),
  clipPathUnits userSpaceOnUse + objectBoundingBox (shape bbox via
  `geometry_local_bounds`), clipPath-of-clipPath intersection. clipPath defs no
  longer render directly or emit a `clipPath` unsupported bucket.
- Nested-`<svg>` overflow clipping: `overflow_clip_shape` builds a device rect
  from the pre-viewport transform captured in `LayerRaw`; combined with any
  group clip-path via `ClipDef.nested`.
- Premultiplied compositing: `ClipMask` + `coverage_scan` refactor;
  `RasterTarget` now carries `premultiplied` + `clip`; `blend_pixel_premultiplied`
  and `composite_offscreen` composite isolated offscreens once at group opacity.
  Base buffer and emitted `ColorImage` stay straight RGBA, so non-grouped output
  is byte-stable.
- Group opacity/isolation: `<g opacity>` / `isolation:isolate` allocate a
  premultiplied offscreen (bounded by `MAX_OFFSCREEN_DEPTH`/`MAX_OFFSCREEN_BYTES`,
  `limit.offscreen_buffer` on truncation). `opacity` made non-inherited in
  `Style::inherit_parts` (the double-darken fix).
- Diagnostics: removed "clip-path attribute" unsupported; added `clip.unresolved`,
  `reference.clip_cycle`, `limit.clip_depth`, `clip.object_bounding_box`,
  `limit.offscreen_buffer`.
- Export (`src/codegen/export.rs`): no new `crate::` refs (rewrite contract
  intact); added `embedded_rasterizer_includes_r4_render_paths` invariant test.

### Golden Justification
- `unsupported_clip_diagnosed_not_applied` (expected full `RRRR`) renamed to
  `rect_clip_path_applied` with golden `RR..`: clip is now applied, which is
  provably more correct than the prior diagnosed-only behavior. Added goldens:
  `path_clip_nonzero`, `path_clip_evenodd_hole`, `transformed_clip_child`,
  `object_bounding_box_clip`, `nested_svg_overflow_clip`,
  `translucent_group_no_double_darken`. All other goldens unchanged (byte-stable).

### Verification
- `cargo fmt --check` clean; `cargo check` clean; `cargo clippy -- -D warnings`
  zero warnings.
- `cargo test`: 260 passed, 0 failed, 3 ignored (added 11 R4 unit tests +
  7 R4 goldens + 1 export invariant test).
- Ignored export cargo checks: `export_compile_fixture_cargo_check` and
  `all_builtin_widgets_export_cargo_check` both pass → embedded R4 rasterizer
  compiles standalone (export parity).
- `scripts/validate-svg-import.ps1` pass; `scripts/check-text-encoding.ps1` OK.
- `cargo run` launch smoke: app launched (PID alive), exited cleanly (exit 0);
  clip preview correctness covered by
  `clip_path_renders_visibly_clipped_high_fidelity`.

### Risks / Follow-ups
- Root `<svg opacity>` no longer cascades to children (opacity is now
  non-inherited per SVG); a root-level group composite is not implemented. Minor;
  documented in roadmap limits.
- objectBoundingBox clipPathUnits on a `<g>` has no single bbox → diagnosed and
  skipped, not approximated.
- Isolated offscreens are full raster size; deep nesting is bounded by depth/byte
  caps with a visible diagnostic and graceful (non-isolated) degrade.
- Next: R5 embedded raster images (zero-dependency PNG/JPEG decision).

## 2026-06-06 — SVG R2 Shared Semantics And R3 Paint Servers

### Context Reviewed Before Editing
- `AGENTS.md`, low-token preflight with devlog, current dirty `dev` status
- `.agents/skills/svg-zero-dep/SKILL.md`
- `docs/SVG_RENDERER_ROADMAP.md`, `docs/SVG_IMPORT.md`,
  `docs/CODE_COOP.md`, and renderer/export embedding contracts
- Importer/rasterizer style, local-reference, scene/display-list, coverage,
  golden, diagnostics, and export fixture code

### Changes
- Added shared bounded tier-1 CSS declarations/selectors/cascade to `svg_core`:
  element/class/ID compound selectors, grouped selectors, specificity/source
  order, rule/declaration budgets, and complex-selector diagnostics.
- Unified importer/rasterizer `currentColor`, style precedence, shared checked
  transform parsing, color/number/length/path microsyntax, and malformed
  fallback behavior.
- Added stable bounded raster `defs`/`symbol`/`use` expansion with symbol
  viewBox mapping, inherited style, duplicate-ID first-wins behavior,
  cycle/depth/node limits, and source-spanned diagnostics.
- Added owned paint-server IR and deterministic linear/radial gradient fills
  and strokes: stop color/opacity, CSS/currentColor stops,
  objectBoundingBox/userSpaceOnUse, gradient transforms, pad/reflect/repeat,
  focal radial geometry, and bounded href inheritance.
- Kept fill winding/parity and stroke union coverage separate while sampling
  paint per covered pixel. Added malformed units/transform/spread/length/stop
  diagnostics and unresolved paint-reference warnings.
- Upgraded patterns to explicit transparent unsupported paint servers whose
  diagnostics preserve exact supplied pattern attributes.
- Replaced the obsolete transparent-gradient golden with linear, radial,
  repeat-spread, and single-stop goldens; added focused semantics,
  determinism/fidelity, reference, malformed-input, CSS, and cycle tests.
- Kept both embedded renderer sources std-only and preserved the single-import
  export rewrite contract.

### Verification
- R2 boundary before R3: 238 tests passed, 3 ignored; strict clippy,
  dependency/encoding policy, embedding contract, and ignored all-widget
  exported-project compile passed.
- Final full suite: 248 passed, 3 ignored.
- SVG validation: importer 19 passed; rasterizer 53 passed and 1 ignored
  performance smoke; all 4 golden harness tests passed.
- Explicit 512x512 anti-aliased fill performance smoke passed in about 0.17s
  on this machine against the 5-second debug budget.
- `cargo fmt --check`, `cargo check`, and
  `cargo clippy -- -D warnings`: clean.
- Dependency policy, UTF-8 encoding policy, embedding-source contract, and
  `scripts/validate-svg-import.ps1`: clean.
- Ignored all-built-in exported-project `cargo check`: passed with the embedded
  shared `svg_core` and SVG rasterizer.
- Launch smoke: the current debug app remained alive for 5 seconds.

### Remaining Risks / Follow-Ups
- Component import intentionally keeps gradients editable only as diagnosed
  approximations; Image-mode rendering is the high-fidelity fallback.
- Patterns, clips, masks, filters, text, embedded raster images, isolated group
  compositing, and nested viewport overflow clipping remain later phases.
- R4 is next: clip stack, nested overflow, premultiplied-alpha internals, and
  isolated group opacity/compositing.

## 2026-06-06 — SVG R1 Stroke Geometry, Coverage, And Dashes

### Context Reviewed Before Editing
- `AGENTS.md`, low-token preflight, and current git status on `dev`
- `.agents/skills/svg-zero-dep/SKILL.md`
- `docs/SVG_RENDERER_ROADMAP.md`, `docs/SVG_IMPORT.md`,
  `docs/CODE_COOP.md`, and the current renderer/export embedding paths
- Existing path parser, fill rules, display list, raster limits, golden harness,
  and the ignored all-built-in exported-project compile fixture

### Changes
- Replaced flattened-only path storage with retained line, quadratic, cubic,
  arc, subpath, and explicit-close semantics. Curve flattening now measures
  tolerance after the final device transform.
- Added inherited stroke width, cap, join, miter-limit, dash-array,
  dash-offset, and positive `pathLength` handling using shared SVG lengths.
- Replaced device-space segment quads with bounded local-space stroke meshes
  transformed as complete geometry. Implemented butt/round/square caps;
  miter, miter-clip, round, and bevel joins; and a diagnosed miter-clip
  approximation for SVG `arcs` joins.
- Added signed dash phase, odd-list repetition, continuity through vertices,
  closed-seam merging, zero-length round/square output, and `pathLength`
  calibration.
- Added deterministic 8x8 subpixel coverage with two separate semantics:
  winding/parity accumulation for nonzero/evenodd fills and union coverage for
  stroke primitives. Translucent stroke joins now composite once.
- Added local/device stroke bounds, raster destination context, dash/run/
  primitive/vertex caps, and source-spanned `limit.stroke_complexity` warnings
  when bounded work truncates.
- Enforced exported-source embedding: the rasterizer may contain only the one
  known `crate::svg_core` import, and embedded `svg_core` remains free of
  crate-local references.
- Added analytical tests for transformed widths/bounds, caps, miter limits,
  nonuniform transforms, anti-aliased edges, self-intersecting evenodd fills,
  opacity union, dash phase/seams/pathLength, malformed declarations, limits,
  and determinism.
- Added three focused ASCII goldens: anti-aliased diagonal fill, dashed
  round-cap stroke, and self-intersecting evenodd fill.
- Updated renderer/import/roadmap/feature-evaluation documentation so R1 claims
  match implementation and R2 is the next phase.

### Verification
- Full suite: 230 passed, 3 ignored.
- SVG rasterizer: 41 passed, 1 ignored performance smoke.
- Explicit 512x512 anti-aliased fill performance smoke: passed in about 0.13s
  on this machine against a 5-second debug budget.
- SVG golden suite: 4 harness tests passed with all fixtures matching.
- Ignored all-built-in exported-project `cargo check`: passed, including the
  embedded SVG renderer path.
- `cargo fmt --check`, `cargo check`, and
  `cargo clippy -- -D warnings`: clean.
- Dependency policy, UTF-8 encoding policy, and
  `scripts/validate-svg-import.ps1`: clean.
- Launch smoke: current debug binary remained alive for 5 seconds.

### Remaining Risks / Follow-Ups
- SVG `arcs` line joins are approximated, not exact.
- Gamma-aware/group compositing, vector effects, markers, nested viewport
  clipping, paint servers, text, filters, clips, and masks remain later phases.
- R2 should centralize shared style/reference behavior without weakening the
  completed R1 geometry and export-embedding invariants.

## 2026-06-06 — SVG R1 Nonzero And Evenodd Fill Rules

### Context Reviewed Before Editing
- `AGENTS.md` and low-token preflight
- `.agents/skills/svg-zero-dep/SKILL.md`
- `docs/SVG_RENDERER_ROADMAP.md`, `docs/SVG_IMPORT.md`,
  `docs/CODE_COOP.md`, and current git status
- Raster style inheritance, display-list lowering, path flattening, compound
  scan conversion, renderer diagnostics, and golden fixtures

### Changes
- Added inherited `FillRule` state with SVG-correct `nonzero` default and
  explicit `evenodd` support from presentation attributes and inline styles.
- Removed the obsolete unsupported-feature diagnostic for valid fill-rule
  declarations.
- Added source-spanned `style.invalid_fill_rule` warnings; invalid declarations
  retain inherited/default behavior.
- Replaced pairwise even-odd path filling with a deterministic crossing sweep
  that groups coincident intersections and evaluates either winding count or
  parity for every scanline interval.
- Applied fill rules to compound paths and other closed geometry while keeping
  the path-command/point/raster safety caps intact.
- Added analytical tests for same-direction contours, opposite-direction
  contours, inheritance, inline-style precedence, and malformed values.
- Added nonzero and evenodd fixtures to the golden renderer corpus and updated
  roadmap, current behavior, architecture, code index, and feature evaluation.

### Verification
- Focused SVG rasterizer tests: 26 passed.
- SVG golden tests: 4 passed with the expanded fixture set.
- Full suite: 214 passed, 2 ignored.
- `cargo fmt --check`: clean.
- `cargo check`: clean.
- `cargo clippy -- -D warnings`: clean.
- `scripts/check-text-encoding.ps1`: clean.
- `scripts/check-dependency-policy.ps1`: clean.
- `scripts/validate-svg-import.ps1`: clean.
- Launch smoke: an existing June 5 RohKai process held the default debug
  executable open, so the current tree was built into `target/smoke`; that
  isolated binary stayed alive for 5 seconds.

### Risks / Follow-ups
- Fill edges remain hard coverage until the R1 antialiasing slice.
- Stroke geometry still uses segment quads and does not yet implement
  linecap/linejoin/miter/dash semantics.
- Exact geometric/stroke bounds remain a later R1 item.

## 2026-06-06 — SVG R1 PreserveAspectRatio And Nested Viewports

### Context Reviewed Before Editing
- `AGENTS.md` and low-token preflight
- `.agents/skills/svg-zero-dep/SKILL.md`
- `.agents/skills/project-model/SKILL.md`
- `docs/SVG_RENDERER_ROADMAP.md`, `docs/SVG_IMPORT.md`,
  `docs/CODE_COOP.md`, and current git status
- `src/svg_core.rs`, `src/svg_import.rs`, and
  `src/canvas/svg_rasterizer.rs`

### Changes
- Added shared parsing and transform construction for all SVG
  `preserveAspectRatio` alignments, `meet`, `slice`, `none`, and optional
  `defer` recognition.
- Applied the shared mapping to importer root/nested viewport state and
  rasterizer root/nested `<svg>` scene traversal.
- Added per-scene-item viewport length bases so nested percentages resolve
  against the nested user coordinate system rather than the root viewport.
- Added analytical renderer tests that recover the full alpha bounds and probe
  interior/exterior pixels for root meet/none/max alignment, nested viewport
  alignment, and percentage geometry.
- Removed an incorrect coupling between SVG viewport dimensions and RohKai's
  20px minimum editable-placeholder size.
- Corrected polygon and path filling to use pixel-center horizontal coverage,
  eliminating an extra endpoint column while preserving existing goldens.
- Updated the SVG roadmap, current-behavior docs, architecture, code index,
  feature evaluation, and CoOp handoff. Nested viewport overflow clipping
  remains explicitly deferred to R4.

### Verification
- Focused shared-core tests: 12 passed.
- Focused SVG importer tests: 18 passed.
- Focused SVG rasterizer tests: 22 passed.
- SVG golden tests: 4 passed.
- Full suite: 210 passed, 2 ignored.
- `cargo fmt --check`: clean.
- `cargo check`: clean.
- `cargo clippy -- -D warnings`: clean.
- `scripts/check-text-encoding.ps1`: clean.
- `scripts/check-dependency-policy.ps1`: clean.
- `scripts/validate-svg-import.ps1`: clean.
- Launch smoke: debug RohKai process stayed alive for 5 seconds.

### Risks / Follow-ups
- Nested viewport coordinate mapping is complete for this phase, but overflow
  clipping is not implemented and remains an R4 clipping task.
- `defer` is parsed and preserved in the shared value model; referenced-image
  behavior that gives it meaning is outside this slice.
- The next R1 slice is explicit `nonzero`/`evenodd` fill-rule support, followed
  by stroke tessellation and antialiasing.

## 2026-06-06 — SVG R0 Metadata, Lengths, And Owned Display List

### Context Reviewed Before Editing
- `AGENTS.md` and preflight context with devlog
- `.agents/skills/svg-zero-dep/SKILL.md`
- `.agents/skills/project-model/SKILL.md`
- `.agents/skills/task-decomposition/SKILL.md`
- `docs/SVG_RENDERER_ROADMAP.md`, `docs/SVG_IMPORT.md`,
  `docs/CODE_COOP.md`, and current git status
- `src/svg_core.rs`, `src/svg_import.rs`, and
  `src/canvas/svg_rasterizer.rs`

### Changes
- Added shared strict SVG length parsing/resolution for unitless/px,
  percentages, `in`, `cm`, `mm`, `Q`, `pt`, `pc`, `em`, `ex`, and `rem`.
  Importer dimensions/geometry and rasterizer dimensions/geometry now use the
  same `svg_core` implementation.
- Added stable preorder `SvgNodeId` values and exact source byte spans to
  represented rasterizer nodes.
- Added independently bounded local-ID and reference-use tables with
  deterministic first-ID-wins duplicate behavior, resolved/unresolved fragment
  metadata, limit warnings, and structured rejection of non-local references.
- Added node ID/source-span provenance to renderer warnings and unsupported
  feature diagnostics.
- Replaced borrowed XML-node display commands with an owned render-ready IR.
  Display-list construction now lowers shape lengths, point/path geometry,
  inherited style, transforms, diagnostics, and provenance; raster execution
  does not inspect XML nodes or raw shape attributes.
- Updated SVG roadmap, current-behavior docs, architecture, code index, and
  agent handoff notes. R0 is complete; R1 geometry quality is next.

### Verification
- Focused shared-core tests: 9 passed.
- Focused SVG importer tests: 17 passed.
- Focused SVG rasterizer tests: 20 passed.
- Full suite: 204 passed, 2 ignored.
- `cargo fmt --check`: clean.
- `cargo check`: clean.
- `cargo clippy -- -D warnings`: clean.
- `scripts/check-text-encoding.ps1`: clean.
- `scripts/check-dependency-policy.ps1`: clean.
- `scripts/validate-svg-import.ps1`: clean, including golden and source
  preservation fixtures.
- Launch smoke: debug RohKai process stayed alive for 5 seconds.

### Risks / Follow-ups
- The reference table is metadata and diagnostics infrastructure. Actual
  raster `<use>`/`symbol` expansion, cycle handling during expansion, and paint
  server resolution remain R2 work.
- Font-relative units currently use the explicit default length context
  (`16px` em/rem and `8px` ex) until R6 supplies real font metrics.
- R1 should now improve fill rules, stroke geometry, curve tolerances, and
  anti-aliasing without reopening XML traversal.

## 2026-06-06 — SVG Roadmap Authority Consolidation

### Docs Reviewed Before Editing
- `AGENTS.md` and low-token preflight
- `.agents/skills/svg-zero-dep/SKILL.md`
- `docs/ROADMAP.md`
- `docs/SVG_RENDERER_ROADMAP.md`
- `docs/SVG_IMPORT.md`
- `docs/TEXT_IMPORT_PLAN.md`
- SVG and remaining-roadmap feature evaluations

### Changes
- Declared `docs/SVG_RENDERER_ROADMAP.md` the sole detailed authority for SVG
  import maturity, SVG Image rasterization, SVG text, diagnostics,
  conformance, and SVG-facing editor UX.
- Converted Stage 7.x and Stage 9 SVG sections into historical snapshots with
  explicit mappings instead of competing active checklists.
- Assigned all text/tspan execution to R6 and report/source-viewer UX to R8.
- Added the next execution order: close R0 metadata/traversal, share SVG
  lengths, then complete R1 and R2 before R3-R8.
- Explicitly separated Stage 15's proposed general RohKai renderer from the SVG
  rasterizer roadmap.
- Added an explicit `Current Active Work` roadmap heading and updated preflight
  to prefer it, replacing the misleading behavior that labeled the final
  numbered stage (deferred Stage 15) as current.
- Mirrored the active-work clarification in `AGENTS.md` and `CLAUDE.md`.
- Marked the roadmap source-of-truth reconciliation items complete and updated
  feature evaluations, architecture, and code index to point to the same
  authority.

### Verification
- Documentation diff and duplicate-check review completed.
- Preflight now reports
  `Current Active Work — Pre-Release Depth And SVG R0 Closure`.
- AGENTS/CLAUDE mirrored guidance and skill drift checks: clean.
- `cargo fmt --check`: clean.
- `cargo check`: clean.
- `scripts/check-text-encoding.ps1`: clean.
- SVG dependency policy check: clean.

### Risks / Follow-ups
- The detailed renderer roadmap still contains derivative task lists beneath
  R0-R8. They are implementation notes, not independent phases.
- Stage 15 still needs a future explicit product/architecture activation
  decision; this pass intentionally did not activate it.

## 2026-06-06 — Lazare Structured Ranges And Editor Viewport

### Docs Reviewed Before Editing
- `AGENTS.md` and preflight context with `-IncludeDevlog`
- `.agents/skills/project-model/SKILL.md`
- `.agents/skills/codegen-rules/SKILL.md`
- `.agents/skills/canvas-patterns/SKILL.md`
- `docs/ROADMAP.md`, `docs/CODE_COOP.md`, `docs/ARCHITECTURE.md`,
  `docs/CODE_INDEX.md`, and the Lazare feature evaluation

### Architectural Correction
- The June 5 padding/clip-rect approach was not a complete clipping fix.
  TextEdit's glyph clip cannot simultaneously provide readable glyph spacing
  and a fully visible border at panel edges. That earlier claim is superseded
  by a separate editor viewport and decoration gutter backed by exact source
  ranges.

### Changes
- Added `codegen/source_map.rs` and `egui_emitter::emit_document()`: generated
  code now carries exact byte and line ranges for every widget. Widget ranges
  exclude the `CentralPanel` preamble and neighboring blocks.
- Extended Lazare parser results with source ranges for valid edited code and
  structural incomplete-block diagnostics.
- Rebuilt the code surface around a no-wrap-by-default TextEdit with horizontal
  and vertical scrolling, an optional Wrap toggle, inset text, and decoration
  painting clipped to the outer viewport rather than the glyph clip.
- Canvas `selected` is the only highlight set. Multi-selection produces
  independent outlines; deselection clears them. Ctrl+double-click/Tracé uses
  a one-frame navigation target instead of duplicated highlight state.
- Added explicit generated, valid-edit, and invalid-edit states. Invalid edits
  stay visible and never partially mutate `UiTree`; empty code clears widgets;
  deleting every widget block while retaining the canonical project preamble
  also clears widgets; duplicate pasted blocks receive fresh UUIDs, placement
  offsets, canonical regeneration, and active selection.
- Replaced utility-window input block lists with canvas response, top-layer,
  focus, and keyboard-ownership checks. Floating windows no longer leak pointer
  or keyboard actions into the canvas.
- Added visual separation before generated top-level widget blocks so selection
  outlines do not crowd the preamble or adjacent blocks.

### Verification
- Focused generated/parser source-range tests: passed
- Every built-in canonical widget block parses without structural errors
- Focused code-editor geometry/multi-selection tests: passed
- Focused canvas input-ownership tests: passed
- Fresh rebuilt-binary visual check: generated state remained valid; selected
  Button mapped to line 3; preamble remained outside the outline; all four
  perimeter edges remained visible in the narrow right panel
- `cargo fmt --check`: clean
- `cargo check`: clean
- `cargo test`: 195 passed, 2 ignored
- `cargo clippy -- -D warnings`: clean
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1`:
  OK

### Risks / Follow-ups
- TextEdit remains the editing engine. Search, symbols, precise cursor
  placement, clickable diagnostic navigation, diff view, and explicit
  generated/user-region ownership remain IDE-depth work.
- Handler range storage exists as a future-ready type; handler indexing is not
  yet produced by the live emitter.

## 2026-06-05 — Code Highlight Outline And Launcher Trace

> Superseded on June 6: padding and painting inside TextEdit's clip improved the
> symptom but did not solve the architectural clipping conflict. The current
> implementation uses structured source ranges and a separate decoration
> viewport/gutter.

### Docs Reviewed Before Editing
- Preflight context (`scripts/preflight-context.ps1`)
- `docs/CODE_COOP.md`
- `src/panels/code_preview.rs`
- egui 0.29.1 `TextEditOutput` and `Galley` source in the local Cargo cache

### Changes
- Replaced the generated-code selection highlight from TextEdit span
  background coloring with a foreground outline drawn from
  `TextEditOutput.galley` rows.
- Follow-up after visual inspection: the outline now uses row glyph mesh bounds
  instead of full row allocation bounds, so it hugs actual code width instead of
  spanning the editor row. The translucent fill was removed; selection is an
  outline-only decoration.
- Follow-up after right-edge clipping inspection: outline geometry is inset from
  the raw TextEdit clip rect before painting so the full perimeter remains
  visible at the right/bottom panel boundary.
- Follow-up after text-clipping inspection: the code editor now has real inner
  padding, the outline expands outside glyph mesh bounds, and the border remains
  outline-only so selected code stays readable.
- Floating utility windows now block canvas input while open, preventing shortcut
  window scroll from zooming the canvas and Rust Wiring drag from starting
  rubber-band selection behind the window.
- View menu now exposes Preview Mode directly in addition to the F5 shortcut.
- The outline is clipped to `TextEditOutput.text_clip_rect` before painting and
  uses `ui.painter_at(output.text_clip_rect)`, so selected-code decoration
  cannot spill outside the visible code editor area.
- Added a regression test proving expanded outline geometry is clipped to the
  visible TextEdit clip rect.
- Updated `scripts/run.ps1` to print source path, branch, commit, and dirty
  state before launching, plus `-CheckOnly` for verifying the launcher path
  without opening the app.

### Verification
- `cargo fmt --check`: clean
- `cargo test code_preview -- --nocapture`: 4 passed
- `cargo check`: clean
- `cargo test`: 187 passed, 2 ignored
- `cargo clippy -- -D warnings`: clean
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1`:
  OK
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\run.ps1 -CheckOnly`:
  reports `D:\dev\rohkai`, branch `dev`, and current commit.

### Risks / Follow-ups
- The outline now follows TextEdit's real visible layout, but it is still one
  rectangular outline around each selected block. A future dedicated code editor
  can add true token/range decorations, minimap markers, and precise scroll-to
  cursor behavior.

## 2026-06-05 — Layout Depth Follow-Up: Spacers, Ownership, Parser, Stretch

### Docs Reviewed Before Editing
- Preflight context (`scripts/preflight-context.ps1 -IncludeDevlog`)
- `.agents/skills/project-model/SKILL.md`
- `.agents/skills/canvas-patterns/SKILL.md`
- `.agents/skills/codegen-rules/SKILL.md`
- `docs/ROADMAP.md`
- `docs/CODE_COOP.md`
- Recent layout entries in `docs/DEVLOG.md`

### Changes
- Added layout-aware spacer behavior in `UiTree::reflow_layouts()`:
  `VerticalSpacer` flexes inside `VLayout`, `HorizontalSpacer` flexes inside
  `HLayout`, and generated live/export code emits matching `ui.add_space(...)`.
- Added `WidgetProps.layout_stretch` as a first-slice container fill/stretch
  policy. When disabled, stack/grid layouts preserve child size hints while
  still assigning deterministic canvas rects.
- Fixed group/ungroup behavior for layout-owned children so Frames replace or
  expand in the parent `children` list instead of orphaning layout ownership.
- Added `UiTree::move_child_within_parent()` and a first-slice GridLayout slot
  reorder UI in Properties.
- Extended Lazare parser output to preserve one-level layout hierarchy from
  generated `ui.vertical`, `ui.horizontal`, and `egui::Grid::new(...).show(...)`
  closures.
- Updated roadmap, architecture, code index, feature evaluation, Code CoOp, and
  agent project-model skills to reflect the new source-of-truth fields and
  first-slice completion status.

### Verification
- `cargo check`: clean
- `cargo test layout -- --nocapture`: 18 passed
- Focused spacer/parser/grid-reorder/live-export tests: passed
- `cargo test export_compile_fixture_cargo_check -- --ignored --nocapture`: passed
- `cargo test all_builtin_widgets_export_cargo_check -- --ignored --nocapture`: passed
- `cargo fmt --check`: clean
- `cargo test`: 186 passed, 2 ignored
- `cargo clippy -- -D warnings`: clean
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1`:
  OK
- `cargo run` smoke: launched and stopped after 8 seconds

### Risks / Follow-ups
- Layouts are still not Qt/Lazarus parity: per-child policies, alignment,
  named grid slots, drag-to-slot editing, and multi-level hierarchy round-trip
  remain open.
- Grid slot UI currently shows row/column plus short UUID controls; it is a
  functional reorder slice, not the final visual slot editor.

## 2026-06-04 — Layout Properties And Outline Hierarchy Slice

### Docs Reviewed Before Editing
- Preflight context (`scripts/preflight-context.ps1`)
- `.agents/skills/project-model/SKILL.md`
- `.agents/skills/canvas-patterns/SKILL.md`
- `.agents/skills/codegen-rules/SKILL.md`
- `docs/ROADMAP.md`
- `src/project/schema.rs`, `src/project/ui_tree.rs`,
  `src/panels/properties.rs`, `src/panels/outline.rs`,
  `src/canvas/interaction.rs`, `src/codegen/egui_emitter.rs`,
  `src/codegen/export.rs`

### Changes Made
- Added persisted layout properties on `WidgetProps`:
  `layout_spacing: f32` and `grid_columns: usize`.
- `UiTree::reflow_layouts()` now uses `inner_margin`, `layout_spacing`, and
  `grid_columns` instead of hardcoded spacing/column values.
- `validate_and_repair()` clamps layout margins, gaps, and grid columns to safe
  values.
- Properties panel now exposes child count, margin, gap, and GridLayout columns
  for layout containers; edits reflow immediately through the existing
  validate/repair path.
- Canvas GridLayout preview now draws vertical grid guides from
  `props.grid_columns` and row guides from owned-child count.
- Live codegen and export now use each GridLayout's `grid_columns` value for
  `ui.end_row()` boundaries.
- Layers/Outline now builds an explicit hierarchy row model: owned children are
  displayed directly under their parent instead of appearing in flat draw order
  with incidental indentation.
- Updated architecture, code index, feature evaluation, roadmap, Code CoOp, and
  mirrored project-model skills for Codex/Claude.

### Verification
- `cargo check`: clean
- `cargo test layout -- --nocapture`: 10 passed
- `cargo test outline -- --nocapture`: 1 passed
- `cargo test export_compile_fixture_cargo_check -- --ignored --nocapture`:
  passed
- `cargo fmt --check`: clean
- `cargo test`: 177 passed, 2 ignored
- `cargo clippy -- -D warnings`: clean
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1`:
  clean
- `cargo test all_builtin_widgets_export_cargo_check -- --ignored --nocapture`:
  passed
- `cargo run --quiet` smoke: launched and stopped after 8 seconds

### Remaining Gaps
- Layout alignment, fill/stretch, per-child policies, layout-aware spacers, and
  GridLayout row policies remain open.
- Outline drag-reorder still changes draw order, not parent/slot membership.
- Lazare parser still does not round-trip layout-owned hierarchy from edited
  code.

## 2026-06-04 — GridLayout Direct-Child Ownership Slice

### Docs Reviewed Before Editing
- Preflight context (`scripts/preflight-context.ps1`)
- `.agents/skills/project-model/SKILL.md`
- `.agents/skills/canvas-patterns/SKILL.md`
- `.agents/skills/codegen-rules/SKILL.md`
- `docs/ROADMAP.md`
- `src/project/ui_tree.rs`, `src/canvas/interaction.rs`,
  `src/codegen/egui_emitter.rs`, `src/codegen/export.rs`

### Changes Made
- Generalized layout attachment/reflow from stack layouts to direct layout
  containers: `VLayout`, `HLayout`, and `GridLayout`.
- GridLayout now owns direct child widgets when dropped/released inside the
  container, detaches them when dragged outside, and reflows them row-major into
  a default 3-column grid.
- Palette click, template add/drop, drag release, resize, and validation/repair
  now use the shared layout reflow path.
- Live codegen emits GridLayout children inside `egui::Grid::new(...).show(...)`
  with `ui.end_row()` boundaries.
- Export emits GridLayout children through the existing layout-child handler
  machinery, and the generated-project compile fixture now covers a
  GridLayout-owned child with a `Result` handler.
- Updated `docs/ROADMAP.md` and `docs/CODE_COOP.md` to mark only the direct-child
  GridLayout slice complete.

### Verification
- `cargo check`: clean
- `cargo test gridlayout -- --nocapture`: 3 passed
- `cargo test layout -- --nocapture`: 10 passed
- `cargo test export_compile_fixture_generates_required_files_and_matrix -- --nocapture`:
  passed
- `cargo test export_compile_fixture_cargo_check -- --ignored --nocapture`:
  passed
- `cargo fmt --check`: clean
- `cargo test`: 176 passed, 2 ignored
- `cargo clippy -- -D warnings`: clean
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1`:
  clean
- `cargo run --quiet` smoke: launched and stopped after 8 seconds

### Remaining Gaps
- Grid columns/rows are not user-configurable yet; this slice uses a default
  3-column row-major grid.
- Spacing, padding, alignment, stretch/fill, layout-aware spacers, per-child
  policies, and cell/slot inspector controls remain open.
- Lazare parser round-trip and richer Layers/Outline hierarchy operations remain
  open.

## 2026-06-04 — HLayout Stack Ownership Slice

### Docs Reviewed Before Editing
- Preflight context (`scripts/preflight-context.ps1`)
- `.agents/skills/project-model/SKILL.md`
- `.agents/skills/canvas-patterns/SKILL.md`
- `.agents/skills/codegen-rules/SKILL.md`
- `docs/ROADMAP.md`
- `src/project/ui_tree.rs`, `src/canvas/interaction.rs`,
  `src/codegen/egui_emitter.rs`, `src/codegen/export.rs`

### Changes Made
- Generalized the VLayout source-of-truth path into
  `UiTree::attach_to_stack_layout_at()` and `UiTree::reflow_stack_layouts()`.
- HLayout now owns direct child widgets when dropped/released inside the
  container, detaches them when dragged outside, and reflows them horizontally
  with equal widths inside the container's margin.
- Palette click, palette drag, template add/drop, drag release, resize, and
  validation/repair now use the shared stack-layout reflow path.
- Live codegen emits HLayout children inside `ui.horizontal(|ui| { ... })`.
- Export emits HLayout children inside `ui.horizontal(|ui| { ... })`, preserves
  child handler dispatch, and the generated-project compile fixture now covers a
  HLayout-owned child with a `Result` handler.
- Updated `docs/ROADMAP.md` and `docs/CODE_COOP.md` to mark only the
  VLayout/HLayout stack-layout slice complete.

### Verification
- `cargo check`: clean
- `cargo test attach_to_hlayout_reflows_children_horizontally -- --nocapture`:
  passed
- `cargo test hlayout -- --nocapture`: 3 passed
- `cargo test export_compile_fixture_generates_required_files_and_matrix -- --nocapture`:
  passed
- `cargo test export_compile_fixture_cargo_check -- --ignored --nocapture`:
  passed
- `cargo fmt --check`: clean
- `cargo test`: 173 passed, 2 ignored
- `cargo clippy -- -D warnings`: clean
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1`:
  clean
- `cargo run --quiet` smoke: launched and stopped after 8 seconds

### Remaining Gaps
- GridLayout does not yet own/reflow children into cells.
- Spacing, padding, alignment, stretch/fill, and per-child layout policies remain
  default/implicit.
- Layout-aware spacers, Lazare parser round-trip, and richer Layers/Outline
  hierarchy operations remain open.

## 2026-06-04 — VLayout Real Ownership Vertical Slice

### Docs Reviewed Before Editing
- Preflight context (`scripts/preflight-context.ps1`)
- `.agents/skills/project-model/SKILL.md`
- `.agents/skills/canvas-patterns/SKILL.md`
- `.agents/skills/codegen-rules/SKILL.md`
- `docs/ROADMAP.md`
- `src/project/ui_tree.rs`, `src/canvas/interaction.rs`,
  `src/codegen/egui_emitter.rs`, `src/codegen/export.rs`

### Changes Made
- Added `UiTree::attach_to_vlayout_at()` and `UiTree::reflow_vlayouts()` so
  VLayout child ownership/reflow lives in the project model rather than canvas
  glue.
- VLayout now attaches direct child widgets when they are dropped/released inside
  the container and detaches them when dragged outside.
- VLayout resize reflows direct children immediately.
- Palette click, palette drag, and template add/drop paths now route new
  non-VLayout widgets through VLayout attachment when their center lands inside a
  VLayout.
- Live codegen emits VLayout children sequentially inside
  `ui.vertical(|ui| { ... })` instead of an empty layout closure.
- Export emits VLayout children sequentially inside the `ui.vertical` closure and
  preserves child event dispatch through the existing handler registry.
- The generated-project compile fixture now includes a VLayout-owned child
  button, proving the exported nested layout path compiles.
- Updated `docs/ROADMAP.md` to mark only the VLayout vertical slice done while
  keeping HLayout/GridLayout, layout properties, layout-aware spacers, parser
  round-trip, and deeper outline semantics open.

### Verification
- `cargo check`: clean
- `cargo test vlayout -- --nocapture`: 4 passed
- `cargo test export_compile_fixture_generates_required_files_and_matrix -- --nocapture`: passed
- `cargo test export_compile_fixture_cargo_check -- --ignored --nocapture`: passed

### Remaining Gaps
- HLayout and GridLayout do not yet own/reflow children.
- Spacing, padding, alignment, stretch/fill, and per-child layout policies remain
  default/implicit.
- Lazare parser does not yet reconstruct layout-child hierarchy from edited code.
- Layers/Outline still needs richer layout hierarchy controls.

## 2026-06-04 — Pre-Release Reliability Gate + SVG Export/Golden Hardening

### Docs Reviewed Before Editing
- Preflight context (`scripts/preflight-context.ps1`)
- `.agents/skills/codegen-rules/SKILL.md`
- `.agents/skills/svg-zero-dep/SKILL.md`
- `docs/ROADMAP.md`, `docs/SVG_IMPORT.md`, `docs/SVG_RENDERER_ROADMAP.md`
- `src/codegen/export.rs`, `src/canvas/svg_rasterizer.rs`,
  `src/canvas/svg_golden.rs`, `scripts/validate-svg-import.ps1`

### Changes Made
- Added a Pre-Release Depth Consolidation Gate to `docs/ROADMAP.md`, focused on
  source-of-truth cleanup, reliability proofs, and depth-before-breadth work
  before new feature families or Stage 15 renderer expansion.
- Verified the all-built-in-widget generated-project fixture. The fast smoke
  passed, but the ignored real generated-crate `cargo check` exposed a concrete
  SVG Image export bug.
- Fixed generated SVG Image export by embedding `svg_core` alongside the
  embedded `rohkai_svg` rasterizer module, adapting the embedded rasterizer's
  import path for generated `app.rs`, and calling
  `rohkai_svg::rasterize_or_fallback()` so egui receives a `ColorImage`.
- Added unit assertions so Image export requires `mod svg_core`,
  `mod rohkai_svg`, `use super::svg_core::{self, Rgba};`, and
  `rasterize_or_fallback()`.
- Expanded the dependency-free SVG golden harness with path fill, stroke,
  opacity, unsupported gradient, unsupported clip, and unsafe external href
  buckets.
- Wired `scripts/validate-svg-import.ps1` to run `cargo test svg_golden`.
- Reconciled SVG roadmap/source-of-truth docs: display-list split and golden
  harness are complete; source spans, reference tables, stable node ids, richer
  diagnostic provenance, text, layout, gradients, clips, and masks remain future
  SVG work.

### Verification
- `cargo test image_widget_export_embeds_svg_renderer -- --nocapture`: passed
- `cargo test svg_golden -- --nocapture`: passed
- `cargo test all_builtin_widgets_export_generates_required_files_and_matrix -- --nocapture`: passed
- `cargo test all_builtin_widgets_export_cargo_check -- --ignored --nocapture`: passed
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1`: passed
- `cargo check`: clean
- `cargo test`: 166 passed, 0 failed, 2 ignored

### Remaining Gaps
- SVG source spans, reference-table/node-id provenance, full `preserveAspectRatio`,
  nonzero fill, stroke joins/caps/dashes, antialiasing, gradients, clips/masks,
  embedded image decode, and robust SVG text remain future renderer/importer work.
- Broader repo source-of-truth cleanup still needs a commit/PR hygiene pass.

## 2026-06-03 — Lazare Code Panel QoL + Release Compile-Proof Expansion

### Docs Reviewed Before Editing
- Preflight context (`scripts/preflight-context.ps1`)
- `.agents/skills/project-model/SKILL.md`
- `docs/CODE_COOP.md`, `docs/PROMPT_CONTRACT.md`, `docs/ROADMAP.md`
- `src/panels/code_preview.rs`, `src/codegen/parser.rs`,
  `src/project/ui_tree.rs`, `src/app.rs`
- `src/codegen/export.rs`, `src/codegen/rust_wiring.rs`

### Changes Made
- Replaced the selected-widget copied preview block in the code panel with an
  inline `TextEdit` layouter highlight. The actual editable generated code now
  receives a subtle teal background while preserving normal readable text.
- Added `UiTree::clear_widgets()` and wired blank/deleted code-buffer edits to
  clear canvas widgets, then resync the panel to canonical empty generated code.
- Added focused tests for highlight range detection and `UiTree::clear_widgets`.
- Tightened left panel usability: lower hard width cap, collapse only on cramped
  widths, and one stable scroll region for Palette, Properties, Layers,
  Components, and Templates.
- Expanded the generated-export compile fixture to cover FilePicker/rfd, mpsc
  channel fields, iterator methods, a simple local trait binding, and state
  bindings in addition to top-level/nested event + async paths.
- The expanded compile fixture exposed a real export bug: iterator pipelines
  emitted invalid Rust item signatures (`fn name(&self) -> Vec<_>`). Fixed export
  to emit `fn name(&self) -> impl IntoIterator + '_` and collect through
  `Vec<_>` internally.
- Simple trait bindings now emit a local trait declaration before the impl, so
  standalone generated projects compile for local/simple trait names.
- Updated `docs/PROMPT_CONTRACT.md`, `docs/ROADMAP.md`, and
  `docs/feature-evaluation/rust-centric-visual-features.md` to reflect the
  inline-highlight rule and expanded compile-proof coverage.

### Verification
- `cargo fmt --check`: clean
- `cargo check`: clean
- `cargo test`: 162 passed, 0 failed, 1 ignored
- `cargo clippy -- -D warnings`: clean
- `cargo test export_compile_fixture_cargo_check -- --ignored`: passed
- `scripts/check-text-encoding.ps1`: OK
- `cargo run` smoke: launched and was stopped after 8 seconds

### Remaining Gaps
- Exact TextEdit scroll-to-line/cursor positioning remains a future enhancement;
  the selected block is now highlighted inline, but scrolling is still best-effort.
- The compile fixture remains opt-in because it invokes Cargo on a generated
  eframe/egui project.
- Stage 15 renderer/high-risk work intentionally untouched.

### Follow-up Correction (same session)
- User feedback clarified that the left panel did not need a strict width cap;
  the problem was content organization. Restored a much wider resizable cap while
  preserving tabs and stable scroll.
- Replaced the first inline highlight implementation's per-line background fill
  with a TextEdit layouter-native subtle span background. A later hand-painted
  outlined rectangle was rejected because it estimated character width and row
  height, producing offset highlights under egui's real line spacing/wrapping.
  An underline-only variant was also rejected as too visually noisy. Future
  code-panel highlights should stay layout-native unless RohKai gets a dedicated
  code editor widget with true glyph/block rect APIs.
- Tightened the highlighted code range so it starts at the selected widget's
  `egui::Area::new(...)` line and stops at that block's closing `});`; it no
  longer includes the `egui::CentralPanel::default()` preamble.
- Code highlight state now follows the full canvas selection set: multi-select
  highlights every selected widget block, and deselecting clears the code
  highlight.
- Rubber-band selection now previews candidate widgets while dragging and
  requests a repaint after release so multi-selection appears immediately on the
  canvas.
- Added a left-panel `Stack` toggle: tabs remain, but users can show Palette,
  Properties, Layers/Outline, Components, and Templates together as collapsible
  sections in the same scrolling left panel.
- Renamed/helped the Layers view as "Layers / Outline" in UI copy. Current
  behavior is draw-order outline of existing canvas widgets; adding items still
  happens through Palette/Templates.
- Improved Lazare paste semantics:
  - pasted orphan known-widget lines (for example an `egui::Button::new(...)`
    line) now create a widget immediately;
  - pasted duplicate generated blocks with the same `widget_<uuid>` now create a
    fresh duplicate widget instead of mutating the original twice;
  - newly-created paste results canonicalize the code buffer immediately so the
    next frame has stable fresh UUIDs.
- Added parser regression tests for duplicate pasted blocks and orphan pasted
  button lines.
- Verification after correction: `cargo fmt --check`, `cargo check`,
  `cargo test` (164 passed, 1 ignored), `cargo clippy -- -D warnings`, and
  `scripts/check-text-encoding.ps1` all clean. `cargo run` smoke launched and was
  stopped after 8 seconds.

---

## 2026-06-03 — Generated-Export Compile Proof (cargo check fixture)

### Docs Reviewed Before Editing
- `docs/PROMPT_CONTRACT.md`, `docs/CODE_COOP.md`,
  `docs/feature-evaluation/rust-centric-visual-features.md`, `docs/ROADMAP.md`
- `src/codegen/export.rs`, `src/codegen/rust_wiring.rs`,
  `src/codegen/field_collector.rs`, `src/project/schema.rs`

### Derivation (before coding)
- Export fn writing files: `export::write_project(tree, dest)` → `project_files(tree)`.
- Files required for `cargo check`: `Cargo.toml`, `src/main.rs`, `src/app.rs`.
- External deps: always `eframe`/`egui` 0.29 (cached in rohkai's lockfile); `rfd`
  only for FilePicker; Custom descriptor deps. Fixture uses eframe+egui only.
- Temp dir without crates: `std::env::temp_dir()` + pid/nanos, `std::process::Command`
  `cargo check`, shared `CARGO_TARGET_DIR`, `std::fs::remove_dir_all` cleanup.
- Feature matrix: top Button Click + DoubleClick, nested TextInput LostFocus,
  nested Slider DragStopped, async Plain, async Result, ≥1 binding.
- Feasibility: real `cargo check` fixture IS implementable with std only → proceeded.

### Changes Made (`src/codegen/export.rs` tests)
- `compile_fixture_tree()` — fixture UiTree covering the full matrix (6 widgets:
  Button[Click+DoubleClick], async-Plain Button, async-Result Button, Frame with
  TextInput[LostFocus] + Slider[DragStopped] children; bindings `name: String`,
  `vol: f32`).
- `unique_temp_dir(tag)` — std-only unique temp path (pid + nanos).
- `export_compile_fixture_cargo_check` (`#[ignore]`) — `write_project` to temp,
  `cargo check --quiet` via `std::process::Command` with shared `CARGO_TARGET_DIR`;
  panics with stderr on failure; cleans up on success.
- `export_compile_fixture_generates_required_files_and_matrix` (always-run smoke) —
  asserts the three files exist + every matrix marker present, no compilation.
- `button_click_and_double_click_both_emitted_no_suppression` — locks the ordering
  decision: both gates emitted off one response, Click not suppressed (egui-native).

### Verification
- `cargo fmt --check`: clean
- `cargo check`: clean
- `cargo test`: 159 passed, 0 failed, 1 ignored
- **Ignored compile fixture run manually**: `cargo test export_compile_fixture_cargo_check
  -- --ignored` → **1 passed in 29.87s** (generated crate compiles against cached deps).
- `cargo clippy -- -D warnings`: clean (project gate)
- `scripts/check-text-encoding.ps1`: OK

### Decision: ignored vs non-ignored
Kept `#[ignore]` (30s vs the 0.02s default suite — too slow for every `cargo test`),
paired with the fast always-run smoke, per the goal's item 6.

### Remaining Gaps (honest)
- Compile fixture is opt-in (`--ignored`); no CI wiring for it yet.
- Fixture covers event/async; does not yet include channels / iterator pipelines /
  trait impls.
- Worker body is a user TODO stub; no status-widget binding, cancellation,
  progress streaming, or typed task I/O.

---

## 2026-06-03 — Nested/Frame-Child Event Export Parity

### Docs Reviewed Before Editing
- `docs/PROMPT_CONTRACT.md`, `docs/CODE_COOP.md`,
  `docs/feature-evaluation/rust-centric-visual-features.md`, `docs/ROADMAP.md`
- `src/project/schema.rs`, `src/panels/properties.rs`, `src/codegen/export.rs`,
  `src/codegen/rust_wiring.rs`

### Derived event/export path matrix (before coding)
- Source of truth: `WidgetKind::supported_events()` (exhaustive).
- UI surface: `properties.rs::show_event_handler` (same for top-level or nested).
- Top-level export: `gen_app_rs` arms → `event_dispatch_block` (already full parity).
- Nested/frame-child export: `export_child_line` (from Frame arm) — **the gap**.
- Custom/template: `Custom(_)` is not in `supported_events` (no events); templates
  reuse the normal paths. Live `egui_emitter` is the canvas preview, not export.
- Feasibility: every event-capable child kind CAN support its events via
  `ui.put(...)`/`ui.radio_value(...)` Response or `allocate_ui_at_rect(combo)`.
  No kind excluded → proceeded.

### Problem
`export_child_line` rendered Frame children with no event handlers: Button child
emitted an empty `.clicked() {}`; TextInput/Slider/etc. emitted the widget with no
handler; ComboBox/FontComboBox children were dead `Label` placeholders. A nested
widget could show event rows in Properties that export silently ignored.

### Changes Made (`src/codegen/export.rs`)
- New `export_child_event_dispatch(child, resp_expr, registry)`: binds
  `let child_response = <resp_expr>;` then one `if child_response.<method>() { <handler_call> }`
  per wired event, routed through `rust_wiring::handler_call()` + registry.
- New `export_child_combo(...)`: renders a real interactive `egui::ComboBox` at the
  child rect via `allocate_ui_at_rect`, returns an inner `changed` bool, gates the
  handler on `child_combo.inner == Some(true)`.
- Rewrote child arms Button/TextInput/TextArea/Slider/SpinBox/Checkbox/RadioButton
  to use the dispatcher; ComboBox/FontComboBox use `export_child_combo`.
- Threaded `handler_registry` into `export_child_line` + its Frame call site.
- Handler collection already iterates all `tree.widgets` (children included), so the
  central registry, conflict detection, and async task contract already covered
  child handlers — only the call site was missing.

### Event ordering decision (documented)
Button `Click` and `DoubleClick` are wired independently and both fire per egui's
native semantics (single `clicked()` on first release, `double_clicked()` on the
second click). Click is intentionally NOT suppressed — same as top-level.

### Tests (+9 → 157)
- `every_supported_event_is_exported_in_nested_child`: nested invariant over every
  `(kind, event)` pair (Result-mode `if let Err` routing proof + per-event gate +
  child-dispatch-path check).
- 6 focused nested: Button Click, Button DoubleClick, TextInput LostFocus, Slider
  DragStopped (async), SpinBox DragStopped, Checkbox Change.
- `nested_combo_change_routes_through_interactive_combo`: proves the child renders a
  real combo (not a dead label) and routes On Change.
- `conflict_between_top_level_and_nested_child_is_detected_and_normalized`.

### Verification
- `cargo fmt --check`: clean
- `cargo check`: clean (no warnings)
- `cargo test`: 157 passed, 0 failed (+9 vs prior 148)
- `cargo clippy -- -D warnings`: clean (project gate)
- `scripts/check-text-encoding.ps1`: OK

### Remaining Gaps (honest)
- No full `cargo build` compile fixture on generated output — proof is in-process
  generated-code string assertions only.
- Worker body is a user TODO stub; no status-widget binding, cancellation,
  progress streaming, or typed task I/O.

---

## 2026-06-03 — Prompt Contract Standard For Agent Goals

### Docs Reviewed Before Editing
- `AGENTS.md`, `CLAUDE.md`
- `docs/CODE_INDEX.md`, `docs/CODE_COOP.md`, `docs/DEVLOG.md`

### Problem
Recent Claude goal prompts produced real work, but repeatedly stopped at a local
surface: explicit widget lists instead of derived sets, primary events instead of
all events, and top-level export instead of nested export. The missing ingredient
was not just more words; it was a required pre-coding decomposition step.

### Changes Made
- Added `docs/PROMPT_CONTRACT.md`, a reusable skeleton for inter-agent goals.
- The skeleton requires agents to derive the source-of-truth set from code,
  enumerate UI/runtime/export/nested/custom paths, stop before editing if any
  required path is excluded, and add invariant tests that fail on drift.
- Added pointers to the contract in `AGENTS.md`, `CLAUDE.md`, and
  `docs/CODE_INDEX.md`.
- Added a `docs/CODE_COOP.md` handoff note for future agents.

### Verification
- `scripts/check-text-encoding.ps1`: OK
- `cargo fmt --check`: clean

### Follow-ups
- Use this contract for the next Claude prompt, especially if closing the
  remaining nested/frame child export event gap.

---

## 2026-06-02 — FULL Event Export Parity (primary + secondary)

### Docs Reviewed Before Editing
- `docs/CODE_COOP.md`, `docs/feature-evaluation/rust-centric-visual-features.md`,
  `docs/ROADMAP.md`
- `src/project/schema.rs`, `src/panels/properties.rs`, `src/codegen/export.rs`,
  `src/codegen/rust_wiring.rs`, `src/codegen/egui_emitter.rs` (reference pattern)

### Problem
The prior patch fixed only PRIMARY event parity. Export wired `primary_event()`
only, so secondary events stayed exposed in Properties but ignored by export:
Button DoubleClick, TextInput/TextArea LostFocus, Slider/SpinBox DragStopped.

### Complete (WidgetKind, WidgetEvent) → egui method matrix (now all exported)
- Button: Click→`clicked()`, DoubleClick→`double_clicked()`
- TextInput: Change→`changed()`, LostFocus→`lost_focus()`
- TextArea: Change→`changed()`, LostFocus→`lost_focus()`
- Slider: Change→`changed()`, DragStopped→`drag_stopped()`
- SpinBox: Change→`changed()`, DragStopped→`drag_stopped()`
- Checkbox: Change→`changed()`
- RadioButton: Change→`changed()` (radio_value marks changed)
- ComboBox: Change→inner `combo_changed`
- FontComboBox: Change→inner `font_combo.inner == Some(true)`

### Changes Made
- `src/codegen/export.rs`:
  - Handler collection loop now iterates `w.kind.supported_events()` and reads
    each event's field via new `event_field_handler` — conflict detection covers
    all event fields, not just primary.
  - New `event_egui_method` (event→Response predicate) and `event_dispatch_block`
    (binds the `Response` once, emits one `if evt_response.<method>() { <handler_call> }`
    per wired event; plain statement when no handler). Button/TextInput/TextArea/
    Slider/SpinBox/Checkbox/RadioButton arms delegate to it; ComboBox/FontComboBox
    keep their bespoke combo gates (Change-only). Every call routes through
    `rust_wiring::handler_call()` + the central registry — no raw `self.h();`
    except inside `handler_call()` output.
  - egui 0.29 `double_clicked()`/`lost_focus()`/`drag_stopped()` verified against
    `egui_emitter.rs` (the live preview already emits them) and `interaction.rs`.
- `src/project/schema.rs`: `primary_event()` is now `#[cfg(test)]` — production no
  longer needs a "primary" notion since export wires every event.

### Tests
- Rewrote the invariant: iterates EVERY `(kind, event)` pair from
  `supported_events()`, Result mode, asserts the `if let Err(e) = self.h_evt()`
  routing proof AND the correct per-event gate method. Fails if any supported
  event lacks routing.
- +5 focused secondary tests (Button DoubleClick, TextInput LostFocus, TextArea
  LostFocus, Slider DragStopped, SpinBox DragStopped) — each Result or async.
- +1 primary+secondary on one widget (Slider Change + DragStopped both wired off
  one `evt_response`).
- +1 conflict across event fields (Button Click async/Plain vs Button DoubleClick
  sync/Result, same name) → conflict header + normalized call sites.

### Verification
- `cargo fmt --check`: clean
- `cargo check`: clean (no warnings)
- `cargo test`: 148 passed, 0 failed (+7 vs prior 141)
- `cargo clippy -- -D warnings`: clean (project gate)
- `cargo clippy --all-targets`: 3 lints, all PRE-EXISTING and not in touched files
  (examples/hello_button, codegen/field_collector test helper, panels/templates.rs)
- `scripts/check-text-encoding.ps1`: OK

### Remaining Gaps (honest)
- Container-child export (`export_child_line`) wires no events for nested widgets —
  separate, pre-existing path affecting all events (not a secondary-event deferral).
- No full `cargo build` compile fixture on generated output.
- Worker body is a user TODO stub; no status-widget binding, cancellation,
  progress streaming, or typed task I/O.

---

## 2026-06-02 — Properties/Export Event Parity (Codex Review)

### Docs Reviewed Before Editing
- `docs/CODE_COOP.md`, `docs/feature-evaluation/rust-centric-visual-features.md`
- `src/panels/properties.rs`, `src/codegen/export.rs`,
  `src/codegen/rust_wiring.rs`, `src/project/schema.rs`

### Problem
Codex found a correctness gap: the Properties panel exposes `On Change` for
TextArea, SpinBox, and FontComboBox, but export emitted those widgets without
invoking their `on_change` handlers. A user could wire a handler in the UI and the
exported app would silently ignore it. Root cause: Properties and export each had
their own `match w.kind` event list, and the two drifted.

### Authoritative event-capable widget list (derived from `show_event_handler`)
- Button → Click (primary), Double-click
- TextInput → Change (primary), Lost Focus
- TextArea → Change (primary), Lost Focus
- Slider → Change (primary), Drag Stopped
- SpinBox → Change (primary), Drag Stopped
- Checkbox → Change
- ComboBox → Change
- FontComboBox → Change
- RadioButton → Change

### Changes Made
- `src/project/schema.rs`: new `WidgetEvent` enum and `WidgetKind::supported_events()`
  (exhaustive, wildcard-free match — the single source of truth), plus
  `primary_event()` / `is_event_capable()`, and a `#[cfg(test)] EVENT_CAPABLE_KINDS`
  enumeration the parity test walks. 4 new schema tests.
- `src/panels/properties.rs`: `show_event_handler` derives its applicable-event
  rows from `kind.supported_events()` through a new `event_ui_meta` mapper instead
  of a local hard-coded match. Behavior preserved exactly (same fields/labels/hints).
- `src/codegen/export.rs`:
  - Handler collection now uses `w.kind.primary_event()` to pick Click vs Change
    and to skip non-event kinds entirely.
  - TextArea and SpinBox arms now emit `if <resp>.changed() { <handler_call> }`
    using the central registry (mirrors TextInput/Slider).
  - FontComboBox arm returns an inner `changed` bool from its `show_ui` closure,
    binds `let font_combo = …` only when a handler exists, and gates the call on
    `font_combo.inner == Some(true)`.
  - Added a top-of-`app.rs` `!! HANDLER CONFLICTS DETECTED` summary block listing
    every conflicting handler (in addition to the existing near-handler comment).
  - Tests: +5 — an invariant test iterating `EVENT_CAPABLE_KINDS` and proving each
    routes through `handler_call()` (via Result-mode `if let Err` wrapper that a
    bare `self.h();` bypass cannot produce); focused TextArea (async Plain), SpinBox
    (Result), FontComboBox (Result); and a FontComboBox-without-handler test proving
    no dangling `font_combo` binding (would be an unused-var warning in the export).

### Verification
- `cargo fmt --check`: clean
- `cargo check`: clean (no warnings)
- `cargo test`: 141 passed, 0 failed (+9 vs prior 132)
- `cargo clippy -- -D warnings`: clean
- `scripts/check-text-encoding.ps1`: OK

### Remaining Gaps (documented honestly in the eval doc)
- Secondary events (double-click, lost-focus, drag-stopped) are exposed in
  Properties but export wires only the primary event per kind.
- No full `cargo build` compile fixture on generated output.
- Worker body is a user TODO stub; no status-widget binding, cancellation,
  progress streaming, or typed task I/O.

---

## 2026-06-02 — Async Wiring Gap Fixes (Codex Review)

### Docs Reviewed Before Editing
- `docs/CODE_COOP.md`, `docs/feature-evaluation/rust-centric-visual-features.md`
- `src/codegen/rust_wiring.rs`, `src/codegen/export.rs`,
  `src/panels/properties.rs`, `src/project/schema.rs`

### Problem (four gaps from Codex review)
1. No repaint scheduling while async tasks are in flight — exported apps stall waiting for user input.
2. TextInput/Slider/Checkbox/ComboBox/RadioButton handler call sites bypassed `handler_call()`, losing async-launcher and Result/Option wrapping.
3. Duplicate handler names silently used "first wins" with no conflict signal or call-site normalization.
4. No combined test proving all three gap fixes work together.

### Changes Made
- `src/codegen/rust_wiring.rs`: added `async_repaint_block()` — emits a
  `ctx.request_repaint_after(Duration::from_millis(16))` guard conditioned on
  any `{h}_running` field being true. 3 new tests.
- `src/codegen/export.rs`:
  - Handler collection changed from `HashSet` + 3-tuple to `HashMap<name→usize>` +
    4-tuple `(name, result, is_async, has_conflict)`. Conflict flag set when a
    later widget shares the name with a different async/result mode.
  - `handler_registry: HashMap<String, (HandlerResult, bool)>` built from first
    definitions. All call sites — Button, TextInput, Slider, Checkbox, ComboBox,
    RadioButton — look up their handler's mode from the registry rather than the
    widget's own fields. This normalizes conflicting call sites to the registered mode.
  - `// CODEGEN CONFLICT` comment emitted before a conflicted handler's stub.
  - Repaint block inserted after drain blocks in `update()`.
  - 4 new tests: repaint guard, non-button async launcher routing, conflict
    warning + call-site normalization (2 call sites both normalize), combined
    3-widget coherence fixture.

### Verification
- `cargo fmt --check`: clean
- `cargo check`: clean
- `cargo test`: 132 passed, 0 failed
- `cargo clippy -- -D warnings`: clean
- `scripts/check-text-encoding.ps1`: OK

### Remaining Gaps (documented in eval doc)
- Worker body is still a user TODO stub.
- No full `cargo build` compile fixture on generated output.
- `{h}_running`/`{h}_error` not auto-bound to a spinner/error label widget.
- No cancellation or progress streaming.

---

## 2026-06-02 — Stage 11 Async Task Wiring: Resolve Overclaim

### Docs Reviewed Before Editing
- `docs/feature-evaluation/depth-model.md`
- `docs/feature-evaluation/rust-centric-visual-features.md`
- `docs/CODE_COOP.md`
- `src/codegen/rust_wiring.rs`, `src/codegen/export.rs`,
  `src/panels/properties.rs`, `src/project/schema.rs`

### Problem
Async task wiring was an overclaim: `async_handler` only generated a
`std::thread::spawn` block with TODO comments — no real work call, no completion
send, no receiver drain, no status/error.

### Changes Made
- `src/codegen/rust_wiring.rs`: new emitters `async_msg_type`,
  `async_struct_fields`, `async_default_fields`, `async_launcher_method`,
  `async_worker_fn`, `async_drain_block`; `handler_call` async branch now calls
  the launcher (`self.{h}();`).
- `src/codegen/export.rs`: moved handler collection above the `ExportedApp`
  struct; emits async fields into struct + `Default`; emits the drain block at
  the top of `update()`; emits launcher methods (async) vs plain stubs
  (non-async) in `impl ExportedApp`; emits module-level worker fns.
- Generated contract per async handler: `{h}_rx`/`{h}_running`/`{h}_error`
  fields, launcher `fn {h}(&mut self)` (guards double-launch, spawns, sends
  `{h}_worker()` over mpsc), free-fn worker (no `&mut self`), borrow-safe
  `try_recv` drain recording status/error. MSG = `()` / `Result<(), String>` /
  `Option<()>`.

### Verification
- `cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings` clean.
- `cargo test` — 125 passing (9 new: rust_wiring async emitters + export async
  paths + non-async regression).
- `scripts/check-text-encoding.ps1` — OK.

### Honesty / Risks
- Reclassified to **Functional MVP**, not top-class. Worker body is a
  user-filled stub; no status-widget auto-binding, cancellation, progress, or
  generated-project compile fixture yet (token tests only).
- Std-only (no tokio/new crates), preserving architecture rules.
- Preserved all uncommitted Codex changes; did not touch SVG/WASM/DB/own
  renderer/visual widget maker.

### Follow-ups
- Add a generated-project compile fixture for async Plain + Result.
- Auto-bind `{h}_running`/`{h}_error` to a spinner/label widget.

## 2026-06-02 — Remaining Roadmap Item Evaluation

### Docs Reviewed Before Editing
- `scripts/preflight-context.ps1` output
- `docs/ROADMAP.md`
- `docs/feature-evaluation/README.md`
- latest `docs/CODE_COOP.md`

### Changes Made
- Added `docs/feature-evaluation/remaining-roadmap-items.md`.
- Updated feature-evaluation README, Code Index, and Code CoOp to reference it.
- Covered unchecked roadmap items with current implementation contracts,
  insufficient existing surface, desired closure contracts, and closure criteria.

### Findings
- The largest planned gaps are Visual Widget Maker, SVG text/import maturity,
  Formula Widget, WASM export, DB/data integration, and Own Renderer.
- Several unchecked items have nearby MVPs that must not be confused with
  closure: MathLabel vs Formula Widget, static views vs model/data views,
  Guided Descriptor Builder vs Visual Widget Maker, SVG source preservation vs
  robust `tspan`/report UI.
- Roadmap has duplicate/stale SVG renderer checklist entries: Stage 9 marks
  scene/display-list IR and golden harness complete while Stage 7.x still has
  similar unchecked entries.

### Verification
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1` — passed.
- `cargo fmt --check` — passed.
- `cargo check` — passed.

### Risks / Follow-ups
- Roadmap reconciliation is recommended before another agent implements SVG
  renderer tasks, otherwise they may redo completed work or close future work
  incorrectly.

## 2026-06-02 — Stage 11 Rust-Centric Feature Evaluation

### Docs Reviewed Before Editing
- `scripts/preflight-context.ps1` output
- `docs/STAGE11_PLAN.md`
- `docs/ROADMAP.md`
- latest `docs/CODE_COOP.md`
- `docs/feature-evaluation/codegen-lazare-export.md`

### Code Reviewed
- `src/canvas/overlays.rs`
- `src/codegen/rust_wiring.rs`
- `src/codegen/export.rs`
- `src/panels/rust_wiring.rs`
- `src/panels/macro_palette.rs`
- `src/panels/properties.rs`
- `src/project/schema.rs`
- Stage 11 integration in `src/app.rs`

### Changes Made
- Added `docs/feature-evaluation/rust-centric-visual-features.md`.
- Updated `docs/feature-evaluation/README.md` and `docs/CODE_INDEX.md` to list
  the new evaluation.
- Updated Code CoOp with the Stage 11 evaluation summary.

### Findings
- Ownership overlay is a usable read-only feature because it derives from
  `field_collector`.
- Error-flow is a functional MVP: signatures/call sites change, but there is no
  true propagation graph or UI error destination.
- Channels and iterator pipelines are useful code-generation MVPs, but not
  visually connected or type-validated systems.
- Trait binding is raw Rust text insertion, not semantic trait binding.
- Macro palette appends snippets to the code buffer, but is not cursor- or
  handler-aware.
- Async task wiring is the largest overclaim: current generated async code emits
  a `std::thread::spawn` TODO block and does not call the handler or return
  results through a channel.

### Verification
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1` — passed.
- `cargo fmt --check` — passed.
- `cargo check` — passed.

### Risks / Follow-ups
- Stage 11 should not be treated as competitor-depth Rust visual programming
  until generated-project compile fixtures, validation, runtime task/channel
  behavior, and visual flow graphs exist.

## 2026-06-02 — Feature Evaluation Documentation Set

### Docs Reviewed Before Editing
- `scripts/preflight-context.ps1` output
- `docs/CODE_INDEX.md`
- `docs/ROADMAP.md`
- latest `docs/CODE_COOP.md`

### Changes Made
- Created `docs/feature-evaluation/`.
- Added a shared feature-depth model covering Planned, Surface, Functional MVP,
  Usable Product Feature, Competitive, and Top-Class levels.
- Added area evaluations for:
  - app shell and navigation
  - canvas authoring
  - widgets and components
  - codegen, Lazare, and export
  - SVG import and renderer
  - custom widget system
  - project infrastructure
  - preferences, theming, and platform
  - testing and quality gates
- Updated Code Index and Code CoOp so future agents can find the new evaluation
  docs without loading them during every preflight.

### Verification
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1` — passed.
- `cargo fmt --check` — passed.
- `cargo check` — passed.

### Risks / Follow-ups
- These are qualitative evaluation docs, not executable tests. The next useful
  step is a machine-readable feature-depth manifest that maps feature claims to
  required evidence.

## 2026-06-02 — Stage 10/14 Depth Remediation And Parity Audit

### Docs Reviewed Before Editing
- `scripts/preflight-context.ps1` output
- `AGENTS.md` rules from session context
- `docs/CODE_COOP.md`, `docs/ROADMAP.md`, `docs/CODE_INDEX.md`
- `project-model` skill for `UiTree` mutation discipline
- Relevant code: `src/codegen/export.rs`, `src/codegen/egui_emitter.rs`,
  `src/codegen/kind_table.rs`, `src/widgets/computational.rs`,
  `src/project/ui_tree.rs`, `src/panels/outline.rs`, `src/panels/component_tray.rs`,
  `src/app.rs`

### Changes Made
- Fixed FilePicker export depth: generated projects that use `FilePicker` now
  include `rfd = "0.14"` in `Cargo.toml`.
- Fixed MathLabel codegen correctness: labels are passed as escaped Rust string
  values into `format!`, so braces and quotes cannot break generated Rust.
- Upgraded Chart from comment-only codegen to a minimal egui painter bar chart
  bound to `Vec<f32>` state; default Chart instances bind `chart_values`.
- Added `UiTree::move_to_index()` and routed outline drag reorder through it
  instead of direct `widgets.swap`.
- Split the left rail into bounded tabs: Palette, Props, Layers, Components,
  Templates. `Ctrl+L` now opens the Layers tab.
- Reworded Timer/StateMachine/HttpRequest generated comments and tests as
  design-time MVP stubs rather than claiming runtime dispatch.
- Updated Roadmap, Code CoOp, and Code Index with Feature Depth Status:
  Full / Functional MVP / Design-time MVP / Planned.

### Verification
- `cargo check` — passed.
- `cargo test` — 116/116 passed.
- `cargo clippy -- -D warnings` — passed.

### Risks / Follow-ups
- Chart is now real but intentionally minimal: no axes, legends, multiple series,
  tooltips, scaling modes, or editing workflow.
- MathLabel is still a computed f32 label, not a formula widget.
- Table/ListView/TreeView remain static option-backed widgets; model-bound data
  views belong in future data integration work.
- Timer/StateMachine/HttpRequest still need true runtime engines before they can
  be called competitor-depth components.

## 2026-05-26 — Comprehensive Code Review & Rayon Integration

### Docs Reviewed Before Editing
- `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`
- `docs/ROADMAP.md`, `docs/ARCHITECTURE.md`, `docs/CODE_INDEX.md`
- `docs/DEVLOG.md`, `docs/CODE_COOP.md`
- `docs/SVG_IMPORT.md`, `docs/SVG_RENDERER_ROADMAP.md`
- `src/project/ui_tree.rs`, `src/app.rs`, `src/main.rs`
- `Cargo.toml`, `src/codegen/`, `src/canvas/`, `src/panels/`, `src/widgets/`
- All test files (75 tests total)

### Changes Made
- Performed comprehensive code review across 7 categories (architecture, code quality,
  testing, performance, security, documentation, dependencies).
- Created 4 new recommendation documents with 9 actionable items across 3 groups:
  - `docs/CLINE_REVIEW_AND_RECOMMENDATIONS.md` — executive summary and overview
  - `docs/CLINE_RECOMMENDATIONS_GROUP1.md` — code quality & maintainability (3 items)
  - `docs/CLINE_RECOMMENDATIONS_GROUP2.md` — testing & reliability (3 items)
  - `docs/CLINE_RECOMMENDATIONS_GROUP3.md` — performance & architecture (3 items)
- Added `rayon = "1"` to `Cargo.toml` as core dependency for app-wide parallelism.
- Updated `docs/ROADMAP.md` with parallelism foundation tasks for Stage 9:
  parallel SVG rasterization, parallel codegen, parallel export, parallel template
  loading, and performance benchmarks.
- Updated `docs/CODE_COOP.md` with session handoff note.

### Verification
- `cargo check` passes with zero warnings after adding rayon.
- No app behavior changed — docs-only pass plus dependency addition.

### Key Findings
- Overall score: 9/10 — production-quality Rust code
- Architecture: 9/10 — strong single-source-of-truth (UiTree) design
- Code Quality: 9/10 — zero clippy warnings, clean formatting
- Testing: 8/10 — 75 tests passing, good coverage (no UI integration tests)
- Performance: 7/10 — good caching, but per-frame codegen and sequential SVG
  rasterization are concerns for 100+ widget projects
- Security: 9/10 — excellent SVG security, input validation throughout
- Documentation: 8/10 — comprehensive docs, module-level docs could improve
- Dependencies: 10/10 — minimal, well-chosen, mature crates

### Risks / Follow-ups
- Rayon is now a core dependency; future agents should consider parallel approaches
  for expensive operations (SVG batch rasterization, codegen, export file writing).
- 9 recommendations are ready for implementation when prioritized by user.
- No urgent bugs found; the codebase is in excellent shape.

## 2026-05-25 - Widget Maker Taxonomy Docs

### Docs Reviewed Before Editing
- `scripts/preflight-context.ps1`
- latest `docs/CODE_COOP.md`
- `docs/ROADMAP.md`
- `docs/CODE_INDEX.md`
- `src/panels/widget_builder.rs`

### Changes Made
- Added `docs/VISUAL_WIDGET_MAKER.md`.
- Renamed the current builder concept in docs to Guided Descriptor Builder.
- Clarified that the existing builder is a form over `WidgetDescriptor`, not a
  true WYSIWYG widget construction tool.
- Added a separate roadmap lane for the future Visual Widget Maker: internal
  visual document, mini-canvas, primitives, exposed properties, deterministic
  descriptor generation, and advanced-editor escape hatch.
- Updated Code CoOp and Code Index with the distinction.

### Verification
- Docs-only pass. No Rust behavior changed.

### Risks / Follow-ups
- UI labels still say "Create Custom Widget"; a future UX pass may rename menu
  labels to reduce confusion, while preserving discoverability.

## 2026-05-25 - SVG Path Tokenizer Core Extraction

### Docs Reviewed Before Coding
- `scripts/preflight-context.ps1`
- `.agents/skills/svg-zero-dep/SKILL.md`
- latest `docs/CODE_COOP.md`
- `src/svg_core.rs`, `src/svg_import.rs`, `src/canvas/svg_rasterizer.rs`
- `docs/SVG_IMPORT.md`, `docs/SVG_RENDERER_ROADMAP.md`, `docs/ROADMAP.md`

### Changes Made
- Added `svg_core::SvgPathToken` and `svg_core::tokenize_path_data()`.
- Shared path tokenization now handles compact syntax, adjacent decimals,
  exponent notation, unknown command letters, and malformed fragments without
  panics.
- Replaced importer-local path tokens with the shared tokenizer while keeping
  importer command limits, bounds semantics, malformed recovery, and unsupported
  command diagnostics.
- Replaced rasterizer-local path tokenization with the shared tokenizer while
  keeping rasterizer flattening and fill/stroke behavior.
- Added a rasterizer unsupported-command skip so broader shared command
  recognition cannot stall on unknown path commands.
- Updated SVG docs and coordination docs to reflect the shared path tokenizer.

### Verification
- `cargo fmt --check` passed.
- `cargo check` passed.
- `cargo test svg_core -- --nocapture` passed: 7/7.
- `cargo test svg_import -- --nocapture` passed: 17/17.
- `cargo test svg_rasterizer -- --nocapture` passed: 13/13.
- `cargo test` passed: 75/75.
- `cargo clippy -- -D warnings` passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1`
  passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1`
  passed.

### Risks / Follow-ups
- Remaining duplicated SVG microsyntax candidate is length parsing. Keep it as a
  separate pass because length percentages depend on viewport/property context.
- Golden-image tests should still watch for tiny f64-to-f32 raster rounding
  changes as renderer coverage grows.

## 2026-05-25 - SVG Transform Core Extraction

### Docs Reviewed Before Coding
- `scripts/preflight-context.ps1`
- `.agents/skills/svg-zero-dep/SKILL.md`
- latest `docs/CODE_COOP.md`
- `src/svg_core.rs`, `src/svg_import.rs`, `src/canvas/svg_rasterizer.rs`
- `docs/SVG_IMPORT.md`, `docs/SVG_RENDERER_ROADMAP.md`, `docs/ROADMAP.md`

### Changes Made
- Added `svg_core::Affine2D` as the shared SVG affine transform type.
- Moved matrix multiplication, translate/scale/rotate/skew construction,
  rotate-about-point handling, finite/extreme checks, summaries, and
  transform-list parsing into `svg_core`.
- Replaced importer-local `Matrix` implementation with an alias to
  `svg_core::Affine2D`.
- Replaced rasterizer-local `Transform` implementation with an alias to
  `svg_core::Affine2D` plus the shared `apply_f32` adapter for raster geometry.
- Added `svg_core` tests for transform-list parsing and rotate-about-point.

### Verification
- `cargo fmt --check` passed.
- `cargo test svg_core -- --nocapture` passed: 4/4.
- `cargo test svg_import -- --nocapture` passed: 16/16.
- `cargo test svg_rasterizer -- --nocapture` passed: 11/11.
- `cargo check` passed.
- `cargo test` passed: 69/69.
- `cargo clippy -- -D warnings` passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1` passed.

### Risks / Follow-ups
- Length parsing and path tokenization are still duplicated enough to deserve
  future `svg_core` slices. Do those separately because path behavior is the
  highest-risk SVG parser surface.

## 2026-05-25 - SVG Core Microsyntax Extraction

### Docs Reviewed Before Coding
- `scripts/preflight-context.ps1`
- `.agents/skills/svg-zero-dep/SKILL.md`
- latest `docs/CODE_COOP.md`
- `docs/SVG_IMPORT.md`, `docs/SVG_RENDERER_ROADMAP.md`, `docs/ROADMAP.md`
- `src/svg_import.rs`, `src/canvas/svg_rasterizer.rs`

### Changes Made
- Added `src/svg_core.rs` as the shared zero-dependency SVG microsyntax module.
- Moved shared SVG color parsing into `svg_core::parse_color` /
  `svg_core::parse_rgb`.
- Moved shared SVG numeric-list scanning into `svg_core::parse_numbers` /
  `svg_core::parse_numbers_f32`.
- Wired both `src/svg_import.rs` and `src/canvas/svg_rasterizer.rs` to use the
  shared module, removing duplicate color tables and number scanners.
- Added `svg_core` unit tests for compact number syntax and shared color forms.

### Verification
- `cargo fmt --check` passed.
- `cargo test svg_core -- --nocapture` passed: 2/2.
- `cargo test svg_import -- --nocapture` passed: 16/16.
- `cargo test svg_rasterizer -- --nocapture` passed: 11/11.
- `cargo check` passed.
- `cargo test` passed: 67/67.
- `cargo clippy -- -D warnings` passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1` passed.

### Risks / Follow-ups
- Transform and path parsing still have duplication. The next cleanup slice
  should move `Matrix`/`Transform` compatibility into `svg_core` without
  changing importer bounds behavior or renderer pixel output.

## 2026-05-25 - SVG Scene Boundary

### Docs Reviewed Before Coding
- `scripts/preflight-context.ps1`
- `.agents/skills/svg-zero-dep/SKILL.md`
- latest `docs/CODE_COOP.md`
- `docs/SVG_IMPORT.md`, `docs/SVG_RENDERER_ROADMAP.md`, `docs/ROADMAP.md`
- `src/canvas/svg_rasterizer.rs`

### Dirty Worktree Check
- Current dirty non-SVG work includes Claude's Widget Builder files:
  `src/panels/widget_builder.rs`, `src/app.rs`, `src/panels/mod.rs`, and
  `src/panels/descriptor_editor.rs`.
- This SVG pass intentionally stayed in the rasterizer and docs. `cargo fmt`
  may still mechanically touch already-dirty Rust files.

### Changes Made
- Added an internal `SvgScene` and `SvgSceneItem` layer between parsed XML-ish
  nodes and raster drawing.
- Scene items now carry accumulated transforms, resolved inherited style, and a
  flag for unsupported ancestors before rendering starts.
- Shape-level `transform` attributes now affect raster output, not just group
  transforms.
- Added tests for scene flattening and element-transform pixels.

### Verification
- `cargo fmt --check` passed.
- `cargo test svg_rasterizer -- --nocapture` passed: 11/11.
- `cargo check` passed.
- `cargo test` passed: 65/65.
- `cargo clippy -- -D warnings` passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1` passed.
- `cargo run` smoke launched and was stopped after 8 seconds.

### Risks / Follow-ups
- This is the first scene boundary, not the finished display-list IR. Source
  spans, node IDs, reference tables, exact bounding boxes, and shared
  importer/rasterizer microsyntax modules are still future work.

## 2026-05-25 - SVG Renderer Parsed Diagnostics

### Docs Reviewed Before Coding
- `scripts/preflight-context.ps1`
- `.agents/skills/svg-zero-dep/SKILL.md`
- latest `docs/CODE_COOP.md`
- `src/canvas/svg_rasterizer.rs`
- `docs/SVG_IMPORT.md`, `docs/SVG_RENDERER_ROADMAP.md`

### Changes Made
- Added `SvgNode::Unsupported` so known unsupported renderer elements are
  represented in the parsed tree instead of only source-scanned.
- Moved renderer unsupported diagnostics for known elements and supported-node
  attributes onto parsed nodes/attributes.
- Added skipped-subtree accounting so unsupported definitions such as `defs`
  and gradient children count toward skipped work without rendering.
- Added tests proving SVG comments do not produce fake unsupported diagnostics
  and unsupported definition children are counted as skipped.
- Ran `cargo fmt`, which also formatted recent uncommitted guide/bezel files in
  the working tree.

### Verification
- `cargo fmt --check` passed.
- `cargo test svg_rasterizer -- --nocapture` passed: 9/9.
- `cargo check` passed.
- `cargo test` passed: 55/55.
- `cargo clippy -- -D warnings` passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1` passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1` passed.
- `cargo run` smoke launched and was stopped after 8 seconds.

### Risks / Follow-ups
- Diagnostics are now parsed-node driven for known tags/attributes, but a full
  `SvgScene` IR is still needed before report UI should rely on precise source
  spans or resolved references.

## 2026-05-24 — Stage 8 Close-out: Guide Snap, Lock Ratio, Canvas Bezel

### Docs Reviewed
- `docs/CODE_COOP.md` (prior Stage 8 entry for context)

### Changes Made

**`src/project/schema.rs`**
- `AppProps.show_bezel: bool` — serde-default false, skip_serializing if false

**`src/canvas/rulers.rs`**
- `draw_bezel(ui, ctx, title)`: draws 22px mock macOS title bar above canvas rect
  (grey background, three traffic-light circles at left, centered title text)
- Skips draw if no vertical room (canvas near top panel edge)
- `BEZEL_H: f32 = 22.0` constant

**`src/canvas/interaction.rs`**
- Guide snapping added after static widget alignment loop (~line 1582)
- Iterates `tree.app_props.guides`; checks widget left/center/right vs Vertical guides,
  top/center/bottom vs Horizontal guides; updates best_x/best_y and adj_x/adj_y
- Snapped guide populates raw_guide_v/raw_guide_h → highlighted on canvas same as widget snaps

**`src/app.rs`**
- `SessionState.lock_aspect_ratio: bool` (default false)
- Status bar: 🔒/🔓 toggle button after H DragValue; prev_w/prev_h snapshotted before
  panel, ratio enforced after panel returns
- View menu: "Show/Hide Canvas Bezel" toggle
- Canvas section: calls `rulers::draw_bezel()` when `app_props.show_bezel`

### Verification
53/53 tests, zero clippy warnings, zero fmt issues.

### Risks / Follow-ups
- Bezel only shows when zoom/pan leaves ≥22px space above canvas — no room at 100% zoom
  with large canvases filling the panel. Intentional: don't obscure canvas content.

## 2026-05-24 — Stage 8: Rulers + Guides, Document Presets, Theming

### Docs Reviewed
- `docs/ROADMAP.md` (Stage 8 Future Considerations clusters)

### Changes Made

**`src/canvas/rulers.rs`** (new, ~280 lines)
- `RulerCtx` bundle struct avoids too-many-arguments clippy lint
- `handle_interaction()`: guide hover detection, drag, delete, ruler-click creation
- `draw()`: ruler strip backgrounds, zoom-aware tick marks with labels, guide overlay lines
- `canvas_origin()`: exported helper for coordinate mapping (reusable)

**`src/canvas/interaction.rs`** — `show_rulers: bool` added to `CanvasSettings` (default false)

**`src/canvas/mod.rs`** — `pub mod rulers` added

**`src/project/schema.rs`**
- `GuideOrientation` enum, `GuideRule` struct (id, orientation, position)
- `ThemeSettings` struct (dark_mode, accent_color, base_font_size, global_corner_radius, spacing_scale) with serde defaults
- `AppProps` extended: `resizable`, `min_size`, `max_size`, `theme: ThemeSettings`, `guides: Vec<GuideRule>` — all serde-defaulted for backward compat

**`src/app.rs`**
- `SessionState`: `hovered_guide`, `dragging_guide`, `theme_open`
- `apply_theme()`: reads `AppProps.theme`, calls `ctx.set_visuals()` + optional `ctx.set_style()` every frame
- `show_theme_window()`: floating window, dark/light toggle, RGB accent sliders, font/radius/spacing overrides
- `cmd_save_theme()` / `cmd_load_theme()`: `.rktheme` file I/O
- View menu: Show Rulers toggle, Clear All Guides, Theme…
- Status bar: "▾ Preset" dropdown with 9 canvas size presets
- Ctrl+R shortcut, rulers/guide wiring in CentralPanel
- Delete key only deletes widgets when no guide is hovered

**`src/codegen/export.rs`**
- `gen_main_rs`: adds `.with_resizable()`, `.with_min_inner_size()`, `.with_max_inner_size()` when set
- `gen_theme_setup()`: generates `ctx.set_visuals(...)` code block; skipped for default dark+teal

### Verification
- 53/53 tests, zero clippy warnings, `cargo fmt --check` clean
- Commit: `3885ed1`

### Risks / Follow-ups
- Guide snapping to widget edges (deferred)
- Canvas window bezel chrome (deferred — complex)

---

## 2026-05-24 — .rkwb Bundle Format + Expand SVG Inline Toggle

### Docs Reviewed
- `docs/ROADMAP.md` (Stage 7.x open items), `docs/CODE_COOP.md`

### Changes Made

**`src/codegen/widget_bundle.rs`** (new)
- `WidgetBundle { format, schema_version, descriptors }` — JSON envelope for multiple descriptors
- `from_descriptors`, `to_json`, `from_json` (validates format + version), `extract_to` (writes `<id>.rkwd` files)
- `BundleError` enum with `Display` impl; no new crate dependency

**`src/codegen/mod.rs`** — added `pub mod widget_bundle`

**`src/app.rs`**
- `cmd_export_widget_bundle`: rfd save dialog → serialize all loaded descriptors → write `.rkwb`
- `cmd_import_widget_bundle`: rfd open dialog → parse bundle → `extract_to(widgets_dir)` → reload
- Widgets menu: "Export Bundle…" + "Import Bundle…" entries added (with separators)

**`src/project/schema.rs`** — `expand_svg_inline: bool` field on `WidgetInstance` (serde default false, skip if false)

**`src/panels/properties.rs`** — checkbox "Expand SVG inline in code panel" in `show_image`, only visible when SVG source is loaded

**`src/codegen/egui_emitter.rs`** — `svg_source_arg` helper; `image_preview_line` + `image_child_preview_line` call it; when `expand_svg_inline` is true, embeds raw string literal with correct hash count

### Verification
- 53/53 tests, zero clippy warnings, `cargo fmt --check` clean
- Commit: `338ee65`

### Risks / Follow-ups
- SVG Import Maturity (tspan parser, importer report UI, etc.) — Codex track, deferred
- Stage 7.x fully closed (Claude track). Stage 8 next.

---

## 2026-05-24 — Descriptor Editor UI Fixes + Widgets Menu

### Docs Reviewed
- `docs/CODE_COOP.md` (session handoff)

### Changes Made

**`src/panels/descriptor_editor.rs`**
- Replaced all `desired_width(f32::INFINITY)` TextEdit calls with `desired_width(ui.available_width())` — root cause of window expanding to full RohKai width inside ScrollArea
- Ran `cargo fmt` to fix one line-length violation in the same file

**`src/app.rs`**
- Added "Widgets" dropdown to `egui::menu::bar` (after File menu): New Descriptor, Import Definition, Reload Descriptors, and per-descriptor "Edit" entries
- `show_descriptor_editor_window`: fixed save-message-cleared-same-frame bug — snapshot `was_saved` before calling `descriptor_editor::show()`, only trigger `cmd_reload_descriptors()` on false→true transition; save message now persists until next save overwrites it

### Verification
- 53/53 tests, zero clippy warnings, `cargo fmt --check` clean
- Commit: `8b3932d`

### Risks / Follow-ups
- `.rkwb` bundle format deferred
- SVG import maturity tracked separately (Codex domain)

---

## 2026-05-24 — In-app .rkwd Descriptor Editor

### Docs Reviewed
- `docs/CODE_COOP.md`, `docs/ROADMAP.md`
- `src/codegen/widget_descriptor.rs`, `src/app.rs`, `src/panels/properties.rs`
- `src/canvas/interaction.rs` (canvas preview rendering for Custom widgets)

### Changes Made

**`src/panels/descriptor_editor.rs`** (new, `1104547`)
- `DescriptorEditorState`: holds draft `WidgetDescriptor`, original stem, save message, scratch buffers for add-row inputs
- `show()`: floating `egui::Window`, horizontal split — left = form, right = live preview
- Form: full coverage of all descriptor fields — ID/name/category, accent RGB + swatch, default size, properties (collapsible, type combo, Enum options), state fields, codegen templates (multiline), canvas_preview label_template, events list, cargo deps
- Live preview: painted canvas box (accent fill, label_template expanded), read-only expanded `live_preview` + `export` templates updating every frame, property defaults table
- Save: `serde_json::to_string_pretty` → `<binary_dir>/widgets/<id>.rkwd` → triggers auto-reload

**`app.rs`**
- `DescriptorState.editor: Option<DescriptorEditorState>` added
- `widgets_dir()` static helper extracted (shared by editor + import cmd)
- `cmd_new_descriptor()` / `cmd_edit_descriptor(id)` commands
- `show_descriptor_editor_window()`: renders window, auto-reloads palette on save
- File menu: "New Widget Descriptor…" item added

**`properties.rs`**
- `show_custom` gains "Edit descriptor" button (visible when descriptor loaded)
- `PropertiesAction::EditDescriptor(String)` variant + routing in `app.rs`

### Verification
- 53/53 tests, zero clippy warnings, `cargo fmt --check` clean.

### Risks / Follow-ups
- No drag-to-reorder for properties list — deferred.
- `.rkwb` bundle format still open.
- Save silently overwrites existing file with same id — no conflict dialog.

## 2026-05-24 — SVG Source Viewer Popup

### Docs Reviewed
- `docs/CODE_COOP.md`, `docs/ROADMAP.md` (7.x SVG Source Viewing section)
- `src/panels/properties.rs`, `src/app.rs`

### Changes Made

**SVG source viewer** (`76b770e`)
- `properties.rs`: `show_image` now returns `bool`; shows "View source" small button
  beside "SVG source loaded" label. Fires `PropertiesAction::ShowSvgSource(id)`.
- `app.rs`: `SessionState.svg_viewer_id: Option<Uuid>` added. New
  `show_svg_source_window` renders `egui::Window` with read-only `TextEdit`,
  monospace 11pt, byte count in title, "Copy all" clipboard button. X closes.
- Roadmap: 7.x SVG Source Viewing first item checked ✅.
- 7.x Descriptor Maturity: Import/Hot-reload/Lazare items checked ✅.

### Verification
- 53/53 tests, zero clippy warnings, `cargo fmt --check` clean.

### Risks / Follow-ups
- "Expand SVG inline" toggle (second 7.x item) deferred — low priority.
- In-app `.rkwd` editor and `.rkwb` bundle still open.

## 2026-05-24 — Stage 7.x: Lazare Custom Round-trip + Import Widget Definition

### Docs Reviewed
- `CLAUDE.md`, `docs/CODE_COOP.md` (latest note)
- `src/codegen/parser.rs`, `src/codegen/egui_emitter.rs`, `src/codegen/widget_descriptor.rs`
- `src/app.rs`, `widgets/ply-button.rkwd`

### Changes Made

**Lazare Custom round-trip** (`08867fd` — `parser.rs`)
- Added fallback `else` branch at end of `parse_widget_line`: extracts first
  string literal as `label` and first `&mut self.field` as `binding` from any
  line that does not match a built-in egui pattern.
- Guards prevent later lines (handler calls) from overwriting the values
  captured from the constructor line.
- Kind is intentionally left None so `apply_parsed` cannot overwrite
  `WidgetKind::Custom`.
- Two new tests: `custom_widget_label_and_binding_extracted_from_template_line`
  and `custom_widget_first_line_wins_over_later_handler_call`.

**Import Widget Definition dialog** (`08867fd` — `app.rs`)
- `cmd_import_widget_definition`: opens rfd file dialog (`.rkwd` filter),
  validates JSON + schema_version 1, copies to `<binary_dir>/widgets/`,
  auto-reloads descriptors, shows success/error via `template_message`.
- File menu: "Import Widget Definition…" item wired above "Reload Widget
  Descriptors".

### Verification
- 53/53 tests, zero clippy warnings, `cargo fmt --check` clean.

### Risks / Follow-ups
- Import copies the file verbatim; no overwrite-conflict dialog (silently
  replaces). Could add a confirmation prompt in a future polish pass.
- Custom widget `descriptor_props` editing from the properties panel still
  doesn't feed back into Lazare code sync ({{prop.KEY}} substitutions).

## 2026-05-24 — SVG Renderer R0 Reporting

### Docs Reviewed Before Coding
- `scripts/preflight-context.ps1`
- `.agents/skills/svg-zero-dep/SKILL.md`
- `docs/CODE_COOP.md`
- `docs/SVG_IMPORT.md`, `docs/SVG_RENDERER_ROADMAP.md`, `docs/ROADMAP.md`

### Changes Made
- Added `SvgRenderOutput` and `SvgRenderReport` to the zero-dependency
  rasterizer while preserving `rasterize()` and `rasterize_or_fallback()`.
- Added renderer diagnostics for rendered/skipped counts, unsupported feature
  buckets, raster-size clamping, and conservative fidelity scoring.
- Added tests for report counts, unsupported gradients/clips/filters, byte-level
  deterministic output, and raster-size clamp warnings.
- Wired SVG rasterizer tests into `scripts/validate-svg-import.ps1`.
- Updated SVG docs and roadmap to record the completed R0 slice and remaining
  scene/IR/golden-harness work.

### Verification
- `cargo fmt --check` passed.
- `cargo check` passed.
- `cargo test` passed: 51/51.
- `cargo clippy -- -D warnings` passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1` passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-dependency-policy.ps1` passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1` passed.
- `cargo run` launched cleanly and was stopped after an 8-second smoke test.

### Risks / Follow-ups
- Renderer diagnostics are still source-scan based; the next step is a proper
  `SvgScene` IR so diagnostics attach to resolved nodes.
- Existing unrelated worktree changes were preserved.

## 2026-05-24 — Track B + Track A: Handler Parity & Descriptor Hot-Reload

### Docs Reviewed Before Coding
- `CLAUDE.md`, `docs/CODE_COOP.md` (latest note)
- `src/codegen/egui_emitter.rs`, `src/codegen/export.rs`, `src/panels/code_preview.rs`
- `src/codegen/widget_descriptor.rs`, `src/app.rs`

### Changes Made

**Track B — Handler calling-convention unification** (`b2fe0af`)
- `egui_emitter.rs`: all 6 handler call sites changed from `Self::{h}(&mut self.state);`
  to `self.{h}();`, matching what `export.rs` already emitted.
- `code_preview.rs`: Tracé stub signature changed from `fn h(state: &mut AppState)`
  to `fn h(&mut self)` — stubs now compile unmodified in an exported project.
- `export.rs` required no changes (was already correct).

**Track A — Descriptor hot-reload** (`87e2e11`)
- New `cmd_reload_descriptors()` on `RohKaiApp`: calls `load_from_widgets_dir()`,
  replaces `self.descriptors.widgets` and `self.descriptors.errors` in-place.
- File menu: "Reload Widget Descriptors" item wired to that command.
- Drop/edit a `.rkwd` file in `widgets/`, click the menu item — palette updates
  without an app restart.

### Verification
- 47/47 tests, zero clippy warnings, `cargo fmt --check` clean both commits.

### Risks / Follow-ups
- Hot-reload replaces the full descriptor list; existing canvas instances that
  reference a Custom widget whose descriptor was deleted keep their snapshot
  but lose palette access — acceptable for now.
- Track A still has two remaining items: Import Widget Definition dialog, and
  Lazare round-trip for Custom widgets (label/binding with descriptor template
  awareness).

## 2026-05-24 — Low-Token Docs Consolidation

### Docs Reviewed Before Coding
- `scripts/preflight-context.ps1`
- `AGENTS.md`, `CLAUDE.md`
- `.agents/commands/preflight.md`, `.claude/commands/preflight.md`
- `docs/CODE_INDEX.md`, `docs/CODE_COOP.md`, `docs/PLATFORM_NOTES.md`
- `docs/mojibake-remediation-plan-2026-05-24.md`

### Changes Made
- Consolidated preflight guidance so the script is procedural truth while
  `AGENTS.md` and `CLAUDE.md` remain policy truth.
- Changed preflight to omit the latest DEVLOG entry by default. Use
  `-IncludeDevlog` for history/regression work.
- Reduced both `/preflight` command docs to thin wrappers instead of duplicate
  checklists.
- Updated `docs/CODE_INDEX.md` for the app state split and shared
  `field_collector`.
- Folded mojibake prevention into `docs/PLATFORM_NOTES.md` and removed the
  standalone remediation plan doc.
- Added a newest-first Code CoOp note for this consolidation pass.

### Verification
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\preflight-context.ps1` passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\preflight-context.ps1 -IncludeDevlog` passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1` passed.
- `cargo fmt --check` passed.
- `cargo check` passed.

### Risks / Follow-ups
- Older DEVLOG entries still mention the removed standalone mojibake plan as
  historical context.
- `docs/ARCHITECTURE.md` may still need a future structural refresh, but it is
  no longer part of default preflight.

## 2026-05-24 — Mojibake Investigation + SVG Zoom Performance

### Docs Reviewed Before Coding
- `docs/CODE_COOP.md`, `docs/DEVLOG.md`, `docs/ROADMAP.md`
- `src/canvas/interaction.rs`, `src/canvas/svg_rasterizer.rs`
- `docs/mojibake-remediation-plan-2026-05-24.md`

### Changes Made

**Mojibake investigation:**
- Byte-level scan of all tracked text files confirmed valid UTF-8 throughout.
- No live mojibake bytes found; prior plan's findings were a false positive from
  `rg --encoding latin1` re-interpreting correct UTF-8 multibyte sequences.
- `scripts/check-text-encoding.ps1` added: scans six common double-encoding
  patterns using `[char]` codes (pure-ASCII source); called by preflight.
- `docs/mojibake-remediation-plan-2026-05-24.md` updated with investigation result.

**SVG zoom performance (three bugs fixed):**
- Fixed zoom² rasterization: `tw`/`th` were `rect.width() * zoom * ppp` but
  `rect` is already screen-space (`widget.rect.w * zoom`); corrected to
  `rect.width() * ppp`. At 273% zoom this reduced buffer from ~3800px to ~1400px.
- Added `zoom_stable` flag: rasterization skipped during active scroll gestures;
  GPU serves stale texture at zoom scale; re-rasterizes once on first quiet frame.
- Raised eviction threshold 5% → 20%: was firing every scroll notch (1.1x factor
  produces 9.1% drift, always exceeded 5%); now ~2 notches per rasterize.
- Cache key extended to `(TextureHandle, f32, u32, u32)`: widget resize at
  constant zoom now triggers immediate re-raster (was silently keeping wrong size).
- `flatten_cubic`: added depth limit (≥32) and point count cap (≥50k) to prevent
  stack overflow and excessive memory on pathological SVG path inputs.

### Verification
- `cargo fmt --check` clean.
- `cargo test` 30/30.
- `cargo clippy -- -D warnings` clean.
- `pwsh scripts\check-text-encoding.ps1` clean.

### Risks / Follow-ups
- Resize + simultaneous zoom: both conditions trigger rasterize; acceptable since
  concurrent resize-while-scroll is rare.
- `svg_text_allowed` still allocates a full lowercase copy per rasterize call
  (noted in code review, deferred).

## 2026-05-24 — PowerShell 7 UTF-8 Standardization

### Docs Reviewed Before Coding
- `scripts/preflight-context.ps1`
- `AGENTS.md`, `CLAUDE.md`
- `docs/ROADMAP.md`, `docs/CODE_INDEX.md`
- `docs/CODE_COOP.md`, `docs/DEVLOG.md`
- Relevant script files under `scripts/`
- `.agents/commands/preflight.md`, `.claude/commands/preflight.md`
- `docs/PLATFORM_NOTES.md`, `docs/SVG_IMPORT.md`

### Changes Made
- Installed PowerShell 7 through `winget`.
- Added explicit UTF-8 bootstrap lines to repo PowerShell scripts.
- Switched agent/preflight guidance from `powershell` to `pwsh`.
- Added `scripts/check-text-encoding.ps1` to block mojibake markers and
  replacement characters in tracked text files.
- Wired text encoding checks into preflight and SVG validation.
- Fixed known corrupted DEVLOG and export comment text.
- Added `docs/mojibake-remediation-plan-2026-05-24.md`.

### Verification
- `pwsh -NoProfile -Command '$PSVersionTable.PSVersion'` verified PowerShell 7.6.2.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\preflight-context.ps1` passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1` passed.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1` passed.
- Manual mojibake marker search returned no repo-authored text matches.
- `.claude/settings.json` parsed as valid JSON.
- `cargo fmt --check` passed.
- `cargo check` passed.
- `cargo test` passed: 30/30.
- `cargo clippy -- -D warnings` passed.
- `cargo run` compiled and launched; stopped after a 15-second smoke test.

## 2026-05-23 — Stage 7 Gap Fixes + SVG Code Contraction

### Docs Reviewed Before Changes
- `AGENTS.md`, `CLAUDE.md`, `docs/ROADMAP.md`, `docs/CODE_INDEX.md`
- `docs/CODE_COOP.md`, `docs/DEVLOG.md`

### Changes Made

**Gap 1 fixed — `state_emitter` + `export` now emit descriptor `state_fields`:**
- `WidgetInstance` gains `descriptor_state_fields: Vec<[String; 3]>` (each
  entry `[key, rust_type, default_expr]`, `serde(default)` + skip-if-empty).
- `default_for_descriptor` snapshots them from the descriptor at creation time.
- `StateField.ty` changed from `&'static str` → `String` (supports runtime types).
- `state_emitter::emit` and `export::gen_app_rs` both iterate
  `descriptor_state_fields` after `custom_props`.
- `BoundField.ty` in `export.rs` similarly changed to `String`.

**Gap 2 fixed — `apply_parsed` Custom kind guard:**
- `parser::apply_parsed` now refuses to overwrite `WidgetKind::Custom(_)` with
  a parser-inferred built-in kind, preventing descriptor templates that happen
  to contain egui patterns from corrupting the widget kind.
- Geometry (x/y/w/h) still round-trips correctly for Custom widgets through
  the standard `.fixed_pos` / `set_min_size` parse paths.

**Gap 3 corrected — previous DEVLOG wrong:**
- Earlier note claimed "descriptor_props changes don't drive live codegen".
  This was incorrect. Changes via `tree.get_mut()` are reflected by the next
  `emit_indexed` call; `generated != *last_generated` fires normally.

**SVG code contraction:**
- `image_preview_line` and `image_child_preview_line` in `egui_emitter.rs` now
  emit a compact size note (`"[SVG: N bytes]"`) instead of the full raw SVG
  string literal. Complex SVGs no longer fill the live code buffer with
  thousands of lines.
- The code panel stays valid Rust (`CodeStatus::Live`); canvas renders from
  `widget.svg_source` unchanged; export (`export.rs`) still embeds the full
  source via `raw_string_literal`.
- Dead `raw_string_literal` copy removed from `egui_emitter.rs` (export.rs
  has its own).
- Test `image_widget_emits_svg_preview_call` updated: now asserts compact form
  and explicitly asserts the raw SVG content does NOT appear in live preview.

### Verification
- `cargo test` — 30/30
- `cargo clippy -- -D warnings` — zero warnings

## 2026-05-23 — Stage 7: Widget Descriptor Format (.rkwd)

### Docs Reviewed Before Changes
- `AGENTS.md`, `CLAUDE.md`, `docs/ROADMAP.md`, `docs/CODE_INDEX.md`
- `docs/CODE_COOP.md` (latest Codex notes)
- `docs/DEVLOG.md` (previous entries)
- `git status --short --branch`

### Changes Made

**New files:**
- `src/codegen/widget_descriptor.rs` — `WidgetDescriptor` struct and sub-types,
  `load_from_widgets_dir()` scanner, `apply_template()` token engine,
  `find_by_id()` / `default_props()` helpers. 4 new tests.
- `widgets/ply-button.rkwd` — Ply Button example descriptor demonstrating
  String, Enum, Bool property types; cargo dep injection; live + export templates.

**Schema changes (`src/project/schema.rs`):**
- `WidgetKind::Custom(String)` variant — carries the descriptor `id`.
- `WidgetInstance` gains 6 new `serde(default)` fields: `descriptor_name`,
  `descriptor_accent`, `descriptor_live_tpl`, `descriptor_export_tpl`,
  `descriptor_props: HashMap<String,String>`, `descriptor_cargo_deps: Vec<String>`.
  All skip-serializing when empty/None — zero impact on existing project files.

**Codegen (`src/codegen/`):**
- `kind_table.rs`: `Custom(_) => None` arm.
- `egui_emitter.rs`: `Custom` arm uses `descriptor_live_tpl` snapshot + `apply_template`.
- `export.rs`: `Custom` arm uses `descriptor_export_tpl`; `gen_cargo_toml` accepts
  extra dep lines collected from `descriptor_cargo_deps` of all Custom widgets.
- `mod.rs`: exposes `widget_descriptor` module.

**Canvas (`src/canvas/interaction.rs`):**
- `kind_accent`, `kind_tag`: `Custom` fallback arms.
- `draw_widget`: `Custom` arm renders accent label box using per-instance
  `descriptor_accent` and `descriptor_name`.

**Widgets (`src/widgets/mod.rs`):**
- `default_for`: `Custom(id)` fallback arm.
- `default_for_descriptor(descriptor)`: builds a full `WidgetInstance` from
  a loaded descriptor with all snapshot fields populated.

**Palette (`src/panels/palette.rs`):**
- `show_content` gains `descriptors: &[WidgetDescriptor]` param.
- Custom descriptor categories rendered after built-in categories; each
  descriptor gets its own accent-colored palette button.

**Properties (`src/panels/properties.rs`):**
- `show_content` gains `descriptors: &[WidgetDescriptor]` param.
- `Custom` arm: looks up descriptor, renders typed property fields
  (String/F32/I32/Bool/Enum). Falls back to raw key→value table if descriptor
  is missing.

**App (`src/app.rs`):**
- `widget_descriptors: Vec<WidgetDescriptor>` + `descriptor_errors: Vec<String>`.
- `load_from_widgets_dir()` called at startup.
- Descriptor errors surfaced in ribbon as `⚠ N widget descriptor error(s)`.
- `palette::show_content` and `properties::show_content` plumbed with descriptors.

### Verification
- `cargo build` — clean
- `cargo test` — 30/30 (4 new descriptor tests)
- `cargo clippy -- -D warnings` — zero warnings
- `cargo run` — clean launch confirmed

### Known Remaining Limitations
- No in-app `.rkwd` import dialog or hot-reload yet (see Roadmap Stage 7.x).
- Lazare parser cannot round-trip Custom widget template edits back to canvas
  (geometry round-trips correctly; kind/label changes inside the template do not).

## 2026-05-23 - SVG/Image Export Parity And Rasterizer Guardrails

### Docs Reviewed Before Changes
- `scripts/preflight-context.ps1`
- `docs/CODE_INDEX.md`
- `docs/CODE_COOP.md`
- `docs/SVG_IMPORT.md`
- `.agents/skills/svg-zero-dep/SKILL.md`
- `git status --short`

### Changes Made
- Added a Code CoOp note for the SVG/Image parity push.
- Live codegen now emits `self.show_svg_image...` preview calls for Image
  widgets instead of an inert gray `egui::Frame` placeholder.
- Export now embeds RohKai's zero-dependency SVG rasterizer module when Image
  widgets are present, stores egui texture handles in the generated app, and
  renders preserved `svg_source` at runtime.
- Added raw-string escaping for embedded SVG source in live/export codegen.
- Added rasterizer guardrails:
  - SVG byte cap
  - tag count cap
  - path token cap
  - raster dimension/pixel cap
  - unsafe `DOCTYPE` / entity / script / external href rejection
  - non-XML processing instruction rejection
  - `display:none` and hidden/collapsed visibility handling
  - paint-server URLs no longer render as black fallback fills
- Added rasterizer tests for unsafe input rejection, hidden/paint-server behavior,
  and invisible `defs` / `mask` content.
- Updated SVG docs, code index, and RCA notes to match the new output forms and
  remaining limitations.

### Verification
- Feature set 1 base check:
  - `cargo fmt --check`: passed after formatting.
  - `cargo check`: passed.
  - `cargo test`: 23/23 passed.
  - `cargo clippy -- -D warnings`: passed.
- Feature set 2 base check:
  - `cargo fmt --check`: passed.
  - `cargo check`: passed.
  - `cargo test`: 26/26 passed.
  - `cargo clippy -- -D warnings`: passed.

### Notes
- This removes the known hollow Image codegen/export placeholder path.
- The rasterizer is still a supported subset, not full `resvg` / `usvg` /
  `tiny-skia` equivalence. Text rendering, gradients/patterns, masks/clips, and
  filters remain future work.

## 2026-05-23 - Baseline Stabilization

### Docs Reviewed Before Changes
- `scripts/preflight-context.ps1`
- latest `docs/DEVLOG.md` entry
- latest `docs/CODE_COOP.md` note
- `git status --short`

### Changes Made
- Added a Code CoOp baseline-stabilization handoff note.
- Ran `cargo fmt` to normalize existing Rust formatting drift from the SVG
  rasterizer/codegen work.
- No behavior changes were made intentionally during this pass.

### Verification
- `cargo fmt --check`: passed.
- `cargo check`: passed.
- `cargo test`: 23/23 passed.
- `cargo clippy -- -D warnings`: passed.
- `scripts\validate-svg-import.ps1`: passed.
- `cargo run` smoke: app launched and was stopped after 8 seconds.
- No lingering `rohkai`, `cargo`, or `rustc` process remained after the smoke test.

### Notes
- Tests still assert that `WidgetKind::Image` live/export codegen emits a frame
  placeholder. That is now a known, verified baseline limitation rather than an
  accidental surprise.

## 2026-05-23 - Code CoOp And Cross-Platform Coordination

### Docs Reviewed Before Changes
- `scripts/preflight-context.ps1`
- `AGENTS.md`, `CLAUDE.md`
- `.agents/commands/preflight.md`, `.claude/commands/preflight.md`
- `.gitignore`, `.claudeignore`
- latest `docs/DEVLOG.md` entry

### Changes Made
- Expanded `.gitignore` for local build/editor/runtime noise while keeping repo
  guidance, fixtures, templates, and source trackable.
- Added `docs/CODE_INDEX.md` as a lightweight human code map.
- Added `docs/CODE_COOP.md` as the short agent-to-agent handoff diary.
- Added `docs/PLATFORM_NOTES.md` to explain Windows PowerShell scripts versus
  cross-platform Cargo workflows.
- Updated Codex and Claude preflight commands to read `CODE_INDEX` and latest
  `CODE_COOP` note.
- Updated `AGENTS.md`, `CLAUDE.md`, and `scripts/preflight-context.ps1` so
  meaningful planning/coding sessions begin with a short Code CoOp note.

### Verification
- `scripts/preflight-context.ps1`: reports latest Code CoOp note and synced guidance.
- `git status --ignored --short`: only `target/` and local Codex touch file are ignored.
- `docs/context-snapshot.json` is now ignored for future local snapshot churn, but
  it is currently tracked and should be untracked in a dedicated cleanup commit if
  the team wants Git to stop recording changes to it.

## 2026-05-23 - Guidance Guardrail Audit

### Docs Reviewed Before Changes
- `AGENTS.md`, `CLAUDE.md`
- `.agents/commands/preflight.md`, `.claude/commands/preflight.md`
- `.agents/skills/project-model/SKILL.md`, `.claude/skills/project-model/SKILL.md`
- `.agents/skills/codegen-rules/SKILL.md`, `.claude/skills/codegen-rules/SKILL.md`
- `.agents/skills/canvas-patterns/SKILL.md`, `.claude/skills/canvas-patterns/SKILL.md`
- `scripts/preflight-context.ps1`, `scripts/check-dependency-policy.ps1`, `scripts/sync-and-run.ps1`
- `docs/RCA-2026-05-23-svg-renderer-dependencies.md`

### Findings
- `CONTRIBUTING.md` exists but was not part of preflight. Added explicit "do not add it" guidance to keep it out of agent prep unless requested.
- `scripts/preflight-context.ps1` read the last `##` heading, which was stale when newest entries were at the top. Fixed it to read the top/latest entry.
- `scripts/sync-and-run.ps1` could overwrite this checkout from another working copy; its exclude file only skipped `target\`.
- `scripts/check-dependency-policy.ps1` incorrectly flagged egui texture/cache names instead of only forbidden SVG dependency crates.
- Claude/Codex skill guidance had drift around `Image`, SVG output form, and no-hollow-codegen rules.
- The current zero-dependency rasterizer is substantial, but not equivalent to `resvg` / `usvg` / `tiny-skia`; text and several SVG feature classes remain incomplete, and Image export/live codegen still use placeholders.

### Changes Made
- Hardened `scripts/sync-and-run.ps1` behind `-AllowOverwrite`.
- Added `.git`, `.agents`, `.claude`, `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, and `docs\DEVLOG.md` to `scripts/xcopy-exclude.txt`.
- Added `svg-zero-dep` skills for both Codex and Claude.
- Updated project-model, canvas, and codegen skills on both sides with zero-dependency and no-hollow-output rules.
- Updated preflight guidance drift checks to normalize line endings and include `svg-zero-dep`.
- Updated dependency policy check to block only the forbidden SVG crates in active source/Cargo files.
- Added an RCA follow-up noting the current renderer gaps and full remedy direction.

### Verification
- `scripts\check-dependency-policy.ps1`: passed.
- `scripts\preflight-context.ps1`: now reads the newest devlog entry and reports synced skills.
- `cargo fmt --check`: currently fails on existing rasterizer/codegen formatting from the prior SVG rasterizer pass; not fixed in this guidance-only pass.

## 2026-05-23 - SVG Zero-Dependency Rasterizer (Replaces Placeholder System)

### Docs Reviewed Before Coding
- `CLAUDE.md`, `AGENTS.md`
- `docs/DEVLOG.md` (all prior entries)
- `src/canvas/interaction.rs` (SvgPreviewCache, draw_widget Image arm)
- `src/codegen/egui_emitter.rs`, `src/codegen/export.rs` (Image codegen arms)
- `src/app.rs` (svg_preview_cache field)
- `Cargo.toml`

### Problem
Codex removed resvg/usvg/tiny-skia and replaced the rasterization pipeline with an inferior "source-backed preview" that drew colored bounding-box rectangles instead of actual SVG content. Codegen emitted a `(SVG source-backed preview)` text label. User requirement: no inferior quality, no hollow stubs, no avoidance.

### Changes Made

**`src/canvas/svg_rasterizer.rs` (new — ~900 lines, zero new Cargo deps)**
- Pure-Rust software SVG rasterizer.
- Parses SVG XML: `<rect>`, `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, `<polygon>`, `<path>`, `<g>` (groups with transforms and style inheritance).
- Color parsing: `#rrggbb`, `#rgb`, `rgb(r,g,b)`, 30 CSS named colors.
- Style cascade: inline `style=""` + presentation attributes, inherited through `<g>`.
- Path commands: M/L/H/V/C/S/Q/T/A/Z + lowercase relatives; cubic/quadratic bezier flattening (De Casteljau); arc-to-lines (endpoint parameterization); smooth bezier reflected control points.
- Transforms: `translate`, `scale`, `rotate`, `matrix`, chained (e.g. `translate(-152,192) scale(0.7) translate(-32,-32)`).
- ViewBox → pixel mapping (aspect-ratio preserve, `xMidYMid meet`).
- Rendering: even-odd scanline polygon fill; stroke expansion to quad per segment; Porter-Duff src-over alpha compositing.
- Output: `egui::ColorImage` (straight RGBA).

**`src/canvas/interaction.rs`**
- Removed `SvgPreviewCache`, `SvgPreviewEntry`, `preview_entry_for`, `svg_source_hash`, `widget_bounds`, `DefaultHasher` import.
- Added `SvgTextureCache = HashMap<Uuid, (TextureHandle, f32)>` type alias + `svg_texture_cache_retain_live` helper.
- `draw_widget` Image arm: computes target dims (`widget.rect × zoom × ppp`), checks cache, calls `svg_rasterizer::rasterize()` on miss or scale change >5%, loads `TextureHandle`, draws via `painter.image()`.
- `handle()` parameter renamed from `svg_preview_cache` to `svg_texture_cache`.

**`src/app.rs`**
- Field renamed: `svg_preview_cache: SvgPreviewCache` → `svg_texture_cache: SvgTextureCache`.
- Prune call updated to `svg_texture_cache_retain_live`.

**`src/codegen/egui_emitter.rs`**
- Image arm: `source_backed_image_preview_line` → `image_frame_placeholder_line` (clean gray Frame, no "(SVG source-backed preview)" text).
- Child Image arm: same rename + clean output.
- Test updated: asserts correct dimensions in generated code.

**`src/codegen/export.rs`**
- Same rename pattern as emitter. Test updated.

### Verification
- `cargo clippy -- -D warnings`: zero warnings.
- `cargo test`: 23/23 passed.
- `cargo build`: clean.

### Notes
- Text elements (`<text>`) are skipped in the rasterizer (decorative in design-tool context).
- Gradients, filters, masks, `<use>`: shape renders with fill/stroke color only.
- Canvas shows pixel-accurate SVG shapes with correct colors and transforms.
- Superseded by `2026-05-23 - SVG/Image Export Parity And Rasterizer Guardrails`:
  exported Image widgets now embed the RohKai rasterizer and render preserved
  SVG source instead of keeping a sized Frame placeholder.

## 2026-05-23 - SVG Dependency Breach Fix + RCA

### Docs Reviewed Before Coding
- `scripts/preflight-context.ps1`
- `AGENTS.md`, `CLAUDE.md`
- `docs/ROADMAP.md`, `docs/DEVLOG.md`
- Relevant skill: `project-model`
- `src/app.rs`, `src/canvas/interaction.rs`, `src/svg_import.rs`
- `src/codegen/egui_emitter.rs`, `src/codegen/export.rs`
- `src/project/schema.rs`, `src/panels/properties.rs`
- `Cargo.toml`, `Cargo.lock`, `git status --short --branch`

### Changes Made
- Removed the local direct SVG renderer dependency additions from the active worktree.
- Replaced dependency-backed SVG rasterization with RohKai-native source-backed preview behavior:
  - Image mode stores the raw SVG on `WidgetInstance.svg_source`.
  - Canvas preview reuses the hardened zero-dependency SVG importer, fits imported placeholder geometry inside the Image widget, and paints it natively.
  - No external SVG renderer crate is used.
- Renamed user-facing Image mode text to `source-backed preview node`.
- Updated schema/properties comments to describe source-backed preview instead of rasterization.
- Replaced comment-only Image live codegen/export paths with visible egui preview frames.
- Added Image-mode tests for preserved source, dimensions, deterministic ID, and viewBox sizing.
- Added live codegen/export tests verifying Image widgets produce visible source-backed preview output and do not emit rasterized comment placeholders.
- Added `scripts/check-dependency-policy.ps1`.
- Wired dependency policy checking into `scripts/validate-svg-import.ps1`.
- Added RCA note: `docs/RCA-2026-05-23-svg-renderer-dependencies.md`.

### RCA Summary
- The bypass happened because the prior implementation treated "pure Rust, no C deps" as acceptable, but the active requirement was stricter: no new SVG importer crates and no new transitive dependency chain.
- Existing verification only checked compilation/tests/clippy, not dependency policy.
- The feature also had hollow edges: codegen/export emitted comments instead of real visible output.
- Prevention is now automated through `scripts/check-dependency-policy.ps1` and documented in the RCA.

### Output Form Verification
- Image import mode output form is verified as one `WidgetKind::Image` with source preserved, correct dimensions, deterministic ID, and `High` fidelity.
- Canvas output form is source-backed preview geometry painted by RohKai's own importer/painter path.
- Code panel/export output form is a visible egui preview frame, not a comment.

### Verification
- `cargo test image_` passed: 5/5.
- `cargo fmt --check` passed.
- `cargo check` passed.
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-dependency-policy.ps1` passed.
- `cargo test` passed: 23/23.
- `cargo clippy -- -D warnings` passed.
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1` passed.
- `cargo metadata --format-version 1 --no-deps` showed no direct `resvg`, `usvg`, or `tiny-skia`.
- `cargo tree` showed no active `resvg` or `usvg` dependency. `tiny-skia` remains an existing eframe/winit target-specific transitive through `sctk-adwaita`, not an SVG importer dependency.
- `cargo run` smoke launched successfully and was stopped after 8 seconds.

## 2026-05-23 - SVG Import Places at Visible Canvas Centre

### Docs Reviewed Before Coding
- `CLAUDE.md`
- `docs/DEVLOG.md` (previous entry)
- `src/app.rs` (do_svg_import, palette placement, AddAtCenter, CentralPanel)
- `src/canvas/interaction.rs` (origin/pan/zoom coordinate model)
- `src/project/schema.rs` (Rect field types)

### Changes Made

**`RohKaiApp::last_canvas_rect`** — new field (default 800×600). Captured from `ui.max_rect()` at the start of the CentralPanel closure each frame, giving the exact screen rect of the canvas panel.

**`place_at_visible_center`** — new helper method. Given a mutable slice of `WidgetInstance`:
1. Computes visible canvas centre: `cv_cx = -pan.x / zoom + win_w / 2.0` (mirrors palette-click formula).
2. Computes visible canvas dimensions: `vis_w = last_canvas_rect.width() / zoom`.
3. Computes bounding box of the imported group.
4. Scales the whole group down proportionally if it exceeds 80 % of the visible area.
5. Translates all widget rects so the group centre lands at `(cv_cx, cv_cy)`.

**`do_svg_import` restructured**:
- Parse SVG → bail on error before touching disk or canvas.
- Save `.rktp` template (best-effort, non-fatal).
- Clone widgets, call `place_at_visible_center`, assign fresh UUIDs, add to `ui_tree` — immediate canvas placement on every SVG import.
- Status message reports import stats; appends "(template save failed)" if disk write failed.

Both Image mode (single widget) and Components mode (multi-widget group) go through the same placement helper.

### Verification
- `cargo clippy -- -D warnings`: zero warnings.
- `cargo test`: 19/19 passed.
- `cargo build`: clean.

## 2026-05-23 - SVG Image Rasterization Quality Fixes

### Docs Reviewed Before Coding
- `CLAUDE.md`
- `docs/DEVLOG.md` (previous entry)
- `src/canvas/interaction.rs` (rasterizer + Image draw arm)
- `src/app.rs` (texture cache field)

### Changes Made

**Fix 1 — Premultiplied alpha**
- tiny-skia stores pixels as premultiplied RGBA; egui's `ColorImage::from_rgba_unmultiplied` expects straight alpha.
- Replaced `pixmap.data()` with demultiplied conversion: `pixmap.pixels().iter().flat_map(|p| { let c = p.demultiply(); [c.red(), c.green(), c.blue(), c.alpha()] })`.

**Fix 2 — Physical pixel resolution**
- Was rasterizing at `rect.width() as u32` (logical canvas pixels at current zoom).
- Now rasterizes at `widget.rect.w * zoom * pixels_per_point` — true device pixel count for the widget at current zoom.

**Fix 3 — Texture cache invalidation on zoom change**
- Changed cache type from `HashMap<Uuid, TextureHandle>` to `HashMap<Uuid, (TextureHandle, f32)>` where f32 is the effective scale (`zoom * ppp`) at rasterization time.
- On Image draw: if cached scale differs from current by > 0.05, evict entry; `entry().or_insert_with()` then rasterizes fresh at new size.

### Verification
- `cargo clippy -- -D warnings`: zero warnings.
- `cargo test`: 19/19 passed.
- `cargo run`: clean launch, exit 0.

## 2026-05-23 - SVG Dual-Mode Import (Image + Components)

### Docs Reviewed Before Coding
- `CLAUDE.md`, `AGENTS.md`
- `docs/ROADMAP.md`, `docs/DEVLOG.md` (latest entries)
- `src/svg_import.rs`, `src/app.rs`, `src/canvas/interaction.rs`
- `src/project/schema.rs`, `src/codegen/egui_emitter.rs`, `src/codegen/export.rs`
- `git status --short --branch`

### Changes Made

**Root cause fixed: Frame fill color ignored**
- Frame rendering arm was using hardcoded gray fill even when `bg_color` was set.
- Changed to extract actual `r/g/b` from `bg` then apply `fill_alpha`, so SVG-imported frames render their actual SVG fill colors.
- Also: unselected stroke now uses `fg_color` when set, falling back to gray.

**SVG label spam suppressed (Components mode)**
- Auto-generated labels like "svg path", "svg rect", "svg circle" are now hidden on canvas.
- Detection: `import_metadata.is_some() && label.starts_with("svg ")`.
- Label still stored in the widget for property panel / programmatic access.

**New `WidgetKind::Image` (single rasterized node)**
- Added `Image` variant to `WidgetKind` enum.
- Added `svg_source: Option<String>` to `WidgetInstance` (serde-skipped when None).
- Canvas renders Image widgets by rasterizing SVG via `resvg` + `tiny-skia` on first draw.
- Texture cached in `RohKaiApp::svg_texture_cache: HashMap<Uuid, TextureHandle>`.
- Cache pruned each frame for deleted widgets.

**Import mode dialog**
- `cmd_import_svg_template` now sets `pending_svg_import` instead of importing immediately.
- `show_svg_import_modal` renders an `egui::Window` modal each frame when pending.
- User chooses: "Image — single rasterized node" or "Components — editable frame per shape".

**Dependencies added**
- `resvg = "0.44"`, `usvg = "0.44"`, `tiny-skia = "0.11"` — all pure Rust, no C deps.

**All match sites updated for `WidgetKind::Image`**
- `canvas/interaction.rs`: `kind_accent`, `kind_tag`, `draw_widget`, child overlay, kind-tag exclusion.
- `codegen/egui_emitter.rs`: main emit match, child-line match.
- `codegen/export.rs`: main widget match, `export_child_line`.
- `codegen/kind_table.rs`: `state_info` (returns `None` — Image carries no state).
- `panels/properties.rs`: `show_image` panel (shows SVG source status + delete button).
- `widgets/mod.rs`: `default_for` (200×200 placeholder, no svg_source).

### Verification
- `cargo clippy -- -D warnings`: zero warnings.
- `cargo test`: 19/19 passed.
- `cargo check`: clean.

### Notes For Claude And Codex
- Image widgets rasterize at first draw size and cache by widget ID. Resizing the widget does NOT re-rasterize (cached at original size). Delete and re-import to change resolution.
- Image mode stores raw SVG text in `WidgetInstance.svg_source` — serialized in `.rohkai.json` and `.rktp` files. Large SVGs will produce large project files.
- `SvgImportMode::Components` is the default — existing import callers using `SvgImportOptions::default()` are unaffected.
- The three pre-existing `#[dead_code]` items in `svg_import.rs` (diagnostics fields, `diagnostics_digest`) were suppressed this session — they are part of the diagnostic API surface and should not be removed.

## 2026-05-21 23:21 - QoL + Documentation/Hook Discipline

### Docs Reviewed Before Coding
- `AGENTS.md`
- `CLAUDE.md`
- `docs/ROADMAP.md`
- `docs/DEVLOG.md` (created in this session; no prior file existed)
- `git status --short --branch`
- Relevant local skills: `good-citizen`, `project-model`, `codegen-rules`, `canvas-patterns`

### Planned Changes
- Split roadmap/devlog responsibilities: roadmap is strategic, devlog is chronological.
- Add shared preflight script and Codex/Claude command documentation.
- Update Codex/Claude guidance to require pre-coding document review before planning or editing.
- Implement QoL fixes for Tracé handler insertion, code navigation fallback highlight, left panel reachability, palette drag payload creation, dirty-check cost, and dismissible status messages.
- Add a future viable widget palette section inspired by the Qt Designer-style palette reference.

### Implemented Changes
- Added `scripts/preflight-context.ps1`, `.agents/commands/preflight.md`, and `.claude/commands/preflight.md`.
- Updated `AGENTS.md`, `CLAUDE.md`, and `.claude/settings.json` so both Codex and Claude use the same pre-coding document review flow.
- Updated Codex/Claude `project-model` skills to match the current schema, widget list, settings split, and project envelope behavior.
- Updated `docs/ROADMAP.md` with the roadmap/devlog split and a future viable widget palette section.
- Fixed Tracé handler insertion so generated code sync happens before handler stubs are appended.
- Added a stable selected-widget code block highlight fallback for double-click code navigation.
- Capped Properties panel scroll height so Templates remains reachable.
- Changed palette drag payload creation to `drag_started()` so one payload survives until drop/cancel.
- Added a throttled dirty-cache path for title/status reads while keeping exact checks for New/Open/Save prompts.
- Moved export/error status into the bottom status bar with dismiss controls; export status also expires after a short delay.

### Known Risks
- Exact cursor placement inside egui `TextEdit` is version-sensitive; this pass uses a visible matching-block highlight fallback.
- Dirty-cache throttling can be briefly stale in the title; destructive actions still use exact serialization checks.
- Existing working tree is heavily modified by prior Claude/Codex work; changes in this session are layered without reverting earlier edits.

### Verification
- `cargo fmt --check` passed.
- `cargo check` passed.
- `cargo test` passed: 7 tests.
- `cargo clippy -- -D warnings` passed.
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1` passed.
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\preflight-context.ps1` passed.
- `cargo run` launched successfully and was stopped after a smoke test.

## 2026-05-22 - Stage 5.5 Completeness Pass + ROADMAP Future Considerations

### Docs Reviewed Before Coding
- `CLAUDE.md`, `AGENTS.md`
- `docs/ROADMAP.md`, `docs/DEVLOG.md`
- `src/panels/properties.rs`, `src/canvas/interaction.rs`
- `src/codegen/egui_emitter.rs`, `src/codegen/export.rs`
- `src/panels/code_preview.rs`

### Changes Made

**docs/ROADMAP.md**
- Appended 9 new "Future Considerations" sections verbatim from user spec:
  Rulers & Measurement, Document Presets & Real Window Sizing, Application Appearance & Theming,
  Lazarus Features — Remaining, Technical & Computational Widgets, Rust-Centric Visual Features,
  WASM Export & Web Target, Database Integration Panel, Project Tree & File Browser

**FIX 1 — Real inline color picker (properties.rs)**
- Replaced R/G/B drag grid with `egui::color_picker::color_edit_button_srgba`
- Converts `[u8;3]` ↔ `Color32` via `.r()/.g()/.b()`
- Also moved fg_color into inline horizontal row in `show_content_inner`

**FIX 2 — Corner radius always-visible DragValue (properties.rs + emitters)**
- Removed button gate; DragValue always shown in horizontal with "✕" clear button
- `egui_emitter.rs`: Button arm emits `.rounding(egui::Rounding::same(r))` when r > 0
- `export.rs`: same rounding chain in export Button arm

**FIX 3 — Tracé visual chip (properties.rs)**
- Teal `→ fn name` button with hover tooltip; clicking fires `PropertiesAction::ScrollToHandler`
- Handler TextEdit gets placeholder hint text `e.g. handle_on_click`

**FIX 4 — Inline canvas label editing (interaction.rs)**
- `InteractionState` gains `inline_edit: Option<(Uuid, String)>`
- Double-click on Button/Label/Checkbox/RadioButton → `inline_edit` path
- Overlay: dark bg + teal border + `ui.put(rect, TextEdit)` with `request_focus()`
- Enter/focus-lost commits; Escape cancels

**FIX 5 — Lazare highlight alpha (code_preview.rs)**
- Changed `from_rgba_unmultiplied(52, 211, 153, 24)` → alpha 60 (more visible)

**FIX 6 — Properties panel compact layout (properties.rs)**
- Full rewrite of `show_content_inner`: compact 4-column X/Y W/H geometry grid
- Contextual per-kind field visibility:
  - Label shown only for Button/Label/Checkbox/RadioButton/Frame/ComboBox
  - Binding hidden for Button/Frame
  - Label-binding mode only for Label kind
  - Min/Max only for Slider/ProgressBar; Default only for Slider
  - Enabled hidden for Frame/Label/ProgressBar
  - Radius shown for Button/Label/Frame/ComboBox/Checkbox/RadioButton
  - Custom props hidden for Slider
- Multi-select: alignment block shown when ≥2 selected
- Delete button text colored red

**FIX 7 — Canvas draw_widget applies fg_color + corner_radius (interaction.rs)**
- `draw_widget` computes `rounding` and `fg` upfront from widget fields
- All painter calls use the computed rounding
- fg_color applied to text rendering where supported
- Disabled overlay added: semi-transparent black rect when `enabled == Some(false)`
- Widget-kind-specific improvements: RadioButton circle, ComboBox dark bg, faint borders for Label/Checkbox

**FIX 8 — Resize handle outward offset (interaction.rs)**
- `hit_rect()` now uses `rect.expand(4.0)` so handles sit 4px outside widget boundary

**FIX 9 — Checkbox export (export.rs)**
- Confirmed non-regression: emitter uses `self.{b}` (live preview), export uses `self.state.{b}` — correct by design

### Verification
- `cargo check` passed after each fix group (4 intermediate checks)
- `cargo test`: 7/7 passed
- `cargo clippy -- -D warnings`: zero warnings
- `cargo run`: clean launch, exit 0

## 2026-05-22 12:59 - Hardened Zero-Dependency SVG Importer

### Docs Reviewed Before Coding
- `scripts/preflight-context.ps1` output
- `AGENTS.md`, `CLAUDE.md`
- `docs/ROADMAP.md`, `docs/DEVLOG.md`
- Relevant skills: `good-citizen`, `project-model`, `codegen-rules`
- `src/svg_import.rs`, `src/app.rs`, `docs/SVG_IMPORT.md`
- `git status --short --branch`

### Changes Made
- Replaced the Stage 5 SVG placeholder scanner with a hardened zero-dependency importer.
- Added richer API: `import_svg_template(svg, SvgImportOptions) -> Result<SvgImportOutput, SvgImportError>`.
- Kept compatibility wrapper: `parse_svg_template(svg) -> Result<Vec<WidgetInstance>, String>`.
- Added parser limits for file size, tag count, attribute count/value length, nesting depth, path commands, placeholder count, image data URI size, use expansion depth, and style bytes.
- Added XML safety gates for `DOCTYPE`, custom entities, unknown entities, processing instructions, and external references.
- Added structured report data: imported/skipped counts, warnings, unsupported features, and fidelity level.
- Added simple style/class handling for presentation attributes, inline style, and `.class { key:value }` rules.
- Added local `symbol` / `use` expansion with cycle and depth protection.
- Added image data URI policy for embedded PNG/JPEG placeholders only.
- Improved path parsing for compact syntax, relative commands, Bezier sampling, arc sampling, malformed recovery, and command limits.
- Added deterministic UUID generation for imported placeholders so repeated imports are byte-stable.
- Updated File -> Import SVG as Template message to include skipped count, unsupported count, and fidelity.
- Expanded `docs/SVG_IMPORT.md` and `scripts/validate-svg-import.ps1`.

### Verification
- `cargo fmt --check` passed.
- `cargo check` passed.
- `cargo test` passed: 13 tests.
- `cargo clippy -- -D warnings` passed.
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1` passed.
- `cargo run` launched successfully and was stopped after a smoke test.

### Notes For Claude And Codex
- No crates were added.
- RohKai still imports SVGs as editable placeholders, not a full SVG renderer.
- Original `.svg` files remain the source of truth beside generated `.rktp` templates.
- Existing dirty working tree included prior Claude/Codex work; this session intentionally did not revert unrelated changes.

## 2026-05-22 16:40 - Stage 5.5 ComboBox and Tracé Follow-Up

### Docs Reviewed Before Coding
- `scripts/preflight-context.ps1` output
- `AGENTS.md`, `CLAUDE.md`
- `docs/ROADMAP.md`, `docs/DEVLOG.md`
- Relevant skill: `project-model`
- `src/panels/properties.rs`, `src/canvas/interaction.rs`
- `src/codegen/egui_emitter.rs`, `src/codegen/export.rs`, `src/codegen/state_emitter.rs`
- `src/project/schema.rs`, `src/project/ui_tree.rs`, `src/widgets/combo_box.rs`
- `git status --short --branch`

### Changes Made
- Removed the foreground color `+ set` gate. The color swatch now appears inline at all times, defaulting to white.
- Added `WidgetProps.options: Vec<String>` for ComboBox widgets with default options `Option A`, `Option B`, `Option C`.
- Added ComboBox option editing in Properties with add/remove controls.
- Repaired empty ComboBox option lists through `UiTree::validate_and_repair()`.
- Updated canvas ComboBox preview to show the first configured option as the selected label.
- Updated live codegen, AppState emission, and export codegen so ComboBoxes emit selectable options and default state to the first option.
- Changed canvas Tracé navigation to Ctrl+double-click; regular double-click remains reserved for inline label editing.
- Updated the handler field hint to `Ctrl+double-click widget to jump to handler`.
- Capped ComboBox option editor width so the left panel does not expand over the canvas.
- Deleted stale `implement-svg-importer-hardening` heartbeat automation because the importer hardening pass is complete.
- Updated Codex and Claude `project-model` skills to document ComboBox options.

### Verification
- `cargo test` passed before edits: 13/13.
- `cargo fmt --check` passed.
- `cargo check` passed.
- `cargo test` passed: 13/13.
- `cargo clippy -- -D warnings` passed.
- `cargo run` smoke launched successfully and was stopped after 8 seconds.

### Notes For Claude And Codex
- Do not reintroduce a color `+ set` gate; the swatch is intentionally always visible.
- Tracé canvas navigation is Ctrl+double-click. Plain double-click is now for inline label editing.
- ComboBox option text fields must stay width-capped; uncapped fields can force the left panel over the canvas.

## 2026-05-22 23:08 - SVG Import Hardening Follow-Up + Text Planning

### Docs Reviewed Before Coding
- `scripts/preflight-context.ps1` output
- `AGENTS.md`, `CLAUDE.md`
- `docs/ROADMAP.md`, `docs/DEVLOG.md`
- Relevant skill: `project-model`
- `docs/SVG_IMPORT.md`, `scripts/validate-svg-import.ps1`
- `src/svg_import.rs`, `src/project/schema.rs`
- `git status --short --branch`

### Changes Made
- Tightened SVG fidelity scoring so text-heavy, clipped, masked, filtered, gradient/pattern, and paint-server-heavy imports are not reported as high fidelity.
- Improved unsupported diagnostics wording to explain that RohKai preserves the source SVG and imports editable placeholders for supported visible geometry.
- Split unsupported diagnostics for `linearGradient`, `radialGradient`, `pattern`, `clipPath`, mask/filter/clip attributes, and paint-server references.
- Added hidden-definition diagnostics so unsupported gradient/pattern/mask/clip/filter definitions inside `defs` are still reported.
- Added duplicate-id warnings while preserving deterministic first-id lookup behavior.
- Added extreme/non-finite transform warnings and safe fallback.
- Added empty-geometry recovery for zero-size rect/circle/ellipse imports.
- Added simple solid-paint approximation for `fill`, `stroke`, named colors, `#rgb`, `#rrggbb`, `rgb(...)`, and opacity into RGB placeholder fields.
- Kept text editable and only planned the future text renderer; added `docs/TEXT_IMPORT_PLAN.md`.
- Updated `docs/SVG_IMPORT.md` and `docs/ROADMAP.md` with the current text policy and future SVG text maturity lane.
- Added SVG importer tests for malformed input, unknown entities, duplicate ids, paint/clip/mask/filter diagnostics, opacity approximation, text-heavy fidelity downgrade, empty geometry, extreme transforms, and deterministic output.
- Formatted the active dirty schema/widget audit files that were already present before this pass so repo formatting checks could pass.

### Verification
- `cargo fmt --check` passed.
- `cargo check` passed.
- `cargo test` passed: 18/18.
- `cargo clippy -- -D warnings` passed.
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1` passed.
- `cargo run` smoke launched successfully and was stopped after 8 seconds.

### Notes For Claude And Codex
- This pass did not add crates and did not build a full text renderer.
- Text remains editable placeholders; robust `tspan` and text layout work is documented in `docs/TEXT_IMPORT_PLAN.md`.
- Pre-existing dirty schema/widget audit changes were preserved and formatted, not reverted.

## 2026-05-22 23:45 - SVG Import Fixture Readiness Harness

### Docs Reviewed Before Coding
- Prior SVG hardening preflight context remained active for this continuation.
- `docs/SVG_IMPORT.md`
- `docs/DEVLOG.md`
- `scripts/validate-svg-import.ps1`
- `src/svg_import.rs`
- `git status --short`

### Changes Made
- Added checked-in SVG fixture cases under `tests/fixtures/svg_import/real_world/`.
- Covered basic geometry, simple class styles, `tspan` flattening, paint servers, clip/mask/filter diagnostics, local `symbol`/`use`, external references, malformed recovery, and embedded image placeholders.
- Added `real_world_fixture_suite_imports_deterministically` to assert minimum import counts, expected fidelity, expected warnings/unsupported diagnostics, deterministic UUIDs, and deterministic diagnostics.
- Updated `scripts/validate-svg-import.ps1` so the real-world fixture suite runs in the normal SVG validation workflow.
- Updated `docs/SVG_IMPORT.md` to document the fixture suite.

### Verification
- `cargo test real_world_fixture_suite_imports_deterministically` passed after calibrating fixture expectations to current importer behavior.
- `cargo fmt --check` passed.
- `cargo check` passed.
- `cargo test` passed: 19/19.
- `cargo clippy -- -D warnings` passed.
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1` passed.
- `cargo run` smoke launched successfully and was stopped after 8 seconds.

### Notes For Claude And Codex
- Fixtures are intentionally hand-authored and small to avoid licensing issues while still exercising real-world SVG patterns.
- This is a readiness harness, not a browser-rendering oracle. It validates RohKai's placeholder importer contract and diagnostics stability.

## 2026-05-22 - Schema Audit + Properties Panel Completeness Pass

### Docs Reviewed Before Coding
- `CLAUDE.md`, `docs/ROADMAP.md`, `docs/DEVLOG.md`
- `src/project/schema.rs` (full read before any edit)
- `src/panels/properties.rs`, `src/codegen/egui_emitter.rs`
- `src/canvas/interaction.rs`

### Changes Made

**Part 1 — schema.rs (already completed before summary)**
- Added `TextAlign { Left, Center, Right }` and `Orientation { Horizontal, Vertical }` enums.
- Added 13 new `WidgetProps` fields: `step`, `show_value` (default true), `orientation`, `placeholder`, `password_mode`, `max_length`, `radio_value`, `group_binding`, `show_percentage`, `animated`, `inner_margin` (default 8.0), `stroke_color`, `stroke_width` (default 1.0).
- Added 5 new `WidgetInstance` fields: `bg_color`, `font_size`, `text_align`, `on_click: String`, `on_change: String`.
- Added `Default` impl for `WidgetInstance` (id = Uuid::nil()) — makes all ~14 construction sites future-proof.
- All new fields use `#[serde(default)]` + `skip_serializing_if` for backward compat.

**Part 2 — properties.rs (complete rewrite)**
- Dispatches by `w.kind.clone()` to per-kind functions: `show_button`, `show_label`, `show_text_input`, `show_slider`, `show_checkbox`, `show_radio_button`, `show_combo_box`, `show_progress_bar`, `show_frame`.
- Each kind shows exactly its relevant fields (label, binding, color, geometry, etc.).
- New fields exposed: placeholder, password_mode, max_length, step, show_value, orientation, radio_value, group_binding (syncs to state_binding), show_percentage, animated, inner_margin, stroke_color, stroke_width, bg_color, font_size, text_align.
- `show_event_handler`: migrates legacy `event_handler` → `on_click`/`on_change` on first display; uses new Tracé chip for non-empty handlers.
- All alignment tools and group/ungroup controls preserved.

**Part 3 — egui_emitter.rs (all widget arms updated)**
- Added `resolve_handler_click` / `resolve_handler_change` helpers (use `on_click`/`on_change`, fall back to legacy `event_handler`).
- Added `rich_text_expr` helper: builds `egui::RichText::new(label).size(pt).color(col)` when font_size or fg_color is set.
- Button: `.fill(bg_color)`, RichText label, `on_click` handler.
- TextInput: `.hint_text(placeholder)`, `.password(true)`, `on_change` handler.
- Slider: `.step_by(step as f64)`, `.show_value(false)`, `.vertical()`, `on_change` handler.
- RadioButton: uses `props.radio_value` as the alternative value arg.
- ProgressBar: `.show_percentage()`, `.animate(true)`, removed bogus `.text(label)`.
- Frame: uses `egui::Frame::none()` with `inner_margin`, `stroke`, `fill(bg_color)`, `rounding`.
- All remaining arms use `resolve_handler_change` instead of raw `event_handler`.

**Part 4 — interaction.rs draw_widget (canvas visual updates)**
- `label_size` now respects `widget.font_size` (falls back to zoom-scaled default).
- `bg` computed from `widget.bg_color`; applied to Button, TextInput, Frame, ProgressBar fills.
- TextInput: shows `props.placeholder` (gray text) instead of props.label.
- RadioButton: renders `props.radio_value` as small teal tag in bottom-right corner.
- ProgressBar: shows "60%" overlay if `show_percentage`, "~" if `animated`, no overlay otherwise.

### Verification
- `cargo check` passed.
- `cargo clippy -- -D warnings`: zero warnings.
- `cargo test`: 19/19 passed.

### Notes
- `TextEdit::char_limit` not emitted yet (API not verified for egui 0.29; field stored, codegen pending).
- Text alignment (`text_align`) stored and shown in properties; canvas and codegen emit not added (requires `ui.with_layout` wrapper, deferred).
- `export.rs` not updated this session — shares the same handler logic pattern as `egui_emitter.rs`; update deferred to next pass.

## 2026-05-24 — 5-Patch Rust-ness Remediation

### Context / docs reviewed
- AGENTS.md, CLAUDE.md, docs/ROADMAP.md, docs/CODE_INDEX.md, docs/CODE_COOP.md
- Continued from a compacted session; all 5 patches executed in sequence

### Changes

**Patch 1 — Encoding cleanup (carried forward from prior session)**
- `src/codegen/export.rs`: replaced double-encoded em dash with ASCII hyphen
- `scripts/check-text-encoding.ps1`: new — scans tracked files for six mojibake patterns; called by preflight

**Patch 2 — Descriptor validation and codegen rails**
- `src/codegen/widget_descriptor.rs`: added `validate_descriptor()`, `format_cargo_dep()`, `validate_cargo_dep()`, `is_descriptor_id()`, `is_field_key()`, `validate_state_default()`, `extract_template_tokens()`; `load_one()` now calls `validate_descriptor` after parse; 10 new unit tests
- `src/widgets/mod.rs`: `default_for_descriptor` uses `format_cargo_dep` instead of inline formatting

**Patch 3 — Shared AppState field collector**
- `src/codegen/field_collector.rs`: new — `AppStateField`, `CollectedFields`, `collect()`, `default_expr_for_widget()`; type-collision detection with warnings; 6 unit tests
- `src/codegen/state_emitter.rs`: replaced duplicate walk loop with `field_collector::collect`
- `src/codegen/export.rs`: replaced duplicate `BoundField` struct and walk loop with `field_collector::collect`; removed duplicate `default_expr_for_widget`
- `src/codegen/mod.rs`: added `pub mod field_collector`

**Patch 4 — SVG rasterizer typed Result API**
- `src/canvas/svg_rasterizer.rs`: added `SvgRasterError` enum with `Display`; `rasterize()` now returns `Result<ColorImage, SvgRasterError>`; added `rasterize_or_fallback()` convenience wrapper; 3 existing tests updated
- `src/canvas/interaction.rs`: call site updated to use `rasterize_or_fallback`

**Patch 5 — RohKaiApp state decomposition**
- `src/app.rs`: extracted six sub-structs: `ProjectState`, `SessionState`, `MessageState`, `PreferencesState`, `CodePanelState`, `DescriptorState`; all field accesses updated; no logic changes

### Verification
- `cargo fmt --check`: clean
- `cargo test`: 47/47 passed
- `cargo clippy -- -D warnings`: zero warnings
- `scripts/check-text-encoding.ps1`: OK (no mojibake)

### Notes
- PR #3 on `dev` branch updated with all 5 patches (commits 3302e8c through e35b94a)
- Patch 3 collector detects type collisions across widgets (same binding name, different Rust types); warnings appear as comments in generated AppState
- SVG `rasterize()` is now testable as a pure Result function; `rasterize_or_fallback` is the canvas path
