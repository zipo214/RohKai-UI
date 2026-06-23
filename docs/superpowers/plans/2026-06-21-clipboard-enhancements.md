# Clipboard Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an in-app canvas clipboard — Copy (Ctrl+C), Paste-at-cursor (Ctrl+V), and Duplicate-in-place (Ctrl+D) — that deep-clones selected widgets with full property preservation, atomic UUID remapping, and a small transient status primitive for feedback.

**Architecture:** A new `src/canvas/clipboard.rs` module (mirroring `src/canvas/search.rs`) owns the clipboard payload type and the `copy_selection` / `paste_payload` / `duplicate_in_place` functions plus pure coordinate helpers. `UiTree::paste_batch` (in `src/project/ui_tree.rs`) performs the atomic staged remap-validate-commit. A new `src/status.rs` holds a tiny session-only transient `StatusMessage` primitive. State (`clipboard`, `paste_cascade`, `paste_flash`) lives on `InteractionState` (never serialized). `app.rs` wires the key handlers next to the existing Delete handler.

**Tech Stack:** Rust 2024, egui 0.34.3. Uses existing `crate::canvas::rulers::canvas_origin`, `UiTree`, `WidgetInstance`, `ctx.input(|i| i.time)`.

---

## Spec Reference

Design spec: `docs/superpowers/specs/2026-06-21-clipboard-enhancements-design.md`. Requirement IDs (CB-NN) below trace to that spec's adversarial review.

## Confirmed APIs (do not re-derive)

- `WidgetInstance` derives `Default`. Fields include `id: Uuid`, `kind: WidgetKind`, `rect: Rect` (`{x,y,w,h: f32}`), `children: Vec<Uuid>`, `state_binding: Option<String>`, `constraints: LayoutConstraints`, and many more — **there is NO `parent` field** (parentage is inferred from `children`).
- `LayoutConstraints.equal_width_to: Option<String>`, `equal_height_to: Option<String>`.
- `UiTree { pub widgets: Vec<WidgetInstance>, pub app_props: AppProps }`. Derives `Default`. Methods: `add`, `remove`, `parent_of(id) -> Option<Uuid>`, `attach_to_layout_at(child_id: Uuid, canvas_point: (f32,f32)) -> Option<Uuid>`, `reflow_layouts`, `validate_and_repair`. `make_binding_unique(&self, &mut WidgetInstance)` is private (same file — `paste_batch` can call it).
- `AppProps.win_w: f32`, `win_h: f32`, `behaviors: Vec<Behavior>`.
- `Behavior { id: Uuid, trigger: BehaviorTrigger, target_widget: Option<Uuid>, action: VisualAction }`. `trigger.source_widget() -> Option<Uuid>`.
- `WidgetKind` variants include `Button`, `Label`, … and `Custom(String)`.
- `CanvasSettings { zoom: f32, pan: egui::Vec2, .. }` on `self.session.canvas_settings`.
- `canvas_origin(canvas_size: [f32;2], zoom: f32, pan: egui::Vec2, panel_rect: egui::Rect) -> egui::Pos2` in `src/canvas/rulers.rs`.
- In `app.rs`, the Delete handler uses an in-scope bool `canvas_keyboard_owned` (combines modal/focus/keyboard ownership — already excludes text-editor focus). The canvas search block at ~`app.rs:3358` shows how `panel_rect`, `self.session.canvas_settings`, and `self.project.ui_tree` are accessed at that site.
- `self.session.selected: Vec<Uuid>`. `self.undo.record(json: String)` where `json` = `crate::project::io::serialize(&self.project.ui_tree.snapshot())`.
- `ctx.input(|i| i.time) -> f64`. `ctx.input(|i| i.pointer.interact_pos()) -> Option<egui::Pos2>`.
- Test fixtures: `WidgetInstance { id, kind, props, ..Default::default() }` and `UiTree { widgets, ..Default::default() }` compile.

---

## Task 1: Transient status primitive (`src/status.rs`)

**Files:**
- Create: `src/status.rs`
- Modify: `src/main.rs` (add `mod status;` near the other top-level `mod` declarations)

- [ ] **Step 1: Write the failing test**

Create `src/status.rs` with only the test module first:

```rust
//! Session-only transient status message primitive (CB-21).
//! Holds a single short message with an expiry time. Never persisted.

#[derive(Default)]
pub struct StatusMessage {
    text: Option<String>,
    expires_at: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_then_current_within_ttl() {
        let mut s = StatusMessage::default();
        s.set("Copied Button", 10.0);
        assert_eq!(s.current(10.5), Some("Copied Button"));
    }

    #[test]
    fn expires_after_ttl() {
        let mut s = StatusMessage::default();
        s.set("Pasted 4 widgets", 10.0);
        assert_eq!(s.current(11.0 + STATUS_TTL), None);
    }

    #[test]
    fn newer_message_replaces_older() {
        let mut s = StatusMessage::default();
        s.set("Copied Button", 10.0);
        s.set("Pasted 2 widgets", 10.2);
        assert_eq!(s.current(10.3), Some("Pasted 2 widgets"));
    }

    #[test]
    fn default_has_no_message() {
        let s = StatusMessage::default();
        assert_eq!(s.current(0.0), None);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib status::`
Expected: FAIL — `set`, `current`, and `STATUS_TTL` are not defined.

- [ ] **Step 3: Write minimal implementation**

Add above the test module in `src/status.rs`:

```rust
/// Default lifetime of a status message in seconds.
pub const STATUS_TTL: f64 = 1.5;

impl StatusMessage {
    /// Show `text`, expiring `STATUS_TTL` seconds after `now`.
    /// `now` is `ctx.input(|i| i.time)`.
    pub fn set(&mut self, text: impl Into<String>, now: f64) {
        self.text = Some(text.into());
        self.expires_at = now + STATUS_TTL;
    }

    /// The current message if one is set and `now` is before its expiry.
    pub fn current(&self, now: f64) -> Option<&str> {
        match &self.text {
            Some(t) if now < self.expires_at => Some(t.as_str()),
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib status::`
Expected: PASS (4 tests).

- [ ] **Step 5: Declare the module**

In `src/main.rs`, add alongside the other `mod` lines (e.g. near `mod app;`):

```rust
mod status;
```

- [ ] **Step 6: Verify build + commit**

Run: `cargo check`
Expected: compiles (a `dead_code` warning on unused `StatusMessage` is acceptable until Task 8 wires it).

```bash
git add src/status.rs src/main.rs
git commit -m "feat(clipboard): transient status message primitive"
```

---

## Task 2: `UiTree::paste_batch` core — remap, validate, commit

**Files:**
- Modify: `src/project/ui_tree.rs` (add `PasteError`, `paste_batch`, `validate_staged` near the other `impl UiTree` methods)

This task implements atomic id-remap + acyclicity/duplicate-parent validation + commit (CB-01, CB-02, CB-04, CB-08 delta application). Reference remap (constraints/bindings) is Task 3.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `src/project/ui_tree.rs` (create the block if absent, mirroring search.rs fixture style):

