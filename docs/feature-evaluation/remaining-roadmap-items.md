# Remaining Roadmap Items Evaluation

## Scope

This evaluates roadmap items that remain unchecked or materially incomplete.
It uses the anti-misread format from `depth-model.md`: the existence of related
MVPs does not close these gaps.

## Summary Table

| Roadmap Area | Current Depth | Current Implementation Contract | Desired Closure Contract |
|---|---:|---|---|
| `.rkwb` descriptor bundle | 0 | `.rkwd` descriptors load individually; no bundle format. | Bundle multiple descriptors, previews, assets, metadata, validation, and import/install flow. |
| Visual Widget Maker | 0-1 | Guided Descriptor Builder exists, but it is a form over descriptors. | True mini-canvas for composing reusable widgets from primitives and emitting deterministic descriptors. |
| SVG inline expansion toggle | 1 | `expand_svg_inline` schema/code behavior exists; UI toggle still roadmap-open. | Per-Image properties toggle with clear code-panel size warning and tests. |
| SVG text/import maturity | 1-3 | Hardened importer/rasterizer subset, source preservation, diagnostics. | Robust `tspan`, grouped editable text, import report UI, calibrated fidelity, optional outline snapshot. |
| SVG scene/display-list and golden harness duplicate items | 3 | Stage 9 says these are done; Stage 7.x still has stale unchecked duplicates. | Reconcile roadmap duplication, or split "initial done" from "renderer v2 complete" with precise closure. |
| Parallelism foundation | 1 | `rayon` dependency exists; no feature-specific parallel workloads. | Parallel SVG/codegen/export/template loading with measured speedups and safe invalidation. |
| Form Layout | 0-1 | GridLayout is the current substitute; egui has no distinct form primitive. | Decide whether to implement a form abstraction or formally retire as "covered by GridLayout." |
| Formula Widget | 0-1 | MathLabel MVP exists. | Expression parser, dependencies, validation, formatting, diagnostics, and multi-input support. |
| WASM export | 0 | No WASM panel/profile/export. | Platform profile, build config, generated web project, browser preview, unsupported-widget diagnostics. |
| Database/data integration | 0 | Static data views and design-time data source MVP exist. | DB connector config, schema viewer, query builder, data binding, generated crate-specific code. |
| Own renderer | 0-1 | egui remains rendering stack; SVG rasterizer is internal but not app renderer. | RohKai-owned pure Rust UI renderer and widget model if project commits to replacing egui. |
| High-risk widgets | 0 | Model views/Dock/MDI/multi-window/QAxWidget-style items are planned only. | Feasibility decisions, architecture constraints, and platform policy before implementation. |

## Descriptor Bundle: `.rkwb`

### Implementation Status

Status: Planned

Current Implementation Contract:

- RohKai loads standalone `.rkwd` descriptor files.
- Descriptors can define metadata, properties, state fields, templates, events,
  and cargo dependencies.
- No multi-file package/install/update format exists.

Insufficient Existing Surface:

- A folder of `.rkwd` files is not a bundle format.
- Hot reload/importing a descriptor does not solve assets, previews, dependency
  policy, compatibility, or versioning.

Desired Closure Contract:

- Define `.rkwb` package manifest: descriptors, preview assets, screenshots,
  examples, versions, dependency list, author/license, and compatibility.
- Add importer/installer UI with validation and conflict handling.
- Add safe extraction/install path under the widgets directory.
- Add tests for malformed bundles, duplicate IDs, missing assets, and dependency
  policy violations.

Closure Criteria:

- A bundle containing two widgets and one preview asset imports successfully.
- Invalid bundle dependency or duplicate descriptor ID is rejected with a clear
  diagnostic.
- Imported widgets appear in the palette without restart or project corruption.

## Visual Widget Maker

### Implementation Status

Status: Planned

Current Implementation Contract:

- `src/panels/widget_builder.rs` is a Guided Descriptor Builder.
- It edits a `WidgetDescriptor` through beginner-friendly fields.
- It is not a WYSIWYG widget-construction canvas.

Insufficient Existing Surface:

- The Guided Descriptor Builder does not satisfy Visual Widget Maker work.
- It cannot compose primitives, expose primitive values as descriptor props, edit
  internal z-order, or produce a visual-document-backed descriptor.

Desired Closure Contract:

- Add `WidgetMakerDocument`: primitives, z-order, selection, exposed props,
  style tokens, event zones, and preview state.
- Add separate Visual Widget Maker window with mini-canvas and inspector.
- Implement primitive vertical slice: rect, text, button-like hit region,
  move/resize/select/z-order.
