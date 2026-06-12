//! P2.3 — Constraint-Based Layout solver.
//!
//! [`apply_constraints`] adjusts each widget's `rect` (x, y, w, h) according to
//! the [`LayoutConstraints`] stored on it. Two properties make it safe to call
//! every frame (it is, from `app.rs`):
//!
//! 1. **Idempotent.** Every operation is an *absolute* assignment derived from
//!    the widget's authored size and its alignment frame — never a cumulative
//!    `+=` / `-=`. Running the solve N times equals running it once, so the
//!    persisted `rect` never drifts and margins never compound. (The earlier
//!    implementation shifted `x += margin.left` every frame, walking widgets off
//!    screen; margin is now folded into the absolute alignment computation.)
//! 2. **Parent-relative.** A widget's alignment frame is its *parent's* solved
//!    rect, falling back to the canvas (app window) only for top-level widgets.
//!    Widgets are solved parents-before-children so a child sees its parent's
//!    final rect. (The earlier implementation always anchored to the canvas
//!    root, so a constrained child of a Frame centred against the whole window.)
//!
//! Margin `[top, right, bottom, left]` insets the widget *within* its aligned
//! anchor — it has no effect on an axis with no alignment, because there is no
//! anchor to inset from (and inventing one is what made the old code drift).
//!
//! [`validate_constraints`] detects conflicting or unsatisfiable constraints and
//! returns a list of [`ConstraintError`] values — surfaced in the Properties
//! panel's Constraints section (see `panels::properties::show_constraints`).

use crate::project::schema::{HAlign, LayoutConstraints, Rect, VAlign};
use crate::project::ui_tree::UiTree;
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// A constraint problem detected by [`validate_constraints`].
#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintError {
    /// `equal_width_to` / `equal_height_to` references a widget ID that does
    /// not exist in the tree.
    UnknownTarget { widget_id: Uuid, target_id: String },
    /// A widget references itself via `equal_width_to` or `equal_height_to`.
    SelfReference { widget_id: Uuid },
    /// Two widgets form a mutual equal-size cycle (A→B and B→A).
    EqualSizeCycle { widget_a: Uuid, widget_b: Uuid },
    /// `aspect_ratio` is set but is not a positive finite value.
    InvalidAspectRatio { widget_id: Uuid, value: f32 },
}

impl ConstraintError {
    /// Human-readable one-line description for the Properties panel.
    pub fn message(&self) -> String {
        match self {
            ConstraintError::UnknownTarget { target_id, .. } => {
                format!("equal-size target {target_id} does not exist")
            }
            ConstraintError::SelfReference { .. } => {
                "equal-size constraint references itself".to_owned()
            }
            ConstraintError::EqualSizeCycle { .. } => {
                "mutual equal-size cycle (A=B and B=A)".to_owned()
            }
            ConstraintError::InvalidAspectRatio { value, .. } => {
                format!("aspect ratio {value} is not a positive number")
            }
        }
    }

