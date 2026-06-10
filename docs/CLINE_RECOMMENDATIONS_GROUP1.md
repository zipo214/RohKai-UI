# Group 1: Code Quality & Maintainability

**Priority:** High  
**Effort:** Low-Medium  
**Risk:** Low

---

## Recommendation 1: Extract Handler Resolution to Shared Module

### Problem

Handler resolution functions (`resolve_handler_click` and `resolve_handler_change`) are duplicated between `src/codegen/egui_emitter.rs` and `src/codegen/export.rs`. This creates maintenance burden — any change to handler logic must be applied in two places.

### Current State

```rust
// In egui_emitter.rs (~line 520)
fn resolve_handler_click(w: &WidgetInstance) -> Option<&str> { ... }
fn resolve_handler_change(w: &WidgetInstance) -> Option<&str> { ... }

// In export.rs (~line 600)
fn resolve_export_handler_click(w: &WidgetInstance) -> Option<&str> { ... }
fn resolve_export_handler_change(w: &WidgetInstance) -> Option<&str> { ... }
```

### Implementation Plan

#### Step 1: Create `src/codegen/handlers.rs`

```rust
//! Shared handler resolution for widget events.
//!
//! Used by both live codegen (egui_emitter) and export codegen to resolve
//! widget event handlers (on_click for buttons, on_change for interactive widgets).

use crate::project::schema::WidgetInstance;

/// Resolve the click handler for a Button widget.
///
/// Returns the handler name if `on_click` is non-empty, None otherwise.
pub fn resolve_click_handler(w: &WidgetInstance) -> Option<&str> {
    if w.on_click.is_empty() {
        return None;
    }
    Some(w.on_click.as_str())
}

/// Resolve the change handler for interactive widgets.
///
/// Returns the handler name if `on_change` is non-empty, None otherwise.
pub fn resolve_change_handler(w: &WidgetInstance) -> Option<&str> {
    if w.on_change.is_empty() {
        return None;
    }
    Some(w.on_change.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::WidgetInstance;

    #[test]
    fn click_handler_returns_some_when_set() {
        let w = WidgetInstance {
            on_click: "on_button_click".into(),
            ..Default::default()
        };
        assert_eq!(resolve_click_handler(&w), Some("on_button_click"));
    }

    #[test]
    fn click_handler_returns_none_when_empty() {
        let w = WidgetInstance::default();
        assert_eq!(resolve_click_handler(&w), None);
    }

    #[test]
    fn change_handler_returns_some_when_set() {
        let w = WidgetInstance {
            on_change: "on_slider_change".into(),
            ..Default::default()
        };
        assert_eq!(resolve_change_handler(&w), Some("on_slider_change"));
    }

    #[test]
    fn change_handler_returns_none_when_empty() {
        let w = WidgetInstance::default();
        assert_eq!(resolve_change_handler(&w), None);
    }
}
```

#### Step 2: Update `src/codegen/mod.rs`

Add the new module:
```rust
pub mod handlers;  // Add this line after existing module declarations
```

#### Step 3: Update `src/codegen/egui_emitter.rs`

Remove the local `resolve_handler_click` and `resolve_handler_change` functions and replace with:
```rust
use crate::codegen::handlers::{resolve_click_handler, resolve_change_handler};
```

Update all call sites:
- Replace `resolve_handler_click(w)` with `resolve_click_handler(w)`
- Replace `resolve_handler_change(w)` with `resolve_change_handler(w)`

#### Step 4: Update `src/codegen/export.rs`

Remove the local `resolve_export_handler_click` and `resolve_export_handler_change` functions and replace with:
```rust
use crate::codegen::handlers::{resolve_click_handler, resolve_change_handler};
```

Update all call sites:
- Replace `resolve_export_handler_click(w)` with `resolve_click_handler(w)`
- Replace `resolve_export_handler_change(w)` with `resolve_change_handler(w)`

### Verification

```bash
cargo test                      # All tests pass
cargo clippy -- -D warnings     # Zero warnings
cargo fmt --check               # Formatting clean
cargo run                       # App launches and works correctly
```