- Generate deterministic `WidgetDescriptor` output from the maker document.
- Save descriptor to `widgets/`, reload palette, and preserve Advanced
  Descriptor escape hatch.

Closure Criteria:

- User visually composes a button-like widget from at least rect + text.
- The generated descriptor exposes `label` and exports valid egui code.
- Reloaded custom widget appears in palette and can be placed on the main canvas.
- Tests prove descriptor output is deterministic for the same maker document.

## SVG Inline Expansion Toggle

### Implementation Status

Status: Surface / Partially Wired

Current Implementation Contract:

- `WidgetInstance.expand_svg_inline` exists.
- Codegen has source contraction/expansion logic for SVG Image widgets.
- User-facing per-widget toggle remains unchecked in the roadmap.

Insufficient Existing Surface:

- A schema field does not close the feature if users cannot discover/control it.

Desired Closure Contract:

- Add Image widget property toggle: "Expand SVG inline in code panel".
- Warn that large SVGs can make the code panel noisy.
- Preserve source viewer as the recommended inspection path.

Closure Criteria:

- Toggling the property changes code-panel SVG representation immediately.
- Save/load preserves the toggle.
- Test proves Image codegen switches between compact and inline output.

## SVG Text And Import Maturity

### Implementation Status

Status: Functional MVP to Usable Subset, depending on subfeature

Current Implementation Contract:

- Importer handles common SVG shapes/text/path placeholders, source order,
  metadata, deterministic IDs, and many diagnostics.
- Rasterizer handles a supported subset with reports and golden tests.
- Simple text can become editable labels; robust multi-span text is not done.

Insufficient Existing Surface:

- Existing simple text flattening does not close robust `tspan` support.
- Source preservation does not replace a real report UI.
- Renderer diagnostics do not equal full clipping/masking/filtering/text support.

Desired Closure Contract:

- Robust `tspan` parser with text chunks, span provenance, per-span style, and
  diagnostics.
- Multi-label grouped import for positioned spans.
- Import report UI showing fidelity score, skipped features, approximations,
  and preserved original source.
- Optional vector-outline snapshot mode for visual comparison.
- Calibrated fidelity scoring for text-heavy, clipped, masked, filtered,
  gradient, and pattern-heavy SVGs.

Closure Criteria:

- `tspan` fixtures with nested positions/styles produce deterministic grouped
  editable labels and warnings.
- Import modal/report shows skipped features before template save.
- Fidelity score demonstrably downgrades for complex text/paint/server SVGs.

## SVG Renderer Roadmap Duplication

### Implementation Status

Status: Documentation Reconciliation Needed

Current Implementation Contract:

- Roadmap Stage 9 marks SVG scene/display-list IR split and golden fixture
  harness complete.
- Stage 7.x SVG Import Maturity still has unchecked items with similar names.

Insufficient Existing Surface:

- Duplicate roadmap items can mislead agents into either redoing completed work
  or marking future renderer depth as complete.

Desired Closure Contract:

- Reconcile the roadmap into two explicit levels:
  - Initial IR/golden harness complete.
  - Renderer v2 display-list/golden depth still planned, if more is desired.

Closure Criteria:

- Roadmap has no duplicate ambiguous SVG renderer checklist items.
- Feature evaluation points to the correct next SVG renderer tasks.

## Parallelism Foundation

### Implementation Status

Status: Surface

Current Implementation Contract:

- `rayon = "1"` is approved and present.
- Parallel SVG rasterization, codegen, export, template loading, and benchmarks
  are not implemented.

Insufficient Existing Surface:

- Adding rayon does not provide parallel workloads or prove speedups.

Desired Closure Contract:

- Identify independent workloads and ownership boundaries.
- Add parallel execution only where deterministic output and UI-thread safety
  are preserved.
- Add performance benchmarks for 50+, 100+, and 500+ widget projects.

Closure Criteria:

- Benchmarks show speedup for at least one real workload.
- Parallel output remains byte-stable/deterministic.
- No UI-thread mutation occurs off-thread.

## Form Layout

### Implementation Status

Status: Planned / Deferred

Current Implementation Contract:

- `GridLayout` exists and can approximate label-field form rows.
- No distinct `FormLayout` widget kind or properties exist.

Insufficient Existing Surface:

- GridLayout only closes Form Layout if the product explicitly defines forms as
  a grid preset and updates docs/UX accordingly.

Desired Closure Contract:

- Either implement a `FormLayout` abstraction with label/control pairs, row
  spacing, alignment, and export behavior, or formally close it as a GridLayout
  preset with a template.

Closure Criteria:

