# Stage 11 — Rust-Centric Visual Features — Implementation Plan

## Context & Philosophy

Stages 9–10 grew RohKai's **widget** vocabulary. Stage 11 grows its **Rust**
vocabulary: features that make the designer aware of Rust-specific structure
(ownership, channels, async, error flow, iterators, traits, macros) and either
*visualize* it on the canvas or *generate* it into the exported project.

Two constraints shape every decision here:

1. **`UiTree` stays the single source of truth.** New wiring data lives on
   `AppProps.rust_wiring` (a serde-persisted struct) or as per-widget fields —
   never as parallel state. Overlays are pure read-only views.
2. **No new crates, sync core.** CLAUDE.md forbids a tokio runtime "unless a
   specific planned feature explicitly requires it." Async wiring does **not**
   require it: we generate the `std::thread::spawn` + `std::sync::mpsc` pattern
   (already the approved background-task model). The roadmap's "tokio::spawn or
   similar" is satisfied by the *or similar* — a std-only pattern that compiles
   with zero extra dependencies. This is a deliberate, constraint-driven choice.

The 7 features split into two kinds:

| Kind | Features | Mechanism |
|------|----------|-----------|
| **Overlays** (read-only views) | Ownership viz, Error-flow viz | Canvas painters toggled from View menu; driven by `field_collector` + per-widget annotations |
| **Wiring** (authoring + codegen) | Async tasks, Channels, Iterator pipelines, Trait bindings, Macro palette | `AppProps.rust_wiring` + per-widget fields → new `codegen/rust_wiring.rs` → live preview + export |

---

## Per-Feature: Function · Depth · Implementation · UX · Impact

### 1. Ownership Visualization

- **Function:** Show which widget writes which `AppState` field, making data
  ownership visible at a glance.
- **Depth:** Read-only overlay. Accurate — reuses `field_collector::collect` so
  it never diverges from generated code. Color-coded by Rust type
  (String/f32/bool). A legend lists every field and its owning widget(s).
- **Implementation:** `canvas/overlays.rs::draw_ownership(painter, tree, …)`.
  For each widget with a `state_binding`, draw a small badge `→ field: T` at the
  widget's corner. No schema change.
- **UX:** View → "Show Ownership (Ctrl+Shift+O)". Toggled overlay; badges appear
  over the canvas; a corner legend summarizes fields. Off by default.
- **Impact:** Pure addition. Helps users understand binding wiring before export.

### 2. Async Task Wiring

- **Function:** Mark a widget's event handler to run on a background thread so
  the UI never blocks; results flow back via a channel.
- **Depth:** Generates a real, compilable std pattern: the handler body runs in
  `std::thread::spawn`, a `std::sync::mpsc::Sender<T>` reports completion, and
  `update()` drains the receiver. No tokio.
- **Implementation:** `WidgetInstance.async_handler: bool`. When set and a
  handler exists, `rust_wiring::async_handler_call(...)` emits the spawn wrapper;
  the handler stub signature stays `fn h(&mut self)` but gains a `// runs on
  background thread` note and a paired `*_done` channel field.
- **UX:** Properties panel → "⚙ Run async (background thread)" checkbox, shown
  only when the widget has a handler. Generated code visibly changes in the code
  panel.
- **Impact:** Per-widget; opt-in; affects codegen only when enabled.

### 3. Channel Connections

- **Function:** Declare named `mpsc` channels the app can use to pass data
  between threads/components.
- **Depth:** Emits `let (name_tx, name_rx) = std::sync::mpsc::channel::<T>();`
  wiring into `ExportedApp`, with `name_tx`/`name_rx` fields and a drain helper.
- **Implementation:** `RustWiring.channels: Vec<ChannelDef{ id, name, ty }>`.
  `rust_wiring::channel_fields()` and `channel_init()` feed the export AppState/
  ExportedApp blocks.
- **UX:** Rust Wiring window → "Channels" section: add/remove rows (name + type).
- **Impact:** Adds fields to the exported app struct; inert until used.

