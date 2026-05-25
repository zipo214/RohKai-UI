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
- `src/app.rs` - `RohKaiApp`, panel wiring, app state, file menu, preferences,
  dirty checks, import/export coordination.
- `src/settings.rs` - user-level settings under app data. These are not project
  state and must not dirty `.rohkai.json`.

## Canvas And Widgets

- `src/canvas/interaction.rs` - canvas input, drawing, selection, drag, resize,
  pan/zoom, guides, image preview cache.
- `src/canvas/svg_rasterizer.rs` - current zero-new-dependency SVG rasterizer
  used by canvas Image preview and generated exports. It is substantial and
  real, but still a supported subset, not full `resvg` / `usvg` / `tiny-skia`
  equivalence.
- `src/canvas/widget_instance.rs` - canvas rect conversion helpers.
- `src/widgets/` - palette/default constructors for each `WidgetKind`.

## Panels

- `src/panels/palette.rs` - widget palette interactions.
- `src/panels/properties.rs` - selected widget inspector.
- `src/panels/code_preview.rs` - live/editable generated code panel.
- `src/panels/templates.rs` - template file interactions and SVG import path.

## Codegen

- `src/codegen/egui_emitter.rs` - live egui preview code.
- `src/codegen/export.rs` - generated standalone eframe project output.
- `src/codegen/state_emitter.rs` - generated `AppState`.
- `src/codegen/parser.rs` - Lazare/bidirectional code parsing.
- `src/codegen/kind_table.rs` - field types and widget metadata for codegen.
- `src/codegen/rust.rs` - Rust string/binding helpers.
- `src/codegen/widget_descriptor.rs` - `.rkwd` descriptor types, loader,
  template engine. Drop `.rkwd` files in `<binary_dir>/widgets/` to extend
  the palette without recompiling.

## SVG Import

- `src/svg_import.rs` - SVG-to-editable-template importer and diagnostics.
- `docs/SVG_IMPORT.md` - current supported subset, security policy, limits.
- `docs/TEXT_IMPORT_PLAN.md` - planned text/tspan architecture.
- `.agents/skills/svg-zero-dep/SKILL.md` and
  `.claude/skills/svg-zero-dep/SKILL.md` - mandatory guidance for SVG work.

## Project Guidance

- `AGENTS.md` - Codex-facing repo rules.
- `CLAUDE.md` - Claude-facing repo rules.
- `docs/ROADMAP.md` - strategic stage plan.
- `docs/DEVLOG.md` - chronological session record.
- `docs/CODE_COOP.md` - short agent-to-agent handoff diary.
- `docs/ARCHITECTURE.md` - structural truth.

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