### Rollback Plan

If issues arise, simply revert the changes to `egui_emitter.rs` and `export.rs` to restore the local functions. The new `handlers.rs` module can be deleted.

---

## Recommendation 2: Add Module-Level Documentation

### Problem

The modules `canvas/`, `panels/`, and `widgets/` lack module-level documentation (`//!` comments). This makes it harder for contributors to understand the purpose and architecture of each module.

### Current State

```rust
// src/canvas/mod.rs (current)
pub mod interaction;
pub mod rulers;
pub mod svg_rasterizer;
pub mod widget_instance;
```

### Implementation Plan

#### Step 1: Update `src/canvas/mod.rs`

```rust
//! Canvas rendering and interaction system.
//!
//! This module handles all visual rendering of widgets on the design canvas,
//! including selection, drag/drop, resize, pan/zoom, smart guides, and rulers.
//!
//! # Architecture
//!
//! The canvas operates in two phases each frame:
//!
//! 1. **Render phase** ([`interaction::handle`]): Draw background, grid, widgets,
//!    selection handles, and guides using egui's painter API.
//! 2. **Interaction phase**: Process mouse/keyboard input to mutate the `UiTree`
//!    in-place via `tree.get_mut(id)`.
//!
//! # Key Types
//!
//! - [`interaction::handle`] — Main entry point; renders canvas and processes input
//! - [`interaction::CanvasSettings`] — Zoom, pan, snap configuration
//! - [`rulers::draw`] — Pixel rulers with guide line creation
//! - [`svg_rasterizer`] — Zero-dependency SVG renderer for Image widgets
//!
//! # Coordinate System
//!
//! The canvas uses a dual coordinate system:
//!
//! - **Schema coordinates**: Stored in `WidgetInstance.rect` (canvas-local pixels)
//! - **Screen coordinates**: Computed by applying zoom and pan transforms
//!
//! The [`widget_instance::canvas_rect`] function converts schema → screen coords.
//!
//! # Submodules
//!
//! - `interaction` — Main canvas rendering and input handling
//! - `rulers` — Pixel rulers and guide line management
//! - `svg_rasterizer` — SVG rendering for Image widget preview
//! - `widget_instance` — Coordinate conversion helpers

pub mod interaction;
pub mod rulers;
pub mod svg_rasterizer;
pub mod widget_instance;
```

#### Step 2: Update `src/panels/mod.rs`

```rust
//! UI panels surrounding the canvas.
//!
//! This module contains all side panels and toolbars that frame the central
//! canvas area. Each panel is responsible for a specific aspect of the designer UI.
//!
//! # Panels
//!
//! - [`palette`] — Widget palette (left panel, top section). Click or drag to add widgets.
//! - [`properties`] — Property inspector (left panel, bottom section). Edit selected widget.
//! - [`code_preview`] — Live/editable code panel (right panel). Shows generated Rust code.
//! - [`templates`] — Template browser (below properties). Load saved widget groups.
//! - [`descriptor_editor`] — Full `.rkwd` descriptor editor for custom widgets.
//! - [`widget_builder`] — Guided beginner descriptor builder.
//!
//! # Action Pattern
//!
//! Each panel returns an action enum that `RohKaiApp` routes to commands:
//!
//! ```ignore
//! match panel.show(ui) {
//!     PanelAction::None => {}
//!     PanelAction::Save => app.cmd_save(),
//!     PanelAction::Export => app.cmd_export(),
//!     // ...
//! }
//! ```
//!
//! # Submodules
//!
//! - `code_preview` — Live/editable generated code panel
//! - `descriptor_editor` — Full power-user `.rkwd` editor
//! - `palette` — Widget palette with categorized buttons
//! - `properties` — Property inspector for selected widgets
//! - `templates` — Template file browser and SVG import
//! - `widget_builder` — Guided beginner descriptor builder

