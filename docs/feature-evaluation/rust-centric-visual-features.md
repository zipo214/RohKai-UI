# Rust-Centric Visual Features Evaluation

## Scope

This evaluates Claude's latest roadmap implementation: Stage 11 Rust-Centric
Visual Features. The stage added ownership/error overlays, async/error handler
metadata, Rust Wiring for channels/iterator pipelines/trait impls, and a macro
palette.

This document is intentionally stricter than the roadmap. Stage 11 has useful
vertical slices, but most features are not yet top-class Rust-aware visual
programming systems.

## Overall Classification

| Feature | Current Depth | Current Status |
|---|---:|---|
| Ownership visualization | 3 | Usable read-only overlay derived from `field_collector`. |
| Error-flow visualization | 2-3 | Useful handler-contract badges, not control-flow analysis. |
| Async task wiring | 2 | Functional MVP: generated code launches a worker thread, sends completion over mpsc, drains via try_recv, records running/error. Worker body is a user-filled stub. |
| Channel connections | 2 | Exported mpsc fields/init exist; no visual connection graph or send/receive wiring. |
| Iterator pipeline builder | 2 | Generates method text from ordered map/filter ops; no type validation or execution. |
| Trait binding | 1-2 | Emits user-authored impl text; no validation beyond simple non-empty checks. |
| Macro palette | 2 | Inserts snippets into code buffer; not contextual or cursor-aware. |

## Competitor / Top-Class Baseline

Top-class Rust-centric visual tooling would combine:

- IDE-grade symbol navigation, refactoring, and diagnostics.
- Visual data-flow graphs for channels, async tasks, state, and error paths.
- Type-aware editing that rejects invalid Rust before export.
- Runtime preview or simulation for events, messages, results, and async work.
- Clear separation of generated code, user-authored code, and editable snippets.
- Exported code that compiles across representative fixtures without manual edits.

Stage 11 does not yet reach that bar. It does provide useful schema and UI
footholds for future work.

## Feature Evaluations

### Ownership Visualization

Status: Usable Product Feature

Current Implementation Contract:

- `src/canvas/overlays.rs::draw_ownership` draws badges for widgets with
  `state_binding`.
- Uses `field_collector::collect(tree)` for type information, so it shares the
  same binding/type source as codegen.
- View menu toggles `SessionState.show_ownership`.
- Draws a small AppState legend in the canvas panel.
- Unit tests cover type-color stability and handler detection helper behavior.

Insufficient Existing Surface:

- This is not a full ownership/lifetime visualizer.
- It does not show borrow scopes, move semantics, mutation flow, aliases, or
  generated-code spans.

Desired Closure Contract:

- Add hover/click inspection for each badge.
- Link badge to generated AppState field and owning widget properties.
- Show multiple writers/readers, derived fields, and data-flow edges.
- Add conflict warnings for ambiguous or invalid binding ownership.

Closure Criteria:

- Clicking a field badge selects the widget and scrolls to the AppState field.
- If two widgets collide on incompatible state usage, the overlay shows a
  warning and tests prove the diagnostic.
- Ownership overlay remains legible on 100+ widget projects.

### Error-Flow Visualization

Status: Functional MVP

Current Implementation Contract:

- `WidgetInstance.handler_result` stores `Plain`, `Result`, or `Option`.
- Properties panel exposes an Error mode dropdown when a handler is present.
- `draw_error_flow` paints `Result<()>` or `Option<()>` badges for handler
  widgets.
- Exported handler signatures and call sites change for `Result` and `Option`.

Insufficient Existing Surface:

- This is not real error propagation analysis.
- It does not trace `?`, `Result<T, E>` values, nested call chains, recovery
  paths, UI error display, or async error delivery.

Desired Closure Contract:

- Model handler outputs, error display destinations, recovery actions, and
  propagation edges.
- Show visual paths from widget event to handler, result, state mutation, and UI
  feedback.
- Generate consistent runtime error handling, not only `eprintln!`.

Closure Criteria:

- A `Result` handler can be configured to display an error label/toast in the
  generated app.
- Error-flow overlay shows the destination and recovery behavior.
- Exported project compile tests cover Plain, Result, Option, and async Result
  variants.

### Async Task Wiring

Status: Functional MVP

Current Implementation Contract:

