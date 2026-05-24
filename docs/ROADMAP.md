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
- [x] Lazare round-trip for Custom widgets: geometry already works; label/binding
      round-trip requires parser to understand descriptor template structure
- [ ] In-app `.rkwd` editor: create / edit descriptors from within RohKai
- [ ] `.rkwb` bundle format — zip of multiple `.rkwd` + preview SVGs + assets

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
- [x] `SvgRenderOutput` / `SvgRenderReport` API with rendered/skipped counts,
      unsupported-feature diagnostics, raster-size warnings, and fidelity
      scoring
- [ ] SVG renderer scene/display-list IR split
- [ ] Golden renderer fixture harness for supported raster output
- [ ] Shared SVG microsyntax module for importer/rasterizer parity

## Future Considerations

### Rulers & Measurement
- [ ] Horizontal ruler along top of canvas
- [ ] Vertical ruler along left of canvas
- [ ] Rulers show units in pixels, update with zoom
- [ ] Click ruler to create a persistent guide line
- [ ] Guide lines are draggable, deletable (Delete key when selected)
- [ ] Toggle rulers with Ctrl+R

### Document Presets & Real Window Sizing
- [ ] Document preset picker: common screen sizes
      (1920x1080, 2560x1440, 1366x768, 1280x720)
- [ ] Mobile presets (375x812 iPhone, 390x844 etc)
- [ ] Custom size with lock aspect ratio toggle
- [ ] Canvas represents actual app window —
      shows title bar chrome, minimize/maximize/close buttons
      as a visual bezel around the canvas area
- [ ] Window appearance settings: title, icon,
      resizable toggle, min/max size constraints
- [ ] All window settings stored in AppProps and used in export

### Application Appearance & Theming
- [ ] Theme panel: dark/light mode toggle
- [ ] Accent color picker for the generated app
- [ ] Font size base setting
- [ ] Widget rounding (global corner radius)
- [ ] Spacing/padding scale
- [ ] Theme exported as startup code in generated app:
      ctx.set_visuals(egui::Visuals { ... })
- [ ] Save themes as .rktheme files
- [ ] Apply a theme to the Rohkai designer itself

### Lazarus Features — Remaining
- [ ] Design-time non-visual components — timers,
      data sources, app lifecycle represented as
      clickable icons on a component tray below canvas
- [ ] Full event list per widget — not just OnClick/OnChange
      but all applicable egui events for that widget kind
- [ ] Object Inspector true bidirectionality —
      editing any property field updates canvas immediately
      with no lag or focus loss

### Technical & Computational Widgets
- [ ] Math/formula widget — displays computed value
      from an expression bound to AppState
- [ ] Timer/interval component — fires event on schedule,
      non-visual, lives in component tray
- [ ] Data table widget — tabular display, bound to Vec<T>
- [ ] File picker widget — browse filesystem, returns path
- [ ] Tree view widget — hierarchical data display
- [ ] Chart widget — 2D line/bar chart bound to Vec<f32>
- [ ] HTTP request component — non-visual, configure
      URL/method/headers, bind response to state
- [ ] State machine component — define states and
      transitions visually, generates match-based logic

### Rust-Centric Visual Features
- [ ] Ownership visualization — canvas overlay showing
      which widgets own which AppState fields
- [ ] Async task wiring — visually connect a widget event
      to an async fn, generates tokio::spawn or similar
- [ ] Channel connections — draw mpsc::channel connections
      between components visually
- [ ] Error propagation — visual Result/Option flow
      from widget events through state
- [ ] Iterator pipeline builder — chain .map/.filter/.collect
      operations visually, generates correct iterator code
- [ ] Trait binding — assign a trait implementation to a
      widget's behavior visually
- [ ] Macro palette — common Rust macros (vec!, format!,
      println!) as droppable canvas components that wire
      into event handlers

### WASM Export & Web Target
- [ ] WASM export panel in File menu
- [ ] Configure: output path, bundler (trunk/wasm-pack),
      generate index.html toggle
- [ ] Generates cargo build --target wasm32-unknown-unknown
      compatible project
- [ ] Web-specific widget considerations (no file dialogs etc)
- [ ] Preview in browser button — runs trunk serve

### Database Integration Panel
- [ ] DB connection configurator — SQLite/PostgreSQL/MySQL
- [ ] Uses sqlx or rusqlite crate (user choice)
- [ ] Visual query builder — select table, columns, filter
- [ ] Bind widget to query result field
- [ ] Generated code uses correct Rust DB crate with
      async/sync query calls
- [ ] Schema viewer — see tables and fields visually
- [ ] Generates AppState with db connection pool field

### Project Tree & File Browser
- [ ] Project tree panel — shows all files in the
      exported project structure
- [ ] Click a file to view/edit its generated content
- [ ] Add non-generated files to project (assets, configs)
- [ ] Assets folder management — images, fonts, data files
      referenced in generated code by path

## Future - Viable Widget Palette

Planning inspired by mature desktop GUI designers. These are roadmap targets, not current scope.

### Layouts
- [ ] Vertical Layout
- [ ] Horizontal Layout
- [ ] Grid Layout
- [ ] Form Layout

### Spacers
- [ ] Horizontal Spacer
- [ ] Vertical Spacer

### Buttons
- [ ] Push Button
- [ ] Tool Button
- [ ] Radio Button
- [ ] Check Box
- [ ] Command Link Button
- [ ] Dialog Button Box

### Containers
- [ ] Group Box
- [ ] Scroll Area
- [ ] Tab Widget
- [ ] Stacked Widget
- [ ] Tool Box
- [ ] Frame

### Inputs
- [ ] Combo Box
- [ ] Font Combo Box
- [ ] Line Edit
- [ ] Text Edit
- [ ] Numeric / spinner controls

### Item Widgets and Views
- [ ] List Widget / List View
- [ ] Tree Widget / Tree View
- [ ] Table Widget / Table View

### Later / High Risk
- [ ] Model-based item views
- [ ] Dock Widget
- [ ] MDI Area
- [ ] QAxWidget-style platform integrations (likely not compatible with RohKai's pure Rust / no C FFI rule)