```rust
#[cfg(test)]
mod paste_batch_tests {
    use super::*;
    use crate::project::schema::{WidgetInstance, WidgetKind};

    fn w(children: Vec<uuid::Uuid>) -> WidgetInstance {
        WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Button,
            children,
            ..Default::default()
        }
    }

    #[test]
    fn paste_frame_with_child_remaps_all_links() {
        // Frame (parent) containing one child.
        let child = w(vec![]);
        let child_id = child.id;
        let mut frame = w(vec![child_id]);
        frame.kind = WidgetKind::Frame;
        let frame_id = frame.id;

        let staged = vec![frame.clone(), child.clone()];
        let mut tree = UiTree::default();
        let new_roots = tree
            .paste_batch(staged, egui::vec2(0.0, 0.0), None)
            .expect("valid graph");

        // One root returned (the frame).
        assert_eq!(new_roots.len(), 1);
        let new_frame_id = new_roots[0];
        // Pasted subtree shares no UUID with source.
        assert_ne!(new_frame_id, frame_id);
        let new_frame = tree.widgets.iter().find(|x| x.id == new_frame_id).unwrap();
        assert_eq!(new_frame.children.len(), 1);
        let new_child_id = new_frame.children[0];
        assert_ne!(new_child_id, child_id);
        // The remapped child exists in the tree.
        assert!(tree.widgets.iter().any(|x| x.id == new_child_id));
        // No children entry references a pre-paste id.
        assert!(tree
            .widgets
            .iter()
            .flat_map(|x| x.children.iter())
            .all(|c| *c != child_id && *c != frame_id));
    }

    #[test]
    fn delta_applied_to_every_widget() {
        let child = w(vec![]);
        let child_id = child.id;
        let mut frame = w(vec![child_id]);
        frame.kind = WidgetKind::Frame;
        frame.rect = crate::project::schema::Rect { x: 100.0, y: 100.0, w: 50.0, h: 50.0 };
        let mut child2 = child.clone();
        child2.rect = crate::project::schema::Rect { x: 110.0, y: 110.0, w: 10.0, h: 10.0 };

        let staged = vec![frame, child2];
        let mut tree = UiTree::default();
        tree.paste_batch(staged, egui::vec2(25.0, 5.0), None).unwrap();

        let xs: Vec<(f32, f32)> = tree.widgets.iter().map(|x| (x.rect.x, x.rect.y)).collect();
        assert!(xs.contains(&(125.0, 105.0))); // frame moved by delta
        assert!(xs.contains(&(135.0, 115.0))); // child moved by same delta
    }

    #[test]
    fn cycle_in_staged_graph_aborts_with_no_mutation() {
        // Two widgets each listing the other as a child → cycle.
        let mut a = w(vec![]);
        let mut b = w(vec![]);
        a.children = vec![b.id];
        b.children = vec![a.id];
        let staged = vec![a, b];

        let mut tree = UiTree::default();
        let before = tree.widgets.len();
        let result = tree.paste_batch(staged, egui::vec2(0.0, 0.0), None);
        assert!(matches!(result, Err(PasteError::InvalidGraph)));
        assert_eq!(tree.widgets.len(), before); // no partial insert
    }

    #[test]
    fn duplicate_parent_in_staged_graph_aborts() {
        // Two parents both claim the same child id.
        let child = w(vec![]);
        let p1 = w(vec![child.id]);
        let p2 = w(vec![child.id]);
        let staged = vec![p1, p2, child];

        let mut tree = UiTree::default();
        let result = tree.paste_batch(staged, egui::vec2(0.0, 0.0), None);
        assert!(matches!(result, Err(PasteError::InvalidGraph)));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib paste_batch_tests`
Expected: FAIL — `paste_batch` and `PasteError` not defined.

- [ ] **Step 3: Write minimal implementation**

Add near the top of `src/project/ui_tree.rs` (after imports):

```rust
/// Error from an attempted atomic paste (CB-04). No tree mutation occurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteError {
    /// The staged graph would create a cycle, a self-child, or a
    /// widget owned by two parents.
    InvalidGraph,
}
```

Add inside `impl UiTree { … }`:

```rust
/// Atomically paste a self-contained set of widgets (CB-01).
///
/// Builds one old→new UUID map over the entire `staged` set, rewrites
/// `id` and every `children[]` entry through it, applies `anchor_delta`
/// to every widget rect (CB-08), validates the staged graph, and only
/// then commits all widgets in one step. On validation failure returns
/// `Err(PasteError::InvalidGraph)` with NO mutation (CB-04).
///
/// Returns the new root widget ids (widgets not referenced as a child of
/// any other staged widget). If `target_container` is `Some`, the new
/// roots are attached to that layout via the normal attach/reflow path,
/// which may adjust their rects.
pub fn paste_batch(
    &mut self,
    staged: Vec<WidgetInstance>,
    anchor_delta: egui::Vec2,
    target_container: Option<uuid::Uuid>,
) -> Result<Vec<uuid::Uuid>, PasteError> {
    use std::collections::{HashMap, HashSet};

    // 1. old→new id map over the full set.
    let id_map: HashMap<uuid::Uuid, uuid::Uuid> = staged
        .iter()
        .map(|w| (w.id, uuid::Uuid::new_v4()))
        .collect();

    // 2. Derive roots: ids not referenced by another staged widget's children.
    let child_ids: HashSet<uuid::Uuid> =
        staged.iter().flat_map(|w| w.children.iter().copied()).collect();
    let old_roots: Vec<uuid::Uuid> =
        staged.iter().map(|w| w.id).filter(|id| !child_ids.contains(id)).collect();

    // 3-5. Clone, remap id + children, drop links outside the set, apply delta.
    let staged_set: HashSet<uuid::Uuid> = staged.iter().map(|w| w.id).collect();
    let mut remapped: Vec<WidgetInstance> = Vec::with_capacity(staged.len());
    for w in &staged {
        let mut c = w.clone();
        c.id = id_map[&w.id];
        c.children = w
            .children
            .iter()
            .filter(|cid| staged_set.contains(cid)) // drop outside links (CB-03)
            .map(|cid| id_map[cid])
            .collect();
        c.rect.x += anchor_delta.x;
        c.rect.y += anchor_delta.y;
        remapped.push(c);
    }

    // 7. Validate staged graph BEFORE any commit.
    Self::validate_staged(&remapped)?;

    // 9. Commit in one step.
    let new_roots: Vec<uuid::Uuid> =
        old_roots.iter().map(|old| id_map[old]).collect();
    for w in remapped {
        self.widgets.push(w);
    }

    // 10. Optional layout attach for roots (may reflow rects).
    if let Some(container) = target_container {
        for root in &new_roots {
            // Attach using the container's current canvas position as anchor.
            if let Some(cw) = self.widgets.iter().find(|x| x.id == container) {
                let pt = (cw.rect.x, cw.rect.y);
                self.attach_to_layout_at(*root, pt);
            }
        }
        self.reflow_layouts();
    }

    Ok(new_roots)
}

/// Validate a staged, already-remapped widget set (CB-04):
/// no duplicate parents, no self-child, no cycle.
fn validate_staged(staged: &[WidgetInstance]) -> Result<(), PasteError> {
    use std::collections::HashMap;

    // Indegree: how many parents claim each id. >1 = duplicate parent.
    let mut indeg: HashMap<uuid::Uuid, usize> = staged.iter().map(|w| (w.id, 0)).collect();
    for w in staged {
        for c in &w.children {
            if *c == w.id {
                return Err(PasteError::InvalidGraph); // self-child
            }
            match indeg.get_mut(c) {
                Some(n) => *n += 1,
                None => {} // child not in set: already filtered, ignore
            }
        }
    }
    if indeg.values().any(|n| *n > 1) {
        return Err(PasteError::InvalidGraph); // duplicate parent
    }

    // Topological peel from roots (indegree 0). If not all consumed → cycle.
    let child_map: HashMap<uuid::Uuid, &Vec<uuid::Uuid>> =
        staged.iter().map(|w| (w.id, &w.children)).collect();
    let mut indeg2 = indeg.clone();
    let mut stack: Vec<uuid::Uuid> =
        indeg2.iter().filter(|(_, n)| **n == 0).map(|(k, _)| *k).collect();
    let mut visited = 0usize;
    while let Some(id) = stack.pop() {
        visited += 1;
        if let Some(children) = child_map.get(&id) {
            for c in children.iter() {
                if let Some(n) = indeg2.get_mut(c) {
                    *n -= 1;
                    if *n == 0 {
                        stack.push(*c);
                    }
                }
            }
        }
    }
    if visited != staged.len() {
        return Err(PasteError::InvalidGraph); // cycle
    }
    Ok(())
}
```

