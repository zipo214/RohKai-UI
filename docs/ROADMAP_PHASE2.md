# RohKai Phase 2 Roadmap

Phase 2 starts after the v0.2.0 release. It collects every unchecked roadmap
item, every deferred recommendation, every "Later:" note, every design gap
surfaced in feature evaluations, and every unorganized idea — regardless of
which document they originally lived in.

The **sorted** section groups items by theme so they can be staged into
releases. The **unsorted scratchpad** at the bottom captures anything that does
not have a home yet. Cross items from the sorted section into stage milestone
documents as they are scheduled.

---

## Priority Starter Lines

These two threads open Phase 2. They unblock the most downstream features.

### P2-A — Font Shaping Engine

**Goal:** Build a HarfBuzz shaping algorithm port in Rust that passes **2252
of 2252** Unicode shaping tests.

No new crates: this must be a pure-Rust, zero-C-dependency implementation living
in `src/codegen/shaper/` or `src/canvas/shaper/`, embedded under the same
single-`crate::` contract as the SVG rasterizer. The implementation must handle:
- OpenType GSUB/GPOS lookup tables (single, pair, contextual, chained)
- Arabic/Hebrew joining and right-to-left shaping
- Indic scripts (Devanagari, Bengali, Tamil) via sequence reordering and
  Matra/Virama handling
