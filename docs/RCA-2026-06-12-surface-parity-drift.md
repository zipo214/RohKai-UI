# RCA — Cross-Surface Parity Drift (2026-06-12)

## Summary

A recurring class of defect in RohKai: a capability is implemented on **one
surface** and silently absent from the others, while the documentation claims it
is done (or claims it is *not* done when it is). The user surfaced it directly —
"I do not believe implementation was done according to the roadmap" and "if this
is true show it in the code. find the lines." This RCA names the class, finds the
root cause, and records the guard we built.

This is not about any single bug. It is about why bugs of this shape keep
shipping green.

## Symptoms observed this session

- `bug:` ROADMAP_PHASE2.md P2.7 listed filter tier-3, progressive JPEG, etc. as
  `[ ]` "not done" — but `feTile`/`feTurbulence`/`feDisplacementMap`/lighting/
  `feImage` are fully implemented (`svg_rasterizer.rs:6760+`, 7 tests at 14177+).
  The doc claim and the code disagreed; nobody noticed until a human re-checked.
- `bug:` P2.4 shipped `child_flex` / `grid_col_span` / `grid_row_span` as schema
  fields with Properties-panel UI, but the egui emitter never read them. Canvas
  showed one thing, export produced another. Invisible because everything
  compiled and ran.
- `risk:` `constraint_solver::validate_constraints` is a `pub fn` marked
  `#[allow(dead_code)]` — a complete validator that no UI ever calls. Shipped,
  tested, unsurfaced.
- `risk:` The `tests/fidelity_audit.rs` cross-surface harness — the very thing
  meant to catch this — was itself hollow: RohKai was a binary-only crate, so the
  harness could not link (`use rohkai::…` → unresolved crate), was never run, and
  was never linted (it carried a clippy error). A guard that never executes is
  not a guard.

## The class

**Cross-surface parity drift.** RohKai's truth is supposed to flow:

```text
UiTree schema  →  Properties UI        (edit)
               →  egui_emitter / export (codegen)
               →  canvas / preview      (render)
               →  tests                 (proof)
               →  roadmap docs          (claim)
```

A defect of this class is any capability that exists on one arrow and is missing
on another, with no signal. The four arrows above can each silently disagree.

## Root cause: asymmetric forcing functions

RohKai already has **strong** anti-drift machinery — but only for *enums*:

- `WidgetKind::supported_events()` is an exhaustive `match` (no wildcard). Adding
  a new `WidgetKind` **will not compile** until its events are declared. The
  comment says so explicitly.
- The main `emit_indexed` match (`egui_emitter.rs:113`) is likewise exhaustive,
  including `WidgetKind::Custom(_)`. A new kind cannot compile without codegen.
- `EVENT_CAPABLE_KINDS` is a `#[cfg(test)]` canonical list walked by a parity test
  so a new event-capable kind that forgets export wiring fails CI.

None of that exists for the other three drift surfaces:

1. **Struct fields have no forcing function.** Adding a field to
   `WidgetInstance` triggers no exhaustiveness error anywhere. It can be
   serialized, defaulted, shown in Properties, and never read by codegen — and
   the build is green. This is exactly how `child_flex` shipped half-wired. The
   asymmetry is the core root cause: *enum variants are forced, struct fields are
   not.*

2. **Roadmap claims have no link to code.** `[x]` / `[ ]` are hand-edited prose.
   Nothing checks that a `[x] DONE` names a symbol that exists, or that a `[ ]`
   item is genuinely absent. Background agents (and compaction-resumed sessions)
   edited checkboxes from memory, and the checkboxes drifted from the code in
   *both* directions.

3. **"Non-goal" language is an escape hatch.** Once a capability is written down
   as a non-goal, "not implemented" and "done" become indistinguishable — there
   is no checkbox to be wrong. Several SVG capabilities were simultaneously
   "implemented" (in code), "deferred" (in one doc), and "rejected" (in another).

### Contributing factors

- **Binary-only crate.** No `[lib]` / `src/lib.rs` meant integration tests in
  `tests/` could not import the crate at all, so the one cross-surface harness was
  structurally unable to run. Inline `#[cfg(test)]` tests cannot easily assert
  *across* schema→codegen→export the way an integration test can.
