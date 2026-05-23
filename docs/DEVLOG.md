# RohKai Devlog

Chronological session record. The roadmap stays strategic; this file records what happened, what was reviewed first, what changed, and what still needs attention.

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

## 2026-05-22 16:40 - Stage 5.5 ComboBox and TracÃ© Follow-Up

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
- Changed canvas TracÃ© navigation to Ctrl+double-click; regular double-click remains reserved for inline label editing.
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
- TracÃ© canvas navigation is Ctrl+double-click. Plain double-click is now for inline label editing.
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
