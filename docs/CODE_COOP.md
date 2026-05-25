# Code CoOp

Short agent-to-agent handoff diary. This is not the devlog and not the roadmap.
Use it for the 3-4 sentence "what I am doing and what the next agent should
know" note at the start of a meaningful planning or coding session.

Keep entries newest-first. Be plain, specific, and honest about uncertainty.
Mention the branch, the immediate goal, touched areas, and any known hazards.

## 2026-05-24 — Claude Guide Drag Fixes

On `dev`, fixed two ruler-guide bugs: (1) guides created from ruler click were
not immediately draggable — fixed by setting `*dragging = Some(id)` right after
`guides.push()` in `rulers::handle_interaction`; (2) dragging a guide also
fired rubber-band selection — fixed by adding `guide_drag_active: bool` to
`CanvasSettings`, set each frame from `session.dragging_guide.is_some()`, which
gates the `just_pressed` block and clears `rubber_band`/`drag` while a guide is
held. Also fixed descriptor editor window stretching by replacing all
`desired_width(ui.available_width())` with explicit computed widths + centered
`default_pos`. 53/53 tests, zero warnings.

## 2026-05-24 — Claude Stage 8 Close-out: Guide Snap, Lock Ratio, Canvas Bezel

On `dev`, closed the remaining three Stage 8 items: (1) Guide snapping —
`interaction.rs` drag loop now checks `tree.app_props.guides` after static widget
alignment; widget edges/center snap to vertical/horizontal guide positions within
`snap_thr` and highlight the snapped guide span; (2) Lock aspect ratio — `lock_aspect_ratio:
bool` on `SessionState`, 🔒/🔓 button in status bar, ratio enforced after DragValue
edits by comparing prev/cur W×H; (3) Canvas bezel — `draw_bezel()` in `rulers.rs`,
View → Show/Hide Canvas Bezel toggle, `show_bezel: bool` on `AppProps`, draws mock
22px macOS-style title bar chrome (three traffic-light dots + centered title) above
canvas rect when enabled. 53/53 tests, zero warnings. Stage 8 fully complete.

## 2026-05-24 — Claude Stage 8: Rulers, Presets, Theming

On `dev`, implemented all Stage 8 clusters (`3885ed1`): (1) Pixel rulers —
`src/canvas/rulers.rs` (new), Ctrl+R toggle via View menu, horizontal +
vertical ruler strips with zoom-aware ticks, click-to-create guides,
drag-to-move, Delete-to-remove, `GuideRule` persisted in `AppProps.guides`; (2)
Document presets — "▾ Preset" dropdown in status bar (9 presets desktop +
mobile), `AppProps` gains `resizable`/`min_size`/`max_size`, export uses them;
(3) Theming — `ThemeSettings` in `AppProps` (dark/light, accent RGB, font size,
corner radius, spacing), View → Theme… floating window, live `apply_theme` each
frame, `.rktheme` save/load, export injects `ctx.set_visuals(...)` when
non-default. 53/53 tests, zero warnings. All existing saves load cleanly
(serde defaults). Remaining Stage 8: guide snapping, canvas bezel.

## 2026-05-24 — Claude .rkwb Bundle + SVG Inline Toggle

On `dev`, closed remaining Stage 7.x non-SVG-maturity items (`338ee65`):
(1) `.rkwb` widget bundle — JSON envelope of multiple `WidgetDescriptor`s,
no new crate; Widgets menu gains "Export Bundle…" + "Import Bundle…";
(2) "Expand SVG inline" toggle per Image widget — checkbox in Properties,
`expand_svg_inline: bool` on schema, `svg_source_arg` helper in emitter
switches between compact `[SVG: N bytes]` and full raw string literal; export
path unchanged (already embeds full SVG). 53/53 tests, zero warnings.
Remaining open 7.x: SVG Import Maturity (Codex track). Next stage is Stage 8.

## 2026-05-24 — Claude Descriptor Editor UI Fixes + Widgets Menu

On `dev`, fixed two bugs in the in-app descriptor editor (`8b3932d`): (1)
`desired_width(f32::INFINITY)` in TextEdit fields caused window to expand to
full RohKai width — replaced with `desired_width(ui.available_width())`; (2)
save-message was cleared the same frame it was set due to eager
`cmd_reload_descriptors()` call — fixed with transition detection (snapshot
`was_saved` before `show()`, reload only on false→true edge). Also added a
"Widgets" top-bar dropdown menu (New Descriptor, Import Definition, Reload,
per-descriptor Edit entries) so descriptors no longer require navigating the
File menu. 53/53 tests, zero warnings, fmt clean. Remaining 7.x: `.rkwb`
bundle; SVG import maturity is Codex's track.

