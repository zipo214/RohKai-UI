# Group 3: Performance & Architecture

**Priority:** High (rayon added as core dependency)  
**Effort:** Medium  
**Risk:** Low

**Status Update:** `rayon = "1"` has been added to `Cargo.toml` as a core dependency. This enables app-wide parallel processing across all recommendation areas.

---

## Recommendation 7: Implement Dirty Rectangle Rendering

### Problem

The canvas currently redraws all widgets every frame, even when nothing has changed. For projects with many widgets (100+), this wastes GPU/CPU resources. Egui supports dirty rectangle tracking natively, but RohKai doesn't leverage it.

### Current State

In `src/canvas/interaction.rs`, the `handle()` function redraws everything each frame:
```rust
pub fn handle(ui: &mut Ui, tree: &mut UiTree, ...) {
    let painter = ui.painter();

    // Always draws everything
    draw_background(&painter, rect);
    draw_grid(&painter, rect, ...);
    for widget in &tree.widgets {
        draw_widget(&painter, widget, ...);
    }
    // ...
}
```

### Implementation Plan

#### Step 1: Track Widget Dirty State

Add a dirty tracking mechanism to `RohKaiApp`:

```rust
// In src/app.rs, add to SessionState or a new struct
pub struct DirtyState {
    /// Widgets that have been modified since last frame.
    dirty_widgets: HashSet<Uuid>,
    /// Whether the entire canvas needs redraw (pan/zoom changed).
    full_redraw: bool,
    /// Last known widget states for change detection.
    last_states: HashMap<Uuid, WidgetStateHash>,
}

#[derive(Clone, Copy, PartialEq)]
pub struct WidgetStateHash {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub kind_hash: u64,
    pub props_hash: u64,
}

impl DirtyState {
    pub fn new() -> Self {
        Self {
            dirty_widgets: HashSet::new(),
            full_redraw: false,
            last_states: HashMap::new(),
        }
    }

    /// Check if any widgets changed and mark them dirty.
    pub fn update(&mut self, tree: &UiTree) {
        let current_ids: HashSet<Uuid> = tree.widgets.iter().map(|w| w.id).collect();

        // Check for removed widgets
        self.last_states.retain(|id, _| current_ids.contains(id));

        // Check for changed widgets
        for widget in &tree.widgets {
            let hash = WidgetStateHash::from_widget(widget);
            match self.last_states.entry(widget.id) {
                Entry::Vacant(e) => {
                    // New widget
                    e.insert(hash);
                    self.dirty_widgets.insert(widget.id);
                }
                Entry::Occupied(mut e) => {
                    if *e.get() != hash {
                        self.dirty_widgets.insert(widget.id);
                        *e.get_mut() = hash;
                    }
                }
            }
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.full_redraw || !self.dirty_widgets.is_empty()
    }

    pub fn mark_full_redraw(&mut self) {
        self.full_redraw = true;
    }

    pub fn take_dirty_widgets(&mut self) -> HashSet<Uuid> {
        self.full_redraw = false;
        std::mem::take(&mut self.dirty_widgets)
    }
}
```

#### Step 2: Optimize Canvas Drawing

Modify `src/canvas/interaction.rs` to skip unchanged widgets:

```rust
pub fn handle(ui: &mut Ui, tree: &mut UiTree, ..., dirty_state: &mut DirtyState) {
    let painter = ui.painter();

    // Full redraw needed (pan/zoom changed or first frame)
    if dirty_state.full_redraw {
        draw_background(&painter, rect);
        draw_grid(&painter, rect, ...);
        for widget in &tree.widgets {
            draw_widget(&painter, widget, ...);
        }
    } else {
        // Only redraw dirty widgets
        let dirty_widgets = dirty_state.take_dirty_widgets();
        if !dirty_widgets.is_empty() {
            // Redraw background (always needed for clean slate)
            draw_background(&painter, rect);
            draw_grid(&painter, rect, ...);

            for widget in &tree.widgets {
                if dirty_widgets.contains(&widget.id) {
                    draw_widget(&painter, widget, ...);
                } else {
                    // Widget unchanged - egui will preserve its visual
                    // This is a simplification; egui doesn't actually cache individual widgets
                    // A more sophisticated approach would use egui::LayerId for widget layers
                }
            }
        }
    }

    // Handle selection, resize handles, etc.
    // ...
}
```