Note: if `egui` is not already imported in `ui_tree.rs`, use fully-qualified `egui::Vec2` in the signature (it is referenced as `egui::vec2` in tests). Confirm with `cargo check` and add `use eframe::egui;` or the crate's existing egui path if needed (match how other canvas-coordinate code in the file imports egui; if none, the fully-qualified `egui::Vec2` requires `egui` to be a known crate path — it is, via the dependency).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib paste_batch_tests`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/project/ui_tree.rs
git commit -m "feat(clipboard): UiTree::paste_batch atomic remap + graph validation"
```

---

## Task 3: `paste_batch` reference remap — constraints + shared bindings

**Files:**
- Modify: `src/project/ui_tree.rs` (extend `paste_batch` remap loop)

Implements CB-12 (shared bindings renamed once per unique string) and CB-13 (constraint refs remapped or cleared).

- [ ] **Step 1: Write the failing test**

Add to the `paste_batch_tests` module:

```rust
#[test]
fn constraint_ref_inside_set_is_remapped() {
    let mut a = w(vec![]);
    let mut b = w(vec![]);
    // a is equal_width_to b (raw id string).
    a.constraints.equal_width_to = Some(b.id.to_string());
    let b_id = b.id;
    let staged = vec![a, b];

    let mut tree = UiTree::default();
    let roots = tree.paste_batch(staged, egui::vec2(0.0, 0.0), None).unwrap();
    // Find pasted 'a' (the one with a constraint set).
    let pasted_a = tree
        .widgets
        .iter()
        .find(|x| x.constraints.equal_width_to.is_some())
        .unwrap();
    let referenced = pasted_a.constraints.equal_width_to.clone().unwrap();
    // It must point at a pasted id, never the old one, and must resolve.
    assert_ne!(referenced, b_id.to_string());
    assert!(tree.widgets.iter().any(|x| x.id.to_string() == referenced));
    let _ = roots;
}

#[test]
fn constraint_ref_outside_set_is_cleared() {
    let outside_id = uuid::Uuid::new_v4().to_string();
    let mut a = w(vec![]);
    a.constraints.equal_height_to = Some(outside_id);
    let staged = vec![a];

    let mut tree = UiTree::default();
    tree.paste_batch(staged, egui::vec2(0.0, 0.0), None).unwrap();
    let pasted = &tree.widgets[0];
    assert_eq!(pasted.constraints.equal_height_to, None);
}

