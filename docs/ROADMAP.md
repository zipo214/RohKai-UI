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
- [ ] SVG import as template placeholder — deferred (requires SVG parser)

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

## Stage 6 — Bidirectional Sync
- [ ] Code panel becomes editable
- [ ] Edits parsed back into UiTree using a minimal Rust expression parser
- [ ] Canvas updates live when code changes
- [ ] Conflicts (invalid syntax) shown as inline error, canvas unchanged

## Stage 7 — Framework Import / Ply Support
- [ ] Widget descriptor format (.rkwd — RohKai Widget Definition)
- [ ] Descriptor defines: name, properties, codegen template string
- [ ] Import a .rkwd file → widget appears in palette
- [ ] Ply widget definitions as first shipped example
- [ ] Community .rkwd files can be dropped into a /widgets folder