#### Step 3: Mark Dirty on Interactions

Update interaction handlers to mark widgets dirty:

```rust
// When a widget is moved
if widget_moved {
    dirty_state.dirty_widgets.insert(widget.id);
    dirty_state.full_redraw = true; // Pan/zoom may have changed
}

// When canvas is panned/zoomed
if panned || zoomed {
    dirty_state.mark_full_redraw();
}
```

### Important Note

Egui's immediate mode doesn't natively support partial redraws in the way a retained mode renderer would. The primary optimization here is skipping unnecessary draw calls for unchanged widgets. For true dirty rectangle rendering, a more significant architectural change would be needed (e.g., using egui layers or a custom rendering approach).

### Alternative: Egui Layers

A more egui-idiomatic approach would use `egui::LayerId` to create separate rendering layers for each widget, allowing egui to cache unchanged layers:

```rust
// Create a layer for each widget
let layer_id = egui::LayerId::new(egui::Order::Middle, egui::Id::new(widget.id));
ui.with_layer_id(layer_id, |ui| {
    draw_widget(ui, widget, ...);
});
```

This approach is more complex but leverages egui's built-in caching.

### Verification

```bash
cargo test                      # All tests pass
cargo run                       # App works correctly
# Manual testing: create 100+ widgets, verify performance improvement
```

---

## Recommendation 8: Implement Parallel SVG Rasterization (rayon)

### Problem

SVG rasterization happens on the UI thread. For complex SVGs or multiple Image widgets, this can cause noticeable frame time spikes. While guardrails exist (byte caps, dimension caps), parallel processing could improve responsiveness.

### Current State

✅ **Rayon has been added to `Cargo.toml` as a core dependency.**

In `src/canvas/interaction.rs`:
```rust
// Synchronous rasterization
let raster = svg_rasterizer::rasterize(&svg_source, width, height);
```

### Implementation Plan

#### Step 1: Update SVG Rasterizer to Use Rayon

Add parallel batch processing to `src/canvas/svg_rasterizer.rs`:

```rust
use rayon::prelude::*;

/// Rasterize multiple SVGs in parallel using rayon.
pub fn rasterize_batch(
    sources: &[(&str, u32, u32)], // (svg_source, width, height)
) -> Vec<Option<egui::ColorImage>> {
    sources
        .par_iter()
        .map(|(svg, w, h)| rasterize(svg, *w, *h).ok())
        .collect()
}
```

#### Step 2: Update Texture Cache to Use Batch Processing

Modify `src/canvas/interaction.rs`:

```rust
fn update_svg_textures(&mut self, ctx: &egui::Context, tree: &UiTree) {
    // Collect SVGs that need rasterization
    let to_rasterize: Vec<(&str, Uuid, u32, u32)> = /* ... */;

    if to_rasterize.is_empty() {
        return;
    }

    // Batch rasterize in parallel
    let sources: Vec<_> = to_rasterize.iter().map(|(s, _, w, h)| (*s, *w, *h)).collect();
    let results = svg_rasterizer::rasterize_batch(&sources);

    // Load textures from results
    for (result, (_, id, _, _)) in results.iter().zip(&to_rasterize) {
        if let Some(image) = result {
            let handle = ctx.load_texture(
                &format!("svg_{}", id),
                image.clone(),
                egui::TextureOptions::LINEAR,
            );
            self.svg_texture_cache.insert(*id, (handle, /* scale */));
        }
    }
}
```

#### Step 2: Create Parallel Rasterization Helper

