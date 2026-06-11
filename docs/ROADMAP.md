# RohKai Roadmap

## Current Active Work — Pre-Release Depth (SVG Renderer Roadmap R0–R8 Closed)

- Keep RohKai on egui/eframe while closing reliability and feature-depth gaps.
- SVG execution follows `docs/SVG_RENDERER_ROADMAP.md`, now **complete R0–R8**:
  R0 metadata, shared microsyntax/style, bounded references, R1 geometry, R3
  linear/radial paint servers, R4 clipping/viewport-overflow/group-compositing,
  R5 PNG + baseline JPEG `data:` embedded images, R6 editable chunked text
  import, R7 alpha/luminance masks + filter tier-1, and R8 in-app report UI +
  source viewer + golden corpus + benchmark + dev-only oracle. Deferred,
  runtime-diagnosed follow-ons: progressive JPEG, the R6 vector-outline snapshot
  / raster text, and filter tier 2/3.
- Stage 15's general RohKai renderer is deferred and is not the current stage.

## Stage 0 — Bootstrap ✅
- [x] Cargo scaffold, eframe window opens
- [x] Module structure: project / canvas / codegen / panels / widgets
- [x] `cargo check` passes, zero warnings

## Stage 1 — Core Loop ✅
- [x] `UiTree` as single source of truth (serde-serializable)
- [x] Palette panel — click to add Button / Label / TextInput / Slider / Checkbox
- [x] Canvas — click to select, drag to move, Delete key to remove
- [x] Properties panel — live-edit label, binding, x/y/w/h, min/max
- [x] Code panel — `egui_emitter` + `state_emitter` show generated Rust in real time
- [x] Window title and maker's mark ` ^ρϗ`

## Stage 2 — File I/O ✅
- [x] Save project to `.rohkai.json` (Ctrl+S / Save As Ctrl+Shift+S)
- [x] Load project from `.rohkai.json` (Ctrl+O)
- [x] New project / clear canvas (Ctrl+N)
- [x] Unsaved-changes indicator in title bar and menu bar
- [x] Native egui menu bar (File → New / Open / Save / Save As with shortcut hints)
- [x] Snapshot-based dirty tracking (no flag threading)

## Stage 3 — Canvas Polish ✅
- [x] Resize handles (8-point, drag to resize; delta converted to canvas space)
- [x] Grid snap (G key toggle, configurable step, snaps move/resize/nudge)
- [x] Widget visual distinction by kind (accent color per kind, kind tag label)
- [x] Multi-select (rubber-band drag + Shift+click toggle; nudge/delete all selected)
- [x] Z-order controls (right-click context menu: Bring to Front/Forward/Back/Send to Back)
- [x] Keyboard nudge (arrow keys, 1px or grid step when snap on; all selected move together)
- [x] Canvas pan/zoom (scroll wheel 25–400%, centered on cursor; middle-click drag; Ctrl+0 reset)
- [x] Alignment tools (6-button panel in Properties when 2+ selected; bounding-box relative)
- [x] Cursor feedback (resize cursors ↔ ↕ ↗ ↙ on handle hover; held during resize)
- [x] Canvas boundary (centered dotted rect, default 800×600, W/H editable in left panel)

## Stage 4 — Export ✅
- [x] Export button writes a complete compilable Rust project to a user-chosen folder
- [x] Generated project contains: `Cargo.toml`, `src/main.rs`, `src/app.rs`
- [x] `Cargo.toml` includes `egui` and `eframe` as dependencies
- [x] `src/app.rs` contains correct `AppState` struct and full `update()` function
- [x] Exported project runs with `cargo run` without any manual editing
- [x] Use `rfd` file dialog (already a dependency) for folder picker
- [x] Show success/error message in UI after export
- [x] Wire into File menu as "Export Project..." with a separator above it

## Stage 5 — Lazare Core + Widget Additions + App Shell ✅

### Widget Additions
- [x] Frame / Group layout container
- [x] ComboBox
- [x] RadioButton group
- [x] ProgressBar
- [x] Use `/new-widget` slash command to scaffold adding new widget types to Rohkai

### The Lazare Features
- [x] Double-click widget on canvas → code panel highlights the selected widget
      block inline inside the editable code panel. Generated output carries exact
      widget byte/line ranges, edited output carries parser-derived ranges, and
      the editor paints one complete outline per canvas selection in a dedicated
      decoration gutter. The `CentralPanel` preamble is never part of a widget
      range. Tracé handler jump inserts the stub if absent and scrolls through a
      one-frame navigation target. Blank/deleted code clears canvas widgets and
      resyncs to the canonical empty generated output.
- [x] Lazare edit-state safety: generated, valid-edit, and invalid-edit states are
      explicit; incomplete edits stay visible without partially mutating
      `UiTree`; duplicate pasted widget blocks receive fresh UUIDs, placement
      offsets, canonical regeneration, and active selection.
- [x] Code editor surface defaults to no-wrap IDE-style horizontal/vertical
      scrolling with an optional Wrap toggle. Selection decoration is painted
      outside TextEdit's glyph clip and remains inside the visible panel.
- [x] App Properties: window title, width, height, icon — stored in project, used in export

### Menu Bar Ribbon
- [x] Expand the menu bar row beyond just "File"
- [x] Add inline editable app title field (what becomes the window title in export)
- [x] Add window W/H fields (moves from left panel bottom to ribbon)
- [x] Add zoom indicator and reset to ribbon
- [x] Clean up left panel bottom — remove items moved to ribbon

