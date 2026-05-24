# RohKai — Pure Rust WYSIWYG egui UI Designer

## Project Location
All work lives at: D:\dev\rohkai
Never write files to any other path.
Never use "Claude Code Projects" or Downloads as a project path.
If CWD is not D:\dev\rohkai, cd there before doing anything.

## What This Is
A native Rust desktop app (built with egui) that lets users visually design
egui UIs with zero gap between canvas and code. Drag a widget → code appears.
No separate signal-wiring step. Canvas and code panel are live views of one
source of truth: the `UiTree`.

The name RohKai evokes Rocaille — the ornamental scroll-and-shell motif of
French Rococo. The designer should feel precise and elegant.

## The Core Principle
`UiTree` (in `src/project/ui_tree.rs`) is the single source of truth.
The canvas renders it. The code panel emits Rust from it. They never diverge.
Never write code that mutates canvas state and code state separately.

## Architecture Rules
- NO C FFI. NO system toolkit bindings. Pure Rust crates only.
- All dependencies via Cargo. No cmake, no pkg-config.
- "Pure Rust crate" does not mean "approved dependency." Do not add new crates
  unless the user explicitly approves that exact dependency by name.
- SVG import, SVG image preview, and SVG raster/vector work are zero-new-crate
  zones: no `resvg`, no `usvg`, no `tiny-skia`, and no substitute renderer
  dependency chain. Implement required SVG behavior in RohKai source.
- No hollow feature surfaces: canvas, properties, code panel, export, docs, and
  tests must all expose a real output form or the feature is not done.
- Rendering stack: egui + eframe (winit + wgpu under the hood).
- Codegen lives entirely in `src/codegen/`. Nothing outside that module
  should know about Rust syntax strings.
- UiTree nodes are serde-serializable. Project files are `.rohkai.json`.

## Module Map
```
src/
  main.rs          — entry point, eframe bootstrap, icon rasteriser
  app.rs           — RohKaiApp struct, implements eframe::App, all panel wiring
  canvas/          — drag/drop canvas (renders UiTree), pan/zoom, smart guides
  widgets/         — one file per WidgetKind: default instance + palette defaults
  codegen/         — egui_emitter, state_emitter, export, parser (Lazare), rust utils
  project/         — schema types, UiTree, io (save/load/serialize)
  panels/          — palette, properties, code_preview, templates
  settings.rs      — UserSettings: load/save to APPDATA/RohKai/settings.json
  svg_import.rs    — SVG → WidgetInstance parser (zero new dependencies)
```

## Current Stage (see docs/ROADMAP.md for full history)
Stages 1–6 complete. Active scope is Stage 7 (widget descriptor format / .rkwd files).

Core features implemented:
- Canvas: drag, drop, select, multi-select, resize, rubber-band, z-order, snap, smart guides
- Widgets: Button, Label, TextInput, Slider, Checkbox, Frame, ComboBox, RadioButton, ProgressBar
- Properties panel: label, binding, geometry, alignment, group/ungroup
- Code panel: live egui Rust output — **editable** (Lazare bidirectional sync, Stage 6)
- AppState panel: auto-generated struct fields
- Save/load `.rohkai.json` (versioned envelope, legacy bare UiTree supported)
- Export: complete compilable Rust project
- Templates: `.rktp` files, SVG import, drag-to-canvas
- Preferences: UI scale, font size, snap step (persisted)

## What NOT to build yet
- Multi-window support
- Custom widget creation
- Undo/redo (design for it, don't implement yet)
- Themes / styling beyond basics
- Any codegen target other than egui

## Rust Patterns We Use
- `RohKaiApp` struct holds all designer state — no globals
- egui immediate mode throughout — no retained widget objects
- serde + serde_json for project serialization
- No async (unnecessary for this app)

## Running
    cargo run

## Testing
    cargo test
    cargo clippy -- -D warnings

## Session Rules
- Before planning or coding, run or manually follow the repo preflight:
  `powershell -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1`
- Preflight means reading `AGENTS.md`, `CLAUDE.md`, `docs/ROADMAP.md`, the latest
  `docs/DEVLOG.md` entry, `docs/CODE_INDEX.md`, the latest `docs/CODE_COOP.md`
  note, `git status --short --branch`, and any relevant `.claude/skills/*/SKILL.md`
  files before edits.
- At the start of a meaningful planning or coding session, append a 3-4 sentence
  `docs/CODE_COOP.md` note for the next agent.
- For SVG/Image work, `svg-zero-dep` is a relevant skill and must be read.
- Do not add `CONTRIBUTING.md` to preflight/prep unless the user explicitly asks
  for contribution-policy work.
- Record meaningful sessions in `docs/DEVLOG.md`: time, docs reviewed, changes,
  verification, risks, and follow-ups.
- `docs/ROADMAP.md` is strategic stage planning. `docs/DEVLOG.md` is chronological.
  `docs/ARCHITECTURE.md` is structural truth, not a timeline.
- Every session ends with `cargo run` confirming a clean launch.
- Zero warnings is required before any session is considered done.
- Always work in D:\dev\rohkai. Never write to any other path.
- If CWD is not D:\dev\rohkai, cd there before doing anything.
- If you suspect context loss, run /restore before doing anything else.
