# Project Surfaces And Dialogs

## Product Goal

RohKai projects should contain multiple editable forms without pretending that a
dialog is an ordinary widget inside the main canvas. The designer should provide
Qt-class modal authoring for the supported egui model while preserving RohKai's
canvas/code/export source-of-truth contract.

## Depth Status

Status: **Depth 4 - Competitive modal subset**

Modeless/native windows, docks, MDI, typed parameters/results, and multiple live
instances are not included. Those are S19B-S19D, not missing checkboxes inside
S19A.

## Current Implementation Contract

- `ProjectDocument` owns global project properties, a protected root surface,
  and ordered `UiSurface` records.
- `SurfaceKind` supports `MainWindow` and `ModalDialog`; no hollow modeless enum
  exists.
- Schema-v2 persistence and lossless bare-tree/schema-v1 migration are covered
  by round-trip fixtures.
- Surfaces have CRUD, templates, tabs, active-surface properties, lifecycle
  behaviors, diagnostics, isolated authoring state, and guarded deletion.
- F5 preview and generated native/WASM source implement Open, Accept, Reject,
  Apply, Reset, Escape/default controls, optional backdrop close, lifecycle
  order, transactional state, nested top-only dialogs, and focus restoration.
- Export emits named `src/surfaces/*.rs` modules, aggregate AppState,
  dependencies, handlers, drafts, and modal runtime.
- Supported transactional fields include strings, booleans, numeric state,
  `Vec<String>`, and `Vec<f32>`. `Vec<Vec<String>>` and unknown custom types
  diagnose and are excluded instead of becoming misleading live bindings.
- Stress coverage creates 50 surfaces and 10,000 widgets, then verifies stable
  serialization and export. The debug fixture completed in 1.29 seconds on the
  implementation machine.

## Competitor Comparison

| Capability | Qt/QDialog baseline | RohKai S19A |
|---|---|---|
| Separate dialog forms | Dedicated form type | Separate `UiSurface` |
| Modal execution | Async `open()` recommended | Immediate-mode `egui::Modal`, no nested event loop |
| Accept/Reject | Semantic result methods | Semantic button roles and lifecycle actions |
| Cancel semantics | Application-defined state handling | Transactional drafts discard on Reject/Escape |
| Multiple dialogs | Saved form classes | Saved reusable modal surfaces |
| Nested modal behavior | Supported with ownership rules | Distinct bounded top-only stack |
| Default/Escape | Default and reject buttons | Shared target resolver in preview/export |
| Focus restoration | Toolkit focus chain | Captured egui opener ID restored on close |
| Code generation | uic/source generation | Rust surface modules and warning-clean Cargo fixtures |
| Native child windows | Available | S19B, intentionally not claimed |

## UX Impact

Users can author a Settings or confirmation dialog with the mouse, wire a main
button to open it, choose semantic OK/Cancel behavior, preview the real modal
flow, and export compiling Rust without manually constructing a dialog state
machine. Surface tabs keep each form's canvas and Lazare work in place, which
prevents the cognitive cost of treating every form as a separate project.

## Acceptance Evidence

- Migration and schema-v2 round trips preserve project and surface data.
- Surface duplication regenerates widget/behavior IDs and internal references.
- Missing targets, recursion, invalid main counts, duplicate surface names/IDs,
  unsupported drafts, and dangling policies produce structured diagnostics.
- Preview tests prove reject discard, accept commit, lifecycle order, top-only
  nested close, semantic roles, scalar/vector drafts, isolated preview, and
  opener-focus restoration.
- Native and WASM-source fixtures run `cargo check` and
  `cargo clippy --all-targets -- -D warnings`.
- The 50-surface/10,000-widget fixture verifies deterministic serialization and
  export under a 15-second debug budget.

## Remaining Acceptance Delta

- Rerun the narrow/normal/wide screenshot and accessibility matrix once the
  Windows automation runtime is repaired; the implementation session was
  blocked by an `@oai/sky` package-export error.
- Add typed modal parameters/results and multiple concurrent instances only in
  S19B/S19D; changing S19A's one-instance contract would require a new runtime
  identity model.
- Build modeless eframe viewports, geometry persistence, multi-monitor/DPI
  lifecycle, docks, workspace layouts, and MDI in their separate roadmap lanes.

## Closure Rule

The presence of `UiSurface` does not close modeless, Dock, MDI, or item-model
work. S19A is closed only for the in-app modal subset described above; later
surface lanes require their own runtime, UX, platform, and generated-project
evidence.
