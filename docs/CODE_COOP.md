# Code CoOp

Short agent-to-agent handoff diary. This is not the devlog and not the roadmap.
Use it for the 3-4 sentence "what I am doing and what the next agent should
know" note at the start of a meaningful planning or coding session.

Keep entries newest-first. Be plain, specific, and honest about uncertainty.
Mention the branch, the immediate goal, touched areas, and any known hazards.

## 2026-05-23 — Claude Stage 7 Gap Fixes + SVG Code Contraction

Three confirmed gaps from the Stage 7 implementation are now fixed: (1)
`descriptor_state_fields` is snapshotted onto `WidgetInstance` and both
`state_emitter` and `export` emit them into AppState; (2) `apply_parsed` guards
against overwriting `WidgetKind::Custom` with a parser-inferred built-in; (3)
`image_preview_line` emits a compact `"[SVG: N bytes]"` placeholder instead of
the full raw SVG, keeping the code buffer readable. Export still embeds full SVG.
Previous DEVLOG claim that `descriptor_props` don't drive live codegen was wrong
and corrected. Roadmap updated with SVG source viewer and descriptor maturity
items. 30/30 tests, zero clippy warnings.

## 2026-05-23 — Claude Stage 7: .rkwd Widget Descriptor Format

Stage 7 is implemented and verified (30/30 tests, zero clippy warnings, clean
build). New `WidgetKind::Custom(String)` variant is live; descriptors load from
`<binary_dir>/widgets/*.rkwd` at startup; palette renders custom categories;
properties panel shows typed descriptor fields; codegen snapshots templates onto
instances so `egui_emitter` and `export` work without threading descriptors
everywhere. `widgets/ply-button.rkwd` is the worked example. Two known gaps
for 7.x: `state_emitter` does not yet emit `state_fields` from descriptors, and
`descriptor_props` changes don't drive live Lazare codegen sync.

## 2026-05-23 - Codex SVG/Image Parity Push

I am continuing from the clean baseline to remove the hollow SVG/Image paths.
The immediate target is live codegen/export parity first, then renderer safety
and semantics, with base verification after each feature set. I will preserve
the no-new-dependency rule and avoid claiming full SVG equivalence unless the
implemented output forms actually prove it. Watch `src/codegen/export.rs`,
`src/codegen/egui_emitter.rs`, and `src/canvas/svg_rasterizer.rs`.

## 2026-05-23 - Codex Baseline Stabilization

I am stabilizing the current dirty worktree without changing SVG behavior. The
first goal is formatting and verification so Codex and Claude have a clean
baseline before deeper renderer/export work. Known hazard: the SVG rasterizer is
real but incomplete and should not be overclaimed as equivalent yet. I will only
make narrow compile/clippy fixes if the verification suite requires them.

## 2026-05-23 - Codex

I am tightening coordination rules rather than changing app behavior. The main
goal is harmony between Codex, Claude, and future agents: `docs/CODE_INDEX.md`
is the map, this file is the short handoff diary, and preflight now points to
both. The current SVG rasterizer work is real but incomplete, especially export
parity and full SVG feature support, so do not overclaim it. The worktree is
already dirty from earlier SVG/Stage 7 changes; avoid broad formatting or
unrelated rewrites unless the user asks.
