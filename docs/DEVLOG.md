# RohKai Devlog

Chronological session record. The roadmap stays strategic; this file records what happened, what was reviewed first, what changed, and what still needs attention.

## 2026-06-05 — Code Highlight Outline And Launcher Trace

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