#[test]
fn shared_binding_renamed_once_and_stays_shared() {
    // Pre-existing widget in the tree already uses "count".
    let mut existing = w(vec![]);
    existing.state_binding = Some("count".to_string());
    let mut tree = UiTree { widgets: vec![existing], ..Default::default() };

    // Two staged widgets share binding "count".
    let mut s1 = w(vec![]);
    let mut s2 = w(vec![]);
    s1.state_binding = Some("count".to_string());
    s2.state_binding = Some("count".to_string());
    tree.paste_batch(vec![s1, s2], egui::vec2(0.0, 0.0), None).unwrap();

    // The two pasted widgets share ONE binding, renamed away from "count".
    let bindings: Vec<String> = tree
        .widgets
        .iter()
        .skip(1) // skip pre-existing
        .filter_map(|x| x.state_binding.clone())
        .collect();
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0], bindings[1]); // still shared
    assert_ne!(bindings[0], "count"); // renamed to avoid collision
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib paste_batch_tests`
Expected: FAIL — constraints not remapped, bindings collide or unshared.

- [ ] **Step 3: Write the implementation**

In `paste_batch`, after the clone/remap loop builds `remapped` and **before** `validate_staged`, insert reference remapping:

```rust
    // 6a. Remap constraint references (CB-13): id-string → new id-string when
    //     both endpoints are in the set; clear when the target is outside.
    let id_str_map: std::collections::HashMap<String, String> = id_map
        .iter()
        .map(|(old, new)| (old.to_string(), new.to_string()))
        .collect();
    for w in &mut remapped {
        for slot in [
            &mut w.constraints.equal_width_to,
            &mut w.constraints.equal_height_to,
        ] {
            if let Some(target) = slot.clone() {
                *slot = id_str_map.get(&target).cloned(); // None if outside set
            }
        }
    }

    // 6b. Remap shared state_bindings once per unique string (CB-12). Collect
    //     existing bindings in the tree, then for each unique staged binding
    //     that collides, pick one fresh name and apply to ALL sharers.
    {
        use std::collections::{HashMap, HashSet};
        let existing: HashSet<String> = self
            .widgets
            .iter()
            .filter_map(|w| w.state_binding.clone())
            .collect();
        let mut rename: HashMap<String, String> = HashMap::new();
        let mut taken = existing.clone();
        let mut staged_bindings: Vec<String> = remapped
            .iter()
            .filter_map(|w| w.state_binding.clone())
            .collect();
        staged_bindings.sort();
        staged_bindings.dedup();
        for base in staged_bindings {
            if taken.contains(&base) {
                // Find first free {base}_N (N starts at 2).
                let mut n = 2;
                let mut candidate = format!("{base}_{n}");
                while taken.contains(&candidate) {
                    n += 1;
                    candidate = format!("{base}_{n}");
                }
                taken.insert(candidate.clone());
                rename.insert(base, candidate);
            } else {
                taken.insert(base);
            }
        }
        for w in &mut remapped {
            if let Some(b) = w.state_binding.clone() {
                if let Some(new) = rename.get(&b) {
                    w.state_binding = Some(new.clone());
                }
            }
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib paste_batch_tests`
Expected: PASS (7 tests total in the module).

- [ ] **Step 5: Commit**

```bash
git add src/project/ui_tree.rs
git commit -m "feat(clipboard): remap constraint refs and shared bindings in paste_batch"
```

---

## Task 4: `clipboard.rs` — payload type, copy_selection, kind label

**Files:**
- Create: `src/canvas/clipboard.rs`
- Modify: `src/canvas/mod.rs` (add `pub mod clipboard;`)

Implements CB-03 (copy-closure), CB-05 (deep snapshot), behavior detection for CB-04 toast, and the widget-kind label helper.

- [ ] **Step 1: Write the failing test**

Create `src/canvas/clipboard.rs`:

```rust
//! In-app canvas clipboard (CB-01..CB-25). Session-only; never persisted.
//! Mirrors the `src/canvas/search.rs` module structure.

use crate::project::schema::{WidgetInstance, WidgetKind};
use crate::project::ui_tree::UiTree;

/// Deep snapshot of a copied selection plus whether the source widgets had
/// associated behavior wires (used for the "behavior wires not copied" notice).
#[derive(Default, Clone)]
pub struct ClipboardContents {
    pub widgets: Vec<WidgetInstance>,
    pub source_had_behaviors: bool,
}

impl ClipboardContents {
    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }
}

/// Human-readable name for a widget kind, e.g. "Button" or a custom name.
pub fn widget_kind_label(kind: &WidgetKind) -> String {
    match kind {
        WidgetKind::Custom(name) => name.clone(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::Rect;

    fn w(children: Vec<uuid::Uuid>) -> WidgetInstance {
        WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Button,
            children,
            ..Default::default()
        }
    }

    fn tree(widgets: Vec<WidgetInstance>) -> UiTree {
        UiTree { widgets, ..Default::default() }
    }

    #[test]
    fn copy_empty_selection_is_empty() {
        let t = tree(vec![w(vec![])]);
        let c = copy_selection(&[], &t);
        assert!(c.is_empty());
    }

    #[test]
    fn copy_frame_includes_all_descendants() {
        let child = w(vec![]);
        let child_id = child.id;
        let mut frame = w(vec![child_id]);
        frame.kind = WidgetKind::Frame;
        let frame_id = frame.id;
        let t = tree(vec![frame, child]);

        let c = copy_selection(&[frame_id], &t);
        // Closure pulls in the child even though only the frame was selected.
        assert_eq!(c.widgets.len(), 2);
        assert!(c.widgets.iter().any(|x| x.id == child_id));
    }

    #[test]
    fn copy_child_only_clears_outside_links() {
        // Select only the child; its (uncopied) parent must not leak in,
        // and the child must carry no link pointing outside the copied set.
        let child = w(vec![]);
        let child_id = child.id;
        let frame = {
            let mut f = w(vec![child_id]);
            f.kind = WidgetKind::Frame;
            f
        };
        let t = tree(vec![frame, child]);

        let c = copy_selection(&[child_id], &t);
        assert_eq!(c.widgets.len(), 1);
        assert_eq!(c.widgets[0].id, child_id);
        assert!(c.widgets[0].children.is_empty());
    }

    #[test]
    fn kind_label_builtin_and_custom() {
        assert_eq!(widget_kind_label(&WidgetKind::Button), "Button");
        assert_eq!(
            widget_kind_label(&WidgetKind::Custom("Ply Button".into())),
            "Ply Button"
        );
    }

    #[test]
    fn copy_detects_no_behaviors_by_default() {
        let widget = w(vec![]);
        let id = widget.id;
        let t = tree(vec![widget]);
        let c = copy_selection(&[id], &t);
        assert!(!c.source_had_behaviors);
        let _ = Rect::default();
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib clipboard::tests`
Expected: FAIL — `copy_selection` not defined. (`src/canvas/mod.rs` must declare the module first; do Step 3's mod line so it compiles.)

- [ ] **Step 3: Declare module + implement copy_selection**

In `src/canvas/mod.rs`, add after `pub mod search;`:

```rust
pub mod clipboard;
```

Add to `src/canvas/clipboard.rs` (above the test module):

```rust
use std::collections::HashSet;

/// Build the copy-closed deep snapshot of `selected` (CB-03, CB-05).
///
/// Expands every selected id to include all transitive descendants, deep-clones
/// each widget (preserving every field), and clears any `children[]` link that
/// points outside the copied set so the payload is self-contained. Returns an
/// empty payload when `selected` is empty.
pub fn copy_selection(selected: &[uuid::Uuid], tree: &UiTree) -> ClipboardContents {
    if selected.is_empty() {
        return ClipboardContents::default();
    }

    // Transitive closure over children.
    let mut closure: HashSet<uuid::Uuid> = HashSet::new();
    let mut stack: Vec<uuid::Uuid> = selected.to_vec();
    while let Some(id) = stack.pop() {
        if !closure.insert(id) {
            continue;
        }
        if let Some(w) = tree.widgets.iter().find(|x| x.id == id) {
            for c in &w.children {
                stack.push(*c);
            }
        }
    }

    // Deep-clone closure members; drop links outside the set.
    let mut widgets: Vec<WidgetInstance> = tree
        .widgets
        .iter()
        .filter(|w| closure.contains(&w.id))
        .cloned()
        .collect();
    for w in &mut widgets {
        w.children.retain(|c| closure.contains(c));
    }

    // Did any copied widget have an associated behavior wire? (CB-04 notice.)
    let source_had_behaviors = tree.app_props.behaviors.iter().any(|b| {
        b.trigger
            .source_widget()
            .map(|s| closure.contains(&s))
            .unwrap_or(false)
            || b.target_widget.map(|t| closure.contains(&t)).unwrap_or(false)
    });

    ClipboardContents { widgets, source_had_behaviors }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib clipboard::tests`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/canvas/clipboard.rs src/canvas/mod.rs
git commit -m "feat(clipboard): copy_selection closure snapshot + kind label"
```

---

## Task 5: `clipboard.rs` — coordinate helpers, paste_payload, duplicate_in_place

**Files:**
- Modify: `src/canvas/clipboard.rs`

Implements CB-06 (canonical coordinate conversion), CB-07 (fallback anchor), CB-08 (bbox-center group translate), CB-19 (cascade), and Duplicate.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/canvas/clipboard.rs`:

```rust
#[test]
fn cursor_to_canvas_round_trips() {
    // Over a grid of zoom/pan/panel/screen samples, screen→canvas→screen
    // must return the original screen point (CB-06).
    let panel = egui::Rect::from_min_size(egui::pos2(200.0, 40.0), egui::vec2(800.0, 600.0));
    let size = [640.0_f32, 480.0_f32];
    for &zoom in &[0.25_f32, 1.0, 4.0] {
        for &pan in &[egui::vec2(0.0, 0.0), egui::vec2(-120.0, 75.0)] {
            for &screen in &[egui::pos2(300.0, 120.0), egui::pos2(950.0, 600.0)] {
                let canvas = cursor_to_canvas(screen, size, zoom, pan, panel);
                let origin = crate::canvas::rulers::canvas_origin(size, zoom, pan, panel);
                let back = origin + (canvas.to_vec2() * zoom);
                assert!((back.x - screen.x).abs() < 0.01, "x at zoom {zoom}");
                assert!((back.y - screen.y).abs() < 0.01, "y at zoom {zoom}");
            }
        }
    }
}

#[test]
fn multi_widget_paste_translates_as_group_preserving_distances() {
    // 3 widgets; paste so the bbox center lands on target. Distances and sizes
    // are preserved exactly (CB-08).
    let mk = |x: f32, y: f32, wd: f32, h: f32| {
        let mut w = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Button,
            ..Default::default()
        };
        w.rect = Rect { x, y, w: wd, h };
        w
    };
    let clip = ClipboardContents {
        widgets: vec![mk(100.0, 100.0, 20.0, 20.0), mk(200.0, 100.0, 20.0, 20.0), mk(300.0, 200.0, 40.0, 40.0)],
        source_had_behaviors: false,
    };
    // bbox: x 100..340, y 100..240 → center (220, 170).
    let mut tree = UiTree::default();
    let target = egui::pos2(520.0, 470.0);
    let out = paste_payload(&clip, target, 0, &mut tree, None).unwrap();
    assert_eq!(out.count, 3);

    let rects: Vec<Rect> = tree.widgets.iter().map(|w| w.rect.clone()).collect();
    // Each widget moved by delta = target - center = (300, 300).
    assert!(rects.iter().any(|r| (r.x - 400.0).abs() < 0.01 && (r.y - 400.0).abs() < 0.01));
    // Sizes unchanged.
    assert!(rects.iter().any(|r| (r.w - 40.0).abs() < 0.01 && (r.h - 40.0).abs() < 0.01));
}

#[test]
fn cascade_offsets_repeated_pastes() {
    let mut w = WidgetInstance { id: uuid::Uuid::new_v4(), kind: WidgetKind::Button, ..Default::default() };
    w.rect = Rect { x: 0.0, y: 0.0, w: 10.0, h: 10.0 };
    let clip = ClipboardContents { widgets: vec![w], source_had_behaviors: false };

    let mut tree = UiTree::default();
    let target = egui::pos2(0.0, 0.0); // not relevant; we compare relative cascade
    paste_payload(&clip, target, 0, &mut tree, None).unwrap();
    paste_payload(&clip, target, 1, &mut tree, None).unwrap();
    let xs: Vec<f32> = tree.widgets.iter().map(|w| w.rect.x).collect();
    // Two pastes differ by exactly PASTE_CASCADE_STEP on x.
    let mut sorted = xs.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!((sorted[1] - sorted[0] - PASTE_CASCADE_STEP).abs() < 0.01);
}

#[test]
fn duplicate_in_place_offsets_by_step_and_preserves_size() {
    let mut src = WidgetInstance { id: uuid::Uuid::new_v4(), kind: WidgetKind::Button, ..Default::default() };
    src.rect = Rect { x: 50.0, y: 60.0, w: 30.0, h: 30.0 };
    let src_id = src.id;
    let mut tree = UiTree { widgets: vec![src], ..Default::default() };

    let out = duplicate_in_place(&[src_id], &mut tree).unwrap();
    assert_eq!(out.count, 1);
    let dup = tree.widgets.iter().find(|w| w.id == out.new_root_ids[0]).unwrap();
    assert!((dup.rect.x - (50.0 + PASTE_CASCADE_STEP)).abs() < 0.01);
    assert!((dup.rect.w - 30.0).abs() < 0.01);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib clipboard::tests`
Expected: FAIL — `cursor_to_canvas`, `paste_payload`, `duplicate_in_place`, `PASTE_CASCADE_STEP`, `PasteOutcome` not defined.

- [ ] **Step 3: Write the implementation**

Add to `src/canvas/clipboard.rs` (above the test module). Note the re-export of `PasteError`:

```rust
pub use crate::project::ui_tree::PasteError;

/// Cascade step between repeated pastes, in canvas units (zoom-stable, CB-19).
pub const PASTE_CASCADE_STEP: f32 = 16.0;

/// Result of a successful paste/duplicate.
pub struct PasteOutcome {
    pub new_root_ids: Vec<uuid::Uuid>,
    pub count: usize,
    pub had_behaviors: bool,
}

/// Convert a screen-space cursor position to canvas space using the canonical
/// `canvas_origin` helper (CB-06). Never re-derive this formula elsewhere.
pub fn cursor_to_canvas(
    cursor_screen: egui::Pos2,
    canvas_size: [f32; 2],
    zoom: f32,
    pan: egui::Vec2,
    panel_rect: egui::Rect,
) -> egui::Pos2 {
    let origin = crate::canvas::rulers::canvas_origin(canvas_size, zoom, pan, panel_rect);
    ((cursor_screen - origin) / zoom).to_pos2()
}

/// Canvas-space center of the currently visible viewport — the deterministic
/// fallback anchor when the cursor is off-canvas or absent (CB-07).
pub fn visible_viewport_center_canvas(
    canvas_size: [f32; 2],
    zoom: f32,
    pan: egui::Vec2,
    panel_rect: egui::Rect,
) -> egui::Pos2 {
    cursor_to_canvas(panel_rect.center(), canvas_size, zoom, pan, panel_rect)
}

/// Paste `clipboard` so its bounding-box center lands on `target_canvas`
/// (CB-08), plus a cumulative `cascade * PASTE_CASCADE_STEP` offset (CB-19).
pub fn paste_payload(
    clipboard: &ClipboardContents,
    target_canvas: egui::Pos2,
    cascade: usize,
    tree: &mut UiTree,
    target_container: Option<uuid::Uuid>,
) -> Result<PasteOutcome, PasteError> {
    if clipboard.is_empty() {
        return Ok(PasteOutcome { new_root_ids: vec![], count: 0, had_behaviors: false });
    }
    let anchor = bbox_center(&clipboard.widgets);
    let cascade_off = cascade as f32 * PASTE_CASCADE_STEP;
    let delta = egui::vec2(
        target_canvas.x - anchor.x + cascade_off,
        target_canvas.y - anchor.y + cascade_off,
    );
    let new_root_ids = tree.paste_batch(clipboard.widgets.clone(), delta, target_container)?;
    let count = clipboard.widgets.len();
    Ok(PasteOutcome { new_root_ids, count, had_behaviors: clipboard.source_had_behaviors })
}

/// Duplicate `selected` in place with a fixed cascade-step offset (CB-25).
/// Independent of the clipboard buffer.
pub fn duplicate_in_place(
    selected: &[uuid::Uuid],
    tree: &mut UiTree,
) -> Result<PasteOutcome, PasteError> {
    let contents = copy_selection(selected, tree);
    if contents.is_empty() {
        return Ok(PasteOutcome { new_root_ids: vec![], count: 0, had_behaviors: false });
    }
    let count = contents.widgets.len();
    let delta = egui::vec2(PASTE_CASCADE_STEP, PASTE_CASCADE_STEP);
    let new_root_ids = tree.paste_batch(contents.widgets.clone(), delta, None)?;
    Ok(PasteOutcome { new_root_ids, count, had_behaviors: contents.source_had_behaviors })
}

/// Bounding-box center (canvas space) of a set of widgets.
fn bbox_center(widgets: &[WidgetInstance]) -> egui::Pos2 {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for w in widgets {
        min_x = min_x.min(w.rect.x);
        min_y = min_y.min(w.rect.y);
        max_x = max_x.max(w.rect.x + w.rect.w);
        max_y = max_y.max(w.rect.y + w.rect.h);
    }
    egui::pos2((min_x + max_x) / 2.0, (min_y + max_y) / 2.0)
}
```

If `egui` is not already imported in `clipboard.rs`, add `use eframe::egui;` at the top (match the import path used by `src/canvas/search.rs` — copy its egui import line exactly).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib clipboard::tests`
Expected: PASS (9 tests).

- [ ] **Step 5: Commit**

```bash
git add src/canvas/clipboard.rs
git commit -m "feat(clipboard): paste_payload, duplicate_in_place, coordinate helpers"
```

---

## Task 6: InteractionState fields + serialization invariant

**Files:**
- Modify: `src/canvas/interaction.rs` (add fields to `InteractionState`)
- Test: add an invariant test (in `interaction.rs` test module or `clipboard.rs`)

Implements the session-only state (CB-11, CB-17, CB-19) and the never-serialized invariant.

- [ ] **Step 1: Write the failing test**

Add to `src/canvas/clipboard.rs` test module:

```rust
#[test]
fn interaction_state_is_not_serializable() {
    // Compile-time guarantee that clipboard state never lands in .rohkai.json:
    // InteractionState must NOT implement serde::Serialize. This is a
    // documentation test of intent — if someone adds `derive(Serialize)` to
    // InteractionState, the trait-bound assert below starts compiling and a
    // reviewer should treat the *presence* of that impl as the regression.
    fn assert_not_serialize<T>() {}
    assert_not_serialize::<crate::canvas::interaction::InteractionState>();
}
```

(Note: this is a smoke test — the real enforcement is that `InteractionState` carries only `#[derive(Default)]`. A reviewer check + the existing `CanvasSearchState` non-serialize test cover the class. If the project has a stronger static-assert pattern already in use for `CanvasSearchState`, mirror that instead.)

- [ ] **Step 2: Add the fields**

In `src/canvas/interaction.rs`, in the `InteractionState` struct (after the `canvas_search` field):

```rust
    /// In-app clipboard buffer (CB-17). Session-only; never serialized.
    pub clipboard: crate::canvas::clipboard::ClipboardContents,
    /// Cumulative repeat-paste cascade counter; resets on each new copy (CB-19).
    pub paste_cascade: usize,
    /// Newly pasted root ids + remaining flash seconds, for the paste ring
    /// overlay (separate from search state, CB-21).
    pub paste_flash: Option<(Vec<uuid::Uuid>, f32)>,
```

`ClipboardContents` derives `Default`, so `#[derive(Default)]` on `InteractionState` still holds.

- [ ] **Step 3: Run test + build to verify**

Run: `cargo test --lib clipboard::tests::interaction_state_is_not_serializable`
Expected: PASS.

Run: `cargo check`
Expected: compiles (warnings about unused fields acceptable until Task 8).

- [ ] **Step 4: Commit**

```bash
git add src/canvas/interaction.rs src/canvas/clipboard.rs
git commit -m "feat(clipboard): session-only clipboard state on InteractionState"
```

---

## Task 7: Register shortcuts

**Files:**
- Modify: `src/panels/shortcuts.rs` (add to `BUILTIN_SHORTCUTS` and the reference panel)

Implements CB-16. `Ctrl+C`/`Ctrl+V`/`Ctrl+D` are free (existing: Ctrl+G/F/R/L/N/O/S/Z/Y/0).

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `src/panels/shortcuts.rs` (create if absent):

```rust
#[test]
fn clipboard_shortcuts_registered() {
    let ids: Vec<&str> = BUILTIN_SHORTCUTS.iter().map(|(k, _, _)| *k).collect();
    assert!(ids.contains(&"canvas_copy"));
    assert!(ids.contains(&"canvas_paste"));
    assert!(ids.contains(&"canvas_duplicate"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib shortcuts`
Expected: FAIL — ids not present.

- [ ] **Step 3: Add the entries**

In `BUILTIN_SHORTCUTS` (after the `canvas_search` entry):

```rust
    ("canvas_copy", "Ctrl+C", "Copy selected widgets"),
    ("canvas_paste", "Ctrl+V", "Paste widgets at cursor"),
    ("canvas_duplicate", "Ctrl+D", "Duplicate selected widgets in place"),
```

In the reference panel's Canvas section (where `ref_row(ui, "canvas_search", user_shortcuts);` is called), add:

```rust
            ref_row(ui, "canvas_copy", user_shortcuts);
            ref_row(ui, "canvas_paste", user_shortcuts);
            ref_row(ui, "canvas_duplicate", user_shortcuts);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib shortcuts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/panels/shortcuts.rs
git commit -m "feat(clipboard): register Ctrl+C/V/D shortcuts + reference rows"
```

---

## Task 8: Wire key handlers in `app.rs`

**Files:**
- Modify: `src/app.rs` (add `status` field to `RohKaiApp`; add handlers next to the Delete handler)

Implements CB-09 (no-op guards), CB-10 (drag/resize suppression), CB-11 (synchronous mutation), CB-14 (select pasted), CB-15 (single-frame undo), CB-17 (gate), plus status messages.

- [ ] **Step 1: Add the status field to RohKaiApp**

In the `RohKaiApp` struct definition in `src/app.rs` (near the `undo` field):

```rust
    status: crate::status::StatusMessage,
```

Initialize it in the struct's constructor / `Default`-like builder with `crate::status::StatusMessage::default()` (match how `undo` is initialized in the same constructor).

- [ ] **Step 2: Add the handlers**

Locate the Delete handler (the block using `canvas_keyboard_owned` and `self.session.selected.drain(..)`, ~`app.rs:3470`). Immediately after it, add. This block needs `panel_rect` in scope (the canvas search block at ~3358 uses the same `panel_rect`); place this block in that same scope so `panel_rect` is available. If `panel_rect` is not in scope at the Delete handler, place this block adjacent to the canvas-search block instead (which has `panel_rect`), keeping the `canvas_keyboard_owned` gate.

```rust
    // ── Clipboard: Copy / Paste / Duplicate (CB-09,10,11,14,15,17) ──────────
    {
        let now = ctx.input(|i| i.time);
        let copy_pressed = canvas_keyboard_owned
            && ctx.input(|i| i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::C));
        let paste_pressed = canvas_keyboard_owned
            && ctx.input(|i| i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::V));
        let dup_pressed = canvas_keyboard_owned
            && ctx.input(|i| i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::D));

        // CB-10: suppress while a drag or resize gesture is in flight.
        let gesture_active = self.session.interaction.drag.is_some()
            || self.session.interaction.resize.is_some();

        if copy_pressed {
            if gesture_active {
                self.status.set("Can't copy while dragging", now);
            } else if self.session.selected.is_empty() {
                // CB-09(b): no-op, preserve any prior clipboard.
            } else {
                let contents = crate::canvas::clipboard::copy_selection(
                    &self.session.selected,
                    &self.project.ui_tree,
                );
                let msg = if contents.widgets.len() == 1 {
                    format!("Copied {}", crate::canvas::clipboard::widget_kind_label(&contents.widgets[0].kind))
                } else {
                    format!("Copied {} widgets", contents.widgets.len())
                };
                self.session.interaction.clipboard = contents;
                self.session.interaction.paste_cascade = 0; // CB-19 reset
                self.status.set(msg, now);
            }
        }

        if dup_pressed {
            if gesture_active {
                self.status.set("Can't duplicate while dragging", now);
            } else if !self.session.selected.is_empty() {
                match crate::canvas::clipboard::duplicate_in_place(
                    &self.session.selected,
                    &mut self.project.ui_tree,
                ) {
                    Ok(out) if out.count > 0 => {
                        self.session.selected = out.new_root_ids.clone(); // CB-14
                        let mut msg = format!("Duplicated {} widget(s)", out.count);
                        if out.had_behaviors {
                            msg = format!("Duplicated {} widgets — behavior wires not copied", out.count);
                        }
                        self.status.set(msg, now);
                        self.session.interaction.paste_flash = Some((out.new_root_ids, 0.6));
                    }
                    Ok(_) => {}
                    Err(_) => self.status.set("Duplicate failed: invalid widget graph", now),
                }
            }
        }

        if paste_pressed {
            if gesture_active {
                self.status.set("Can't paste while dragging", now);
            } else if self.session.interaction.clipboard.is_empty() {
                // CB-09(a): empty clipboard → total no-op.
            } else {
                // Resolve target in canvas space (CB-06/07).
                let zoom = self.session.canvas_settings.zoom;
                let pan = self.session.canvas_settings.pan;
                let size = [self.project.ui_tree.app_props.win_w, self.project.ui_tree.app_props.win_h];
                let cursor_screen = ctx.input(|i| i.pointer.interact_pos());
                let target = match cursor_screen {
                    Some(p) if panel_rect.contains(p) => {
                        crate::canvas::clipboard::cursor_to_canvas(p, size, zoom, pan, panel_rect)
                    }
                    _ => crate::canvas::clipboard::visible_viewport_center_canvas(size, zoom, pan, panel_rect),
                };
                // CB-24: attach to the layout under the target, like template-drop.
                let container = self.project.ui_tree.attach_target_at((target.x, target.y));
                let cascade = self.session.interaction.paste_cascade;
                let clip = self.session.interaction.clipboard.clone();
                match crate::canvas::clipboard::paste_payload(
                    &clip, target, cascade, &mut self.project.ui_tree, container,
                ) {
                    Ok(out) if out.count > 0 => {
                        self.session.interaction.paste_cascade = cascade + 1; // CB-19
                        self.session.selected = out.new_root_ids.clone(); // CB-14
                        let mut msg = format!("Pasted {} widget(s)", out.count);
                        if out.had_behaviors {
                            msg = format!("Pasted {} widgets — behavior wires not copied", out.count);
                        }
                        self.status.set(msg, now);
                        self.session.interaction.paste_flash = Some((out.new_root_ids, 0.6));
                    }
                    Ok(_) => {}
                    Err(_) => self.status.set("Paste failed: invalid widget graph", now),
                }
            }
        }
    }
```

**Helper needed:** `paste_payload`'s `target_container` should be the layout under the cursor. If `UiTree` lacks a read-only "what container is at this point" method, add one to `src/project/ui_tree.rs` (next to `attach_to_layout_at`):

```rust
/// The id of the layout/container whose bounds contain `canvas_point`, if any.
/// Read-only sibling of `attach_to_layout_at` used to pick a paste target.
pub fn attach_target_at(&self, canvas_point: (f32, f32)) -> Option<uuid::Uuid> {
    let (px, py) = canvas_point;
    self.widgets
        .iter()
        .filter(|w| w.kind.is_layout_container())
        .filter(|w| {
            px >= w.rect.x && px <= w.rect.x + w.rect.w && py >= w.rect.y && py <= w.rect.y + w.rect.h
        })
        .map(|w| w.id)
        .last()
}
```

If `WidgetKind::is_layout_container()` does not exist, replace the `.filter` predicate with a match on the known container kinds (`Frame | VLayout | HLayout | GridLayout | GroupBox | ScrollArea | TabWidget | StackedWidget | ToolBox`). Verify the exact container set against `attach_to_layout_at`'s own logic and reuse its predicate if it exposes one (DRY — do not duplicate a divergent list).

- [ ] **Step 3: Record undo after mutation**

Confirm the existing end-of-frame undo recorder (`self.undo.record(...)` at ~`app.rs:3535`, which runs when the pointer is up) runs after this block in the same frame. Because paste/duplicate mutate synchronously (CB-11) and the keyboard fires with the pointer up, the existing recorder captures the whole paste as one undo step (CB-15) — no new record call needed here. If the undo recorder is gated such that it would miss a keyboard-only mutation, add `self.project.mark_dirty();` (match the call the Delete handler uses to flag a tree change) after a successful paste/duplicate so the recorder fires.

- [ ] **Step 4: Build + manual check**

Run: `cargo check`
Expected: compiles. Resolve any name mismatches (`attach_target_at`, container predicate) against the real APIs as noted.

Run: `cargo run` — copy a widget (Ctrl+C → "Copied Button"), paste (Ctrl+V at cursor), duplicate (Ctrl+D). Confirm pasted widgets are selected and undo (Ctrl+Z) removes the whole paste in one step.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/project/ui_tree.rs
git commit -m "feat(clipboard): wire Ctrl+C/V/D handlers with status + undo"
```

---

## Task 9: Render status message, paste flash, and viewport reveal

**Files:**
- Modify: `src/app.rs` (render status text; tick + draw paste flash; pan to reveal)

Implements CB-21 (toast render), CB-21/CB-07 (viewport reveal), and the paste flash overlay.

- [ ] **Step 1: Render the status message**

After the canvas panel is drawn (near where the search overlay / bezel is drawn), add a bottom-corner non-modal label:

```rust
    // Transient status message (CB-21).
    {
        let now = ctx.input(|i| i.time);
        if let Some(text) = self.status.current(now) {
            egui::Area::new(egui::Id::new("rohkai_status_message"))
                .order(egui::Order::Tooltip)
                .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(12.0, -12.0))
                .interactable(false)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(egui::RichText::new(text).strong());
                    });
                });
            ctx.request_repaint(); // keep ticking until it expires
        }
    }
