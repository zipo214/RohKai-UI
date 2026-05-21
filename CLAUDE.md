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
- Rendering stack: egui + eframe (winit + wgpu under the hood).
- Codegen lives entirely in `src/codegen/`. Nothing outside that module
  should know about Rust syntax strings.
- UiTree nodes are serde-serializable. Project files are `.rohkai.json`.

## Module Map
```
src/
  main.rs          — entry point, eframe bootstrap
  app.rs           — RohKaiApp struct, implements eframe::App
  canvas/          — drag/drop canvas (renders UiTree)
  widgets/         — palette widget definitions + defaults
  codegen/         — walks UiTree, emits Rust strings
  project/         — UiTree data model + serde schema
  panels/          — egui panel UIs (palette, properties, code)
```

## Current MVP Scope (do not scope-creep)
- [ ] Canvas: drag widgets from palette, place, select, move, resize
- [ ] Widgets: Button, Label, TextInput, Slider, Checkbox
- [ ] Properties panel: label, size, position, state binding
- [ ] Code panel: live egui Rust output, read-only, monospace
- [ ] State panel: auto-generated AppState struct fields
- [ ] Save/load `.rohkai.json` project files

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
- Every session ends with `cargo run` confirming a clean launch.
- Zero warnings is required before any session is considered done.
- Always work in D:\dev\rohkai. Never write to any other path.
- If CWD is not D:\dev\rohkai, cd there before doing anything.
