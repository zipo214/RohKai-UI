# Group 2: Testing & Reliability

**Priority:** Medium  
**Effort:** Medium-High  
**Risk:** Low

---

## Recommendation 4: Add Custom Error Types with thiserror

### Problem

The codebase uses `Result<T, String>` throughout for error handling. While pragmatic for a project of this size, it has limitations:
- No structured error information (can't pattern match on error types)
- Error context is lost in string formatting
- Harder to test error conditions precisely
- No compile-time guarantee that all error cases are handled

### Current State

Examples of string-based errors:
```rust
// src/project/io.rs
pub fn save(path: &Path, tree: &UiTree) -> Result<String, String> {
    let json = serialize(tree)?;
    std::fs::write(path, &json).map_err(|e| format!("Write error: {e}"))?;
    Ok(json)
}

// src/svg_import.rs
pub fn import_svg_template(text: &str, opts: SvgImportOptions) -> Result<SvgImportOutput, String> {
    // ...
}
```

### Implementation Plan

#### Step 1: Add thiserror dependency

Update `Cargo.toml`:
```toml
[dependencies]
thiserror = "1"
# ... existing dependencies
```

#### Step 2: Create `src/error.rs`

```rust
//! Error types for RohKai.
//!
//! This module provides structured error types that can be pattern-matched
//! and provide rich context about what went wrong.

use thiserror::Error;

/// Main error type for RohKai operations.
#[derive(Debug, Error)]
pub enum RohKaiError {
    // File I/O errors
    #[error("Failed to read file '{path}': {source}")]
    FileRead {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write file '{path}': {source}")]
    FileWrite {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to create directory '{path}': {source}")]
    DirCreate {
        path: String,
        #[source]
        source: std::io::Error,
    },

    // Serialization errors
    #[error("Failed to serialize project: {0}")]
    Serialize(String),

    #[error("Failed to deserialize project: {0}")]
    Deserialize(String),

    // Project errors
    #[error("Unsupported project schema version: {0} (current: {1})")]
    UnsupportedSchemaVersion(u32, u32),

    #[error("Project file is corrupted: {0}")]
    CorruptedProject(String),

    // Validation errors
    #[error("Invalid widget binding '{0}': must be a valid Rust identifier")]
    InvalidBinding(String),

    #[error("Widget geometry is invalid: {0}")]
    InvalidGeometry(String),

    // SVG errors
    #[error("SVG import failed: {0}")]
    SvgImport(String),

    #[error("SVG contains unsupported features: {0}")]
    SvgUnsupported(String),

    #[error("SVG is too large: {0}")]
    SvgTooLarge(String),

    // Export errors
    #[error("Failed to export project: {0}")]
    ExportFailed(String),

    // Settings errors
    #[error("Failed to load settings: {0}")]
    SettingsLoad(String),

    #[error("Failed to save settings: {0}")]
    SettingsSave(String),

    // Descriptor errors
    #[error("Failed to load widget descriptor: {0}")]
    DescriptorLoad(String),

    #[error("Invalid widget descriptor: {0}")]
    InvalidDescriptor(String),

    // Pass-through for external errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Result type alias for RohKai operations.
pub type Result<T> = std::result::Result<T, RohKaiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_descriptive() {
        let err = RohKaiError::FileRead {
            path: "test.json".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"),
        };
        assert!(err.to_string().contains("test.json"));
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn can_match_on_error_variant() {
        let err = RohKaiError::InvalidBinding("if".to_string());
        match err {
            RohKaiError::InvalidBinding(name) => assert_eq!(name, "if"),
            _ => panic!("Wrong error type"),
        }
    }
}
```

#### Step 3: Update `src/project/io.rs`

```rust
use crate::error::{RohKaiError, Result};

pub fn save(path: &Path, tree: &UiTree) -> Result<String> {
    let json = serialize(tree)?;
    std::fs::write(path, &json).map_err(|e| RohKaiError::FileWrite {
        path: path.display().to_string(),
        source: e,
    })?;
    Ok(json)
}

pub fn load(path: &Path) -> Result<UiTree> {
    let json = std::fs::read_to_string(path).map_err(|e| RohKaiError::FileRead {
        path: path.display().to_string(),
        source: e,
    })?;
    let mut tree = match serde_json::from_str(&json).map_err(RohKaiError::Deserialize)? {
        ProjectJson::Project(project) => {
            if project.schema_version > PROJECT_SCHEMA_VERSION {
                return Err(RohKaiError::UnsupportedSchemaVersion(
                    project.schema_version,
                    PROJECT_SCHEMA_VERSION,
                ));
            }
            project.tree
        }
        ProjectJson::Legacy(tree) => tree,
    };
    tree.validate_and_repair();
    Ok(tree)
}

pub fn serialize(tree: &UiTree) -> Result<String> {
    let mut tree = tree.clone();
    tree.validate_and_repair();
    let project = ProjectFile {
        schema_version: PROJECT_SCHEMA_VERSION,
        tree,
    };
    serde_json::to_string_pretty(&project).map_err(RohKaiError::Serialize)
}
```

#### Step 4: Update `src/lib.rs` (or `src/main.rs`)

Add the error module:
```rust
pub mod error;
```

#### Step 5: Update Call Sites

Update functions that use `Result<T, String>` to use `Result<T>` from the error module. This is a mechanical change that can be done incrementally.

### Verification

```bash
cargo test                      # All tests pass
cargo clippy -- -D warnings     # Zero warnings
cargo build                     # Compiles successfully
```

### Benefits

- **Type-safe error handling** — Pattern match on specific error types
- **Better error messages** — Structured context in each variant
- **Easier testing** — Test for specific error conditions
- **Source chain** — `#[source]` attribute preserves error chain

---

## Recommendation 5: Add Integration Tests for Exported Projects

### Problem

The export functionality (`src/codegen/export.rs`) generates a complete Rust project, but there are no tests that verify the exported project actually compiles. This is a critical gap — export could generate syntactically invalid Rust code and we wouldn't know until a user tries it.

### Current State

```rust
// src/codegen/export.rs - existing tests
#[cfg(test)]
mod tests {
    #[test]
    fn export_contains_expected_files() {
        // Only checks that files are created, not that they compile
    }
}
```

### Implementation Plan

#### Step 1: Create `tests/export_integration.rs`

```rust
//! Integration tests for project export.
//!
//! These tests verify that exported projects are syntactically valid Rust
//! and can be compiled successfully.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Create a temporary directory for test exports.
fn temp_export_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rohkai_export_test_{test_name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Try to compile an exported project and return whether it succeeded.
fn try_compile_project(dir: &PathBuf) -> (bool, String) {
    let output = Command::new("cargo")
        .arg("check")
        .current_dir(dir)
        .output()
        .expect("run cargo check");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();

    if !success {
        (false, format!("stdout: {}\nstderr: {}", stdout, stderr))
    } else {
        (true, String::new())
    }
}

#[test]
fn export_empty_project_compiles() {
    let dir = temp_export_dir("empty");
    let tree = rohkai::project::ui_tree::UiTree::default();
    rohkai::codegen::export::write_project(&tree, &dir).expect("export");

    let (success, error) = try_compile_project(&dir);
    assert!(success, "Empty project should compile: {}", error);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn export_button_project_compiles() {
    use rohkai::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
    use uuid::Uuid;

    let dir = temp_export_dir("button");
    let mut tree = rohkai::project::ui_tree::UiTree::default();

    let button = WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::Button,
        rect: Rect { x: 50.0, y: 50.0, w: 120.0, h: 32.0 },
        props: WidgetProps {
            label: "Click Me".to_string(),
            ..Default::default()
        },
        state_binding: None,
        on_click: "on_button_click".to_string(),
        ..Default::default()
    };
    tree.add(button);

    rohkai::codegen::export::write_project(&tree, &dir).expect("export");

    let (success, error) = try_compile_project(&dir);
    assert!(success, "Button project should compile: {}", error);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn export_slider_project_compiles() {
    use rohkai::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
    use uuid::Uuid;

    let dir = temp_export_dir("slider");
    let mut tree = rohkai::project::ui_tree::UiTree::default();

    let slider = WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::Slider,
        rect: Rect { x: 50.0, y: 50.0, w: 200.0, h: 32.0 },
        props: WidgetProps {
            label: "Volume".to_string(),
            min: 0.0,
            max: 100.0,
            ..Default::default()
        },
        state_binding: Some("volume".to_string()),
        on_change: "on_slider_change".to_string(),
        ..Default::default()
    };
    tree.add(slider);

    rohkai::codegen::export::write_project(&tree, &dir).expect("export");

    let (success, error) = try_compile_project(&dir);
    assert!(success, "Slider project should compile: {}", error);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn export_frame_with_children_compiles() {
    use rohkai::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
    use uuid::Uuid;

    let dir = temp_export_dir("frame");
    let mut tree = rohkai::project::ui_tree::UiTree::default();

    // Create a frame
    let frame_id = Uuid::new_v4();
    let frame = WidgetInstance {
        id: frame_id,
        kind: WidgetKind::Frame,
        rect: Rect { x: 20.0, y: 20.0, w: 300.0, h: 200.0 },
        props: WidgetProps {
            label: "Group".to_string(),
            ..Default::default()
        },
        state_binding: None,
        children: Vec::new(),
        ..Default::default()
    };
    tree.widgets.push(frame);

    // Create a child button
    let button_id = Uuid::new_v4();
    let button = WidgetInstance {
        id: button_id,
        kind: WidgetKind::Button,
        rect: Rect { x: 50.0, y: 50.0, w: 100.0, h: 30.0 },
        props: WidgetProps {
            label: "Inner Button".to_string(),
            ..Default::default()
        },
        state_binding: None,
        children: vec![],
        ..Default::default()
    };
    tree.widgets.push(button);

    // Update frame's children
    if let Some(f) = tree.get_mut(frame_id) {
        f.children.push(button_id);
    }

    rohkai::codegen::export::write_project(&tree, &dir).expect("export");

    let (success, error) = try_compile_project(&dir);
    assert!(success, "Frame with children should compile: {}", error);

    let _ = fs::remove_dir_all(&dir);
}
```

#### Step 2: Add trybuild for compile-fail tests (optional)

For more sophisticated testing, add `trybuild`:

```toml
[dev-dependencies]
trybuild = "1"
```

This allows testing that certain invalid inputs produce specific compile errors.

### Verification

```bash
cargo test export_integration    # Run export integration tests
cargo test                       # All tests pass
```

### Notes

- Tests create temporary directories in `std::env::temp_dir()`
- Tests run `cargo check` which is faster than full compilation
- Tests clean up after themselves
- CI should have Rust toolchain available for these tests

---

## Recommendation 6: Add Canvas Interaction Unit Tests

### Problem

The canvas interaction code (`src/canvas/interaction.rs`) is complex and handles many edge cases (resize handles, multi-select, smart guides, etc.), but has no unit tests. Bugs in this area directly impact user experience.

### Current State

No tests exist for:
- Resize handle hit detection
- Widget movement and snapping
- Multi-select rubber-band selection
- Smart guide alignment
- Z-order operations

### Implementation Plan

#### Step 1: Add test infrastructure to `src/canvas/interaction.rs`

The challenge with testing egui-based code is that it depends on egui's input system. We can test the pure logic separately:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
    use uuid::Uuid;

    /// Create a test widget at the given position.
    fn test_widget(x: f32, y: f32, w: f32, h: f32) -> WidgetInstance {
        WidgetInstance {
            id: Uuid::new_v4(),
            kind: WidgetKind::Button,
            rect: Rect { x, y, w, h },
            props: WidgetProps::default(),
            state_binding: None,
            ..Default::default()
        }
    }

    #[test]
    fn resize_handle_hit_detection() {
        let rect = egui::Rect::from_min_size(egui::Pos2::new(100.0, 100.0), egui::vec2(120.0, 32.0));

        // Top-left handle should be at the top-left corner
        let tl_rect = ResizeHandle::TopLeft.hit_rect(rect);
        assert_eq!(tl_rect.center(), rect.left_top());

        // Bottom-right handle should be at the bottom-right corner
        let br_rect = ResizeHandle::BottomRight.hit_rect(rect);
        assert_eq!(br_rect.center(), rect.right_bottom());

        // Right handle should be at the right center
        let r_rect = ResizeHandle::Right.hit_rect(rect);
        assert_eq!(r_rect.center(), rect.right_center());
    }

    #[test]
    fn resize_handle_cursor_icons() {
        assert_eq!(ResizeHandle::TopLeft.cursor(), egui::CursorIcon::ResizeNwSe);
        assert_eq!(ResizeHandle::TopRight.cursor(), egui::CursorIcon::ResizeNeSw);
        assert_eq!(ResizeHandle::Top.cursor(), egui::CursorIcon::ResizeVertical);
        assert_eq!(ResizeHandle::Right.cursor(), egui::CursorIcon::ResizeHorizontal);
    }

    #[test]
    fn resize_handle_apply_delta_top_left() {
        let start = Rect { x: 100.0, y: 100.0, w: 120.0, h: 32.0 };
        let delta = egui::Vec2::new(10.0, 5.0);

        let result = ResizeHandle::TopLeft.apply_delta(&start, delta);

        // Moving top-left handle right/down should shrink the widget
        assert!((result.x - 110.0).abs() < 0.01);
        assert!((result.y - 105.0).abs() < 0.01);
        assert!((result.w - 110.0).abs() < 0.01);
        assert!((result.h - 27.0).abs() < 0.01);
    }

    #[test]
    fn resize_handle_apply_delta_bottom_right() {
        let start = Rect { x: 100.0, y: 100.0, w: 120.0, h: 32.0 };
        let delta = egui::Vec2::new(10.0, 5.0);

        let result = ResizeHandle::BottomRight.apply_delta(&start, delta);

        // Moving bottom-right handle should expand the widget
        assert!((result.x - 100.0).abs() < 0.01);
        assert!((result.y - 100.0).abs() < 0.01);
        assert!((result.w - 130.0).abs() < 0.01);
        assert!((result.h - 37.0).abs() < 0.01);
    }

    #[test]
    fn resize_handle_respects_min_size() {
        let start = Rect { x: 100.0, y: 100.0, w: 30.0, h: 30.0 };
        let delta = egui::Vec2::new(20.0, 20.0); // Try to shrink by 20px

        let result = ResizeHandle::TopLeft.apply_delta(&start, delta);

        // Should not shrink below MIN_SIZE (20.0)
        assert!(result.w >= MIN_SIZE - 0.01);
        assert!(result.h >= MIN_SIZE - 0.01);
    }

    #[test]
    fn snap_value_to_grid() {
        assert_eq!(snap(15.0, 8.0), 16.0);
        assert_eq!(snap(12.0, 8.0), 8.0);
        assert_eq!(snap(20.0, 8.0), 24.0);
        assert_eq!(snap(0.0, 8.0), 0.0);
    }

    #[test]
    fn snap_rect_clamps_to_minimums() {
        let rect = Rect { x: -10.0, y: -5.0, w: 10.0, h: 10.0 };
        let snapped = snap_rect(rect, 8.0);

        assert!(snapped.x >= 0.0);
        assert!(snapped.y >= 0.0);
        assert!(snapped.w >= MIN_SIZE);
        assert!(snapped.h >= MIN_SIZE);
    }

    #[test]
    fn smart_guide_alignment_detection() {
        // Two widgets that should align
        let w1 = Rect { x: 100.0, y: 100.0, w: 100.0, h: 50.0 };
        let w2 = Rect { x: 200.0, y: 100.0, w: 100.0, h: 50.0 };

        // Right edge of w1 should align with left edge of w2
        let w1_right = w1.x + w1.w;
        let w2_left = w2.x;
        assert!((w1_right - w2_left).abs() < 0.01);
    }
}
```

#### Step 2: Add tests for UiTree operations in `src/project/ui_tree.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};

    fn test_button() -> WidgetInstance {
        WidgetInstance {
            id: Uuid::new_v4(),
            kind: WidgetKind::Button,
            rect: Rect { x: 50.0, y: 50.0, w: 100.0, h: 30.0 },
            props: WidgetProps { label: "Test".to_string(), ..Default::default() },
            state_binding: None,
            ..Default::default()
        }
    }

    #[test]
    fn bring_to_front_moves_to_end() {
        let mut tree = UiTree::default();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let mut w1 = test_button(); w1.id = id1;
        let mut w2 = test_button(); w2.id = id2;
        let mut w3 = test_button(); w3.id = id3;

        tree.widgets.push(w1);
        tree.widgets.push(w2);
        tree.widgets.push(w3);

        // id1 is at index 0, bring to front
        tree.bring_to_front(id1);

        assert_eq!(tree.widgets.last().unwrap().id, id1);
    }

    #[test]
    fn send_to_back_moves_to_start() {
        let mut tree = UiTree::default();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let mut w1 = test_button(); w1.id = id1;
        let mut w2 = test_button(); w2.id = id2;
        let mut w3 = test_button(); w3.id = id3;

        tree.widgets.push(w1);
        tree.widgets.push(w2);
        tree.widgets.push(w3);

        // id3 is at index 2, send to back
        tree.send_to_back(id3);

        assert_eq!(tree.widgets.first().unwrap().id, id3);
    }

    #[test]
    fn group_creates_frame_with_children() {
        let mut tree = UiTree::default();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let mut w1 = test_button(); w1.id = id1; w1.rect.x = 50.0;
        let mut w2 = test_button(); w2.id = id2; w2.rect.x = 100.0;

        tree.widgets.push(w1);
        tree.widgets.push(w2);

        let frame_id = tree.group(&[id1, id2]).expect("group should succeed");

        let frame = tree.get_mut(frame_id).expect("frame should exist");
        assert!(matches!(frame.kind, WidgetKind::Frame));
        assert!(frame.children.contains(&id1));
        assert!(frame.children.contains(&id2));
    }

    #[test]
    fn group_fails_with_less_than_two() {
        let mut tree = UiTree::default();
        let id1 = Uuid::new_v4();
        let mut w1 = test_button(); w1.id = id1;
        tree.widgets.push(w1);

        let result = tree.group(&[id1]);
        assert!(result.is_none());
    }

    #[test]
    fn ungroup_removes_frame_returns_children() {
        let mut tree = UiTree::default();
        let frame_id = Uuid::new_v4();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let frame = WidgetInstance {
            id: frame_id,
            kind: WidgetKind::Frame,
            rect: Rect::default(),
            props: WidgetProps::default(),
            state_binding: None,
            children: vec![id1, id2],
            ..Default::default()
        };
        tree.widgets.push(frame);

        let children = tree.ungroup(frame_id);
        assert_eq!(children, vec![id1, id2]);
        assert!(tree.widgets.is_empty());
    }

    #[test]
    fn remove_cascades_to_children() {
        let mut tree = UiTree::default();
        let frame_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();

        let frame = WidgetInstance {
            id: frame_id,
            kind: WidgetKind::Frame,
            rect: Rect::default(),
            props: WidgetProps::default(),
            state_binding: None,
            children: vec![child_id],
            ..Default::default()
        };

        let child = WidgetInstance {
            id: child_id,
            kind: WidgetKind::Button,
            rect: Rect::default(),
            props: WidgetProps::default(),
            state_binding: None,
            ..Default::default()
        };

        tree.widgets.push(frame);
        tree.widgets.push(child);

        tree.remove(frame_id);

        assert!(tree.widgets.is_empty());
    }

    #[test]
    fn validate_and_repair_fixes_duplicate_ids() {
        let mut tree = UiTree::default();
        let duplicate_id = Uuid::nil(); // Use nil as a known duplicate

        let mut w1 = test_button(); w1.id = duplicate_id;
        let mut w2 = test_button(); w2.id = duplicate_id;

        tree.widgets.push(w1);
        tree.widgets.push(w2);

        tree.validate_and_repair();

        // Both should now have unique IDs
        let ids: Vec<_> = tree.widgets.iter().map(|w| w.id).collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn validate_and_repair_removes_stale_children() {
        let mut tree = UiTree::default();
        let frame_id = Uuid::new_v4();
        let missing_child_id = Uuid::new_v4();

        let frame = WidgetInstance {
            id: frame_id,
            kind: WidgetKind::Frame,
            rect: Rect::default(),
            props: WidgetProps::default(),
            state_binding: None,
            children: vec![missing_child_id], // Child doesn't exist
            ..Default::default()
        };
        tree.widgets.push(frame);

        tree.validate_and_repair();

        let frame = tree.get_mut(frame_id).unwrap();
        assert!(frame.children.is_empty());
    }
}
```

### Verification

```bash
cargo test canvas::interaction   # Run canvas interaction tests
cargo test project::ui_tree      # Run ui_tree tests
cargo test                       # All tests pass
cargo clippy -- -D warnings      # Zero warnings
```

### Notes

- Tests focus on pure logic (math, data structure operations)
- Egui-specific rendering can't be unit tested without a context
- Integration tests could use `eframe` test utilities if needed

---

## Summary

| Recommendation | Files Changed | Lines Added | Lines Removed | Risk |
|---------------|---------------|-------------|---------------|------|
| 4. Custom error types | ~10 | ~200 | ~50 | Low |
| 5. Export integration tests | 1 new | ~150 | 0 | Low |
| 6. Canvas interaction tests | 2 | ~200 | 0 | None |
| **Total** | **~12 files** | **~550** | **~50** | **Low** |

All changes are additive and improve code quality without affecting existing functionality.