### 4. Error Propagation Visualization

- **Function:** Make each handler's error-handling contract explicit and visible.
- **Depth:** Per-handler result mode (`Plain | Result | Option`). The handler
  stub signature reflects it (`-> Result<(), String>` / `-> Option<()>`), and the
  call site wraps appropriately (`if let Err(e) = self.h() { … }`). An overlay
  badges each handler widget with its mode.
- **Implementation:** `WidgetInstance.handler_result: HandlerResult` (enum,
  serde-default `Plain`). Threaded through stub generation in egui_emitter +
  export. `overlays.rs::draw_error_flow`.
- **UX:** Properties → "Error mode" dropdown (Plain/Result/Option) next to the
  handler field. View → "Show Error Flow" overlay.
- **Impact:** Changes handler stub signature + call site when non-Plain.

### 5. Iterator Pipeline Builder

- **Function:** Build a `.iter().map().filter().collect()` chain visually and
  generate the method.
- **Depth:** Emits a real method:
  `fn pipeline_name(&self) -> Vec<_> { self.source.iter().map(|x| expr).filter(|x| expr).collect() }`.
  Ops are ordered; each carries a Rust expression string.
- **Implementation:** `RustWiring.iterators: Vec<IteratorPipeline{ id, name,
  source, ops: Vec<IterOp> }>`, `IterOp = Map(String)|Filter(String)`.
  `rust_wiring::iterator_methods()` emits the impl methods.
- **UX:** Rust Wiring window → "Iterator Pipelines" section: name, source
  expression, ordered op list (add Map/Filter, edit expr, remove), live preview
  of the generated expression.
- **Impact:** Appends methods to `impl ExportedApp`; never auto-called.

### 6. Trait Binding

- **Function:** Attach a trait implementation to the app's behavior.
- **Depth:** Emits an `impl TraitName for ExportedApp { fn method(&mut self) {
  body } }` block from user-supplied trait name, method, and body.
- **Implementation:** `RustWiring.trait_impls: Vec<TraitImpl{ id, trait_name,
  method, body }>`. `rust_wiring::trait_impls()` emits the blocks.
- **UX:** Rust Wiring window → "Trait Impls" section: trait name, method
  signature, body editor.
- **Impact:** Appends impl blocks to the exported file.

### 7. Macro Palette

- **Function:** Quick insertion of common Rust macros into handler/code bodies.
- **Depth:** A palette of snippets (`vec![]`, `format!("{}", x)`, `println!`,
  `dbg!`, `todo!()`, `assert!`) that insert into the live (Lazare) code buffer at
  its end, ready to edit. Real output = code buffer mutation that round-trips
  through the existing parser.
- **Implementation:** `panels/macro_palette.rs` — a window listing macros;
  clicking appends the snippet to `CodePanelState.buffer`.
- **UX:** View → "Macro Palette"; click a macro → snippet appears in the code
  panel.
- **Impact:** Edits the code buffer only; no schema change.

---

## Data Model (schema.rs)

```rust
// AppProps gains:
pub rust_wiring: RustWiring          // #[serde(default)]

pub struct RustWiring {
    pub channels: Vec<ChannelDef>,
    pub iterators: Vec<IteratorPipeline>,
    pub trait_impls: Vec<TraitImpl>,
}
pub struct ChannelDef { id: Uuid, name: String, ty: String }
pub struct IteratorPipeline { id: Uuid, name: String, source: String, ops: Vec<IterOp> }
pub enum IterOp { Map(String), Filter(String) }
pub struct TraitImpl { id: Uuid, trait_name: String, method: String, body: String }

// WidgetInstance gains (both #[serde(default)]):
pub async_handler: bool
pub handler_result: HandlerResult      // Plain | Result | Option
```

All new fields use `#[serde(default, skip_serializing_if = …)]` so every
existing `.rohkai.json` loads unchanged.

---

## Codegen (`src/codegen/rust_wiring.rs`, new)

