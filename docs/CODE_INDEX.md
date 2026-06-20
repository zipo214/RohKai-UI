# RohKai Code Index

Lightweight orientation map for agents. Keep this human-sized; do not turn it
into generated metadata unless the repo becomes large enough that search stops
being comfortable.

## Source Of Truth

- `src/project/schema.rs` - serializable project types: `WidgetInstance`,
  `WidgetProps`, `WidgetKind`, typed widget/surface behavior triggers, semantic
  dialog button roles, `AppProps`, SVG import metadata.
- `src/project/document.rs` - schema-v2 project root: `ProjectDocument`,
  `ProjectProps`, `UiSurface`, `SurfaceKind`, modal policy, diagnostics,
  duplication/remapping, and the `ActiveDocument` current-surface adapter.
- `src/project/ui_tree.rs` - mutation API for the widget tree. Prefer this over
  direct `Vec` edits.
- `src/project/io.rs` - save/load and versioned `.rohkai.json` envelope,
  including bare-tree/schema-v1 migration to one main surface.
- `src/project/constraint_solver.rs` - P2.3 layout constraints: `apply_constraints`
  (equal-size/aspect/min-max + margin folded into absolute alignment) — idempotent
  (safe every frame) and parent-relative (frame = parent's solved rect). Canvas
  constraint handles write the same model. Plus `validate_constraints`
  (cycle/self-ref/unknown-target/bad-ratio), surfaced in Properties.
- `src/project/db_engine.rs` - P2.6 `DatabaseEngine` trait + `SqliteEngine`
  (rusqlite, `params![]` only — never `format!()` SQL).
- `src/project/undo.rs` - Stage 14 snapshot undo/redo (50-step cap).

## App Shell

- `src/lib.rs` - **crate root**. RohKai is a lib + bin: every designer module is
  declared `pub mod` here so integration tests in `tests/` (e.g. the
  cross-surface `fidelity_audit.rs` parity harness) can import the real public
  API. `crate::` paths inside modules resolve here.
- `src/main.rs` - thin binary shell: eframe startup and the window-icon
  rasteriser. Constructs `rohkai::app::RohKaiApp`; declares no modules itself.
- `src/app.rs` - `RohKaiApp`, panel wiring, file menu, preferences, dirty
  checks, import/export coordination. App state is split into focused structs:
  `ProjectState`, `SessionState`, `MessageState`, `PreferencesState`,
  `CodePanelState`, and `DescriptorState`.
- `src/settings.rs` - user-level settings under app data. These are not project
  state and must not dirty `.rohkai.json`.

## Canvas And Widgets

- `src/canvas/interaction.rs` - canvas input, recursive widget drawing,
  selection, drag, resize, parent-anchor constraint handles, Grid drag-to-slot,
  pan/zoom, guides, image preview cache.
- `src/canvas/svg_rasterizer.rs` - current zero-new-dependency SVG rasterizer;
  stable source-spanned node IDs, bounded local reference metadata,
  `SvgSceneItem` flattening, and an owned display list that lowers geometry,
  style, transform, diagnostics, provenance, and per-viewport length bases
  before raster drawing. Root/nested viewports implement full
  `preserveAspectRatio` mapping; compound paths implement inherited
  nonzero/evenodd fill rules. Used by canvas Image preview and generated
  exports. It is substantial and real, but still a supported subset, not full
  `resvg` / `usvg` / `tiny-skia` equivalence.
- `src/svg_core.rs` - shared zero-dependency SVG microsyntax helpers used by the
  importer and rasterizer; currently owns color, numeric-list, affine
  transform, path token, length/unit, and preserveAspectRatio/viewBox mapping.
- `src/canvas/widget_instance.rs` - canvas rect conversion helpers.
- `src/canvas/preview.rs` - F5 preview mode: shared project state, typed behavior
  dispatch, transactional modal drafts, bounded nested modal stack, semantic
  Accept/Reject/Apply/Reset, and focus entry/restoration.
- `src/canvas/overlays.rs` - Stage 11 read-only overlays: ownership (widget→field)
  and error-flow (handler Result/Option) badges. Toggled from the View menu.
- `src/widgets/` - palette/default constructors for each `WidgetKind`.

## Panels

- `src/panels/palette.rs` - widget palette interactions.
- `src/panels/properties.rs` - selected widget inspector.
- `src/panels/surfaces.rs` - project surface tabs/panel, modal CRUD/templates,
  lifecycle behavior entry, active-surface properties, and diagnostics.
- `src/panels/code_preview.rs` - live/editable generated code panel. Owns the
  no-wrap/wrap editor viewport, decoration gutter, source-span outlines,
  navigation scrolling, and generated/valid/invalid edit states.
- `src/panels/templates.rs` - template file interactions and SVG import path.
- `src/panels/descriptor_editor.rs` - full power-user `.rkwd` editor (all
  template/schema fields). Entry: Widgets → Advanced Descriptor Editor….
- `src/panels/widget_builder.rs` - Guided Descriptor Builder. Beginner-friendly
  form over `WidgetDescriptor` (name, type, label, click handler) with live
  descriptor preview. It creates simple `.rkwd` descriptors and can hand off to
  the full editor. Entry: Widgets → Guided Descriptor Builder….
- `src/panels/widget_maker_panel.rs` - true Visual Widget Maker: primitive
  composition canvas, properties/code tabs, state variants, slots/groups, and
  `.rkwd` output. Entry: Widgets → Create New Widget….
- `src/panels/window_bounds.rs` - shared viewport-safe sizing for the three
  widget-authoring windows. Clamps default/min/max sizes to the live viewport.
- `src/panels/outline.rs` - document outline / layers panel (Ctrl+L).
- `src/panels/project_tree.rs` - project file tree + read-only viewer + asset
  registry (File → Project Files…).
- `src/panels/component_tray.rs` - design-time non-visual component tray
  (timers, data sources, state machines, HTTP).
- `src/panels/db_panel.rs` - P2.6 Database floating window (connection config +
  per-widget `DbBinding` editor via `show_db_binding`).
- `src/panels/shortcuts.rs` - keyboard shortcut reference (F1 / ?).
- `src/panels/rust_wiring.rs` - Stage 11 Global Rust Wiring editor (advanced
  app-wide infrastructure): mpsc channels, iterator pipelines, trait impls, with
  live generated-code preview. Title via `PANEL_TITLE`.
- `src/panels/macro_palette.rs` - Stage 11 macro snippet palette → code buffer.

## Feature Depth Status

- **Full enough for current export:** FilePicker includes generated `rfd`
  dependency wiring and path state.
- **Functional MVP:** MathLabel is a safe computed `f32` label; Chart is a
  minimal `Vec<f32>` bar painter; Table/ListView/TreeView are static
  option-backed widgets.
- **Recursive layout slice:** VLayout/HLayout/GridLayout own and reflow nested
  children in parent-depth order, expose alignment/flex/size/grid policies,
  support visual parent anchors and named Grid slots with drag-to-slot, and
  preserve hierarchy across canvas, live code, export, and Lazare.
- **Component runtime status:** Timer has a designer-side repaint/scheduling
  slice but generated handler dispatch remains a documented hook. StateMachine
  has schema/table/current-state codegen but no generated transition runtime.
  HttpRequest remains a documented stub pending an approved HTTP crate.
- **Formula — real engine (P2.5):** `codegen/formula.rs` recursive-descent infix
  parser with semantic function/arity validation, dependency collection, and
  context-aware live/export Rust paths; live diagnostics appear in Properties.
- **Database — SQLite slice (P2.6):** `DatabaseEngine`/`SqliteEngine`, `DbBinding`
  on widgets, `DbPanelState` window, `state_emitter` `load_from_db()` codegen.
  Multi-backend, query builder, and schema viewer are ordered backlog, not
  deferred.
- **Shaper — P2-A scaffolding:** `src/canvas/shaper/` has the `ShaperEngine` trait
  with `RustyBuzzShaper` (main-app canvas) and `HersheyShaper` fallback. The
  export-embedded `svg_rasterizer.rs` stays std-only and does NOT use rustybuzz.
- **Planned depth (ordered, not deferred):** model-bound data views, chart
  axes/series, runtime HTTP dispatch, constraint-solver recursion into layout
  children + validation UI, and true visual widget construction. See
  `docs/ROADMAP_PHASE2.md` for the ordered backlog.

## Codegen

- `src/codegen/egui_emitter.rs` - live egui preview code.
- `src/codegen/source_map.rs` - exact byte/line ranges for generated widget and
  future handler blocks; shared line-span utility used by Lazare parsing.
- `src/codegen/export.rs` - generated standalone eframe project output.
  Normal tests compile both a focused fixture and the complete built-in catalog
  as separate warning-denied Cargo projects. Multi-surface exports add
  `src/surfaces/*.rs`, aggregate state/handlers/dependencies, and generated
  `egui::Modal` runtime/draft structs for native and WASM source.
- `src/codegen/field_collector.rs` - shared AppState field collection for live
  preview, export, and descriptor state fields.
- `src/codegen/state_emitter.rs` - generated `AppState`.
- `src/codegen/parser.rs` - Lazare/bidirectional code parsing, diagnostics, and
  source ranges for valid manually edited widget blocks.
- `src/codegen/kind_table.rs` - field types and widget metadata for codegen.
- `src/codegen/behavior.rs` - behavior-graph emitter: typed `VisualAction`s →
  state-mutation statements (single helper for live + export, field-prefix only).
- `src/codegen/behavior_recipes.rs` - interaction matrix / smart constructor:
  `(source event, sink type)` → suggested typed actions (not the source of truth).
- `src/panels/behaviors.rs` - Behaviors panel (Properties tab): recipe
  suggestions + per-wire event/action/param editing.
- `src/codegen/rust.rs` - Rust string/binding helpers.
- `src/codegen/rust_wiring.rs` - Stage 11 Rust-centric emitters: mpsc channel
  fields, iterator-pipeline methods, trait-impl blocks, async/Result-aware
  handler signatures + call sites (std-only, no tokio).
- `src/codegen/widget_descriptor.rs` - `.rkwd` descriptor types, loader,
  template engine. Drop `.rkwd` files in `<binary_dir>/widgets/` to extend
  the palette without recompiling.
- `src/codegen/formula.rs` - P2.5 recursive-descent infix formula parser;
  `deps()` / `validate()` plus the Rust emitter for `MathLabel`.
- `src/codegen/component_state.rs` - design-time component (Timer/StateMachine/
  HttpRequest/DataSource) AppState fields and update() hooks.
- `src/codegen/widget_bundle.rs` - `.rkwb` ZIP bundle (store-only, std::io,
  hand-coded CRC-32) packing `.rkwd` entries + `manifest.json`.
- `src/codegen/widget_maker_emit.rs` - Visual Widget Maker doc → `.rkwd`
  descriptor emission.

## SVG Import

- `src/svg_import.rs` - SVG-to-editable-template importer and diagnostics.
- `docs/SVG_IMPORT.md` - current supported subset, security policy, limits.
- `docs/SVG_RENDERER_ROADMAP.md` - single detailed authority for future SVG
  import, rasterization, text, diagnostics, conformance, and editor UX work.
- `docs/TEXT_IMPORT_PLAN.md` - planned text/tspan architecture.
- `.agents/skills/svg-zero-dep/SKILL.md` and
  `.claude/skills/svg-zero-dep/SKILL.md` - mandatory guidance for SVG work.

## Project Guidance

- `AGENTS.md` - Codex-facing repo rules.
- `CLAUDE.md` - Claude-facing repo rules.
- `docs/ROADMAP.md` - strategic stage plan.
- `docs/CODE_COOP.md` - short newest-first agent handoff diary; this is the
  default context-sharing doc.
- `docs/PROMPT_CONTRACT.md` - reusable Codex/Claude goal skeleton for tasks that
  must derive source-of-truth sets, enumerate every output path, and add
  invariant tests before claiming parity.
- `docs/DEVLOG.md` - chronological session record; read for history,
  regression investigation, or when preflight is run with `-IncludeDevlog`.
- `docs/ARCHITECTURE.md` - structural truth.
- `docs/VISUAL_WIDGET_MAKER.md` - future WYSIWYG widget construction studio
  plan. Distinguishes the true visual maker from the existing guided descriptor
  builder.
- `docs/feature-evaluation/` - exhaustive feature-depth evaluations by product
  area. Use this when deciding whether a feature is Full, MVP, Stub, or Planned,
  and what top-class behavior should mean. Includes a dedicated Stage 11
  Rust-centric feature evaluation and a remaining-roadmap gap-closure audit.
- Historical bug review/RCA docs are reference material, not normal preflight.

## Scripts

- `scripts/preflight-context.ps1` - agent preflight summary.
- `scripts/check-dependency-policy.ps1` - blocks forbidden SVG dependency crates.
- `scripts/check-surface-parity.ps1` - caveman-review cross-surface drift auditor:
  flags schema fields with no codegen, roadmap `[x]`/`[ ]` claims that disagree
  with code, and `#[allow(dead_code)]` public APIs. It also materializes native
  and WASM multi-surface export fixtures and runs warning-denied Cargo checks.
  Advisory static findings remain non-fatal unless `-Strict` is used. See
  `docs/RCA-2026-06-12-surface-parity-drift.md`.
- `scripts/check-text-encoding.ps1` - blocks mojibake/replacement-character text
  from entering tracked repo files.
- `scripts/validate-svg-import.ps1` - SVG importer validation suite.
- `scripts/snapshot-context.ps1` / `scripts/restore-context.ps1` - Claude
  context snapshot helpers.
- `scripts/sync-and-run.ps1` - dangerous local sync helper, now gated behind
  `-AllowOverwrite`.