```rust
// In src/canvas/svg_rasterizer.rs or a new file

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Rasterize multiple SVGs in parallel if the feature is enabled.
#[cfg(feature = "parallel")]
pub fn rasterize_batch(
    sources: &[(&str, &str)], // (svg_source, widget_id)
    dimensions: &[(u32, u32)],
) -> Vec<Option<egui::ColorImage>> {
    sources
        .par_iter()
        .zip(dimensions.par_iter())
        .map(|((svg_source, _), (w, h))| {
            rasterize(svg_source, *w, *h).ok()
        })
        .collect()
}

/// Single SVG rasterization (unchanged).
pub fn rasterize(svg_source: &str, width: u32, height: u32) -> Result<egui::ColorImage, String> {
    // Existing implementation
}
```

#### Step 3: Update Texture Cache to Use Batch Processing

```rust
// In src/canvas/interaction.rs

#[cfg(feature = "parallel")]
fn update_svg_textures(
    &mut self,
    ctx: &egui::Context,
    tree: &UiTree,
) {
    // Collect SVGs that need rasterization
    let mut to_rasterize: Vec<(&str, Uuid, u32, u32)> = Vec::new();

    for widget in &tree.widgets {
        if let WidgetKind::Image = &widget.kind {
            if let Some(svg_source) = &widget.svg_source {
                if let Some(entry) = self.svg_texture_cache.get(&widget.id) {
                    // Check if re-rasterization needed
                    if needs_reraster(entry, widget, ctx) {
                        let (w, h) = compute_raster_size(widget);
                        to_rasterize.push((svg_source, widget.id, w, h));
                    }
                } else {
                    let (w, h) = compute_raster_size(widget);
                    to_rasterize.push((svg_source, widget.id, w, h));
                }
            }
        }
    }

    if to_rasterize.is_empty() {
        return;
    }

    // Rasterize in parallel
    let sources: Vec<&str> = to_rasterize.iter().map(|(s, _, _, _)| *s).collect();
    let dims: Vec<(u32, u32)> = to_rasterize.iter().map(|(_, _, w, h)| (*w, *h)).collect();

    let results = svg_rasterizer::rasterize_batch(
        &sources.iter().zip(std::iter::repeat("")).collect::<Vec<_>>(),
        &dims,
    );

    // Load textures
    for (result, (_, id, _, _)) in results.iter().zip(&to_rasterize) {
        if let Some(image) = result {
            let handle = ctx.load_texture(
                &format!("svg_{}", id),
                image.clone(),
                egui::TextureOptions::LINEAR,
            );
            self.svg_texture_cache.insert(*id, (handle, /* scale */));
        }
    }
}
```

#### Step 4: Fallback for Non-Parallel

```rust
#[cfg(not(feature = "parallel"))]
fn update_svg_textures(&mut self, ctx: &egui::Context, tree: &UiTree) {
    // Existing sequential implementation
    for widget in &tree.widgets {
        // ... existing code
    }
}
```

### Verification

```bash
cargo test                           # All tests pass
cargo clippy -- -D warnings          # Zero warnings
cargo run                            # App works correctly
```

### Performance Considerations

- **Small projects (< 5 Image widgets):** Sequential is fine, rayon's overhead is minimal
- **Medium projects (5-20 Image widgets):** Parallel shows improvement
- **Large projects (20+ Image widgets):** Significant improvement, especially with complex SVGs

### Rayon Benefits (Now Available)

- **Thread pool reuse** — global pool created once, amortized cost
- **Work stealing** — optimal load balancing across CPU cores
- **Zero configuration** — automatically uses available parallelism
- **Battle-tested** — used by rustc, servo, ripgrep, and many production projects

### Alternative: Async Rasterization

A more sophisticated approach would use async rasterization with `tokio` or `async-std`, but this would require significant architectural changes and goes against the project's "no async" principle stated in CLAUDE.md.

---

## Recommendation 9: Design Command Pattern Interface for Undo/Redo

### Problem

The roadmap identifies undo/redo as Stage 14. While implementation is future work, the interface should be designed now to ensure the codebase is prepared. The current in-place mutation model (`tree.get_mut(id)`) makes undo/redo difficult to add later.

