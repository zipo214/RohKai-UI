# RohKai Code Index

Lightweight orientation map for agents. Keep this human-sized; do not turn it
into generated metadata unless the repo becomes large enough that search stops
being comfortable.

## Source Of Truth

- `src/project/schema.rs` - serializable project types: `WidgetInstance`,
  `WidgetProps`, `WidgetKind`, `AppProps`, SVG import metadata.
- `src/project/ui_tree.rs` - mutation API for the widget tree. Prefer this over
  direct `Vec` edits.
- `src/project/io.rs` - save/load and versioned `.rohkai.json` envelope.

## App Shell

- `src/main.rs` - eframe startup and app icon.
- `src/app.rs` - `RohKaiApp`, panel wiring, file menu, preferences, dirty
  checks, import/export coordination. App state is split into focused structs:
  `ProjectState`, `SessionState`, `MessageState`, `PreferencesState`,
  `CodePanelState`, and `DescriptorState`.
- `src/settings.rs` - user-level settings under app data. These are not project
  state and must not dirty `.rohkai.json`.

## Canvas And Widgets

- `src/canvas/interaction.rs` - canvas input, drawing, selection, drag, resize,
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
- `src/canvas/preview.rs` - F5 preview mode: renders the canvas as live egui widgets.
- `src/canvas/overlays.rs` - Stage 11 read-only overlays: ownership (widget→field)
  and error-flow (handler Result/Option) badges. Toggled from the View menu.
- `src/widgets/` - palette/default constructors for each `WidgetKind`.

## Panels

- `src/panels/palette.rs` - widget palette interactions.
- `src/panels/properties.rs` - selected widget inspector.
- `src/panels/code_preview.rs` - live/editable generated code panel. Owns the
  no-wrap/wrap editor viewport, decoration gutter, source-span outlines,
  navigation scrolling, and generated/valid/invalid edit states.
- `src/panels/templates.rs` - template file interactions and SVG import path.
- `src/panels/descriptor_editor.rs` - full power-user `.rkwd` editor (all
  template/schema fields). Entry: File → New Widget Descriptor… or Widgets menu.
- `src/panels/widget_builder.rs` - Guided Descriptor Builder. Beginner-friendly
  form over `WidgetDescriptor` (name, type, label, click handler) with live
  descriptor preview. It is not the future Visual Widget Maker; it creates
  simple `.rkwd` descriptors and can hand off to the full editor. Entry:
  File → Create Custom Widget… or Widgets menu.
- `src/panels/outline.rs` - document outline / layers panel (Ctrl+L).
- `src/panels/project_tree.rs` - project file tree + read-only viewer + asset
  registry (File → Project Files…).
- `src/panels/component_tray.rs` - design-time non-visual component tray
  (timers, data sources, state machines, HTTP).
- `src/panels/shortcuts.rs` - keyboard shortcut reference (F1 / ?).
- `src/panels/rust_wiring.rs` - Stage 11 Rust Wiring editor: mpsc channels,
  iterator pipelines, trait impls, with live generated-code preview.
- `src/panels/macro_palette.rs` - Stage 11 macro snippet palette → code buffer.

## Feature Depth Status

- **Full enough for current export:** FilePicker includes generated `rfd`
  dependency wiring and path state.
- **Functional MVP:** MathLabel is a safe computed `f32` label; Chart is a
  minimal `Vec<f32>` bar painter; Table/ListView/TreeView are static
  option-backed widgets.
- **Direct-child layout slice:** VLayout/HLayout/GridLayout own and reflow direct
  children with first-slice margins/gaps/grid columns, layout-aware spacers,
  container stretch, grid child reorder controls, and one-level Lazare parser
  hierarchy round-trip. Rich alignment, per-child policies, slot editor, and
  multi-level layout semantics are still planned.
- **Design-time MVP / documented stubs:** Timer, StateMachine, and HttpRequest
  components expose state/config and generated comments, not full runtime
  schedulers, transition engines, or HTTP clients.
- **Planned depth:** formula engine, model-bound data views, chart axes/series,
  runtime component dispatch, and true visual widget construction.

## Codegen

- `src/codegen/egui_emitter.rs` - live egui preview code.
- `src/codegen/source_map.rs` - exact byte/line ranges for generated widget and
  future handler blocks; shared line-span utility used by Lazare parsing.
- `src/codegen/export.rs` - generated standalone eframe project output.
- `src/codegen/field_collector.rs` - shared AppState field collection for live
  preview, export, and descriptor state fields.
- `src/codegen/state_emitter.rs` - generated `AppState`.
- `src/codegen/parser.rs` - Lazare/bidirectional code parsing, diagnostics, and
  source ranges for valid manually edited widget blocks.
- `src/codegen/kind_table.rs` - field types and widget metadata for codegen.
- `src/codegen/rust.rs` - Rust string/binding helpers.
- `src/codegen/rust_wiring.rs` - Stage 11 Rust-centric emitters: mpsc channel
  fields, iterator-pipeline methods, trait-impl blocks, async/Result-aware
  handler signatures + call sites (std-only, no tokio).
- `src/codegen/widget_descriptor.rs` - `.rkwd` descriptor types, loader,
  template engine. Drop `.rkwd` files in `<binary_dir>/widgets/` to extend
  the palette without recompiling.

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
- `scripts/check-text-encoding.ps1` - blocks mojibake/replacement-character text
  from entering tracked repo files.
- `scripts/validate-svg-import.ps1` - SVG importer validation suite.
- `scripts/snapshot-context.ps1` / `scripts/restore-context.ps1` - Claude
  context snapshot helpers.
- `scripts/sync-and-run.ps1` - dangerous local sync helper, now gated behind
  `-AllowOverwrite`.
