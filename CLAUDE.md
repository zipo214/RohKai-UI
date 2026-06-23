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
source of truth: the active surface tree inside `ProjectDocument`.

The name RohKai evokes Rocaille — the ornamental scroll-and-shell motif of
French Rococo. The designer should feel precise and elegant.

## The Core Principle
`ProjectDocument` (in `src/project/document.rs`) is the project source of truth.
Each `UiSurface.tree` is the sole canvas/code source for that surface.
`ActiveDocument` exposes the active tree without copying it. Never mutate
canvas/code state separately or duplicate project-global state into surfaces.

## Architecture Rules
- NO C FFI. NO system toolkit bindings. Pure Rust crates only.
- All dependencies via Cargo. No cmake, no pkg-config.
- "Pure Rust crate" does not mean "approved dependency." Do not add new crates
  unless the user explicitly approves that exact dependency by name.
- `rayon = "1"` is an approved dependency for parallel processing. Planned
  future replacement with `src/platform/thread_pool.rs` when Stage 15 renderer
  needs its own scheduler.
- `rustybuzz` is an approved dependency for font shaping (P2-A interim shaper).
  Wire behind a `ShaperEngine` trait in `src/canvas/shaper/` so it can be
  swapped out for a bespoke HarfBuzz port later. Do NOT embed it in the
  export-embedded svg_rasterizer.rs (that source must remain std-only).
- `rusqlite = { version = "0.40", features = ["bundled"] }` is an approved
  dependency for Stage 13 / P2.6 database integration. Use only behind a
  `DatabaseEngine` trait (see `docs/DB_INTEGRATION_RESEARCH.md` Section 7).
  Never add to Cargo.toml until P2.6 implementation actually begins. SQL
  codegen must never use `format!()` to build SQL — all values via `params![]`
  (Invariant 10, pending addition to ENGINEERING_INVARIANTS.md).
- SVG import, SVG image preview, and SVG raster/vector work are zero-new-crate
  zones: no `resvg`, no `usvg`, no `tiny-skia`, and no substitute renderer
  dependency chain. Implement required SVG behavior in RohKai source.
- No hollow feature surfaces: canvas, properties, code panel, export, docs, and
  tests must all expose a real output form or the feature is not done.
- Rendering stack: egui + eframe (winit + wgpu under the hood).
- Codegen lives entirely in `src/codegen/`. Nothing outside that module
  should know about Rust syntax strings. Generated identifiers must be valid
  (no leading digit), keyword-escaped, collision-resistant (deterministic), and
  must `cargo check` in the exported project.
- Surface parity: a behavior visible in one of {canvas, preview, export} must be
  matched in the others or carry an explicit, tested reason for differing.
  Derive classifications from the canonical API (`UiTree`,
  `WidgetKind::supported_events()`); never re-list its members elsewhere.
- ProjectDocument/UiTree nodes are serde-serializable. Project files are
  `.rohkai.json`.

## Module Map
```
src/
  main.rs          — entry point, eframe bootstrap, icon rasteriser
  app.rs           — RohKaiApp struct, implements eframe::App, all panel wiring
  canvas/          — drag/drop canvas (renders UiTree), pan/zoom, smart guides
  widgets/         — one file per WidgetKind: default instance + palette defaults
  codegen/         — egui_emitter, state_emitter, export, parser (Lazare), rust utils
  project/         — ProjectDocument, surfaces, schema, UiTree, io
  panels/          — surfaces, palette, properties, code_preview, templates
  settings.rs      — UserSettings: load/save to APPDATA/RohKai/settings.json
  svg_import.rs    — SVG → WidgetInstance parser (zero new dependencies)
```

## Current Stage (see docs/ROADMAP.md for full history)
Stages 0–8, 8.5, 9 (core), 10, 11, and 14 complete. Stage 9's parallel-processing
sub-cluster (rayon-based parallel rasterization/codegen/export/template-load +
benchmarks) and Form Layout remain deferred. Open stages: 12 (Platform Targets),
13 (Data & Integration), 15 (Own Renderer), plus the deferred Stage 9 items.
S19A project surfaces and modal dialogs is implemented at the competitive modal
subset; S19B-D modeless/native windows, main-window docking, and MDI remain
planned. Current focused work remains the pre-release depth gate; Stage 15 is a
separate final architecture decision.

Core features implemented:
- Canvas: drag, drop, select, multi-select, resize, rubber-band, z-order, snap, smart guides,
  pixel rulers, guide lines, guide snapping
- Widgets: Button, Label, TextInput, Slider, Checkbox, Frame, ComboBox, RadioButton, ProgressBar
- Custom widgets: `.rkwd` descriptor format, in-app editor, beginner builder, hot-reload
- Properties panel: label, binding, geometry, alignment, group/ungroup, events, custom props
- Code panel: live egui Rust output — **editable** (Lazare bidirectional sync, Stage 6)
- AppState panel: auto-generated struct fields
- Save/load schema-v2 multi-surface `.rohkai.json` (legacy bare/v1 supported)
- Project surfaces: root form + modal dialogs, tabs/CRUD/templates, typed
  lifecycle behaviors, transactional preview, native/WASM export
- Export: complete compilable Rust project (with theming, presets, descriptor cargo deps)
- Templates: `.rktp` files, SVG import, drag-to-canvas
- Theming: dark/light, accent color, font size, corner radius, spacing — saved as `.rktheme`
- Preferences: UI scale, font size, snap step (persisted)