## 2026-05-24 — Claude In-app .rkwd Editor

On `dev`, implemented the in-app descriptor editor (`1104547`): split-pane
`egui::Window` — left = full descriptor form (all fields, collapsible props,
add/remove rows), right = live canvas preview + expanded template TextEdits
updating every frame. Entry: File → New Widget Descriptor… or "Edit descriptor"
button in Custom widget properties panel. Save writes `.rkwd` to `widgets/` and
auto-reloads palette. 53/53 tests, zero warnings. Remaining 7.x: `.rkwb` bundle;
SVG import maturity is Codex's track.

## 2026-05-24 — Claude SVG Source Viewer

On `dev`, added SVG source viewer popup (`76b770e`): properties panel for Image
widgets now shows a "View source" button when svg_source is loaded; clicking
opens a read-only egui::Window with the full SVG text, byte count, and "Copy all"
button. `SessionState.svg_viewer_id` tracks open state; `PropertiesAction::ShowSvgSource`
routes the event. 53/53 tests, zero warnings, fmt clean. 7.x SVG Source Viewing
item ✅. Remaining 7.x open: in-app .rkwd editor, .rkwb bundle, SVG import
maturity (Codex domain).

## 2026-05-24 — Claude Stage 7.x Complete

On `dev`, finished all three Stage 7.x items: (1) handler calling-convention
unification (`egui_emitter.rs` + `code_preview.rs`); (2) descriptor hot-reload
and Import Widget Definition dialog (`app.rs`); (3) Lazare Custom round-trip
(`parser.rs` fallback extracts label/binding from template-expanded lines,
guarded so constructor line wins over handler calls). 53/53 tests, zero warnings,
`cargo fmt` clean. Remaining known gap: `descriptor_props` (`{{prop.KEY}}`
substitutions) don't feed back into Lazare sync — deferred.

## 2026-05-24 — Codex SVG Renderer R0

On `dev`, I am implementing the first SVG renderer roadmap slice: structured
render output/reporting, renderer diagnostics, and tests that prove current
behavior is deterministic and honest. I will preserve the stable `rasterize()`
and `rasterize_or_fallback()` wrappers and avoid touching Claude's recent
handler/hot-reload source work. The key hazard is overclaiming: this pass should
make renderer limits visible, not pretend gradients/text/clips are done.

## 2026-05-24 — Claude Track B + Track A

On `dev`, completed handler calling-convention unification (Track B): live
preview now emits `self.h();` and Tracé stubs use `fn h(&mut self)`, matching
export.rs — `egui_emitter.rs` and `code_preview.rs` touched. Also added
descriptor hot-reload (Track A partial): File → Reload Widget Descriptors
rescans `widgets/` without restart — `app.rs` touched. Remaining Track A work:
Import Widget Definition dialog and Lazare Custom round-trip (label/binding
needing descriptor template awareness). 47/47 tests, zero warnings, two commits.

## 2026-05-24 — Codex Low-Token Docs Consolidation

Working on `dev`, I am consolidating agent prep so future sessions do not burn
context by reading every guidance document by default. The procedural source is
`scripts/preflight-context.ps1`; AGENTS/CLAUDE hold policy; Code CoOp is the
normal short handoff; DEVLOG becomes history-on-demand. I am not changing app
behavior in this pass. Watch for stale older entries that are historically
useful but no longer part of default preflight.

## 2026-05-24 — Codex PowerShell 7 UTF-8 Standardization

Working on `dev`, I installed PowerShell 7 and am updating repo scripts and
agent guidance to prefer `pwsh` with explicit UTF-8 handling. The immediate
hazard is recurring mojibake from legacy Windows PowerShell 5.1 text paths, so
this pass adds a text encoding guard and fixes the known corrupted lines. Next
agents should use `pwsh -NoProfile -ExecutionPolicy Bypass -File ...` for repo
scripts and avoid shell text writers unless `-Encoding utf8` is explicit.

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

## 2026-05-24 - Claude

Completed the full 5-patch Rust-ness Remediation Plan. All patches verified
(47 tests, zero clippy warnings, clean fmt) and pushed to PR #3 on dev branch.
Key structural changes: `RohKaiApp` is now decomposed into sub-structs
(`project`, `session`, `messages`, `prefs`, `code`, `descriptors`); all field
accesses in `app.rs` updated. The SVG rasterizer has a typed `Result` API with
`rasterize_or_fallback` as the canvas path. `field_collector` is the single
source of truth for AppState field collection. Next session: `export.rs` handler
parity, or Stage 7 descriptor UX (descriptor hot-reload, palette custom section).