### Templates System
- [x] .rktp file format (serialized UiTree subtree, serde_json)
- [x] Rubber-band selection → "Save as Template" button appears in Properties area
- [x] Templates panel below Properties — scrollable, folder-organized
- [x] Saving a template appears instantly in the panel in the same session
- [x] Click template to instantiate at canvas centre; drag onto canvas to place at cursor
- [x] Templates stored in a /templates folder next to the binary, not baked in
- [x] SVG import as template placeholder — zero-dependency parser maps SVG shapes/text to RohKai template widgets and preserves the source `.svg`

## Stage 5.5 — Properties Depth + Event Wiring ✅

### Properties Panel Expansion
- [x] Tooltip field — generates `.on_hover_text("...")` on the widget
- [x] Enabled toggle — checkbox, generates `ui.set_enabled(false)` when unchecked
- [x] Foreground color picker — R/G/B DragValues with color swatch preview
- [x] Corner radius — DragValue (0–32 px), stored per widget
- [x] Binding mode toggle — Static | Bound per widget
      Static: label is a string literal in generated code
      Bound: label pulls from an AppState `String` field
- [x] Custom property — "+ Add" collapsible form, name + type (String/f32/bool/i32),
      adds field to AppState in both live preview and export

### Event Wiring — The Accessible Lazare Layer
- [x] "On Click" / "On Change" text field in properties panel
      per widget kind (Click for Button, Change for TextInput/Slider/Checkbox/ComboBox/RadioButton)
- [x] Typing a handler name wires click/change in live codegen and generates
      a stub method on `ExportedApp` in export
- [x] Double-clicking the event name field triggers Tracé —
      inserts handler stub into code panel if absent, signals scroll
- [x] Handler stub inserted when not yet present in code buffer
- [x] Accessible Lazare entry point — field for simple users, code for power users

### Palette Organization
- [x] Collapsible category sections in palette
      Categories: Basic (Button, Label, TextInput),
      Input (Slider, Checkbox, RadioButton, ComboBox),
      Layout (Frame/Group),
      Display (ProgressBar)
- [x] Category headers are clickable to collapse/expand
- [x] Last expanded state persists during session (egui Memory)
- [x] Widget accent color shown as small dot next to palette item

## Patch — v0.1 Stability & Polish

### Codex Bug Fixes (2026-05-20) ✅
- [x] Dirty snapshot uses canonical `project::io::serialize()` — no divergence between save/open/dirty paths
- [x] New/Open prompt unsaved-changes dialog when project is dirty (Save / Discard / Cancel)
- [x] Rust string escaping and binding validation in `src/codegen/rust.rs`; invalid bindings not emitted
- [x] `UiTree::add()` makes default bindings unique; `state_emitter` skips duplicates
- [x] Live preview and export emit `egui::Area::fixed_pos(...)` based on `WidgetInstance.rect`
- [x] `UiTree::validate_and_repair()` clamps geometry, repairs inverted min/max; used on load/save/edit
- [x] `.rohkai.json` saves use versioned `ProjectFile` envelope; legacy bare `UiTree` files still load
- [x] Snap step clamped to `1.0..=256.0`; grid/snap math cannot freeze on zero or negative

### Group 1 — Template & Palette Drag Fixes ✅
- [x] Template click (`AddAtCenter`) processed before canvas `handle()` — no same-frame collision with `primary_released`
- [x] Template drag (`BeginDrag`) sets `interaction.template_drag` for canvas drop at cursor position
- [x] Palette drag returns `Some(instance)` to caller; caller sets `interaction.template_drag` for canvas drop

### Group 2 — Layout Cleanup ✅
- [x] Move W/H canvas-size controls from ribbon to bottom status bar (`TopBottomPanel::bottom`)
- [x] Move grid snap controls (Grid ON/OFF, step, [G]) from left panel to bottom status bar
- [x] Move "Save as Template…" from Properties panel into File menu (below Export; disabled when nothing selected)
- [x] Ribbon slimmed to: File menu | app title field | zoom indicator | reset button | dirty indicator | error

### Group 3 — Visual Polish ✅
- [x] Palette buttons: hover shows accent color matching canvas widget kind colors
- [x] Slider canvas rendering: track line + thumb indicator instead of plain box

### Group 4 — SVG Template Import ✅
- [x] File → Import SVG as Template…
- [x] Raw Rust parser: no new dependencies
- [x] Supported import placeholders: `rect`, `circle`, `ellipse`, `line`, `polyline`, `polygon`, `path`, `image`, `use`, `text`
- [x] Handles attributes, inline styles, units, `viewBox`, groups, nested transforms, hidden/defs containers, and path command bounds
- [x] Saves both generated `.rktp` and original `.svg` beside it for validation/source preservation
- [x] Validation workflow: `scripts/validate-svg-import.ps1`

## Stage 6 — Bidirectional Sync ✅

### Smart Guides (before Lazare parser) ✅
- [x] Inferential snapping — dashed red guide lines appear when a widget
      edge or center aligns with another widget's edge or center
- [x] Equidistant spacing indicators — show spacing marks when
      three or more widgets are evenly spaced