- `WidgetInstance.async_handler: bool` persists per widget.
- Properties panel shows "Run async (background thread)" when a handler exists.
- Export generates a **real** std-only task contract (no tokio), via
  `codegen::rust_wiring`:
  - Per-handler `ExportedApp` fields: `{h}_rx: Option<Receiver<MSG>>`,
    `{h}_running: bool`, and (Result mode) `{h}_error: Option<String>`, all
    initialized in `Default`.
  - A launcher method `fn {h}(&mut self)` that guards double-launch, clears the
    error, creates an `mpsc::channel::<MSG>()`, `std::thread::spawn`s a closure
    that `tx.send({h}_worker())`, and stores the receiver.
  - A module-level free fn `fn {h}_worker() -> MSG` that runs off the UI thread
    and takes **no** `&mut self` (honest UI/worker split).
  - A drain block at the top of `update()` that `try_recv`s completion,
    borrow-safely sets `{h}_running = false`, records `{h}_error` for `Err`, and
    clears the receiver.
  - A `ctx.request_repaint_after(Duration::from_millis(16))` guard emitted after
    drain blocks: the exported app repaints promptly while any task is in flight,
    then returns to event-driven repainting once all tasks complete.
  - `MSG` is `()` (Plain), `Result<(), String>` (Result), or `Option<()>`
    (Option).
- **Full Properties/export event parity is enforced — every supported event is
  exported.** The event set is a single source of truth —
  `WidgetKind::supported_events()` in `project::schema` (an exhaustive,
  wildcard-free match, so a new kind will not compile until its events are
  declared). Both the Properties panel and export derive from it. Export now
  collects a handler from **every** event field (not just the primary) and emits,
  per widget, a single bound `Response` with one `if evt_response.<method>() { … }`
  per event that has a handler — every call routed through
  `rust_wiring::handler_call()` via the central registry, so async/plain/result/
  option semantics are consistent. Complete `(WidgetKind, WidgetEvent) → egui
  method)` matrix:
  - Button: Click → `clicked()`, DoubleClick → `double_clicked()`
  - TextInput: Change → `changed()`, LostFocus → `lost_focus()`
  - TextArea: Change → `changed()`, LostFocus → `lost_focus()`
  - Slider: Change → `changed()`, DragStopped → `drag_stopped()`
  - SpinBox: Change → `changed()`, DragStopped → `drag_stopped()`
  - Checkbox: Change → `changed()`
  - RadioButton: Change → `changed()` (radio_value marks the response changed)
  - ComboBox: Change → inner `combo_changed` flag
  - FontComboBox: Change → inner `font_combo.inner == Some(true)` flag
  No event row visible in Properties is ignored by export. `drag_stopped()`,
  `double_clicked()`, and `lost_focus()` are the exact egui 0.29 `Response`
  methods used (verified against the live `egui_emitter` preview path).
- **Parity holds in the nested/frame-child export path too.** `export_child_line`
  now binds a `child_response` (or `child_combo` for combos) and emits the same
  per-event `if child_response.<method>() { … }` dispatch through
  `rust_wiring::handler_call()` + the registry. The same `(kind, event) → method`
  matrix applies to children. ComboBox/FontComboBox children — previously dead
  `Label` placeholders that could not fire `On Change` — now render a real
  interactive `egui::ComboBox` (via `allocate_ui_at_rect`) gated on
  `child_combo.inner == Some(true)`. Handler collection already iterates all
  `tree.widgets` (children included), so the central registry, conflict detection,
  and async task contract cover child handlers; a top-level↔child conflict is
  detected and normalized. **Both top-level and nested/frame-child export reach
  full event parity** — no Properties event row is ignored by either path.
  Event ordering: Button `Click` and `DoubleClick` are wired independently and
  both fire per egui's native semantics (single `clicked()` on first release,
  `double_clicked()` on the second click); Click is intentionally not suppressed.
- Duplicate handler names detected across **all** event fields (not just primary):
  first definition wins; all call sites normalized to that definition's mode; a
  `// CODEGEN CONFLICT` comment is emitted near the handler **and** a
  `!! HANDLER CONFLICTS DETECTED` summary block at the top of generated `app.rs`
  lists every conflicting handler.
- Tests assert: receiver field, spawn+send+try_recv, Result error storage, no
  TODO-only placeholder, non-async Plain/Result/Option, repaint guard present,
  non-button async launcher routing, conflict detection + call-site normalization,
  combined three-widget async fixture coherence, **two invariants — over every
  `(kind, event)` pair from `supported_events()` for BOTH the top-level and the
  nested/frame-child export paths** (each fails if any supported event lacks
  `handler_call()` routing or the correct gate method), focused secondary events
  (Button DoubleClick, TextInput/TextArea LostFocus, Slider/SpinBox DragStopped),
  focused nested-child events (Button Click/DoubleClick, TextInput LostFocus,
  Slider/SpinBox DragStopped, Checkbox Change, interactive ComboBox child),
  primary+secondary on one widget, an across-event-field conflict, and a
  top-level↔nested-child conflict case.

Insufficient Existing Surface (remaining gaps to top-class):