## What NOT to build yet
- Modeless/native secondary windows, docking, and MDI - planned S19B-D. In-app
  modal project surfaces are implemented and are not native child windows.
- Non-egui codegen targets beyond the existing egui native/WASM exporters —
  planned Stage 12; do not start before that scoped platform-target work begins
- Database integration — planned Stage 13; requires user-approved crate at stage start
- Own renderer — planned Stage 15; do not touch rendering stack before then

## Rust Patterns We Use
- `RohKaiApp` struct holds all designer state — no globals
- egui immediate mode throughout — no retained widget objects
- serde + serde_json for project serialization
- No async in the core designer loop (canvas, codegen, UiTree mutations stay
  synchronous). Background tasks use `std::sync::mpsc` channels. No tokio
  runtime unless a specific planned feature explicitly requires it.

## Running
    cargo run

## Rust Version Contract
- RohKai source edition: Rust 2024.
- Minimum supported Rust version (MSRV): 1.92.
- Pinned and CI-tested toolchain: 1.96.0 via `rust-toolchain.toml`.
- Generated projects intentionally use edition 2021, but share the 1.92 MSRV
  and the designer's egui/eframe/rfd versions.
- Run `scripts/check-toolchain-alignment.ps1` after version changes and
  `scripts/audit-dependency-updates.ps1` when checking for newer releases.

## Testing
    cargo test
    cargo clippy --all-targets -- -D warnings

`--all-targets` is required: plain `cargo clippy` skips `examples/` and `tests/`,
where real lints have hidden.

## Session Rules
- Before planning or coding, run the low-token preflight:
  `pwsh -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1`
- Default context is intentionally small: this file, the preflight output,
  `git status --short --branch`, latest `docs/CODE_COOP.md` note, and relevant
  skills. Read heavier docs only when needed:
  - `docs/ROADMAP.md` for scope/stage decisions.
  - `docs/CODE_INDEX.md` for codebase orientation.
  - `docs/ARCHITECTURE.md` for structural changes.
  - `docs/ENGINEERING_INVARIANTS.md` when fixing a bug or reviewer finding, or
    touching codegen, preview/canvas/export parity, input gating, reset paths,
    string truncation, or filename/identifier sanitizing.
  - `docs/DEVLOG.md` for regression/history investigation; use preflight
    `-IncludeDevlog` when you need it.
- When fixing a bug or reviewer finding, fix the *class* not the symptom: follow
  the systemic-fix workflow in `docs/ENGINEERING_INVARIANTS.md` (root cause →
  sibling-surface parity → name/add the invariant → class-level regression test →
  minimal patch).
- Before coding feature or bug-fix work, derive the source-of-truth data model,
  all ownership/topology cases, all UI surfaces, all codegen/export/parser paths,
  and invariant tests for every required path. Do not mark complete from
  representative tests; if any required path is unknown or excluded, stop and
  report before editing. For `WidgetInstance.children`, use Engineering
  Invariant 13's topology matrix.
- At the start of a meaningful planning or coding session, append a 3-4 sentence
  newest-first `docs/CODE_COOP.md` note for the next agent.
- When writing a goal/prompt for another agent, use `docs/PROMPT_CONTRACT.md`
  so the task derives source-of-truth sets, enumerates all output paths, and
  requires invariant tests instead of fixing only the obvious surface.
- For SVG/Image work, `svg-zero-dep` is a relevant skill and must be read.
- SVG renderer roadmap step protocol: before starting ANY SVG renderer lane
  (R0–R8 and R8.1 are done; remaining lanes are R9, R10, R11, R12, plus the
  optional R8.2 deep-fuzz hardening lane that does not block the others), FIRST
  read that lane's paste-ready goal prompt in
  `docs/svg-goal-plan-prompts/<lane>-*.goal.md` (e.g. `R9-*.goal.md`) and follow
  it exactly: derive source-of-truth paths before coding, render in BOTH the
  in-app and export-embedded rasterizers, add golden/unit tests + diagnostics +
  caps, keep both embedded sources std-only with the single-`crate::` export
  contract, and end on the zero-warning verification gate. See
  `docs/svg-goal-plan-prompts/README.md` for the authoritative run order. Execute
  lanes in order; do not skip the prompt read.
- Record meaningful sessions in `docs/DEVLOG.md`: time, docs reviewed, changes,
  verification, risks, and follow-ups.
- `docs/ROADMAP.md` is strategic stage planning. `docs/DEVLOG.md` is chronological.
  `docs/ARCHITECTURE.md` is structural truth, not a timeline.
- Feature or behavior sessions end with appropriate cargo verification and, when
  practical, `cargo run` launch smoke. Docs-only sessions may use script/encoding
  checks plus `cargo fmt --check`/`cargo check` as appropriate.
- Zero warnings is required before any code session is considered done.
- Prefer `pwsh`/PowerShell 7 for repo scripts. Do not use Windows PowerShell 5.1
  text-writing commands for repo files.
- Do not use `Set-Content`, `Add-Content`, or `Out-File` without explicit
  `-Encoding utf8`. Prefer `apply_patch` for source edits.
- Always work in D:\dev\rohkai. Never write to any other path.
- If CWD is not D:\dev\rohkai, cd there before doing anything.
- If you suspect context loss, run /restore before doing anything else.