- Ligature substitution (Latin: fi/fl, Arabic: lam-alef)
- Mark-to-base and mark-to-mark attachment
- Kerning (GPOS pair adjustment)
- Unicode bidirectional algorithm (UAX #9) full implementation
- CJK trivial shaping (no ligatures, no marks)

Quality gate: run the full HarfBuzz Unicode shaping test suite (2252 tests,
see `hb-shape --verify` corpus) against the port's output. The port ships only
when `cargo test -- shaping` passes all 2252. This unlocks real font-file
rendering in the SVG rasterizer and the code panel.

Reference: [HarfBuzz shaping tests](https://github.com/harfbuzz/harfbuzz/tree/main/test/shaping/data/text-rendering-tests)

### P2-B — Database Integration Research

**Goal:** Research and propose a concrete DB integration design before writing
any code (Stage 13 unblock).

Deliverables:
- Comparison matrix: `rusqlite` vs `sqlx` vs `sea-orm` — async model,
  compile-time query checking, connection-pool overhead, WASM compatibility.
- Decision: which crate(s) to approve for Stage 13 widget-to-DB binding.
- Threat model: user-entered SQL in the query builder, injection surface,
  sandboxing in the designer vs exported project.
- Sketch of the generated AppState shape for a typical DB-backed view.

No code until the crate is explicitly user-approved.

---

## P2.1 — Visual Widget Maker (Later Capabilities)

Continues from the MVP (commit 4f60e72). See `docs/VISUAL_WIDGET_MAKER.md`.

- [x] Z-order reorder of primitives — ↑/↓ buttons in the primitive list panel;
      mini-canvas re-renders immediately in the new order.
- [x] Primitive constraints: anchor (TL/TR/BL/BR/Center), min_w, min_h —
      `PrimAnchor` enum + `min_w`/`min_h` fields on `MakerPrimitive`; exposed
      in the properties panel; `apply_corner_resize` clamps to min_w/min_h.
- [x] Codegen preview: "Code Preview" tab beside Properties shows live
      `live_preview` and `export template` strings, updated every frame.
- [x] Round-trip: `doc_from_descriptor` in `widget_maker.rs` reconstructs a
      `WidgetMakerDoc` (metadata only) from any descriptor whose
      `live_preview` starts with `"    {"` (the VWM sentinel).
- [x] Hit regions: interactive zones that receive click/hover/drag events,
      distinct from visual shapes (a rect can be a hit region without being visible)
- [ ] Layout groups: horizontal, vertical, grid, stack inside the maker canvas
- [x] State variants: normal, hover, pressed, disabled, checked — each variant
      carries independent primitive style overrides
- [ ] Slots: named child content areas that accept widget instances at canvas time
- [x] Event zones: named interactive areas that emit click/change/custom signals
      (implemented via HitRegion primitives; see Hit regions above)
- [x] Style tokens: accent color, border color, corner radius, text color,
      spacing — expose as a property group in the properties panel

---

## P2.2 — Canvas UX Depth

From Stage 8.5 comparative analysis and feature evaluation gaps.

- [x] Widget naming convention: auto-generate meaningful labels on drop
      (`button_1`, `label_1`); per-kind counter in `RohKaiApp::name_counter`;
      counter resets on New project. (feat(P2.2))
- [x] Zoom to selection: `F` key fits the selected widget(s) in view with 10 %
      padding; `F` with nothing selected fits all canvas widgets.
      `compute_fit_rect` in `src/canvas/interaction.rs`. (feat(P2.2))
- [x] Property reset: right-click the Label field or any geometry (x, y, w, h)
      field in the Properties panel → "Reset to default". Label → kind name;
      x/y → 0; w/h → widget-kind default. (feat(P2.2))
- [x] Error highlighting: red 2 px outline on canvas widgets with duplicate ID,
      invalid handler name, or missing binding. `compute_widget_errors` in
      `src/canvas/interaction.rs`. (feat(P2.2))

**Deferred from this batch (not implemented):**
- [ ] Search in canvas: Ctrl+F to find widgets by name, kind, or property value
      (separate from the code panel search) — deferred: architecturally complex,
      overlaps with Lazare search.
- [ ] Clipboard enhancements: cross-session clipboard is risky; paste-multiple
      and paste-at-cursor are scoped separately — deferred.
- [ ] Minimap: small overview of the entire canvas in a corner panel — deferred:
      requires retained off-screen render pass, too complex for this batch.
- [ ] Multi-select property editing: edit the same property across all selected
      widgets simultaneously — deferred: architecturally complex.
- [ ] Context tooltips: hover any designer UI element to see its purpose —
      deferred: broad scope, incremental addition.

---

## P2.3 — Constraint-Based Layout

From Stage 9.5 comparative analysis (Qt Auto Layout / iOS Auto Layout class).

- [ ] Horizontal and vertical constraint rules (leading/trailing/top/bottom)
- [ ] Center-alignment constraints (horizontal center, vertical center)
- [ ] Equal-size constraints between two widgets
- [ ] Aspect-ratio constraints (lock w:h ratio)
- [ ] Min/max size constraints per widget
- [ ] Margin and padding visual editor (inset handles)
- [ ] Anchor system with visual handle drag
- [ ] Constraint validation: detect conflicting or unsatisfiable constraints
- [ ] Responsive size class breakpoints (e.g., compact vs. regular)
- [ ] Layout preview: scrub canvas size to see layout reflow live
- [ ] Layout templates: save and reuse constraint presets
- [ ] Nested layout hierarchy round-trip in Lazare

---

## P2.4 — Stage 9 Layout Deferred Items

From ROADMAP.md Stage 9 remaining `[ ]` items.

- [ ] Properties panel exposes per-child alignment, grid row height policies,
      and per-child stretch/fixed-size behavior
- [ ] Hit testing and rubber-band selection are layout-aware (select only
      children of a clicked layout container, not a flat z-order hit)
- [ ] Richer drag-reorder: drag a layout child to reorder within its container
      with animated placeholder feedback
- [ ] Richer cell/slot editor for Grid: named slots, drag-to-slot behavior,
      and multi-level layout hierarchy tests

---

## P2.5 — Widget & Component Depth

From Stage 10 and feature evaluation remaining gaps.

- [ ] Formula Widget full depth: dependency tracking, multi-input fields,
      validation, formatting decimals/prefix/suffix, live expression diagnostics
- [ ] Timer component: runtime tick scheduling via mpsc channel; wired to
      eframe's `request_repaint_after`
- [ ] State machine component: visual transition graph editor, guard conditions,
      entry/exit actions
- [ ] HTTP request component: runtime execution using user-approved crate
      (reqwest / ureq); response parsing; error UI; requires Stage 13 crate approval
- [ ] Data-bound Table/ListView/TreeView: real model with `data_source_binding`,
      virtual scroll, sort, filter
- [ ] Interactive chart: axes, series editor, legend, zoom/pan, data model
      binding; requires user-approved charting crate
- [ ] `.rkwb` descriptor bundle: zip of multiple `.rkwd` + preview SVGs +
      assets + manifest; installer UI with conflict resolution
- [ ] Keyboard shortcut customization: user-configurable shortcuts in preferences

---

## P2.6 — Stage 13 — Data & Database Integration

Blocked until crate is user-approved (P2-B research must complete first).

- [ ] DB connection configurator — SQLite / PostgreSQL / MySQL / Supabase
- [ ] Visual query builder — select table, columns, WHERE filter
- [ ] Widget-to-query-result binding (field name → widget binding)
- [ ] Schema viewer: see tables and columns visually in a side panel
- [ ] Generates `AppState` with a connection pool field and async query calls
- [ ] Generated code uses correct Rust DB crate with documented dependency line
- [ ] Design-time data preview: show sample rows without runtime execution

---

## P2.7 — SVG Renderer Remaining Lanes

From `docs/SVG_RENDERER_ROADMAP.md` deferred items.

- [ ] **Filter tier-3** (identity passthrough + diagnostic currently):
      `feTile`, `feImage`, `feTurbulence`, `feDisplacementMap`,
      `feConvolveMatrix`, `feDiffuseLighting`, `feSpecularLighting`
- [ ] **Progressive JPEG** (SOF2): currently diagnosed; implement
      progressive scan decoding
- [ ] **Real font-file glyph rendering**: parse TrueType/OpenType `.ttf`/`.otf`
      within the zero-dependency profile (no fontdue/rusttype); or gate on the
      P2-A shaping engine completing first
- [ ] **Full shaping and BIDI** in SVG rasterizer: Arabic, Hebrew, Indic scripts;
      depends on P2-A shaping engine
- [ ] **ICC colour management**: parse embedded ICC profiles; map to sRGB
- [ ] **`foreignObject`**: render embedded HTML content as a rasterized blob or
      diagnosed stub
- [ ] **`SvgRenderOptions` struct**: add only if scene split needs caller-controlled
      rendering options
- [ ] **R8.2 deep-fuzz hardening** (optional): 50k+ iteration fuzz pass with
      structured mutation over the full W3C corpus
- [ ] **SMIL animation** (long range): `animate`, `animateTransform`,
      `animateMotion` — requires a clock/repaint loop
- [ ] **CSS animation / transitions** (long range): `@keyframes`, `transition`
- [ ] **Scripting** (deliberately out of scope / security): JavaScript/ECMAScript
      execution is not planned; document explicitly as unsupported

---

## P2.8 — High-Risk / Long-Range Widgets

Requires feasibility decision before implementation.

- [ ] Model-based item views (MVC tree, virtual list with 100k+ items)
- [ ] Dock Widget (dockable panels, split views, tab groups)
- [ ] MDI Area (multiple document interface with floating sub-windows)
- [ ] Multi-window support (secondary eframe windows)
- [ ] QAxWidget-style platform integrations (native controls via platform APIs)

---

## P2.9 — Code Panel & Codegen Depth

- [ ] Codegen memoization: `CodegenCache` keyed on `UiTree` hash; skip
      re-emit when tree did not change (Cline Rec 3)
- [ ] Dirty rectangle rendering: only re-rasterize changed regions of the canvas
      (Cline Rec 7 — complex, deferred)
- [ ] Handler rename: rename a handler in the code panel and propagate the
      rename to all widgets that reference it
- [ ] Handler deduplication: warn when two widgets share the same handler name
      but have different event kinds
- [ ] Diff view: show a diff between the current generated code and the last
      committed/saved state
- [ ] Custom error types with `thiserror` (Cline Rec 4 — requires crate approval)
- [ ] VSCode-style IntelliSense for the code panel: Rust keyword completion,
      local binding suggestions, handler autocomplete

---

## P2.10 — Platform Targets

From Stage 12 remaining gaps and WASM depth.

- [ ] WASM: unsupported-widget diagnostic report (list widgets that have no
      WASM-safe codegen path)
- [ ] WASM: in-app build status panel showing trunk build output
- [ ] WASM: size budget report (`.wasm` binary size estimate)
- [ ] Native desktop packaging: `.deb` / `.rpm` / `.msi` / `.dmg` generation
- [ ] iOS/Android research (WASM + WebView bridge or pure egui mobile support)

---

## P2.11 — Accessibility & Internationalisation

- [ ] ARIA role annotations on exported egui widgets (screen reader support)
- [ ] RTL canvas mode: flip canvas origin for right-to-left UI authoring
- [ ] Locale-aware string externalisation: export strings to a `strings.toml`
      and generate `t!("key")` calls
- [ ] Keyboard-only authoring: every canvas action achievable without a mouse

---

## P2.12 — Stage 15 — Own Renderer

Final ordered stage. Starts only when P2.1–P2.11 are complete and a separate
architecture decision is ratified. "Final" means last in this list, not last
in the project's lifetime — the list grows as new stages are added above it.

- [ ] Replace egui rendering layer with RohKai-owned pure Rust renderer
- [ ] Widget descriptor format drives renderer widget model directly
- [ ] Zero transient C dependencies in the rendering stack
- [ ] Custom layout engine with constraint and flex support
- [ ] GPU-accelerated rasterizer (wgpu-based or own path rasterizer)
- [ ] All previously constrained visual properties become available:
      arbitrary shapes, gradients, shadows, blend modes per widget
- [ ] Text pipeline uses P2-A shaping engine end-to-end

---

## Unsorted Scratchpad

Everything below is captured verbatim and has not been assigned to a section
yet. Pull items up into a sorted section as soon as they have a concrete plan.

- Design token system: named color/spacing/radius variables that replace
  hard-coded values throughout the widget tree; export as CSS custom properties
  or Rust constants
- Color theme editor: visual editor for the `.rktheme` format; live preview
  across all palette widgets; export as Figma-compatible token JSON
- Template marketplace: upload/download `.rktp` template packs from a URL;
  import validation + conflict resolution
- Component library sharing: publish a set of `.rkwd` descriptors as a named
  library; version pinning
- Multiplayer / collaboration: two designers on the same `.rohkai.json`
  simultaneously; CRDT-based merge (very far future)
- Design-to-code diff: given a Figma/Sketch import and a live canvas, highlight
  which widgets diverge from the design
- Visual state preview: click into hover/pressed/disabled state in the designer
  without running the app
- Pixel-perfect grid snapping modes: isometric, 8pt grid, 4pt grid, custom
- Canvas annotations: sticky notes, margin comments, redline measurements
- Performance profiling overlay: show per-widget repaint cost in designer mode
- Export to SwiftUI / Compose / React Native stubs (long range)
- Localization workflow: mark strings as translatable in the designer; generate
  `.po` / `.ftl` / `.arb` files
- Custom font loading in designer: load a `.ttf`/`.otf` and preview text widgets
  with that font immediately
- Widget property inspector scripting: run a user Lua/Rhai script over the
  selected widget's props for batch editing
- Smart layout suggestions: AI-assisted layout reflow proposals when widgets
  overlap or overflow
- Undo/redo command pattern (full): replace snapshot undo with true command
  objects for granular undo steps and better memory efficiency
- Canvas ruler guides: drag from ruler to create named guide lines that snap
  all widgets; save guides in the project file (partially done — named guides;
  persist guides across save/load TBD)
- Pressure-sensitive canvas input: support stylus/tablet pressure for
  future freehand drawing tools
- Dark mode canvas: separate dark/light canvas background from app theme

---

## Completed in Phase 1 (v0.1.0 → v0.2.0)

Cross-referenced for context. Full history in `docs/DEVLOG.md` and git log.

| Area | Completion |
|---|---|
| Canvas: drag, select, resize, rubber-band, z-order, snap, smart guides, rulers | ✅ |
| Widgets: Button, Label, TextInput, Slider, Checkbox, Frame, ComboBox, RadioButton, ProgressBar, TextArea, SpinBox, FontComboBox, GroupBox, VLayout, HLayout, ScrollArea, GridLayout, TabWidget, ToolButton, CommandLinkButton, DialogButtonBox, MathLabel, FilePicker, Chart, Table, ListView, TreeView, StackedWidget, ToolBox, Image | ✅ |
| Custom widgets: `.rkwd` descriptor format, Advanced Editor, Guided Builder, Visual Maker MVP | ✅ |
| Codegen: live egui Rust output, Lazare bidirectional sync, Ctrl+F search, symbol list, clickable diagnostics | ✅ |
| Export: complete compilable Rust project, WASM export, browser preview | ✅ |
| SVG renderer: R0–R12 complete (geometry, gradients, clip, masks, filters tier-1+2, patterns, markers, text, namespace recovery) | ✅ |
| Stage 14: snapshot undo/redo | ✅ |
| Stage 11: async task wiring, Rust wiring panel, iterator/trait snippets, Object Inspector | ✅ |
| Engineering invariants: 9 bug-class guards, 412 tests, zero clippy warnings | ✅ |