- The `{h}_worker()` body is still a user-filled TODO stub — RohKai generates the
  plumbing, not the work.
- No cancellation, no progress streaming, no typed task input model.
- `{h}_running`/`{h}_error` are not yet auto-bound to a UI widget (spinner,
  error label); the user reads them manually.

Closed proof gap — generated-export compile fixture:

- A real `cargo check` now runs against a generated export crate
  (`export_compile_fixture_cargo_check`, `#[ignore]`, run via
  `cargo test export_compile_fixture_cargo_check -- --ignored`). The fixture
  exercises the release-critical export surface: top-level Button Click +
  DoubleClick, async Plain + Result, Frame child TextInput LostFocus + Slider
  DragStopped, FilePicker/rfd, mpsc channel fields, iterator pipeline method,
  simple local trait binding, and `String`/`f32` state bindings. An always-run
  smoke (`export_compile_fixture_generates_required_files_and_matrix`) proves the
  project is generatable and every matrix marker is present without compiling.
  This replaces the prior "string-level proof only" caveat for those paths:
  generated export code is now proven to compile, not just to contain the right
  substrings. Remaining: the fixture is opt-in (deps make it too slow for the
  default suite); no automated CI wiring for the `--ignored` run yet.

Desired Closure Contract:

- Auto-bind `{h}_running`/`{h}_error` to chosen widgets (spinner/label) so status
  is visible without manual code.
- Add a typed task input/output model and optional progress channel.
- Add cancellation policy and runtime behavior tests beyond compile proof.

Closure Criteria:

- Exported app with an async button compiles and updates a bound status widget
  when the worker sends a result.
- A generated-project compile fixture covers async Plain + async Result.
- Cancellation and progress are expressible from the UI.

### Channel Connections

Status: Functional MVP

Current Implementation Contract:

- `AppProps.rust_wiring.channels` stores `ChannelDef { id, name, ty }`.
- Rust Wiring window lets users add/remove/edit channel name/type rows.
- Export emits `Sender<T>` and `Receiver<T>` fields and initialization in
  `ExportedApp::default`.
- Tests assert field/init strings are emitted.

Insufficient Existing Surface:

- There are no visual connections between producers and consumers.
- Receivers are not drained automatically.
- Channels are not linked to widgets, async tasks, components, or handlers.
- Type strings are user-authored and not validated as Rust types.

Desired Closure Contract:

- Add a channel graph model: producers, consumers, message type, buffer policy,
  and delivery behavior.
- Add UI for connecting widget handlers/components to channels.
- Generate send and drain code with explicit state update behavior.

Closure Criteria:

- User can connect a background task output to a ProgressBar binding through a
  channel.
- Exported app drains receiver in `update()` and updates AppState.
- Invalid channel names/types are blocked before export.

### Iterator Pipeline Builder

Status: Functional MVP

Current Implementation Contract:

- `AppProps.rust_wiring.iterators` stores named pipelines with source expression
  and ordered `Map`/`Filter` ops.
- Rust Wiring window edits pipeline name/source/op expressions.
- Export emits a compile-valid `impl ExportedApp` method returning
  `impl IntoIterator + '_` and collecting through `Vec<_>` internally. This
  replaced the previous invalid `fn name(&self) -> Vec<_>` item signature.
- Tests assert operation order, emitted method strings, and generated-project
  compile proof covers one pipeline method.

Insufficient Existing Surface:

- This is string-based code assembly, not a type-aware pipeline builder.
- It does not validate source existence, item type, expression syntax, borrow
  mode, or semantic return type. The compile fixture proves the representative
  generated method compiles, not that arbitrary user expressions are type-safe.
- The generated method is not automatically used by widgets or previews.

Desired Closure Contract:

- Bind pipelines to known AppState fields or data sources.
- Provide typed operation templates and syntax validation.
- Show preview output where sample data exists.
- Allow widgets to consume pipeline output.

Closure Criteria:

- A ListView can bind to a pipeline output.
- Invalid source or expression shows a diagnostic before export.
- Generated project compile tests cover map/filter chains and widget consumption.

### Trait Binding

Status: Surface / Power-User Escape Hatch

Current Implementation Contract:

- `AppProps.rust_wiring.trait_impls` stores trait name, method signature, and
  method body strings.
- Rust Wiring window edits these strings.
- Export appends a local `trait TraitName { method; }` declaration for simple
  trait names, then `impl TraitName for ExportedApp { ... }`. Path-like external
  traits remain user-authored/external.
- Tests assert the block text is formed, and the generated-project compile
  fixture covers one simple local trait binding.

Insufficient Existing Surface:

- This is raw Rust text insertion, not visual trait binding.
- RohKai does not deeply validate method signatures, body semantics, external
  trait existence, or whether widgets use the trait.