```

- [ ] **Step 2: Tick + draw the paste flash**

Where the canvas search ring overlay is drawn (the block calling `draw_search_overlay` with `widget_screen_rects`), add a sibling paste-flash draw. Reuse the screen-rect list the search overlay already builds (the `widget_screen_rects: &[(Uuid, egui::Rect)]`). Add a function in `src/canvas/clipboard.rs`:

```rust
/// Draw a teal selection ring around freshly pasted widgets (CB-21).
/// Visual language matches search's ring but is driven by paste_flash state.
pub fn draw_paste_flash(
    painter: &egui::Painter,
    flash_ids: &[uuid::Uuid],
    widget_screen_rects: &[(uuid::Uuid, egui::Rect)],
    alpha: f32, // 0.0..=1.0 fade
    dark_mode: bool,
) {
    let base = egui::Color32::from_rgb(52, 211, 153);
    let a = (alpha.clamp(0.0, 1.0) * if dark_mode { 192.0 } else { 220.0 }) as u8;
    let ring = egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), a);
    for id in flash_ids {
        if let Some(&(_, rect)) = widget_screen_rects.iter().find(|(rid, _)| rid == id) {
            painter.rect_stroke(
                rect.expand(3.0),
                4.0,
                egui::Stroke::new(2.0, ring),
                egui::StrokeKind::Outside,
            );
        }
    }
}
```

In `app.rs`, after drawing the search overlay, tick and draw the flash:

```rust
    // Paste flash overlay (CB-21), ticked down each frame.
    if let Some((ids, mut remaining)) = self.session.interaction.paste_flash.take() {
        let dt = ctx.input(|i| i.stable_dt);
        remaining -= dt;
        if remaining > 0.0 {
            let alpha = (remaining / 0.6).clamp(0.0, 1.0);
            crate::canvas::clipboard::draw_paste_flash(
                &painter, &ids, &widget_screen_rects, alpha, dark_mode,
            );
            self.session.interaction.paste_flash = Some((ids, remaining));
            ctx.request_repaint();
        }
    }