    /// The widget this error is attached to (for per-widget filtering in the UI).
    pub fn widget_id(&self) -> Uuid {
        match self {
            ConstraintError::UnknownTarget { widget_id, .. }
            | ConstraintError::SelfReference { widget_id }
            | ConstraintError::InvalidAspectRatio { widget_id, .. } => *widget_id,
            ConstraintError::EqualSizeCycle { widget_a, .. } => *widget_a,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Apply all [`LayoutConstraints`] stored in `tree`, adjusting widget rects
/// in-place. Idempotent and parent-relative (see module docs). Safe to call
/// every frame.
pub fn apply_constraints(tree: &mut UiTree) {
    let canvas = Rect {
        x: 0.0,
        y: 0.0,
        w: tree.app_props.win_w,
        h: tree.app_props.win_h,
    };

    // Snapshot authored sizes for equal-size links (id string → (w, h)).
    let size_snapshot: HashMap<String, (f32, f32)> = tree
        .widgets
        .iter()
        .map(|w| (w.id.to_string(), (w.rect.w, w.rect.h)))
        .collect();

    // Solve parents before children so a child's frame is its parent's *solved*
    // rect. Widgets with default constraints solve to a no-op.
    for id in solve_order(tree) {
        let Some(i) = tree.widgets.iter().position(|w| w.id == id) else {
            continue;
        };
        let constraints = tree.widgets[i].constraints.clone();

        let frame = match tree.parent_of(id) {
            Some(pid) => tree
                .widgets
                .iter()
                .find(|w| w.id == pid)
                .map(|p| p.rect.clone())
                .unwrap_or_else(|| canvas.clone()),
            None => canvas.clone(),
        };

        let mut r = tree.widgets[i].rect.clone();
        solve_one(&mut r, &constraints, &size_snapshot, frame);
        tree.widgets[i].rect = r;
    }
}

/// Validate all constraints in `tree` and return a list of detected problems.
/// An empty result means no problems were found.
pub fn validate_constraints(tree: &UiTree) -> Vec<ConstraintError> {
    let mut errors = Vec::new();
    let id_set: std::collections::HashSet<String> =
        tree.widgets.iter().map(|w| w.id.to_string()).collect();

    for w in &tree.widgets {
        let c = &w.constraints;

        // Check equal_width_to reference
        if let Some(ref target) = c.equal_width_to {
            if target == &w.id.to_string() {
                errors.push(ConstraintError::SelfReference { widget_id: w.id });
            } else if !id_set.contains(target) {
                errors.push(ConstraintError::UnknownTarget {
                    widget_id: w.id,
                    target_id: target.clone(),
                });
            }
        }

        // Check equal_height_to reference
        if let Some(ref target) = c.equal_height_to {
            if target == &w.id.to_string() {
                // Avoid duplicate SelfReference if both equal_width_to and equal_height_to
                // point to self — only report once per widget.
                if c.equal_width_to.as_deref() != Some(target.as_str()) {
                    errors.push(ConstraintError::SelfReference { widget_id: w.id });
                }
            } else if !id_set.contains(target) {
                // Avoid duplicate UnknownTarget if both fields name the same bad id.
                if c.equal_width_to.as_deref() != Some(target.as_str()) {
                    errors.push(ConstraintError::UnknownTarget {
                        widget_id: w.id,
                        target_id: target.clone(),
                    });
                }
            }
        }

        // Check aspect_ratio validity
        if let Some(ar) = c.aspect_ratio {
            if !ar.is_finite() || ar <= 0.0 {
                errors.push(ConstraintError::InvalidAspectRatio {
                    widget_id: w.id,
                    value: ar,
                });
            }
        }
    }

    // Detect mutual equal-size cycles (A→B and B→A for width or height).
    detect_equal_size_cycles(tree, &mut errors);

    errors
}

// ---------------------------------------------------------------------------
// Private solver
// ---------------------------------------------------------------------------

/// Order widget IDs so that every parent precedes its children (ascending tree
/// depth). A child's alignment frame is its parent's already-solved rect.
fn solve_order(tree: &UiTree) -> Vec<Uuid> {
    let mut depths: Vec<(usize, Uuid)> = tree
        .widgets
        .iter()
        .map(|w| {
            let mut depth = 0usize;
            let mut cur = w.id;
            // Bounded walk up the parent chain (guard against cycles in data).
            while let Some(p) = tree.parent_of(cur) {
                depth += 1;
                cur = p;
                if depth > 64 {
                    break;
                }
            }
            (depth, w.id)
        })
        .collect();
    depths.sort_by_key(|(d, _)| *d);
    depths.into_iter().map(|(_, id)| id).collect()
}

/// Idempotent, absolute solve of one widget's rect within `frame`.
fn solve_one(
    r: &mut Rect,
    c: &LayoutConstraints,
    snapshot: &HashMap<String, (f32, f32)>,
    frame: Rect,
) {
    // 1. Equal-size links — copy authored w/h from the target's snapshot.
    if let Some(ref target_id) = c.equal_width_to {
        if let Some(&(target_w, _)) = snapshot.get(target_id) {
            r.w = target_w;
        }
    }
    if let Some(ref target_id) = c.equal_height_to {
        if let Some(&(_, target_h)) = snapshot.get(target_id) {
            r.h = target_h;
        }
    }

    // 2. Aspect-ratio lock — preserve width, derive height.
    if let Some(ratio) = c.aspect_ratio {
        if ratio.is_finite() && ratio > 0.0 {
            r.h = r.w / ratio;
        }
    }

    // 3. Min/max clamps.
    if let Some(min_w) = c.min_w {
        r.w = r.w.max(min_w);
    }
    if let Some(max_w) = c.max_w {
        r.w = r.w.min(max_w);
    }
    if let Some(min_h) = c.min_h {
        r.h = r.h.max(min_h);
    }
    if let Some(max_h) = c.max_h {
        r.h = r.h.min(max_h);
    }

    // 4. Alignment + margin, folded into one absolute computation per axis so the
    //    result is idempotent. margin = [top, right, bottom, left].
    let [m_top, m_right, m_bottom, m_left] = c.margin;

    match c.h_align {
        Some(HAlign::Stretch) => {
            r.x = frame.x + m_left;
            r.w = (frame.w - m_left - m_right).max(0.0);
        }
        Some(HAlign::Leading) => r.x = frame.x + m_left,
        Some(HAlign::Trailing) => r.x = frame.x + frame.w - r.w - m_right,
        Some(HAlign::Center) => {
            let avail = frame.w - m_left - m_right;
            r.x = frame.x + m_left + (avail - r.w) / 2.0;
        }
        // No horizontal anchor: position is free, so a horizontal margin has
        // nothing to inset against and is intentionally a no-op (keeps the solve
        // idempotent — see module docs).
        None => {}
    }

    match c.v_align {
        Some(VAlign::Stretch) => {
            r.y = frame.y + m_top;
            r.h = (frame.h - m_top - m_bottom).max(0.0);
        }
        Some(VAlign::Top) => r.y = frame.y + m_top,
        Some(VAlign::Bottom) => r.y = frame.y + frame.h - r.h - m_bottom,
        Some(VAlign::Center) => {
            let avail = frame.h - m_top - m_bottom;
            r.y = frame.y + m_top + (avail - r.h) / 2.0;
        }
        None => {}
    }
}

fn detect_equal_size_cycles(tree: &UiTree, errors: &mut Vec<ConstraintError>) {
    // Build adjacency for equal-width links: id → target_id.
    let mut width_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut height_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for w in &tree.widgets {
        let id_str = w.id.to_string();
        if let Some(ref t) = w.constraints.equal_width_to {
            width_map.insert(id_str.clone(), t.clone());
        }
        if let Some(ref t) = w.constraints.equal_height_to {
            height_map.insert(id_str, t.clone());
        }
    }

    // Check mutual cycles in width map.
    for (a, b) in &width_map {
        if width_map.get(b) == Some(a) && a < b {
            // Find UUIDs from string.
            if let (Some(wa), Some(wb)) =
                (uuid_from_str_in_tree(tree, a), uuid_from_str_in_tree(tree, b))
            {
                errors.push(ConstraintError::EqualSizeCycle {
                    widget_a: wa,
                    widget_b: wb,
                });
            }
        }
    }

    // Check mutual cycles in height map.
    for (a, b) in &height_map {
        if height_map.get(b) == Some(a) && a < b {
            if let (Some(wa), Some(wb)) =
                (uuid_from_str_in_tree(tree, a), uuid_from_str_in_tree(tree, b))
            {
                // Only report if not already reported for width.
                let already = errors.iter().any(|e| {
                    matches!(e, ConstraintError::EqualSizeCycle { widget_a, widget_b }
                        if (*widget_a == wa && *widget_b == wb) || (*widget_a == wb && *widget_b == wa))
                });
                if !already {
                    errors.push(ConstraintError::EqualSizeCycle {
                        widget_a: wa,
                        widget_b: wb,
                    });
                }
            }
        }
    }
}

fn uuid_from_str_in_tree(tree: &UiTree, id_str: &str) -> Option<Uuid> {
    tree.widgets
        .iter()
        .find(|w| w.id.to_string() == id_str)
        .map(|w| w.id)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{Rect, WidgetInstance, WidgetKind};
    use uuid::Uuid;

    fn make_tree_with_widgets(widgets: Vec<WidgetInstance>) -> UiTree {
        let mut tree = UiTree::default();
        tree.app_props.win_w = 800.0;
        tree.app_props.win_h = 600.0;
        for w in widgets {
            tree.widgets.push(w);
        }
        tree
    }

    fn widget_at(x: f32, y: f32, w: f32, h: f32) -> WidgetInstance {
        WidgetInstance {
            id: Uuid::new_v4(),
            rect: Rect { x, y, w, h },
            ..WidgetInstance::default()
        }
    }

    #[test]
    fn aspect_ratio_locks_width_to_height() {
        let mut w = widget_at(10.0, 10.0, 100.0, 50.0);
        w.constraints.aspect_ratio = Some(2.0); // w:h = 2:1 → h = w/2 = 50
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        let ww = tree.widgets[0].rect.w;
        let wh = tree.widgets[0].rect.h;
        assert!((ww - 100.0).abs() < 0.01, "width should remain 100, got {ww}");
        assert!((wh - 50.0).abs() < 0.01, "height should be 50 (100/2.0), got {wh}");
    }

    #[test]
    fn aspect_ratio_square() {
        let mut w = widget_at(0.0, 0.0, 80.0, 999.0);
        w.constraints.aspect_ratio = Some(1.0); // square
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        let ww = tree.widgets[0].rect.w;
        let wh = tree.widgets[0].rect.h;
        assert!((ww - wh).abs() < 0.01, "square: w={ww} h={wh} should be equal");
    }

    #[test]
    fn margin_insets_within_alignment() {
        // New idempotent semantics: margin insets *within* the alignment anchor.
        // Leading + left margin 20 → x = frame.x + 20; Top + top margin 10 → y = 10.
        let mut w = widget_at(0.0, 0.0, 200.0, 100.0);
        w.constraints.h_align = Some(HAlign::Leading);
        w.constraints.v_align = Some(VAlign::Top);
        w.constraints.margin = [10.0, 20.0, 10.0, 20.0]; // [top,right,bottom,left]
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        let r = tree.widgets[0].rect.clone();
        assert!((r.x - 20.0).abs() < 0.01, "x should be left margin 20, got {}", r.x);
        assert!((r.y - 10.0).abs() < 0.01, "y should be top margin 10, got {}", r.y);
    }

    #[test]
    fn margin_without_alignment_is_noop() {
        // No anchor → margin must not move or resize the widget (idempotency
        // contract: there is nothing to inset against).
        let mut w = widget_at(50.0, 60.0, 200.0, 100.0);
        w.constraints.margin = [10.0, 20.0, 10.0, 20.0];
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        let r = tree.widgets[0].rect.clone();
        assert_eq!((r.x, r.y, r.w, r.h), (50.0, 60.0, 200.0, 100.0));
    }

    #[test]
    fn solve_is_idempotent_across_frames() {
        // The regression that motivated the rewrite: running the solve every
        // frame must not walk a margined/aligned widget off screen.
        let mut w = widget_at(0.0, 0.0, 120.0, 40.0);
        w.constraints.h_align = Some(HAlign::Trailing);
        w.constraints.v_align = Some(VAlign::Bottom);
        w.constraints.margin = [5.0, 8.0, 5.0, 8.0];
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        let after_one = tree.widgets[0].rect.clone();
        for _ in 0..30 {
            apply_constraints(&mut tree);
        }
        let after_many = tree.widgets[0].rect.clone();
        assert_eq!(
            (after_one.x, after_one.y, after_one.w, after_one.h),
            (after_many.x, after_many.y, after_many.w, after_many.h),
            "solve must be idempotent: 1 pass != 31 passes"
        );
        // Trailing anchor: x = 800 - 120 - 8 = 672; Bottom: y = 600 - 40 - 5 = 555.
        assert!((after_one.x - 672.0).abs() < 0.01, "x={}", after_one.x);
        assert!((after_one.y - 555.0).abs() < 0.01, "y={}", after_one.y);
    }

    #[test]
    fn stretch_fills_frame_minus_margins() {
        let mut w = widget_at(0.0, 0.0, 50.0, 50.0);
        w.constraints.h_align = Some(HAlign::Stretch);
        w.constraints.v_align = Some(VAlign::Stretch);
        w.constraints.margin = [10.0, 20.0, 30.0, 40.0]; // t,r,b,l
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        let r = tree.widgets[0].rect.clone();
        assert!((r.x - 40.0).abs() < 0.01, "x=left margin 40, got {}", r.x);
        assert!((r.y - 10.0).abs() < 0.01, "y=top margin 10, got {}", r.y);
        assert!((r.w - (800.0 - 40.0 - 20.0)).abs() < 0.01, "w fills minus l+r, got {}", r.w);
        assert!((r.h - (600.0 - 10.0 - 30.0)).abs() < 0.01, "h fills minus t+b, got {}", r.h);
    }

    #[test]
    fn alignment_is_parent_relative_not_canvas() {
        // A Center-aligned child of a Frame centres within the Frame, not the
        // 800x600 canvas. Frame at (100,100) size 400x300 → child centres at
        // x = 100 + (400-80)/2 = 260, y = 100 + (300-40)/2 = 230.
        let parent_id = Uuid::from_u128(0x100);
        let child_id = Uuid::from_u128(0x101);
        let parent = WidgetInstance {
            id: parent_id,
            kind: WidgetKind::Frame,
            rect: Rect { x: 100.0, y: 100.0, w: 400.0, h: 300.0 },
            children: vec![child_id],
            ..Default::default()
        };
        let mut child = WidgetInstance {
            id: child_id,
            kind: WidgetKind::Button,
            rect: Rect { x: 0.0, y: 0.0, w: 80.0, h: 40.0 },
            ..Default::default()
        };
        child.constraints.h_align = Some(HAlign::Center);
        child.constraints.v_align = Some(VAlign::Center);
        let mut tree = make_tree_with_widgets(vec![parent, child]);
        apply_constraints(&mut tree);
        let r = tree.widgets[1].rect.clone();
        assert!((r.x - 260.0).abs() < 0.01, "child x should centre in frame (260), got {}", r.x);
        assert!((r.y - 230.0).abs() < 0.01, "child y should centre in frame (230), got {}", r.y);
    }

    #[test]
    fn equal_width_constraint_links_sizes() {
        let source = widget_at(0.0, 0.0, 150.0, 40.0);
        let source_id = source.id.to_string();
        let mut follower = widget_at(50.0, 50.0, 80.0, 40.0);
        follower.constraints.equal_width_to = Some(source_id);
        let mut tree = make_tree_with_widgets(vec![source, follower]);
        apply_constraints(&mut tree);
        let fw = tree.widgets[1].rect.w;
        assert!((fw - 150.0).abs() < 0.01, "follower width should equal source (150), got {fw}");
    }

    #[test]
    fn equal_height_constraint_links_sizes() {
        let source = widget_at(0.0, 0.0, 80.0, 60.0);
        let source_id = source.id.to_string();
        let mut follower = widget_at(0.0, 0.0, 80.0, 30.0);
        follower.constraints.equal_height_to = Some(source_id);
        let mut tree = make_tree_with_widgets(vec![source, follower]);
        apply_constraints(&mut tree);
        let fh = tree.widgets[1].rect.h;
        assert!((fh - 60.0).abs() < 0.01, "follower height should equal source (60), got {fh}");
    }

    #[test]
    fn min_w_clamps_small_width() {
        let mut w = widget_at(0.0, 0.0, 10.0, 40.0);
        w.constraints.min_w = Some(50.0);
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        assert!(tree.widgets[0].rect.w >= 50.0, "min_w should clamp width to 50");
    }

    #[test]
    fn max_w_clamps_large_width() {
        let mut w = widget_at(0.0, 0.0, 300.0, 40.0);
        w.constraints.max_w = Some(100.0);
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        assert!(tree.widgets[0].rect.w <= 100.0, "max_w should clamp width to 100");
    }

    #[test]
    fn validate_detects_unknown_target() {
        let mut w = widget_at(0.0, 0.0, 100.0, 40.0);
        w.constraints.equal_width_to = Some("nonexistent-id".to_owned());
        let tree = make_tree_with_widgets(vec![w]);
        let errors = validate_constraints(&tree);
        assert!(
            errors.iter().any(|e| matches!(e, ConstraintError::UnknownTarget { .. })),
            "should detect unknown target, errors={errors:?}"
        );
    }

    #[test]
    fn validate_detects_self_reference() {
        let mut w = widget_at(0.0, 0.0, 100.0, 40.0);
        let self_id = w.id.to_string();
        w.constraints.equal_width_to = Some(self_id);
        let tree = make_tree_with_widgets(vec![w]);
        let errors = validate_constraints(&tree);
        assert!(
            errors.iter().any(|e| matches!(e, ConstraintError::SelfReference { .. })),
            "should detect self-reference, errors={errors:?}"
        );
    }

    #[test]
    fn validate_detects_invalid_aspect_ratio() {
        let mut w = widget_at(0.0, 0.0, 100.0, 40.0);
        w.constraints.aspect_ratio = Some(-1.0);
        let tree = make_tree_with_widgets(vec![w]);
        let errors = validate_constraints(&tree);
        assert!(
            errors.iter().any(|e| matches!(e, ConstraintError::InvalidAspectRatio { .. })),
            "should detect invalid aspect ratio, errors={errors:?}"
        );
    }

    #[test]
    fn validate_clean_tree_has_no_errors() {
        let w1 = widget_at(0.0, 0.0, 100.0, 40.0);
        let w2 = widget_at(200.0, 0.0, 80.0, 40.0);
        let tree = make_tree_with_widgets(vec![w1, w2]);
        let errors = validate_constraints(&tree);
        assert!(errors.is_empty(), "clean tree should have no errors, got {errors:?}");
    }

    #[test]
    fn validate_detects_equal_size_cycle() {
        let mut a = widget_at(0.0, 0.0, 100.0, 40.0);
        let mut b = widget_at(200.0, 0.0, 80.0, 40.0);
        a.constraints.equal_width_to = Some(b.id.to_string());
        b.constraints.equal_width_to = Some(a.id.to_string());
        let tree = make_tree_with_widgets(vec![a, b]);
        let errors = validate_constraints(&tree);
        assert!(
            errors.iter().any(|e| matches!(e, ConstraintError::EqualSizeCycle { .. })),
            "should detect equal-size cycle, errors={errors:?}"
        );
    }

    #[test]
    fn h_align_leading_sets_x_to_frame_origin() {
        let mut w = widget_at(200.0, 100.0, 80.0, 40.0);
        w.constraints.h_align = Some(HAlign::Leading);
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        assert!(
            tree.widgets[0].rect.x.abs() < 0.01,
            "HAlign::Leading should set x=0, got {}",
            tree.widgets[0].rect.x
        );
    }

    #[test]
    fn v_align_bottom_positions_widget() {
        let mut w = widget_at(0.0, 0.0, 80.0, 40.0);
        w.constraints.v_align = Some(VAlign::Bottom);
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        let expected_y = tree.app_props.win_h - 40.0; // 600 - 40 = 560
        assert!(
            (tree.widgets[0].rect.y - expected_y).abs() < 0.01,
            "VAlign::Bottom y should be {expected_y}, got {}",
            tree.widgets[0].rect.y
        );
    }
}