### Current State

Widgets are mutated in-place:
```rust
if let Some(widget) = tree.get_mut(id) {
    widget.rect.x = new_x;
    widget.rect.y = new_y;
}
```

### Implementation Plan

#### Step 1: Define Command Trait

Create `src/project/command.rs`:

```rust
//! Command pattern for undo/redo support.
//!
//! This module defines the interface for commands that can be executed,
//! undone, and redone. Implementation is deferred to Stage 14.

use crate::project::ui_tree::UiTree;
use uuid::Uuid;

/// A command that can be executed on a UiTree.
///
/// Commands must be able to execute their effect and undo it.
/// The command stores any state needed for undo/redo internally.
pub trait Command: std::fmt::Debug {
    /// Execute the command on the tree.
    fn execute(&mut self, tree: &mut UiTree);

    /// Undo the command's effect.
    fn undo(&mut self, tree: &mut UiTree);

    /// Returns a human-readable description for UI display.
    fn description(&self) -> &str;

    /// Returns the widget IDs affected by this command.
    fn affected_widgets(&self) -> Vec<Uuid>;
}

/// A stack of commands for undo/redo.
///
/// This is the core of the undo/redo system. It maintains:
/// - A stack of executed commands
/// - A position indicator for the current state
///
/// When a new command is executed, any commands after the current position
/// are discarded (standard undo/redo behavior).
#[derive(Debug, Default)]
pub struct CommandStack {
    commands: Vec<Box<dyn Command>>,
    /// Index of the next command to execute (also = number of executed commands).
    position: usize,
    /// Maximum number of commands to keep (0 = unlimited).
    max_commands: usize,
}

impl CommandStack {
    pub fn new(max_commands: usize) -> Self {
        Self {
            commands: Vec::new(),
            position: 0,
            max_commands,
        }
    }

    /// Execute a command and add it to the stack.
    pub fn execute(&mut self, mut command: Box<dyn Command>, tree: &mut UiTree) {
        command.execute(tree);

        // Discard any commands after current position (branching)
        if self.position < self.commands.len() {
            self.commands.truncate(self.position);
        }

        self.commands.push(command);
        self.position = self.commands.len();

        // Enforce max commands limit
        if self.max_commands > 0 && self.commands.len() > self.max_commands {
            let to_remove = self.commands.len() - self.max_commands;
            self.commands.drain(0..to_remove);
            self.position = self.position.saturating_sub(to_remove);
        }
    }

    /// Undo the last command if possible.
    pub fn undo(&mut self, tree: &mut UiTree) -> bool {
        if self.position == 0 {
            return false;
        }

        self.position -= 1;
        if let Some(command) = self.commands.get_mut(self.position) {
            command.undo(tree);
            true
        } else {
            false
        }
    }

    /// Redo the next command if possible.
    pub fn redo(&mut self, tree: &mut UiTree) -> bool {
        if self.position >= self.commands.len() {
            return false;
        }

        if let Some(command) = self.commands.get_mut(self.position) {
            command.execute(tree);
            self.position += 1;
            true
        } else {
            false
        }
    }

    /// Returns true if undo is available.
    pub fn can_undo(&self) -> bool {
        self.position > 0
    }

    /// Returns true if redo is available.
    pub fn can_redo(&self) -> bool {
        self.position < self.commands.len()
    }

    /// Get the description of the next undo action.
    pub fn undo_description(&self) -> Option<&str> {
        if self.position > 0 {
            self.commands.get(self.position - 1).map(|c| c.description())
        } else {
            None
        }
    }

    /// Get the description of the next redo action.
    pub fn redo_description(&self) -> Option<&str> {
        if self.position < self.commands.len() {
            self.commands.get(self.position).map(|c| c.description())
        } else {
            None
        }
    }

    /// Clear the command stack.
    pub fn clear(&mut self) {
        self.commands.clear();
        self.position = 0;
    }
}
```

#### Step 2: Define Concrete Command Types