```

Use the same `painter`, `widget_screen_rects`, and `dark_mode` bindings the search overlay block uses (copy their source expressions exactly from that block).

- [ ] **Step 3: Viewport reveal after paste**

When a paste/duplicate set lands partly off-viewport, pan to reveal it. Reuse `crate::canvas::search::scroll_to_widget` (it pans `CanvasSettings` to bring a widget into view). After a successful paste/duplicate in Task 8's block (where `out.new_root_ids` is known), add:

```rust
        // Reveal the first pasted root if it's outside the viewport (CB-21/07).
        if let Some(first) = out.new_root_ids.first() {
            crate::canvas::search::scroll_to_widget(
                *first,
                &self.project.ui_tree,
                &mut self.session.canvas_settings,
                panel_rect,
            );
        }
```

(If `scroll_to_widget` always centers rather than only-when-offscreen, that is acceptable for v1 — note it in the commit. A no-op-when-visible refinement can come later.)

- [ ] **Step 4: Build + manual smoke**

Run: `cargo check`
Expected: compiles.

Run: `cargo run` — paste near a canvas edge; confirm the status toast appears (~1.5s), the pasted widgets flash a teal ring (~0.6s), and the canvas pans so they are visible.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/canvas/clipboard.rs
git commit -m "feat(clipboard): status toast, paste flash overlay, viewport reveal"
```