Desired Closure Contract:

- Add trait catalog/import management and method signature validation.
- Connect trait methods to widgets/components or generated app lifecycle.
- Provide generated compile fixtures and diagnostics.

Closure Criteria:

- User can select or define a trait, bind a widget/component behavior to a trait
  method, and export a compiling project.
- Invalid trait/method signatures are caught before export or by generated
  compile tests with clear diagnostics.

### Macro Palette

Status: Functional MVP

Current Implementation Contract:

- `panels/macro_palette.rs` lists fixed macro snippets.
- View menu opens Macro Palette.
- Clicking a macro appends the snippet to the end of the Lazare code buffer.
- Tests assert known macro names and non-empty snippets.

Insufficient Existing Surface:

- Snippets are not inserted at cursor position.
- Snippets are not context-aware: handler body, expression position, statement
  position, or selection replacement are not considered.
- Parser round-trip is not guaranteed for arbitrary inserted snippet location.

Desired Closure Contract:

- Insert at the code cursor or active handler body.
- Offer context-filtered snippets.
- Validate whether inserted code parses or mark it as user-authored unsupported
  code with diagnostics.

Closure Criteria:

- Selecting a handler and clicking `format!` inserts into that handler body.
- Invalid insertion location produces a recoverable code-panel diagnostic.
- Tests cover snippet insertion target and parser behavior.

## Cross-Feature Gaps

| Gap | Why It Matters |
|---|---|
| ~~No generated-project compile fixture~~ **Closed for key release export paths** — a real `cargo check` runs on a generated crate covering top-level + nested events, async Plain/Result, FilePicker/rfd, channels, one iterator pipeline, one simple local trait binding, and state bindings. | Proves generated export code compiles across those paths, not just matches substrings. |
| Raw Rust strings are mostly unvalidated | Trait impls, iterator expressions, channel types, and handler names can create invalid Rust. |
| No runtime simulation | Users cannot see channels, tasks, iterator outputs, or errors run in preview. |
| No visual connection graph | Stage 11 is mostly forms + overlays, not yet node/edge visual programming. |
| No refactoring model | Renaming fields, handlers, channels, or traits is not propagated as a semantic operation. |

## Recommended Next Work

1. ~~Add a Stage 11 cargo compile fixture~~ **Done for key release paths** — a
   real `cargo check` runs on a generated crate (top-level Button
   Click+DoubleClick, nested TextInput LostFocus + Slider DragStopped, async
   Plain + Result, FilePicker/rfd, mpsc channel fields, iterator method, simple
   local trait binding, and state bindings) via
   `export_compile_fixture_cargo_check` (`#[ignore]`) plus a fast always-run
   smoke. Extend later to project-tree/assets, SVG image export permutations, and
   data widgets.
2. ~~Handler contract consistency~~ **Done** — all event-capable widgets route
   through `handler_call()`; async call sites invoke the launcher; duplicate
   handler conflict is detected and call sites are normalized.
3. ~~Repaint gap~~ **Done** — `ctx.request_repaint_after(16ms)` guard emitted
   while any async task is in flight.
3a. ~~Properties/export event parity~~ **Done (full, all paths)** — export collects
   handlers from every event field and emits one bound-`Response` gate per event;
   all primary AND secondary events (DoubleClick, LostFocus, DragStopped) route
   through `handler_call()` in BOTH the top-level and the nested/frame-child
   (`export_child_line`) export paths. ComboBox/FontComboBox children upgraded
   from dead labels to real interactive combos. Two invariants (top-level +
   nested) over every `(kind, event)` pair fail on any future drift. No event row
   in Properties is ignored by any export path.
4. Add validation helpers for Rust identifiers and simple type strings used by
   channels, pipelines, traits, and handlers.
5. Add a visual Rust Flow panel showing widgets, handlers, channels, tasks,
   results, and AppState fields as nodes/edges.
6. Make Macro Palette insertion target-aware, starting with "insert into active
   handler" rather than append-to-buffer.

## Evaluation Summary

Claude's Stage 11 implementation is not hollow overall: it added real schema,
menus, panels, overlays, and codegen/export paths. But the depth varies sharply.
Ownership overlay and error-mode signatures are the strongest. Channels and
iterator pipelines are useful MVP code-generation helpers. Trait binding and
macro palette are power-user text surfaces.

Async task wiring — previously the clearest overclaim — was resolved: export now
generates a working std-only task pipeline (launcher → `thread::spawn` → `mpsc`
send → `try_recv` drain → running/error status), with an honest UI/worker split
(the worker is a free fn with no `&mut self`). It is now a genuine Functional MVP;
the remaining gaps are auto-binding status to a widget, cancellation/progress, and
a generated-project compile fixture.