- User can create label-field pairs without manually constructing every grid
  detail, or roadmap says this is intentionally not a separate widget.

## Formula Widget

### Implementation Status

Status: Planned

Current Implementation Contract:

- `MathLabel` formats one bound `f32` value.
- No expression parser, dependency graph, validation, or formula editor exists.

Insufficient Existing Surface:

- MathLabel does not satisfy Formula Widget.

Desired Closure Contract:

- Formula expression model with references to AppState fields.
- Parser/evaluator for a safe supported expression subset.
- Properties UI for expression, formatting, units, fallback value, and errors.
- Dependency tracking so changes in inputs update output.

Closure Criteria:

- Formula can reference at least two numeric fields and compute a value.
- Invalid formula shows diagnostics and does not generate broken Rust.
- Exported app compiles and computes the same value.

## WASM Export

### Implementation Status

Status: Planned

Current Implementation Contract:

- Desktop eframe export exists.
- No WASM export profile, panel, build command, or generated web scaffold exists.

Insufficient Existing Surface:

- Desktop export does not close WASM export.
- FilePicker/native-dialog behavior must be platform-gated before web export.

Desired Closure Contract:

- Platform target model: Desktop vs Web.
- WASM export panel with output path, bundler choice, index.html toggle, and
  preview command.
- Generated project compatible with `wasm32-unknown-unknown`.
- Diagnostics for unsupported native-only widgets/features.

Closure Criteria:

- Minimal project exports to WASM and builds with configured toolchain.
- Native-only widgets produce clear warnings or web fallbacks.
- Preview in browser launches the generated web app.

## Database And Data Integration

### Implementation Status

Status: Planned

Current Implementation Contract:

- Static Table/ListView/TreeView MVPs exist.
- DataSource component is design-time/state/comment MVP.
- No DB connection, schema viewer, query builder, or generated DB code exists.

Insufficient Existing Surface:

- Static option-backed data views do not satisfy model-bound data integration.
- DataSource name/state field does not satisfy database integration.

Desired Closure Contract:

- User-approved DB crate decision at stage start.
- Connection configuration and secure persistence policy.
- Schema viewer and query builder.
- Data binding from query result fields to widgets.
- Generated async/sync DB code matching selected crate.

Closure Criteria:

- User configures a SQLite/Postgres/MySQL connection and sees schema.
- Query result binds to a Table/ListView.
- Exported app compiles with the approved DB crate and query path.

## Own Renderer

### Implementation Status

Status: Planned / Strategic

Current Implementation Contract:

- RohKai uses egui/eframe for app UI and generated apps.
- SVG rasterizer is an internal software renderer for SVG Image support only.
- No RohKai-owned general widget renderer exists.

Insufficient Existing Surface:

- Owning an SVG rasterizer does not mean RohKai owns the app renderer.
- Canvas painter approximations do not replace egui's runtime widget system.

Desired Closure Contract:

- Formal renderer architecture: layout, input, accessibility, text, painting,
  widget model, theme, invalidation, and platform windows.
- Migration plan for existing egui codegen/export.
- Compatibility strategy for descriptors and visual properties.
- Performance and accessibility test harness.

Closure Criteria:

- A non-egui prototype renders and interacts with a subset of RohKai widgets.
- Descriptor/widget model drives both design canvas and runtime output.
- Pure Rust/dependency policy is documented and enforced.

## Later / High-Risk Widgets

### Implementation Status

Status: Planned / Feasibility Required

Current Implementation Contract:

- Model-based item views, Dock Widget, MDI Area, Multi-window, and QAxWidget-style
  integrations are roadmap placeholders.

Insufficient Existing Surface:

- Existing static Table/List/Tree, project tree, or desktop window do not close
  these high-risk items.

Desired Closure Contract:

- Feasibility review for each item against RohKai architecture and pure-Rust/no
  C FFI rules.
- For QAxWidget-style integrations, likely reject or redefine because ActiveX
  conflicts with project constraints.
- For Dock/MDI/multi-window, define platform/windowing model before UI.

Closure Criteria:

- Each high-risk item has an explicit decision: implement, redefine, or reject.
- Implemented items have schema, UI, codegen/export, persistence, tests, and
  platform policy.

## Recommended Ordering

1. Reconcile duplicate SVG renderer roadmap entries.
2. Close the small SVG inline expansion toggle if still desired.
3. Build Formula Widget or Visual Widget Maker next, depending on product focus.
4. Add generated-project compile fixtures before WASM/DB.
5. Decide whether Stage 15 Own Renderer is strategic research or active product
   work before starting code.