pub mod code_preview;
pub mod descriptor_editor;
pub mod palette;
pub mod properties;
pub mod templates;
pub mod widget_builder;
```

#### Step 3: Update `src/widgets/mod.rs`

```rust
//! Widget default instances and palette defaults.
//!
//! Each built-in widget kind has a dedicated module providing:
//!
//! - `default_instance()` — Creates a new widget with sensible defaults
//! - `palette_defaults()` — Returns the label shown in the palette
//!
//! # Widget Kinds
//!
//! ## Original Five (Stage 1)
//! - [`button`] — Clickable button with optional handler
//! - [`label`] — Static or bound text label
//! - [`text_input`] — Single-line text input field
//! - [`slider`] — Numeric slider with range
//! - [`checkbox`] — Boolean toggle
//!
//! ## Stage 5 Additions
//! - [`frame`] — Container for grouping widgets
//! - [`combo_box`] — Dropdown selection
//! - [`radio_button`] — Mutually exclusive option
//! - [`progress_bar`] — Visual progress indicator
//!
//! ## Stage 7 Additions
//! - `Image` — SVG image with source-backed preview (handled in canvas, not here)
//!
//! # Custom Widgets
//!
//! Custom widgets are handled via `.rkwd` descriptors loaded at runtime,
//! not through this module. See [`crate::codegen::widget_descriptor`].
//!
//! # Submodules

pub mod button;
pub mod checkbox;
pub mod combo_box;
pub mod frame;
pub mod label;
pub mod progress_bar;
pub mod radio_button;
pub mod slider;
pub mod text_input;
```

### Verification

```bash
cargo doc --no-deps         # Generates clean documentation
cargo fmt --check           # Formatting clean
cargo check                 # No compilation issues
```

### Additional Benefit

Running `cargo doc --open` after this change will produce professional API documentation that can be shared with contributors.

---

## Recommendation 3: Add Simple Codegen Memoization

### Problem

`egui_emitter::emit_indexed()` is called every frame, regenerating all code even when the `UiTree` hasn't changed. For projects with many widgets (100+), this could cause noticeable CPU usage.

### Current State

In `src/app.rs`, the code panel rendering section calls:
```rust
let indexed_lines = egui_emitter::emit_indexed(&self.project.ui_tree);
```
This happens every frame, even when nothing changed.

### Implementation Plan

#### Step 1: Add Cache Structure to `src/app.rs`

Add a new struct for caching codegen results:

```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Cached codegen state to avoid regenerating code every frame.
struct CodegenCache {
    /// Hash of the UiTree state when cache was generated.
    tree_hash: u64,
    /// Cached indexed lines from egui_emitter.
    indexed_lines: Vec<(Option<Uuid>, String)>,
}

impl CodegenCache {
    fn new() -> Self {
        Self {
            tree_hash: 0,
            indexed_lines: Vec::new(),
        }
    }