Pure functions, unit-tested without egui:

| Function | Emits |
|----------|-------|
| `channel_struct_fields(&RustWiring)` | `name_tx: Sender<T>, name_rx: Receiver<T>` field lines |
| `channel_init_lines(&RustWiring)` | `let (name_tx, name_rx) = mpsc::channel();` |
| `iterator_methods(&RustWiring)` | `fn name(&self) -> Vec<_> { … }` blocks |
| `trait_impl_blocks(&RustWiring)` | `impl Trait for ExportedApp { … }` blocks |
| `async_wrap(handler, result_mode)` | spawn wrapper / error-wrapped call-site |
| `handler_signature(handler, result_mode)` | `fn h(&mut self) -> Result<(),String>` etc. |

Integration points (existing code):
- `state_emitter.rs` / `export.rs` AppState build → append channel fields.
- `egui_emitter.rs` + `export.rs` handler call sites → use `async_wrap` /
  error-mode call form.
- `export.rs` handler-stub block → use `handler_signature`; append
  `iterator_methods` + `trait_impl_blocks` after stubs.

---

## UI Surfaces

- **`panels/rust_wiring.rs`** — one floating window, sectioned (Channels /
  Iterator Pipelines / Trait Impls). Edits `AppProps.rust_wiring`.
- **`panels/macro_palette.rs`** — macro snippet window → code buffer.
- **`canvas/overlays.rs`** — `draw_ownership`, `draw_error_flow`.
- **Properties panel** — async checkbox + error-mode dropdown on handler-bearing widgets.
- **View menu** — toggles: Show Ownership, Show Error Flow, Rust Wiring…, Macro Palette.
- **SessionState** — `show_ownership: bool`, `show_error_flow: bool`,
  `rust_wiring_open: bool`, `macro_palette_open: bool`.

---

## End-User Walkthrough

1. User builds a form (TextInput → `username`, Slider → `volume`).
2. Toggles **Show Ownership** → badges show `→ username: String`, `→ volume: f32`;
   legend lists both. They immediately see what state exists.
3. Gives a Button an `on_save` handler, ticks **Run async**, sets **Error mode =
   Result**. Code panel now shows the click spawning a thread and
   `fn on_save(&mut self) -> Result<(), String>`.
4. Opens **Rust Wiring…**, adds a channel `progress: f32`, an iterator pipeline
   `active_users = users .filter(|u| u.active) .collect`, and a trait impl.
5. Exports → the project compiles with the channel fields, the iterator method,
   the trait impl, and the async handler — zero manual edits to wire them.

---

## Build Order (clusters, each: implement → `cargo test`/`clippy` → commit)

1. **Schema + overlays** — `RustWiring`/per-widget fields; `overlays.rs`
   (ownership + error-flow); View toggles; SessionState. (visual, low-risk first)
2. **Async + error codegen** — `rust_wiring.rs` async_wrap/handler_signature;
   egui_emitter + export integration; properties controls.
3. **Channels + iterators + traits** — remaining `rust_wiring.rs` emitters;
   `panels/rust_wiring.rs` editor window; state/export integration.
4. **Macro palette** — `panels/macro_palette.rs`; View toggle.
5. **Docs** — ROADMAP ticks, CODE_COOP, CODE_INDEX, ARCHITECTURE.

## Verification

`cargo test` (emitter + schema round-trip tests per cluster),
`cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo run` smoke
(toggle each overlay, open each window, export a project and confirm it compiles).

## Risks

| Risk | Mitigation |
|------|------------|
| Generated async/channel code doesn't compile | Keep patterns std-only + minimal; add an export round-trip test asserting key tokens |
| Overlay clutter at high widget counts | Off by default; badges small; legend scrolls |
| Scope creep across 7 features | Each capped to one real vertical slice; trait/iterator bodies are user-authored strings, not a full expression builder |
| serde back-compat | All new fields `#[serde(default)]`; existing-save load test |