- [x] Guide lines persist while dragging, disappear on release
- [x] Works with multi-select drag
- [x] Snap to guide lines within 4px threshold
- [x] Guide lines are dashed, rgba(255,80,80,180), span widget extents ±40px only
- [x] Max one horizontal + one vertical guide at once (strongest alignment only)
- [x] Smart guides also fire during resize — snaps moving edge to other widget edges/centers
- [x] Key-object alignment — Shift held during align operations aligns to last selected widget;
      key object stays fixed, all others align to it

### Lazare — Bidirectional Sync ✅
- [x] Code panel TextEdit always editable — no mode switching, no Edit button
- [x] Edits parsed back into UiTree via src/codegen/parser.rs
- [x] Canvas updates live when code changes (parse success → apply immediately)
- [x] Conflicts (invalid syntax) shown as "error" with red border, canvas unchanged
- [x] Code panel header indicator: "live" teal / "pending" amber / "error" red
- [x] Editor stays open during typing — never resets on parse attempt
- [x] Partial apply: filter to widgets with non-zero geometry; no count constraint
- [x] Canvas changes (drag/nudge) always reset editor to "live" mode

### Patch — Layout & UX ✅
- [x] Group/Ungroup buttons (⊞/⊟) in Properties panel alongside alignment icons;
      enabled when selection qualifies (2+ for group, Frame selected for ungroup)
- [x] Palette click placement accounts for zoom and pan — new widget appears at
      center of visible canvas area (formula: visible_center = (viewport_center - pan) / zoom)
- [x] File → Preferences… window backed by user-level settings, not `.rohkai.json`
- [x] Global UI scale applies on OK/Apply so the Preferences window does not resize while dragging
- [x] Code/AppState font size, canvas label/tag scale, and default snap step controls

## Housekeeping (pre-Stage 7)

- [x] Roadmap/devlog split: roadmap stays strategic, `docs/DEVLOG.md` records chronological session notes
- [x] Shared preflight workflow for Codex and Claude before planning or code edits
- [x] Settings fallback uses `temp_dir()` — falls back to binary-adjacent `<exe_dir>/user-settings/settings.json` instead (not cleaned by OS)
- [x] Save failure applies settings for current session but error message doesn't say so — clarify to "Applied this session only — save failed: …"

## Stage 7 — Framework Import / Ply Support ✅
- [x] Widget descriptor format (.rkwd — RohKai Widget Definition)
- [x] Descriptor defines: name, category, accent, properties (String/F32/I32/Bool/Enum),
      codegen templates (live + export), cargo_deps, events
- [x] Drop `.rkwd` in `<binary_dir>/widgets/` → widget appears in palette at startup
- [x] `WidgetKind::Custom(String)` in schema — serializes to existing project files safely
- [x] Properties panel renders typed fields from descriptor; falls back to raw
      key→value table if descriptor file is missing
- [x] Export injects descriptor `cargo_deps` into generated `Cargo.toml`
- [x] Ply Button (`widgets/ply-button.rkwd`) as first shipped example
- [x] Descriptor load errors shown in ribbon — non-fatal, app runs without them