    /// Compute a hash of the UiTree fields that affect codegen output.
    fn compute_tree_hash(tree: &crate::project::ui_tree::UiTree) -> u64 {
        let mut hasher = DefaultHasher::new();
        // Hash widget count first (quick rejection)
        tree.widgets.len().hash(&mut hasher);
        // Hash key fields that affect codegen output
        for w in &tree.widgets {
            w.id.hash(&mut hasher);
            // Kind discriminant
            std::mem::discriminant(&w.kind).hash(&mut hasher);
            // Geometry
            w.rect.x.to_bits().hash(&mut hasher);
            w.rect.y.to_bits().hash(&mut hasher);
            w.rect.w.to_bits().hash(&mut hasher);
            w.rect.h.to_bits().hash(&mut hasher);
            // Content
            w.props.label.hash(&mut hasher);
            w.state_binding.hash(&mut hasher);
            // Handlers
            w.on_click.hash(&mut hasher);
            w.on_change.hash(&mut hasher);
            // Properties that affect codegen
            w.tooltip.hash(&mut hasher);
            w.enabled.hash(&mut hasher);
            w.fg_color.hash(&mut hasher);
            w.bg_color.hash(&mut hasher);
            w.corner_radius.map(|r| r.to_bits()).hash(&mut hasher);
            w.font_size.map(|r| r.to_bits()).hash(&mut hasher);
            w.text_align.hash(&mut hasher);
            w.label_binding.hash(&mut hasher);
            w.props.min.to_bits().hash(&mut hasher);
            w.props.max.to_bits().hash(&mut hasher);
            w.props.step.map(|r| r.to_bits()).hash(&mut hasher);
            w.props.show_value.hash(&mut hasher);
            w.props.orientation.hash(&mut hasher);
            w.props.placeholder.hash(&mut hasher);
            w.props.password_mode.hash(&mut hasher);
            w.props.options.hash(&mut hasher);
            w.props.radio_value.hash(&mut hasher);
            w.props.group_binding.hash(&mut hasher);
            w.props.show_percentage.hash(&mut hasher);
            w.props.animated.hash(&mut hasher);
            w.props.inner_margin.to_bits().hash(&mut hasher);
            w.props.stroke_color.hash(&mut hasher);
            w.props.stroke_width.to_bits().hash(&mut hasher);
            // Custom widget fields
            w.descriptor_name.hash(&mut hasher);
            w.descriptor_live_tpl.hash(&mut hasher);
            w.descriptor_export_tpl.hash(&mut hasher);
            // Note: descriptor_props HashMap iteration order is not deterministic,
            // so we hash the sorted entries
            let mut sorted_props: Vec<_> = w.descriptor_props.iter().collect();
            sorted_props.sort_by_key(|(k, _)| *k);
            for (k, v) in &sorted_props {
                k.hash(&mut hasher);
                v.hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}
```

#### Step 2: Add Cache Field to `RohKaiApp`

In the `RohKaiApp` struct, add:
```rust
pub struct RohKaiApp {
    // ... existing fields ...
    codegen_cache: CodegenCache,
}
```

Initialize in `new()`:
```rust
codegen_cache: CodegenCache::new(),
```

#### Step 3: Update Code Panel Rendering

Replace the direct call to `emit_indexed` with cached version:

```rust
// In the code panel rendering section of update():
let tree_hash = CodegenCache::compute_tree_hash(&self.project.ui_tree);
let indexed_lines = if self.codegen_cache.tree_hash == tree_hash && !self.codegen_cache.indexed_lines.is_empty() {
    &self.codegen_cache.indexed_lines
} else {
    // Regenerate and update cache
    self.codegen_cache.indexed_lines = egui_emitter::emit_indexed(&self.project.ui_tree);
    self.codegen_cache.tree_hash = tree_hash;
    &self.codegen_cache.indexed_lines
};
```

### Verification

```bash
cargo test                      # All tests pass
cargo clippy -- -D warnings     # Zero warnings
cargo fmt --check               # Formatting clean
cargo run                       # App works correctly, code panel updates
```

### Performance Measurement

To verify the improvement, you can add a simple timer:
```rust
let start = std::time::Instant::now();
let indexed_lines = /* cached or regenerated */;
let elapsed = start.elapsed();
if elapsed.as_millis() > 5 {
    eprintln!("Codegen took {:?} (cache {})", elapsed, if cached { "hit" } else { "miss" });
}
```

### Limitations

- Hash computation iterates all widgets, so for very small projects (< 10 widgets) the overhead may exceed the savings
- Cache is per-session; not persisted across restarts (appropriate for ephemeral UI state)
- If hash collisions occur (extremely unlikely with DefaultHasher), stale code may be shown briefly

---

## Summary

| Recommendation | Files Changed | Lines Added | Lines Removed | Risk |
|---------------|---------------|-------------|---------------|------|
| 1. Handler extraction | 4 | ~60 | ~30 | Low |
| 2. Module documentation | 3 | ~150 | 0 | None |
| 3. Codegen memoization | 1 | ~80 | ~5 | Low |
| **Total** | **4 unique files** | **~290** | **~35** | **Low** |

All changes are backward-compatible and can be rolled back independently.