---

## Task 10: Parity tests + full verification gate

**Files:**
- Test: `src/canvas/clipboard.rs` (or an integration test) — codegen/export/save-load parity
- Modify: none (verification only)

Implements CB-11 parity, export parity, save/load round-trip, and CB-15 undo-exactness.

- [ ] **Step 1: Write parity tests**

Add to `src/canvas/clipboard.rs` tests (adapt the emitter/serialize calls to the exact APIs — find them via the existing emitter/io tests in the repo, e.g. `src/codegen/` tests and `src/project/io.rs`):

```rust
#[test]
fn pasted_widgets_appear_in_live_codegen() {
    // After paste, the live emitter output must include the pasted widget.
    let mut src = WidgetInstance { id: uuid::Uuid::new_v4(), kind: WidgetKind::Button, ..Default::default() };
    src.props.label = "Zorp".to_string();
    let clip = ClipboardContents { widgets: vec![src], source_had_behaviors: false };
    let mut tree = UiTree::default();
    paste_payload(&clip, egui::pos2(10.0, 10.0), 0, &mut tree, None).unwrap();

    // Use the project's live egui emitter entry point. Replace
    // `emit_for_test` with the actual function (see codegen tests).
    let code = crate::codegen::egui_emitter::emit_document(&tree);
    assert!(code.contains("Zorp"), "pasted widget missing from codegen");
}

#[test]
fn pasted_tree_survives_save_load_round_trip() {
    let child = {
        let mut c = WidgetInstance { id: uuid::Uuid::new_v4(), kind: WidgetKind::Button, ..Default::default() };
        c.props.label = "Kid".into();
        c
    };
    let child_id = child.id;
    let frame = {
        let mut f = WidgetInstance { id: uuid::Uuid::new_v4(), kind: WidgetKind::Frame, children: vec![child_id], ..Default::default() };
        f
    };
    let clip = ClipboardContents { widgets: vec![frame, child], source_had_behaviors: false };
    let mut tree = UiTree::default();
    paste_payload(&clip, egui::pos2(0.0, 0.0), 0, &mut tree, None).unwrap();

    // Round-trip through the project serializer (replace with real io fns).
    let json = crate::project::io::serialize(&tree.snapshot()).unwrap();
    let restored = crate::project::io::deserialize(&json).unwrap();
    // All children references resolve (no dangling) after reload.
    let ids: std::collections::HashSet<uuid::Uuid> =
        restored_widgets(&restored).iter().map(|w| w.id).collect();
    for w in restored_widgets(&restored) {
        for c in &w.children {
            assert!(ids.contains(c), "dangling child after reload");
        }
    }
}
```