```rust
// In src/project/command.rs or separate files

/// Move a widget by a delta.
#[derive(Debug)]
pub struct MoveCommand {
    widget_id: Uuid,
    delta_x: f32,
    delta_y: f32,
}

impl MoveCommand {
    pub fn new(widget_id: Uuid, delta_x: f32, delta_y: f32) -> Self {
        Self { widget_id, delta_x, delta_y }
    }
}

impl Command for MoveCommand {
    fn execute(&mut self, tree: &mut UiTree) {
        if let Some(widget) = tree.get_mut(self.widget_id) {
            widget.rect.x += self.delta_x;
            widget.rect.y += self.delta_y;
        }
    }

    fn undo(&mut self, tree: &mut UiTree) {
        if let Some(widget) = tree.get_mut(self.widget_id) {
            widget.rect.x -= self.delta_x;
            widget.rect.y -= self.delta_y;
        }
    }

    fn description(&self) -> &str {
        "Move"
    }

    fn affected_widgets(&self) -> Vec<Uuid> {
        vec![self.widget_id]
    }
}

/// Resize a widget.
#[derive(Debug)]
pub struct ResizeCommand {
    widget_id: Uuid,
    old_rect: Option<Rect>,
    new_rect: Rect,
}

impl ResizeCommand {
    pub fn new(widget_id: Uuid, new_rect: Rect) -> Self {
        Self {
            widget_id,
            old_rect: None,
            new_rect,
        }
    }
}

impl Command for ResizeCommand {
    fn execute(&mut self, tree: &mut UiTree) {
        if let Some(widget) = tree.get_mut(self.widget_id) {
            self.old_rect = Some(widget.rect.clone());
            widget.rect = self.new_rect.clone();
        }
    }

    fn undo(&mut self, tree: &mut UiTree) {
        if let Some(widget) = tree.get_mut(self.widget_id) {
            if let Some(old) = &self.old_rect {
                widget.rect = old.clone();
            }
        }
    }

    fn description(&self) -> &str {
        "Resize"
    }

    fn affected_widgets(&self) -> Vec<Uuid> {
        vec![self.widget_id]
    }
}

/// Add a widget.
#[derive(Debug)]
pub struct AddCommand {
    widget: WidgetInstance,
    inserted: bool,
}

impl AddCommand {
    pub fn new(widget: WidgetInstance) -> Self {
        Self {
            widget,
            inserted: false,
        }
    }
}

impl Command for AddCommand {
    fn execute(&mut self, tree: &mut UiTree) {
        tree.widgets.push(self.widget.clone());
        self.inserted = true;
    }

    fn undo(&mut self, tree: &mut UiTree) {
        if self.inserted {
            tree.widgets.retain(|w| w.id != self.widget.id);
            self.inserted = false;
        }
    }

    fn description(&self) -> &str {
        "Add Widget"
    }

    fn affected_widgets(&self) -> Vec<Uuid> {
        vec![self.widget.id]
    }
}

/// Remove a widget (and its children).
#[derive(Debug)]
pub struct RemoveCommand {
    widget_id: Uuid,
    removed_widget: Option<WidgetInstance>,
}

impl RemoveCommand {
    pub fn new(widget_id: Uuid) -> Self {
        Self {
            widget_id,
            removed_widget: None,
        }
    }
}

impl Command for RemoveCommand {
    fn execute(&mut self, tree: &mut UiTree) {
        // Find and remove the widget, storing it for undo
        let index = tree.widgets.iter().position(|w| w.id == self.widget_id);
        if let Some(idx) = index {
            self.removed_widget = Some(tree.widgets.remove(idx));
        }
    }

    fn undo(&mut self, tree: &mut UiTree) {
        if let Some(widget) = self.removed_widget.take() {
            tree.widgets.push(widget);
        }
    }

    fn description(&self) -> &str {
        "Delete"
    }

    fn affected_widgets(&self) -> Vec<Uuid> {
        vec![self.widget_id]
    }
}

/// Group widgets into a Frame.
#[derive(Debug)]
pub struct GroupCommand {
    selected_ids: Vec<Uuid>,
    frame_id: Option<Uuid>,
}

impl GroupCommand {
    pub fn new(selected_ids: Vec<Uuid>) -> Self {
        Self { selected_ids, frame_id: None }
    }
}

impl Command for GroupCommand {
    fn execute(&mut self, tree: &mut UiTree) {
        self.frame_id = tree.group(&self.selected_ids);
    }

    fn undo(&mut self, tree: &mut UiTree) {
        if let Some(frame_id) = self.frame_id.take() {
            tree.ungroup(frame_id);
        }
    }

    fn description(&self) -> &str {
        "Group"
    }

    fn affected_widgets(&self) -> Vec<Uuid> {
        self.selected_ids.clone()
    }
}
```

