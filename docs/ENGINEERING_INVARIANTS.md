# RohKai Engineering Invariants

Recurring bug *classes* and the invariant that prevents each. This is a
read-on-demand reference (like `ARCHITECTURE.md`), not part of the low-token
preflight default. Read the relevant row before touching the layer it names;
read the whole file when fixing a reviewer finding so the same class does not
return.

These rows were distilled from real review findings. The point is not to memorize
fixes — it is to fix the *class*, add a `class`-level regression test, and keep
the patch minimal.

## Systemic-fix workflow (use for any bug or reviewer finding)

Before editing code:

1. Identify the requested change.
2. Identify the **root cause**, not the visible symptom.
3. Identify all **sibling surfaces** that may need parity (see Invariant 1):
   canvas/editor · preview · export/codegen · persistence (save/load) · docs · tests.
4. Derive all ownership/topology cases, UI surfaces, codegen/export/parser paths,
   and invariant tests required by the source-of-truth data model. Representative
   tests are not enough if a feature has hidden topologies or output paths.
5. Identify the project **invariant** this change touches (the table below). If
   none exists yet, add a row here.
6. Add or update a regression test for the **bug class**, not only the exact bug.
7. Prefer single-source-of-truth APIs over duplicated logic (Invariant 2).
8. Do not add logic to the wrong architectural layer (e.g. Rust-syntax strings
   only in `src/codegen/`).
9. Keep the patch minimal, but make the fix systemic enough that the class is
   less likely to recur.

Completion report should state: root cause fixed · files changed · tests added/
updated · invariant(s) added or reinforced · validation commands run · any
skipped reviewer finding with reason · any remaining risk.

## Invariant table