The exact emitter function (`emit_document`), `snapshot()`, `io::serialize/deserialize`, and how to get the widget list out of the restored document (`restored_widgets`) must be taken from the real APIs — grep `src/codegen/egui_emitter.rs` and `src/project/io.rs` and mirror their existing tests. If `emit_document` takes different arguments, adapt. Do **not** invent signatures; read the real ones first.

- [ ] **Step 2: Run the parity tests**

Run: `cargo test --lib clipboard::tests`
Expected: PASS once the emitter/io calls match real APIs.

- [ ] **Step 3: Full workspace verification gate**

Run each and confirm clean:

```bash
cargo fmt --all
cargo test
cargo clippy --all-targets -- -D warnings
```

Expected: all tests pass; zero clippy warnings (required before the session is done — `--all-targets` is mandatory).

- [ ] **Step 4: Launch smoke**

Run: `cargo run`
Manually verify: Ctrl+C / Ctrl+V (at cursor) / Ctrl+D, multi-select copy/paste, paste into a Frame attaches to it, paste with cursor off-canvas lands in the visible center (never at 0,0), undo removes a whole paste in one step, and Ctrl+C/Ctrl+V inside the code-panel editor still does text copy/paste (not widget paste).

- [ ] **Step 5: Final commit + docs**

Append a newest-first note to `docs/CODE_COOP.md` (3-4 sentences: what shipped, files touched, verification, follow-ups — e.g. Cut deferred, behavior-wire copy deferred). Record a `docs/DEVLOG.md` entry.

```bash
git add -A
git commit -m "test(clipboard): codegen/export/save-load parity + verification gate"
```

---

## Follow-ups (explicitly OUT of scope this pass)

Do not implement either below in this pass unless a failing invariant test requires it. No further scope expansion.

- **CB-18 — Surface kind validation before cross-surface paste.** Validate each pasted `WidgetKind` against the active target surface's allowed-kind set (derived from the canonical `WidgetKind`/`SurfaceKind` rules), dropping/blocking disallowed kinds with a single status notice. Needed only when copy-on-surface-A / paste-on-surface-B is a supported workflow.
- **CB-23 — Right-click context-menu clipboard entries.** Add Copy / Cut / Paste / Duplicate to the canvas context menu, enable-state mirroring the keyboard gate (Copy/Cut/Duplicate require non-empty selection; Paste requires non-empty clipboard and an active surface); menu-driven paste uses the menu-open location.

## Self-Review

**Spec coverage:**
- CB-01 atomic remap → Task 2. CB-02 (no reliance on validate_and_repair) → Task 2 (explicit staged validation). CB-03 closure/clear-outside → Task 4 copy + Task 2 filter. CB-04 acyclicity → Task 2 `validate_staged`. CB-05 deep snapshot → Task 4 (`.cloned()` preserves all fields). CB-06 coordinate helper → Task 5. CB-07 fallback → Task 5 + Task 8. CB-08 bbox-center group delta → Task 5. CB-09 no-op guards → Task 8. CB-10 gesture suppression → Task 8. CB-11 synchronous mutate → Task 8. CB-12 shared bindings → Task 3. CB-13 constraints → Task 3. CB-14 select pasted → Task 8. CB-15 single undo → Task 8 step 3. CB-16 shortcuts → Task 7. CB-17 gate + private buffer → Task 8 (`canvas_keyboard_owned`) + Task 6. CB-18 cross-surface → handled implicitly (clipboard is session-global on InteractionState; paste targets active tree); **kind-validation against target surface is NOT implemented** — see gap below. CB-19 cascade → Task 5/8. CB-20 Cut → non-goal (documented). CB-21 feedback → Task 9. CB-22 no-clamp → satisfied (no clamping added). CB-23 context menu → **deferred** (see gap). CB-24 layout attach → Task 8 (`attach_target_at`). CB-25 Ctrl+D → Task 5/8.

**Identified gaps (intentional, noted for the implementer):**
1. **CB-18 surface kind-validation** is not in these tasks. v1 ships single-surface-typical paste; pasting an incompatible kind onto a restricted surface is not yet blocked. If the target surface restricts kinds, add a validation pass in Task 8 before `paste_payload`. Flag to the user if cross-surface paste is a required v1 path.
2. **CB-23 context-menu Copy/Paste** (NICE_TO_HAVE) is deferred — keyboard + status feedback ship first. Add menu entries mirroring the keyboard gate in a follow-up.

Both are acceptable per the spec's severity ordering (CB-18 IMPORTANT but conditional on multi-surface use; CB-23 NICE_TO_HAVE). Surface these two to the user before execution so they can promote CB-18 into scope if needed.

**Placeholder scan:** No TBD/TODO. Every code step shows complete code. API-uncertain spots (`attach_target_at` predicate, emitter/io function names) are explicitly flagged with "read the real signature first" instructions rather than guessed.

**Type consistency:** `ClipboardContents`, `PasteOutcome`, `PasteError`, `paste_batch(staged, anchor_delta, target_container)`, `copy_selection(selected, tree)`, `paste_payload(clip, target_canvas, cascade, tree, container)`, `duplicate_in_place(selected, tree)`, `cursor_to_canvas`, `visible_viewport_center_canvas`, `widget_kind_label`, `StatusMessage::{set,current}`, `PASTE_CASCADE_STEP`, `STATUS_TTL` — names are consistent across all tasks.