- **Parallel background agents.** Each agent based on a pre-merge snapshot
  implemented its slice (schema + Properties) and left codegen/test parity to
  "the merge", which no single agent owned.

## The guard (what we built)

Defence in depth, matching each root cause to a control:

| Root cause | Control added |
|---|---|
| Harness could not run | **lib + bin split** (`src/lib.rs`) so `tests/` link the real public API |
| Struct-field codegen drift | **`tests/fidelity_audit.rs`** parity tests: every layout field set to a non-default value must change codegen output (`child_flex`→`allocate_ui`, grid spans→filler/comment) |
| Silent-nothing codegen | **completeness test** walking `widgets::ALL_KINDS`: every kind must emit a real (non-comment) line |
| Roadmap ↔ code drift | **`scripts/check-surface-parity.ps1`**: flags `[x] DONE` naming a missing symbol (overclaim) and `[ ] TODO` naming an existing symbol (stale underclaim) |
| Unsurfaced public API | same script flags `#[allow(dead_code)]` on `pub` items |
| Non-goal escape hatch | **de-deferral policy** in `ROADMAP_PHASE2.md`: no non-goal/deferred state exists; everything is an ordered to-do with an annotated depth (`[x] DONE` / `[~] SHALLOW` / `[ ] TODO`) |

The hard gates are the exhaustive matches and the `fidelity_audit.rs` tests (they
fail CI). The script is the **advisory** human-review supplement — deliberately
not a hard gate, because static heuristics over prose produce false positives and
a checker that hard-fails on noise gets disabled.

### Why the script stays advisory

`check-surface-parity.ps1` reports, it does not block (except `-Strict` on a clean
DONE-overclaim signal). Field→codegen coverage and dead-code-pub are genuinely
ambiguous (a field may be geometry-only; a `pub` item may be planned API or
test-only). The tool's job is to put those candidates in front of a human in
caveman-review form, not to pretend it can decide.

## Open follow-ups

The audit surfaced real shallow surfaces. The S1 batch was **resolved the same
session** (2026-06-12 follow-up commit); the rest stay ordered, not parked:

- ✅ **DONE** — `apply_constraints` was not just non-recursive but
  *non-idempotent*: it ran every frame and `margin += …` walked widgets off
  screen. Rewritten to be idempotent (margin folded into absolute alignment) and
  **parent-relative** (frame = parent's solved rect, parents-before-children).
  Tests: `solve_is_idempotent_across_frames`, `alignment_is_parent_relative_not_canvas`.
- ✅ **DONE** — `validate_constraints` now renders under `show_constraints()`
  (red per-widget messages); `#[allow(dead_code)]` removed.
- ✅ **DONE** — `text_align` (was dead everywhere) wired into egui_emitter +
  export + preview; `child_cross_align` per-child override wired into VLayout/
  HLayout codegen (Stretch dropped from the UI — no proven egui path). Parity
  tests added to `fidelity_audit.rs`. `constraints` / `descriptor_accent` confirmed
  geometry/canvas-only (correctly no codegen reference).
- ✅ **DONE** — Canvas constraint authoring is no longer Properties-only. Four
  drag handles attach the selected widget to parent-relative horizontal/
  vertical targets and derive margins without moving it on commit.
- ✅ **DONE** — Grid slot depth: persistent slot names, canvas/Properties/code
  visibility, arrow reorder, and direct drag-to-slot feedback.
- ✅ **DONE** — Multi-level layouts reflow and draw recursively, emit in both
  live and export code, and Lazare reconstructs explicit parent relationships.
  The parser distinguishes an intentionally empty container so deleting nested
  child code clears ownership instead of silently retaining stale children.
- `nit:` ~15 remaining `#[allow(dead_code)]` `pub` items (several redundant after
  the lib split). Sweep them: wire, delete, or drop the attribute. → **S4**.

## Verification

- `cargo test` — 495 lib + 13 `fidelity_audit` (added completeness + list guard)
  + 1 doctest, green.
- `cargo clippy --all-targets -- -D warnings` — zero warnings.
- `pwsh scripts/check-surface-parity.ps1` — runs; 0 overclaims; advisory findings
  triaged above.