| # | Bug class | Invariant | Cheap guard |
|---|---|---|---|
| 1 | **Surface parity drift** — a behavior changes in one render/state surface but not its siblings (e.g. radio write-back works in export but not preview; image renders on canvas but is a placeholder in preview; state not re-seeded after a tree swap). | A behavior visible in any of {canvas, preview, export/codegen} must be matched in the others, or carry an explicit, tested reason for differing. Tree/state replacement goes through one helper that also refreshes derived state (e.g. `refresh_preview_state`). | Parity unit test, or a `// PARITY:` comment naming the surface + why it differs. |
| 2 | **Classification duplication** — re-listing members the canonical API already owns (e.g. an overlay hardcoding which `WidgetKind`s are click-capable instead of asking `WidgetKind::supported_events()`). | Derive from the authoritative source (`UiTree`, `WidgetKind::supported_events()`, `kind_table`, schema). Never copy its member list into a second place. | Match/iterate the canonical source; a test asserting the two agree. |
| 3 | **Input-ownership gaps** — a global handler fires without checking who owns the input (undo/redo during text edit; guide create/drag under a floating window; a post-panel correction that runs after *any* change and clobbers an explicit choice). | Global shortcuts gate on `ctx.wants_keyboard_input()` / editor focus. Canvas pointer actions gate on `ui.ctx().layer_id_at(pos) == Some(ui.layer_id())` **and** rect containment. Post-edit corrections gate on *which* control actually changed. | Reuse the existing ownership helper in `src/canvas/interaction.rs`; unit-test the gate. |
| 4 | **Reset/default no-op** — a reset or "load without overrides" path skips the set entirely, leaving stale state (e.g. `apply_theme` only setting style when an override is present). | Reset/apply paths build from a fresh base and set state **unconditionally**; absence of an override means "restore base", never "do nothing". | Test that reset after a change returns to base. |
| 5 | **Unsafe generated identifiers** — codegen emits field/handler names that are invalid Rust idents, keyword-collisions, or name-collisions; or Rust-syntax strings leak outside `src/codegen/`. | Codegen lives **entirely** in `src/codegen/`. Generated identifiers must be: valid (no leading digit / empty), keyword-escaped, collision-resistant (deterministic suffix from a stable id), and deterministic. The exported project must `cargo check`. | Sanitizer unit tests (leading-digit, keyword, collision); the ignored exported-project cargo-check test. |
| 6 | **String byte-slicing panic** — `s[..n]` / `&s[a..b]` on user/label/text strings panics on a multi-byte UTF-8 boundary. | Never byte-index a `&str` for truncation/display. Use `chars().take(n)` or `char_indices()`. | A unit test that truncates a label containing a multi-byte char (emoji/CJK). |
| 7 | **Incomplete filename sanitizing** — only replacing `.`/`/` leaves Windows-reserved (`<>:"\|?*\`) and control bytes that break save on the primary OS (Windows). | Filename sanitizers whitelist `[A-Za-z0-9_-]` (mapping the rest to `-`/`_`) or explicitly strip Windows-reserved + control chars. Sanitize for the strictest target OS. | Unit test with reserved/control chars in the id. |
| 8 | **Permissive shipped defaults** — a default value that flows into a generated artifact is unsafe (e.g. cargo dep version `"*"`). | Defaults that reach generated output are conservative (e.g. cargo version `"0.1"`/`"1"`, never `"*"`); match the field's own hint text. | Snapshot/export test asserting the conservative default. |
| 9 | **Doc contradiction / lint** — related docs disagree (status "closed" while a "Later Tasks" list still shows the item open), redundant phrasing ("SVG Image" — the G is "graphics"), or a fenced block with no language. | When you change one doc's status/scope, reconcile sibling docs in the same pass. Fenced code blocks carry a language (` ```text `). | `markdownlint`-style review; grep for the changed claim across `docs/`. |
| 10 | **SQL injection via `format!()`** — concatenating a run-time value into a SQL string (e.g. `format!("SELECT * FROM {table} WHERE id = {id}")`) exposes the exported app to SQL injection and crashes on names with special characters. | All SQL *values* (column filter values, limit integers, any user-supplied input) must be passed through `rusqlite::params![]` as bound parameters. Table and column *names* cannot be parameterised by SQLite; instead validate them as plain ASCII identifiers (`[A-Za-z0-9_]+`) before interpolation. Direct `rusqlite` imports outside `src/project/db_engine.rs` are forbidden in the designer binary (generated/exported projects legitimately import `rusqlite` in their own `main.rs`). See `DB_INTEGRATION_RESEARCH.md` §"Invariant 10" for the full codegen rules (named parameters, prepared-statements-only, identifier quoting) — this row is the authoritative summary; that section is the detail. | Unit test with a table name containing `;` — must return `DbError::Query`, not execute SQL. |
| 11 | **Multi-surface authority drift** - project-global state is copied into a surface tree, active-surface edits replace neighboring trees, or tab switches lose selections/code buffers. | `ProjectDocument` is the project authority; each `UiSurface.tree` is authoritative only for that surface. Cross-surface mutations go through `ProjectDocument`/`ActiveDocument`. Switching surfaces flushes the active adapter and preserves per-surface selection, zoom, pan, and Lazare edit state. | Schema migration/round-trip tests, duplication-ID remap test, and a workspace capture/restore test containing an invalid code buffer. |
| 12 | **Modal lifecycle parity drift** - preview and export disagree about drafts, button roles, event order, nesting, Escape/default behavior, or focus. | Preview and export derive targets from `resolve_dialog_button_target` / `resolve_dialog_initial_focus_target`, enforce a bounded top-only stack, copy supported fields into drafts, commit only on Accept/Apply, fire result before `Closed`, and restore opener focus. Unsupported fields must diagnose rather than silently bind live state. | Preview lifecycle/focus/vector tests plus warning-denied native/WASM generated-project fixtures from `check-surface-parity.ps1`. |
| 13 | **Ownership/topology parity drift** - a child/container behavior works in one tree shape but not siblings (e.g. top-level layout works, Frame-owned layout emits placeholders, or moving a child leaves duplicate parents). | If behavior depends on `WidgetInstance.children`, derive and cover every ownership topology before coding: top-level, Frame-owned, layout-owned, layout-inside-layout, Frame-owned-layout, moved child, empty/cleared container parse, duplicate-parent repair, and cycle repair. Claims about recursive canvas/live/export/Lazare behavior are incomplete until every topology has real output or an explicit, tested unsupported diagnostic. | Topology matrix tests across `UiTree`, canvas/preview, emitter/export, and parser/Lazare as applicable. |

## The verification gate

The gate that actually catches these is **`--all-targets`** clippy plus the full
test run. Plain `cargo clippy` skips `examples/` and `tests/`; reviewer findings
in this repo have included lints living exactly there.

Run, one cargo at a time, before any code session is "done":

```text
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1   # SVG/codegen-adjacent work
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1
```

Zero warnings is required. If a step cannot be run, say exactly why and what risk
remains.