#### Step 3: Add CommandStack to RohKaiApp

```rust
// In src/app.rs

pub struct RohKaiApp {
    // ... existing fields ...
    command_stack: CommandStack,
}

impl RohKaiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // ... existing initialization ...
        Self {
            // ...
            command_stack: CommandStack::new(50), // 50 undo steps
        }
    }

    /// Execute a command through the command stack.
    fn execute_command(&mut self, command: Box<dyn Command>) {
        self.command_stack.execute(command, &mut self.project.ui_tree);
        // Update dirty state
        self.dirty_cache = false;
        self.dirty_cache_checked_at = 0.0;
    }

    /// Undo the last command.
    fn undo(&mut self) {
        if self.command_stack.undo(&mut self.project.ui_tree) {
            // Mark dirty, update UI
        }
    }

    /// Redo the last undone command.
    fn redo(&mut self) {
        if self.command_stack.redo(&mut self.project.ui_tree) {
            // Mark dirty, update UI
        }
    }
}
```

#### Step 4: Update Interaction Handlers

Modify canvas interaction to use commands instead of direct mutation:

```rust
// Instead of:
// if let Some(widget) = tree.get_mut(id) {
//     widget.rect.x = new_x;
// }

// Use:
self.execute_command(Box::new(MoveCommand::new(id, delta_x, delta_y)));
```

### Verification

This is a design-only recommendation. Implementation is deferred to Stage 14.

```bash
cargo check    # Interface compiles
cargo test     # Existing tests pass (no behavior change yet)
```

### Future Work (Stage 14)

1. Implement remaining command types (property changes, z-order, etc.)
2. Wire keyboard shortcuts (Ctrl+Z, Ctrl+Y)
3. Add undo/redo indicators to the UI
4. Consider snapshot optimization (full tree snapshots every N commands)
5. Persist undo stack to project file (optional)

### Benefits of Designing Now

- **Clean interface** — Commands are well-defined before being scattered across codebase
- **No refactoring debt** — Existing mutation sites can be converted incrementally
- **Testable** — Command trait can be unit tested independently
- **Extensible** — New command types are easy to add

---

## Summary

| Recommendation | Files Changed | Lines Added | Lines Removed | Risk |
|---------------|---------------|-------------|---------------|------|
| 7. Dirty rectangle rendering | 2-3 | ~150 | ~20 | Medium |
| 8. Parallel SVG rasterization | 2-3 | ~100 | ~10 | Low |
| 9. Command pattern design | 2-3 | ~400 | 0 | None |
| **Total** | **~6 files** | **~650** | **~30** | **Low-Medium** |

### Implementation Priority

1. **Recommendation 9** (Command Pattern) — Design now, implement later. Zero risk, high future value.
2. **Recommendation 8** (Parallel SVG) — Only if users report performance issues with many Image widgets.
3. **Recommendation 7** (Dirty Rectangles) — Complex, egui doesn't fully support this pattern. Consider only for very large projects.

### Notes

- All three recommendations are performance/architecture improvements for future scale
- The current codebase performs well for typical use cases (< 50 widgets)
- These optimizations become relevant at 100+ widgets or with heavy SVG usage
- Command pattern design should be done early even if implementation is deferred