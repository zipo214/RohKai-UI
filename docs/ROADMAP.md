# RohKai Roadmap

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
- [x] Double-click widget on canvas → code panel scrolls to and highlights that widget's handler, cursor ready
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

## Stage 7.x - SVG Source Viewing (from code panel contraction)
- [x] Read-only SVG source viewer panel or popup for Image widgets
      (SVG is intentionally contracted in the live code panel — this gives a
      way to inspect/copy the raw SVG without polluting the code buffer)
- [ ] Optional "expand SVG inline" toggle per Image widget for power users

## Stage 7.x - SVG Import Maturity
- [ ] Robust `tspan` parser with span provenance and diagnostics
- [ ] Editable multi-label grouped import for positioned spans
- [ ] Optional vector-outline snapshot mode for visual comparison
- [ ] RohKai-owned text layout/shaping engine only if editable text still needs it
- [ ] More granular fidelity scoring for text-heavy, clipped, masked, filtered,
      gradient, and pattern-heavy SVGs
- [ ] Dedicated importer report UI showing skipped features and approximation notes
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
- [ ] SVG renderer scene/display-list IR split
- [ ] Golden renderer fixture harness for supported raster output
- [x] Shared SVG microsyntax module for importer/rasterizer parity

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

## Stage 8.5 — Document Outline & Preview Mode

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
- [ ] Parallel SVG rasterization — batch rasterize multiple Image widgets using `rayon::par_iter`
- [ ] Parallel codegen — emit egui code for independent widgets in parallel
- [ ] Parallel export — write exported project files concurrently
- [ ] Parallel template loading — load multiple template files concurrently
- [ ] Performance benchmarks — measure speedup for projects with 50+, 100+, 500+ widgets

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
- [x] SVG renderer scene/display-list IR split — `DisplayList`/`DrawCommand` IR
      in `svg_rasterizer.rs`; build() lowers scene graph → flat command stream,
      execute() rasterizes; pixel output unchanged
- [x] Golden renderer fixture harness for supported raster output —
      `src/canvas/svg_golden.rs` (#[cfg(test)]); deterministic ASCII-grid
      signatures, 5 fixtures, drift-detecting tests; zero new dependencies

### New Widget Kinds — Layouts & Spacers
- [x] Vertical Layout (`VLayout`) — canvas box with ↕ indicator
- [x] Horizontal Layout (`HLayout`) — canvas box with ↔ indicator
- [x] Grid Layout (`GridLayout`) — canvas box with 3×3 grid lines; emits egui::Grid
- [ ] Form Layout (deferred — egui has no distinct form primitive; Grid covers it)
- [x] Horizontal Spacer — dashed horizontal bar
- [x] Vertical Spacer — dashed vertical bar

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

### Computational & Non-Visual Components
- [x] Math/formula widget — `MathLabel` (f32-bound); emits `ui.label(format!("{} = {:.2}", …))`
- [x] Timer/interval component — done in Stage 9 component tray (Timer ComponentKind)
- [x] State machine component — `StateMachine` ComponentKind; emits usize state field +
      `on_transition` update hook
- [x] HTTP request component — `HttpRequest` ComponentKind; emits String response field +
      `on_response` dispatch hook (mpsc note for async)

### Data Display Widgets
- [x] Data table widget — `Table` (egui::Grid, columns from options) — merged with Table View
- [x] File picker widget — `FilePicker`; emits rfd::FileDialog browse + path field
- [x] Chart widget — `Chart`; canvas bar preview, codegen painter comment for Vec<f32>

### New Widget Kinds — Data Views
- [x] List Widget / List View — `ListView` (egui::ScrollArea + labels from options)
- [x] Tree Widget / Tree View — `TreeView` (egui::CollapsingHeader hierarchy)
- [x] Table Widget / Table View — merged into `Table` above

### New Widget Kinds — Additional Containers & Buttons
- [x] Stacked Widget — `StackedWidget` container (active-page preview)
- [x] Tool Box — `ToolBox` (vertical collapsing sections)
- [x] Tool Button — `ToolButton` (egui small_button)
- [x] Command Link Button — `CommandLinkButton` (title + description)
- [x] Dialog Button Box — `DialogButtonBox` (button row from options)

---

## Stage 11 — Rust-Centric Visual Features
- [ ] Ownership visualization — canvas overlay showing which widgets own which AppState fields
- [ ] Async task wiring — visually connect a widget event to an async fn,
      generates tokio::spawn or similar
- [ ] Channel connections — draw mpsc::channel connections between components visually
- [ ] Error propagation — visual Result/Option flow from widget events through state
- [ ] Iterator pipeline builder — chain .map/.filter/.collect operations visually,
      generates correct iterator code
- [ ] Trait binding — assign a trait implementation to a widget's behavior visually
- [ ] Macro palette — common Rust macros (vec!, format!, println!) as droppable canvas
      components that wire into event handlers

---

## Stage 12 — Platform Targets
- [ ] WASM export panel in File menu
- [ ] Configure: output path, bundler (trunk/wasm-pack), generate index.html toggle
- [ ] Generates `cargo build --target wasm32-unknown-unknown` compatible project
- [ ] Web-specific widget considerations (no file dialogs, no native paths)
- [ ] Preview in browser button — runs trunk serve

---

## Stage 13 — Data & Integration
- [ ] DB connection configurator — SQLite/PostgreSQL/MySQL
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

## Stage 15 — Own Renderer
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
