# Codegen, Lazare, And Export Evaluation

## Scope

This covers live egui code generation, AppState generation, Lazare
bidirectional parsing, handler stubs, Rust wiring, export, and generated
project quality.

## Top-Class Expectation

RohKai's unique promise is "canvas and code are the same thing." Top-class depth
means code is not a decorative preview: it is navigable, editable, round-trippable,
safe to export, and honest about what cannot round-trip.

## Current State

| Feature | Current Depth | What It Does Now | Gap |
|---|---:|---|---|
| Live codegen | 4 | Emits egui code from `UiTree`, with widget markers and AppState. | Needs memoization/perf and generated-code formatting polish. |
| AppState generation | 4 | Shared field collection, binding validation, defaults. | Needs richer typed models and user-defined structs. |
| Export | 4 | Writes compilable eframe project for supported widgets/assets/images. | Needs generated-project compile fixtures and platform profiles. |
| Lazare parser | 3 | Parses supported generated code back into `UiTree`; partial apply. | Needs structured edit ranges, richer syntax support, conflict UI. |
| Handler wiring | 3 | Properties write event names; codegen/export stubs/calls. | Needs event catalog depth, refactor/rename support, duplicate detection. |
| Rust wiring | 2-3 | mpsc channels, iterator pipelines, trait impls, async thread wrapper. | Needs data-flow validation, preview execution, and richer editing UX. |
| Code navigation | 2-3 | Highlight and scroll-to-handler/widget support. | Needs robust cursor placement, search, symbol list, diff view. |

## Utility

- Runtime utility: very high. Export quality determines whether RohKai builds
  actual apps or only mockups.
- Inspection utility: very high. Users trust the tool when code is readable.
- Safety utility: high. Invalid generated Rust is a product failure.

## Ideal State

| Capability | Ideal Behavior |
|---|---|
| Generated-code contract | Every emitted fragment has tests and compiles in representative exported projects. |
| Editable code sync | Changes are parsed into precise `UiTree` patches with diagnostics and conflict resolution. |
| Refactoring | Rename handler/binding/widget updates properties, code, AppState, and export together. |
| Diff tooling | Users can see canvas changes as code diffs and code edits as canvas diffs. |
| Export profiles | Desktop, WASM, test harness, and package profiles with explicit dependency policy. |
| Code ownership | Generated vs user-authored regions are clearly separated and preserved. |

## Depth Measurements

| Measure | Target For Top-Class |
|---|---|
| Compile rate | 100% generated projects compile for supported feature fixtures. |
| Round-trip rate | 95% of supported generated syntax round-trips without geometry/state loss. |
| Diagnostic quality | Every failed parse includes location, reason, and safe recovery state. |
| Export determinism | Same `UiTree` produces byte-stable output except timestamps if explicitly enabled. |
| User-code preservation | User-authored handlers survive export/regeneration cycles. |

## Recommended Next Work

1. Add generated project compile tests for "all widgets", "events", "assets",
   "custom descriptor", and "SVG image" fixtures.
2. Add structured code spans around widgets and handlers for robust navigation.
3. Add binding/handler rename operations as first-class commands.
4. Add export preview diff and generated-file health panel.
5. Add platform profile abstraction before WASM work.