[//]: # (did ply button actually get implemented?)

## Stage 7.x - Widget Descriptor Maturity
- [x] File → Import Widget Definition… dialog (load a single `.rkwd` without restart)
- [x] Hot-reload: rescan `/widgets` folder without restart (file-watcher or menu action)
- [x] Guided Descriptor Builder (`src/panels/widget_builder.rs`) — beginner
      form over `WidgetDescriptor` with name, id auto-derive, type, label
      default, click handler, and live descriptor preview. This is not the full
      visual widget maker; it creates simple `.rkwd` descriptors safely. Hides
      raw templates behind Label/Button/RawTemplate selector. "Advanced
      Descriptor…" closes builder and opens the full editor with current draft.
- [x] Lazare round-trip for Custom widgets: geometry already works; label/binding
      round-trip requires parser to understand descriptor template structure
- [x] In-app `.rkwd` editor: create / edit descriptors from within RohKai
- [ ] `.rkwb` bundle format — zip of multiple `.rkwd` + preview SVGs + assets

## Stage 7.x - Visual Widget Maker
- [x] Clarified product taxonomy:
      Advanced Descriptor Editor, Guided Descriptor Builder, and future Visual
      Widget Maker are separate layers.
- [x] Added `docs/VISUAL_WIDGET_MAKER.md` design note.
- [ ] Add `WidgetMakerDocument` internal model for visual primitive composition.
- [ ] Add separate Visual Widget Maker window with mini-canvas and inspector.
- [ ] Primitive vertical slice: rect, text, button-like hit region, z-order,
      resize/move, and selection.
- [ ] Expose primitive values as descriptor properties, starting with `label`.
- [ ] Generate deterministic `WidgetDescriptor` output from the visual document.
- [ ] Save generated descriptor to `widgets/`, reload palette, and preserve
      Advanced Descriptor escape hatch.
- [ ] Later: slots, layout groups, constraints, state variants, event zones,
      style tokens, and import simple descriptors into maker documents.

## Stage 7.x - SVG Source Viewing — Historical Snapshot

> Detailed SVG work is now owned by `docs/SVG_RENDERER_ROADMAP.md`. This
> snapshot records why the source viewer was introduced; its remaining inline
> expansion control is tracked as editor UX in renderer Phase R8.

- [x] Read-only SVG source viewer panel or popup for Image widgets
      (SVG is intentionally contracted in the live code panel — this gives a
      way to inspect/copy the raw SVG without polluting the code buffer)
- [x] "Expand SVG inline" toggle per Image widget — checkbox in properties
      panel, warns on large SVGs (>10 KB), test in `egui_emitter` proves
      compact vs raw-literal switch.

## Stage 7.x - SVG Import Maturity — Historical Snapshot

> Superseded as an active checklist by `docs/SVG_RENDERER_ROADMAP.md`. The
> original items remain mapped below so history is preserved without creating
> competing definitions of completion.

| Former Stage 7.x item | Authoritative phase |
|---|---|
| Robust `tspan` parser with span provenance and diagnostics | R6 Text Import And Optional Rendering |
| Editable multi-label grouped import for positioned spans | R6 |
| Optional vector-outline snapshot mode | R6 |
| RohKai-owned text layout/shaping engine, only if justified | R6 deferred decision |
| More granular import/render fidelity scoring | R3, R4, R6, and R8 evidence |
| Dedicated importer/render report UI | R8 Conformance And Editor UX |

Historical foundation completed here:

- [x] SVG renderer roadmap/truth inventory comparing RohKai to mature engines
- [x] Shared `src/svg_core.rs` microsyntax module started for color, numeric
      list, affine transform, and path token parsing across importer/rasterizer
- [x] `SvgRenderOutput` / `SvgRenderReport` API with rendered/skipped counts,
      unsupported-feature diagnostics, raster-size warnings, and fidelity
      scoring
- [x] Renderer diagnostics attached to parsed nodes/attributes for known
      unsupported elements and attributes
- [x] Initial internal SVG scene-item flattening boundary with accumulated
      transforms and resolved inherited style
- [x] SVG renderer scene/display-list IR split
- [x] Stable source-spanned SVG node IDs and bounded local reference metadata
- [x] Owned render-ready display list; raster execution no longer traverses XML
      nodes or parses raw shape attributes
- [x] Shared SVG length/unit parsing and viewport-relative raster percentages
- [x] Golden renderer fixture harness for supported raster output
- [x] Shared SVG microsyntax module for importer/rasterizer parity
- [x] Full SVG `preserveAspectRatio` semantics for root and nested viewports:
      `none`, all nine alignments, `meet`/`slice`, and per-viewport percentage
      bases; nested overflow clipping remains R4
- [x] SVG compound-path fill semantics: inherited `nonzero` default and
      explicit `evenodd`, inline-style precedence, invalid-value warnings,
      analytical winding tests, and golden fixtures

## Stage 8 Addendum — Rulers, Presets, Theming ✅

### Rulers & Measurement
- [x] Horizontal ruler along top of canvas
- [x] Vertical ruler along left of canvas
- [x] Rulers show units in pixels, update with zoom
- [x] Click ruler to create a persistent guide line
- [x] Guide lines are draggable, deletable (Delete key when selected)
- [x] Toggle rulers with Ctrl+R
- [x] Guide snapping: snap widget edges to guide lines

### Document Presets & Real Window Sizing
- [x] Document preset picker: common screen sizes
      (1920x1080, 2560x1440, 1366x768, 1280x720)
- [x] Mobile presets (375x812 iPhone, 390x844 etc)
- [x] Window resizable toggle, min/max size constraints stored in AppProps and used in export
- [x] Custom size with lock aspect ratio toggle
- [x] Canvas represents actual app window —
      shows title bar chrome, minimize/maximize/close buttons
      as a visual bezel around the canvas area

### Application Appearance & Theming
- [x] Theme panel: dark/light mode toggle
- [x] Accent color picker for the generated app
- [x] Font size base setting
- [x] Widget rounding (global corner radius)
- [x] Spacing/padding scale
- [x] Theme exported as startup code in generated app:
      ctx.set_visuals(egui::Visuals { ... })
- [x] Save themes as .rktheme files
- [x] Apply a theme to the RohKai designer itself

---

## Cline Review 2026-05-26 — Approved / Deferred

Comparative analysis of RohKai vs mature egui-based tools produced nine
recommendations. Status recorded here for traceability.

**Approved (implement at appropriate stage):**
- Rec 1 — Handler extraction: pull repeated handler-dispatch logic out of
  `update()` into dedicated command methods (aligns with existing `cmd_*` pattern).
- Rec 2 — Module-level doc comments: add `//!` crate/module docs to all
  `src/**/*.rs` files for discoverability.
- Rec 3 — Codegen memoization: cache emitter output keyed on `UiTree` hash to
  skip regeneration on unchanged frames. **Caveat:** Lazare requires the live code
  buffer to stay in sync with canvas state — memoization must not suppress updates
  that reflect canvas mutations. Implement with care.
- Rec 5 — Export integration tests: add `#[test]` cases that call `export::emit`
  and verify the generated project compiles with `cargo check`.
- Rec 6 — Canvas/UiTree unit tests: add tests for `UiTree::add`, `remove`,
  `validate_and_repair`, and `canvas_rect` coordinate math.
- Rec 9 — Command pattern (design only): document the intended `AppCommand` enum
  shape so future undo/redo (Stage 14) can slot in without a large refactor.
  Do not implement the stack yet.

**Deferred:**
- Rec 4 — `thiserror` crate: not approved. Implement error types with manual
  `Display` impls internally until a specific feature justifies the dependency.
- Rec 7 — Dirty rectangles / partial repaint: not applicable. egui repaints the
  full frame on demand; partial repaint is managed by the egui/wgpu integration,
  not by RohKai application code.
- Rec 8 — Parallel SVG processing: `rayon = "1"` is now an approved dependency
  (see Architecture Rules), so parallelism is available. Specific use sites
  (e.g. batch SVG import, rasterizer tile dispatch) are addressed per-stage.

---

## Stage 8.5 — Document Outline & Preview Mode ✅

Bridges Stage 8 polish and Stage 9 depth. Improves designer usability without
requiring schema changes.

- [x] Document outline panel — `src/panels/outline.rs`; Ctrl+L toggle; layers
      sidebar in left panel showing all widgets in draw order with accent dots,
      labels, kind tags; click-select, Ctrl+click multi-select, double-click
      canvas-center, drag-to-reorder z-order; indents Frame children; read-only
      in preview mode
- [x] Preview mode — F5 toggle; `src/canvas/preview.rs`; replaces painter-based
      canvas with actual egui widget calls at 1:1 zoom; `PreviewState` holds
      runtime values; code panel hidden; status bar shows PREVIEW MODE indicator;
      PREVIEW badge + "Exit Preview [F5]" button overlaid on canvas; outline
      panel remains visible read-only
- [x] Keyboard shortcut reference — `?` button in menu bar or F1; `src/panels/shortcuts.rs`;
      floating window listing all shortcuts by category (File, Canvas, Selection,
      Grid Snap, Grouping, Help)

---

## Stage 9 — Widget Depth & Lazarus Completeness

### Parallelism Foundation (rayon integration)
- [x] Add `rayon = "1"` as core dependency — enables app-wide parallel processing
- [x] Parallel SVG rasterization — `rasterize_batch()` in `svg_rasterizer.rs`; `par_iter` over `(&str,u32,u32)` slices
- [x] Parallel codegen — `emit_indexed` uses `par_iter` over top-level widget Area blocks; output order preserved
- [x] Parallel export — `project_files` generates Cargo.toml/main.rs/app.rs via `rayon::join`; `write_project` writes in parallel
- [x] Parallel template loading — `load_all_templates()` in `templates.rs` batch-loads `.rktp` via `par_iter`
- [x] Performance benchmarks — 50/100/500-widget Area-block count tests + determinism test in `egui_emitter`

### Lazarus Completeness
- [x] Full contextual properties per widget kind — schema audit pass: `text_wrap`
      field added (Label/TextArea), TextInput bg_color+corner_radius exposed,
      ProgressBar fg_color wired to `.fill()` in codegen, TextArea fully audited
- [x] Design-time non-visual components — `src/panels/component_tray.rs`;
      Timer/DataSource/Lifecycle as clickable icon-chips in left-panel
      "Components" section; per-component config editor; codegen emits
      DataSource AppState fields + Timer update() interval comments
- [x] Full event list per widget — `on_double_click` (Button), `on_lost_focus`
      (TextInput/TextArea), `on_drag_stopped` (Slider/SpinBox) added to schema
      and codegen; properties panel shows dynamic per-kind event list with Tracé chips
- [x] Object Inspector true bidirectionality — properties edits update UiTree
      immediately (immediate-mode architecture); canvas re-renders every frame;
      code panel resyncs via `generated != last_generated`; pending-code warning
      added to properties panel when code has unsaved edits

### SVG Renderer Progression

> Historical completion mirror only. Active renderer sequencing and closure
> criteria live exclusively in `docs/SVG_RENDERER_ROADMAP.md`.

- [x] SVG renderer scene/display-list IR split — `DisplayList`/`DrawCommand` IR
      in `svg_rasterizer.rs`; build() lowers scene graph → flat command stream,
      including geometry, diagnostics, source provenance, style, and transform;
      execute() rasterizes without XML-node access
- [x] SVG R0 metadata closure — stable preorder node IDs, exact byte spans,
      bounded first-id-wins local references, structured non-local reference
      rejection, and shared importer/rasterizer length parsing
- [x] SVG R1 viewport semantics — full root/nested `preserveAspectRatio`
      alignment, meet/slice/none mapping, and nested percentage coordinate
      bases; clipping remains explicitly deferred to R4
- [x] SVG R1 fill semantics — nonzero/evenodd winding behavior reaches the
      raster backend through inherited style and is protected by analytical
      and golden tests
- [x] SVG R1 stroke geometry — retained path segments, local-space affine
      stroke expansion, cap/join/miter semantics, dash arrays/offsets,
      `pathLength` calibration, zero-length caps, transformed bounds, and
      visible complexity-limit diagnostics
- [x] SVG R1 anti-aliasing — deterministic 8x8 fill winding/parity coverage and
      separate unioned stroke coverage, protected by analytical, golden,
      determinism, and coarse performance tests
- [x] Golden renderer fixture harness for supported raster output —
      `src/canvas/svg_golden.rs` (#[cfg(test)]); deterministic ASCII-grid
      signatures, supported + unsupported buckets, drift-detecting tests; zero
      new dependencies

### New Widget Kinds — Layouts & Spacers
- [x] Vertical Layout (`VLayout`) — canvas box with ↕ indicator
- [x] Horizontal Layout (`HLayout`) — canvas box with ↔ indicator
- [x] Grid Layout (`GridLayout`) — canvas box with 3×3 grid lines; emits egui::Grid
- [x] Form Layout — closed as a 2-column GridLayout preset; built-in "Form Layout"
      and "Login Dialog" templates ship in `builtin_templates()` in
      `src/panels/templates.rs` with structural tests.
- [x] Horizontal Spacer — dashed horizontal bar
- [x] Vertical Spacer — dashed vertical bar

### Real Layout Manager Behavior — Not Yet Complete
Current `VLayout`, `HLayout`, and `GridLayout` now have a **first-slice
direct-child ownership model**: they can own/reflow direct children through
`WidgetInstance.children` and emit matching egui layout calls. They are still
not full Qt/Lazarus layout managers because full alignment controls, per-child
policies, richer slot editing, and runtime-style constraint behavior remain
open. Do not treat the first-slice ownership surface as closing the real layout
gap.

- [x] Stack layout slice: `VLayout` and `HLayout` can own/drop child widgets as
      explicit `WidgetInstance.children`, detach them when dragged outside, and
      reflow direct children in source-of-truth `UiTree` coordinates.
- [x] Stack layout slice: canvas preview reflows direct `VLayout`/`HLayout`
      children when the container resizes.
- [x] GridLayout first slice: direct children attach/detach and reflow row-major
      into a default 3-column grid.
- [x] GridLayout first slice: live codegen/export emit direct children inside
      `egui::Grid::new(...).show(ui, |ui| { ... })` with row boundaries.
- [x] Properties expose first-slice layout knobs: margins, spacing/gap, and
      GridLayout columns; these drive canvas reflow and generated row breaks.
- [x] Properties expose first-slice stretch/fill behavior through
      `layout_stretch`; container reflow preserves child size hints when
      stretch is disabled.
- [ ] Properties expose alignment, grid row policies, and per-child
      stretch/fixed-size behavior.
- [x] Spacers are layout-aware first-slice items: `VerticalSpacer` flexes inside
      `VLayout`, `HorizontalSpacer` flexes inside `HLayout`, and generated code
      emits matching `ui.add_space(...)`.
- [x] Layers/Outline displays owned layout children directly under their parent
      instead of as a flat draw-order row with incidental indentation.
- [x] Delete, group, ungroup, and first-slice child reorder semantics respect
      layout-child ownership and reflow through `UiTree`.
- [ ] Hit testing, rubber-band selection, and richer drag-reorder semantics need
      more layout-aware polish.
- [x] Stack layout slice: live codegen and export place direct
      `VLayout`/`HLayout` children inside `ui.vertical(|ui| { ... })` or
      `ui.horizontal(|ui| { ... })` closures in child order.
- [x] Lazare parser first-slice round-trips one-level layout-owned hierarchy
      from generated/edited layout closures.
- [x] Add tests proving canvas child order, resize reflow, generated code
      nesting, export output, and first-slice parser behavior stay consistent.
- [ ] Add richer cell/slot editor, named slots, drag-to-slot behavior, and
      multi-level layout hierarchy round-trip tests.

### New Widget Kinds — Containers
- [x] Scroll Area — canvas box with simulated scrollbar indicator
- [x] Group Box — labeled group frame (egui::Frame::group with heading)
- [x] Tab Widget — tab header bar; options = tab names; emits top-panel tabs

### New Widget Kinds — Input Additions
- [x] Font Combo Box — `FontComboBox` with Aa indicator
- [x] Multi-line Text Edit — `TextArea` (egui::TextEdit::multiline)
- [x] Numeric / spinner controls — `SpinBox` (egui::DragValue)

---

## Stage 10 — Technical & Computational Widgets ✅

### Feature Depth Status
- **Full enough for current export:** `FilePicker` now emits `rfd::FileDialog`,
      generated AppState path storage, and the required exported `rfd = "0.14"`
      Cargo dependency.
- **Functional MVP:** `MathLabel` is a computed `f32` label, not a formula
      editor; `Chart` is a minimal `Vec<f32>` bar painter, not a charting
      library; `Table`, `ListView`, and `TreeView` are static option-backed
      display widgets, not model-bound data views.
- **Design-time MVP / documented stubs:** `Timer`, `StateMachine`, and
      `HttpRequest` create design-time components and generated state/comment
      hooks; they do not yet schedule runtime ticks, execute transitions, or
      perform network requests.
- **Planned competitor-depth work:** formula parsing/validation, data models,
      interactive chart axes/series/legends, runtime component dispatch, and
      data-bound table/list/tree views remain future work.

### Computational & Non-Visual Components
- [x] Math/formula widget MVP — `MathLabel` (f32-bound); emits safe
      `ui.label(format!("{} = {:.2}", label, value))`
- [ ] Formula Widget — expression parser, dependency tracking, validation,
      formatting controls, multiple input fields, and diagnostics
- [x] Timer/interval component MVP — design-time ComponentKind with documented
      generated update comment; runtime scheduling remains future work
- [x] State machine component MVP — emits usize state field + documented
      state-transition stub comment; visual transition editor remains future work
- [x] HTTP request component MVP — emits String response field + documented
      stub comment; runtime HTTP needs a user-approved crate/stage decision

### Data Display Widgets
- [x] Data table widget MVP — `Table` (egui::Grid, columns from static options)
      — merged with Table View; model/data binding remains future work
- [x] File picker widget — `FilePicker`; emits `rfd::FileDialog`, path field,
      and generated `rfd = "0.14"` dependency
- [x] Chart widget MVP — `Chart`; canvas bar preview and generated egui painter
      bar output from a `Vec<f32>` binding

### New Widget Kinds — Data Views
- [x] List Widget / List View MVP — static option-backed `ScrollArea` labels
- [x] Tree Widget / Tree View MVP — static option-backed `CollapsingHeader` hierarchy
- [x] Table Widget / Table View — merged into `Table` above

### New Widget Kinds — Additional Containers & Buttons
- [x] Stacked Widget — `StackedWidget` container (active-page preview)
- [x] Tool Box — `ToolBox` (vertical collapsing sections)
- [x] Tool Button — `ToolButton` (egui small_button)
- [x] Command Link Button — `CommandLinkButton` (title + description)
- [x] Dialog Button Box — `DialogButtonBox` (button row from options)

---

## Stage 11 — Rust-Centric Visual Features ✅

See `docs/STAGE11_PLAN.md` for the full design (function, depth, UX, impact).

- [x] Ownership visualization — `canvas/overlays.rs::draw_ownership`; View → Show
      Ownership Overlay; per-widget `→ field: Type` badges + AppState legend,
      driven by `field_collector` (never diverges from codegen)
- [x] Async task wiring — `WidgetInstance.async_handler`; Properties → "Run async".
      Functional MVP: export generates a real std-only task contract (no tokio) —
      per-handler `{h}_rx`/`{h}_running`/`{h}_error` fields, a launcher
      `fn {h}(&mut self)` that `thread::spawn`s + `mpsc::send`s `{h}_worker()`, a
      free-fn worker with no `&mut self`, a `try_recv` drain in `update()` that
      records status/error, and a `ctx.request_repaint_after(16ms)` guard so the UI
      repaints promptly while tasks are in flight without user input. Handler call
      sites for **every** event (primary AND secondary) on every event-capable
      widget route through `handler_call()` so async/plain/result/option semantics
      are consistent. The event set is a single source of truth
      (`WidgetKind::supported_events`, exhaustive match) that both Properties and
      export derive from: Button (Click/DoubleClick); TextInput/TextArea
      (Change/LostFocus); Slider/SpinBox (Change/DragStopped); Checkbox, ComboBox,
      FontComboBox, RadioButton (Change). Export binds the widget `Response` once
      and emits one `if evt_response.<method>() { … }` per wired event
      (`clicked`/`double_clicked`/`changed`/`lost_focus`/`drag_stopped`). Parity
      holds in BOTH the top-level and the nested/frame-child (`export_child_line`)
      export paths — children bind `child_response`/`child_combo` and route the
      same way; ComboBox/FontComboBox children render real interactive combos.
      Enforced by two invariant tests (top-level + nested) over every `(kind, event)`
      pair — no Properties event row is ignored by any export path. Duplicate handler
      names across any event fields (including top-level↔child) are detected: first
      definition wins, a near-handler `// CODEGEN CONFLICT` comment plus a
      top-of-`app.rs` conflict summary block. Button Click+DoubleClick fire
      independently per egui semantics (Click not suppressed). A real `cargo check`
      compile fixture (`export_compile_fixture_cargo_check`, `#[ignore]`) +
      always-run smoke now prove the generated crate compiles across top + nested
      events, async Plain/Result, FilePicker/rfd, channel fields, iterator methods,
      simple local trait binding, and state bindings. Remaining (not top-class):
      auto-bind status to a widget, cancellation/progress, typed task I/O, and
      deeper runtime simulation. See
      `codegen/rust_wiring.rs`, `codegen/export.rs`.
- [x] Channel connections — `RustWiring.channels`; Rust Wiring window; export emits
      `std::sync::mpsc` Sender/Receiver fields + init on `ExportedApp`
- [x] Error propagation — `WidgetInstance.handler_result` (Plain/Result/Option);
      Properties → Error mode dropdown; `overlays.rs::draw_error_flow`; handler
      signature + call site reflect the contract
- [x] Iterator pipeline builder — `RustWiring.iterators` (source + ordered
      Map/Filter ops); Rust Wiring window with live preview; export emits a
      compile-valid `fn name(&self) -> impl IntoIterator + '_` method that
      collects through `Vec<_>` internally
- [x] Trait binding — `RustWiring.trait_impls`; Rust Wiring window; export emits
      a local trait declaration for simple trait names plus
      `impl Trait for ExportedApp { method { body } }`
- [x] Macro palette — `panels/macro_palette.rs`; View → Macro Palette…; clicking a
      macro (vec!/format!/println!/dbg!/assert!/todo!/matches!) appends its snippet
      to the live Lazare code buffer

> Note on async: the roadmap originally said "tokio::spawn or similar". RohKai's
> architecture rules forbid a tokio runtime without an explicit need, so Stage 11
> generates the std `thread::spawn` + `mpsc` pattern — the "or similar" — which
> compiles with zero added dependencies. Recorded as an intentional decision.

---

## Stage 12 — Platform Targets
- [ ] WASM export panel in File menu
- [ ] Configure: output path, bundler (trunk/wasm-pack), generate index.html toggle
- [ ] Generates `cargo build --target wasm32-unknown-unknown` compatible project
- [ ] Web-specific widget considerations (no file dialogs, no native paths)
- [ ] Preview in browser button — runs trunk serve

---

## Stage 13 — Data & Integration
- [ ] DB connection configurator — SQLite/PostgreSQL/MySQL/Supabase
- [ ] Uses sqlx or rusqlite crate (user approves exact crate at stage start)
- [ ] Visual query builder — select table, columns, filter
- [ ] Bind widget to query result field
- [ ] Generated code uses correct Rust DB crate with async/sync query calls
- [ ] Schema viewer — see tables and fields visually
- [ ] Generates AppState with db connection pool field

---

## Stage 14 — Project Infrastructure ✅
- [x] Project tree panel — `src/panels/project_tree.rs`; File → Project Files…;
      lists generated files from `export::project_files()` (single source of truth)
- [x] Click a file to view/edit its generated content — read-only code viewer pane
      (right side of the project tree window)
- [x] Add non-generated files to project (assets, configs) — asset registry with
      rfd file picker; `AppProps.assets: Vec<AssetEntry>`
- [x] Assets folder management — `AssetEntry`/`AssetKind` (Image/Font/Data/Other);
      `assets/MANIFEST.txt` emitted on export listing referenced files
- [x] Help system — done in Stage 8.5 (`src/panels/shortcuts.rs`, F1 / `?` button)
- [x] Interactive sandbox mode — done in Stage 8.5 (`src/canvas/preview.rs`, F5 toggle)
- [x] Full undo/redo stack — `src/project/undo.rs`; serialized UiTree snapshots,
      50-step cap, Ctrl+Z / Ctrl+Y (and Ctrl+Shift+Z); Edit menu; drag-coalescing
      commit boundaries
- [x] Widget hierarchy/layers panel — done in Stage 8.5 (`src/panels/outline.rs`,
      Ctrl+L, click-select, drag-reorder z-order)

---

## Pre-Release Depth Consolidation Gate

Before starting broad new feature families or Stage 15 renderer work, RohKai
should become a brutally reliable Rust/egui designer. This gate consolidates
the current dirty/in-flight truth, separates MVP surfaces from closure criteria,
and prioritizes depth over more palette breadth.

### Source-Of-Truth Consolidation
- [x] Resolve the current dirty worktree into intentional commits/PRs so current
      truth is not spread across uncommitted code and docs.
- [x] Reconcile duplicated or stale roadmap lines against
      `docs/feature-evaluation/*`, especially SVG renderer maturity and
      Stage 10/11 depth claims.
- [x] Keep `docs/ROADMAP.md` strategic, `docs/DEVLOG.md` chronological, and
      `docs/CODE_COOP.md` as short agent handoff only.

### Reliability Proofs
- [x] Add an all-built-in-widget generated-project compile fixture covering every
      palette kind plus Image/SVG. `src/codegen/export.rs` now has a fast smoke
      plus ignored real generated-crate `cargo check`; the proof caught and fixed
      SVG Image export module embedding.
- [x] Add export compile fixtures for assets, custom descriptors, SVG Image, and
      mixed event/async/data widgets where not already covered.
      `asset_manifest_is_generated_with_correct_entries` and
      `custom_descriptor_export_renders_template_not_placeholder` added to
      `src/codegen/export.rs`.
- [x] Add release smoke checklist: save/load, export, preview mode, code paste,
      multi-select, templates, preferences, theme, and SVG import.
      See `docs/RELEASE_SMOKE_CHECKLIST.md`.

### Depth Before Breadth
- [ ] True layout ownership: `VLayout`, `HLayout`, and `GridLayout` own/reflow
      children with spacing, padding, alignment, stretch/fill rules, and matching
      nested codegen/export/parser behavior.
- [x] Lazare structured-range/editor-decoration foundation: generated and parsed
      widget spans, canvas-authoritative multi-selection, no-wrap scrolling,
      gutter-painted outlines, safe invalid edits, empty-code clearing, and
      duplicate-paste repair.
- [ ] Lazare IDE depth: precise cursor placement, search, symbol list, clickable
      diagnostic navigation, diff view, generated/user-region ownership, and
      handler-range indexing beyond the current navigation slice.
- [ ] Data model groundwork: typed data source model, binding model for
      Table/List/Tree, and explicit separation of static option widgets from
      model-backed views.
- [ ] Runtime component semantics: Timer, StateMachine, HttpRequest, Lifecycle,
      and DataSource either execute real generated behavior or remain clearly
      labeled design-time/documented stubs.
- [ ] Formula and chart depth: keep `MathLabel` and current `Chart` as MVPs while
      planning separate formula parser/evaluator and chart series/axes/legend
      systems.
- [ ] Visual Widget Maker: build the real primitive mini-canvas tool separately
      from the Guided Descriptor Builder and Advanced Descriptor Editor.
- [ ] Object Inspector/component tray depth: improve discoverability, contextual
      property grouping, component runtime status, and parity with mature
      designer workflows.

---

## Stage 15 — Own Renderer

> This is **not** the SVG renderer roadmap. SVG R0-R8 improves SVG import,
> Image preview, and SVG export while RohKai continues to run on egui/eframe.
> Stage 15 would replace RohKai's general widget/runtime rendering layer and
> remains deferred until the pre-release depth gate is closed and a separate
> architecture decision explicitly activates it.

- [ ] Replace egui rendering layer with RohKai-owned pure Rust renderer
- [ ] Widget descriptor format drives renderer widget model directly
- [ ] Zero transient C dependencies
- [ ] All previously constrained visual properties become available:
      per-widget color, corner radius on all types, border widths, drop shadows

### Later / High Risk
- [ ] Model-based item views
- [ ] Dock Widget
- [ ] MDI Area
- [ ] Multi-window support
- [ ] QAxWidget-style platform integrations
      (not compatible with RohKai's pure Rust / no C FFI rule)
