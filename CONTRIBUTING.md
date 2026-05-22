# Contributing to Rohkai ^ρϗ

Thanks for looking. This is a focused tool — contributions that sharpen it are welcome. Contributions that bloat it are not.

---

## Before You Start

Read `docs/ARCHITECTURE.md`. Seriously. The single most important rule:

> **`UiTree` is the only source of truth.** The canvas renders it. The code panel emits Rust from it. They never diverge.

If your change mutates canvas state and code state separately, it's wrong before it compiles.

---

## Building

```powershell
git clone https://github.com/zipo214/RohKai-UI.git
cd RohKai-UI
cargo run
```

Requirements: Rust stable toolchain. Nothing else. No system dependencies, no cmake, no pkg-config. Pure Rust crates only — that constraint is permanent.

---

## The Zero Warnings Rule

Every commit must leave:

```
cargo clippy -- -D warnings   # zero warnings
cargo check                   # clean
cargo run                     # launches without panic
```

No exceptions. A warning is a failing build.

---

## Module Map

```
src/
  app.rs           — RohKaiApp, implements eframe::App, all panel wiring
  canvas/          — canvas interaction, widget drawing, pan/zoom
  widgets/         — palette definitions and defaults per WidgetKind
  codegen/         — walks UiTree, emits Rust strings (nothing else touches syntax)
  project/         — schema types, UiTree, io (save/load/serialize)
  panels/          — egui panels: palette, properties, code preview, templates
  svg_import.rs    — SVG → WidgetInstance parser (zero new dependencies)
  settings.rs      — UserSettings load/save (APPDATA/RohKai/settings.json)
```

Codegen lives in `src/codegen/` only. Nothing outside that module should construct Rust syntax strings.

---

## Adding a New Widget Kind

Use the `/new-widget` Claude Code slash command — it scaffolds the 7 steps automatically (or type it by hand):

1. Add variant to `WidgetKind` in `src/project/schema.rs`
2. Create `src/widgets/<kind>.rs` with `default_for_<kind>()` returning a `WidgetInstance`
3. Register in `src/widgets/mod.rs` (`ALL_KINDS` array + `default_for` match arm)
4. Add accent color in `kind_accent()` in `src/canvas/interaction.rs`
5. Add kind tag in `kind_tag()`
6. Add canvas drawing arm in `draw_widget()`
7. Add codegen arm in `src/codegen/egui_emitter.rs`, `src/codegen/state_emitter.rs`, and `src/codegen/export.rs`

Each step has a single correct place. There is no magic wiring.

---

## Commit Style

Short imperative subject line. No period.

```
Add ImageWidget kind with canvas placeholder rendering
Fix palette drag drop in same-frame collision with primary_released
Move W/H controls to bottom status bar
```

Prefix tags for machine-readability when relevant:

| Prefix | Use |
|--------|-----|
| `feat:` | new capability visible to the user |
| `fix:` | bug fix |
| `refactor:` | internal restructure, no behavior change |
| `docs:` | ROADMAP, ARCHITECTURE, comments only |
| `chore:` | deps, CI, build scripts |

---

## Pull Requests

- One PR per logical change. Don't bundle a bug fix with a new feature.
- PR title = commit subject line (same rules).
- Description: what changed, why, and what you tested.
- `cargo clippy -- -D warnings` must pass on your branch before opening.
- If you're adding a widget, include a screenshot of it on canvas.

---

## What's In Scope

- New `WidgetKind` variants that map cleanly to egui primitives
- Canvas interaction improvements (snap, alignment, selection)
- Codegen improvements (more accurate/idiomatic output)
- Export improvements (generated project quality)
- Bug fixes with a clear reproduction

## What's Out of Scope (for now)

- Multi-window / multi-canvas support
- Undo/redo (the architecture supports it; the implementation doesn't exist yet — open an issue before touching this)
- Themes / visual styling of the designer itself
- Any codegen target other than egui
- Web/WASM build (not a priority)
- Breaking the pure-Rust-only constraint

If you're unsure, open an issue first.

---

## The Spirit

Rohkai exists because Lazarus/Delphi had the tightest WYSIWYG-to-code loop ever built for native desktop, and nothing since has come close. The form *was* the code. That's the goal here — not a mockup tool, not a drag-and-drop layer on top of separate state, but a canvas where the Rust is already written.

Contributions that move toward that are welcome. Contributions that are away from it, are not.

The mark *^ρϗ* goes in every generated file. Keep it there. pls. ('sokay if you forget, but I like it cuz Im a nerd).
