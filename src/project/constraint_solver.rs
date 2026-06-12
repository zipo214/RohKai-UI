//! P2.3 — Constraint-Based Layout solver.
//!
//! [`apply_constraints`] iterates all widgets in a [`UiTree`] and adjusts each
//! widget's `rect` (x, y, w, h) according to the [`LayoutConstraints`] stored on
//! it.  The solve is single-pass and intentionally simple: it handles margin
//! insets, equal-size links, and aspect-ratio locking.  Alignment anchors are
//! applied relative to the canvas root (the app window rect defined by
//! `UiTree::app_props.win_w` / `win_h`).
//!
//! [`validate_constraints`] detects conflicting or unsatisfiable constraints and
//! returns a list of [`ConstraintError`] values — one per problem found.

use crate::project::schema::{HAlign, LayoutConstraints, VAlign};
use crate::project::ui_tree::UiTree;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// A constraint problem detected by [`validate_constraints`].
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // used in tests and future validation UI
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

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Apply all [`LayoutConstraints`] stored in `tree`, adjusting widget rects
/// in-place.
///
/// Pass order:
/// 1. Equal-size links — copy w/h from target widget.
/// 2. Aspect-ratio lock — adjust h from w (preserves width, adjusts height).
/// 3. Min/max clamps on w and h.
/// 4. Margin insets — shift x/y and shrink w/h by the margin.
/// 5. Alignment anchors — reposition relative to the canvas root.
///
/// The solve is non-iterative; constraints that depend on other constrained
/// widgets are resolved in document order.  For production layouts that require
/// full constraint propagation, a multi-pass solve would be needed — that is
/// deferred to P2.3's follow-on stages.
pub fn apply_constraints(tree: &mut UiTree) {
    let canvas_w = tree.app_props.win_w;
    let canvas_h = tree.app_props.win_h;

    // Pass 1 — collect target rects for equal-size links (snapshot before mutation).
    // Build a map: widget_id → (w, h) before any changes.
    let size_snapshot: std::collections::HashMap<String, (f32, f32)> = tree
        .widgets
        .iter()
        .map(|w| (w.id.to_string(), (w.rect.w, w.rect.h)))
        .collect();

    let n = tree.widgets.len();
    for i in 0..n {
        let constraints = tree.widgets[i].constraints.clone();

        // Extract values before taking mutable borrows to avoid the borrow-checker
        // complaining about multiple simultaneous mutable borrows of `widgets[i]`.
        let mut rx = tree.widgets[i].rect.x;
        let mut ry = tree.widgets[i].rect.y;
        let mut rw = tree.widgets[i].rect.w;
        let mut rh = tree.widgets[i].rect.h;

        apply_equal_size(&mut rw, &mut rh, &constraints, &size_snapshot);
        apply_aspect_ratio(&mut rw, &mut rh, &constraints);
        apply_min_max_clamps(&mut rw, &mut rh, &constraints);
        apply_margin(&mut rx, &mut ry, &mut rw, &mut rh, &constraints);
        apply_alignment(&mut rx, &mut ry, rw, rh, &constraints, canvas_w, canvas_h);

        tree.widgets[i].rect.x = rx;
        tree.widgets[i].rect.y = ry;
        tree.widgets[i].rect.w = rw;
        tree.widgets[i].rect.h = rh;
    }
}

/// Validate all constraints in `tree` and return a list of detected problems.
/// An empty result means no problems were found.
#[allow(dead_code)] // used in tests; future validation UI will call this
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
// Private helpers
// ---------------------------------------------------------------------------

fn apply_equal_size(
    w: &mut f32,
    h: &mut f32,
    c: &LayoutConstraints,
    snapshot: &std::collections::HashMap<String, (f32, f32)>,
) {
    if let Some(ref target_id) = c.equal_width_to {
        if let Some(&(target_w, _)) = snapshot.get(target_id) {
            *w = target_w;
        }
    }
    if let Some(ref target_id) = c.equal_height_to {
        if let Some(&(_, target_h)) = snapshot.get(target_id) {
            *h = target_h;
        }
    }
}

fn apply_aspect_ratio(w: &mut f32, h: &mut f32, c: &LayoutConstraints) {
    if let Some(ratio) = c.aspect_ratio {
        if ratio.is_finite() && ratio > 0.0 {
            // Preserve width; derive height.
            *h = *w / ratio;
        }
    }
}

fn apply_min_max_clamps(w: &mut f32, h: &mut f32, c: &LayoutConstraints) {
    if let Some(min_w) = c.min_w {
        if *w < min_w {
            *w = min_w;
        }
    }
    if let Some(max_w) = c.max_w {
        if *w > max_w {
            *w = max_w;
        }
    }
    if let Some(min_h) = c.min_h {
        if *h < min_h {
            *h = min_h;
        }
    }
    if let Some(max_h) = c.max_h {
        if *h > max_h {
            *h = max_h;
        }
    }
}

/// Apply margin insets: shift x/y by the left/top margin, shrink w/h by total
/// horizontal/vertical margin.
fn apply_margin(x: &mut f32, y: &mut f32, w: &mut f32, h: &mut f32, c: &LayoutConstraints) {
    let [top, right, bottom, left] = c.margin;
    *x += left;
    *y += top;
    let dw = left + right;
    let dh = top + bottom;
    if *w > dw {
        *w -= dw;
    }
    if *h > dh {
        *h -= dh;
    }
}

fn apply_alignment(
    x: &mut f32,
    y: &mut f32,
    w: f32,
    h: f32,
    c: &LayoutConstraints,
    canvas_w: f32,
    canvas_h: f32,
) {
    if let Some(h_align) = c.h_align {
        match h_align {
            HAlign::Leading => *x = 0.0,
            HAlign::Trailing => *x = (canvas_w - w).max(0.0),
            HAlign::Center => *x = ((canvas_w - w) / 2.0).max(0.0),
            HAlign::Stretch => {
                *x = 0.0;
                // width is handled by the caller via Stretch logic; here we
                // only reposition.  Full Stretch width requires the caller to
                // set w = canvas_w, which is done as a separate pass if needed.
            }
        }
    }
    if let Some(v_align) = c.v_align {
        match v_align {
            VAlign::Top => *y = 0.0,
            VAlign::Bottom => *y = (canvas_h - h).max(0.0),
            VAlign::Center => *y = ((canvas_h - h) / 2.0).max(0.0),
            VAlign::Stretch => {
                *y = 0.0;
            }
        }
    }
}

#[allow(dead_code)]
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
            if let (Some(wa), Some(wb)) = (
                uuid_from_str_in_tree(tree, a),
                uuid_from_str_in_tree(tree, b),
            ) {
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
            if let (Some(wa), Some(wb)) = (
                uuid_from_str_in_tree(tree, a),
                uuid_from_str_in_tree(tree, b),
            ) {
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

#[allow(dead_code)]
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
    use crate::project::schema::WidgetInstance;
    use crate::project::ui_tree::UiTree;
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
        use crate::project::schema::Rect;
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
        // Width should be unchanged; height should equal width/ratio.
        assert!(
            (ww - 100.0).abs() < 0.01,
            "width should remain 100, got {ww}"
        );
        assert!(
            (wh - 50.0).abs() < 0.01,
            "height should be 50 (100/2.0), got {wh}"
        );
    }

    #[test]
    fn aspect_ratio_square() {
        let mut w = widget_at(0.0, 0.0, 80.0, 999.0);
        w.constraints.aspect_ratio = Some(1.0); // square
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        let ww = tree.widgets[0].rect.w;
        let wh = tree.widgets[0].rect.h;
        assert!(
            (ww - wh).abs() < 0.01,
            "square: w={ww} h={wh} should be equal"
        );
    }

    #[test]
    fn margin_insets_rect() {
        let mut w = widget_at(0.0, 0.0, 200.0, 100.0);
        // margin: [top=10, right=20, bottom=10, left=20]
        w.constraints.margin = [10.0, 20.0, 10.0, 20.0];
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        let r = &tree.widgets[0].rect;
        // x += left(20) = 20, y += top(10) = 10
        assert!((r.x - 20.0).abs() < 0.01, "x should be 20, got {}", r.x);
        assert!((r.y - 10.0).abs() < 0.01, "y should be 10, got {}", r.y);
        // w -= left+right = 200-40 = 160
        assert!((r.w - 160.0).abs() < 0.01, "w should be 160, got {}", r.w);
        // h -= top+bottom = 100-20 = 80
        assert!((r.h - 80.0).abs() < 0.01, "h should be 80, got {}", r.h);
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
        assert!(
            (fw - 150.0).abs() < 0.01,
            "follower width should equal source (150), got {fw}"
        );
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
        assert!(
            (fh - 60.0).abs() < 0.01,
            "follower height should equal source (60), got {fh}"
        );
    }

    #[test]
    fn min_w_clamps_small_width() {
        let mut w = widget_at(0.0, 0.0, 10.0, 40.0);
        w.constraints.min_w = Some(50.0);
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        let rw = tree.widgets[0].rect.w;
        assert!(rw >= 50.0, "min_w should clamp width to 50, got {rw}");
    }

    #[test]
    fn max_w_clamps_large_width() {
        let mut w = widget_at(0.0, 0.0, 300.0, 40.0);
        w.constraints.max_w = Some(100.0);
        let mut tree = make_tree_with_widgets(vec![w]);
        apply_constraints(&mut tree);
        let rw = tree.widgets[0].rect.w;
        assert!(rw <= 100.0, "max_w should clamp width to 100, got {rw}");
    }

    #[test]
    fn validate_detects_unknown_target() {
        let mut w = widget_at(0.0, 0.0, 100.0, 40.0);
        w.constraints.equal_width_to = Some("nonexistent-id".to_owned());
        let tree = make_tree_with_widgets(vec![w]);
        let errors = validate_constraints(&tree);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ConstraintError::UnknownTarget { .. })),
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
            errors
                .iter()
                .any(|e| matches!(e, ConstraintError::SelfReference { .. })),
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
            errors
                .iter()
                .any(|e| matches!(e, ConstraintError::InvalidAspectRatio { .. })),
            "should detect invalid aspect ratio, errors={errors:?}"
        );
    }

    #[test]
    fn validate_clean_tree_has_no_errors() {
        let w1 = widget_at(0.0, 0.0, 100.0, 40.0);
        let w2_id = widget_at(200.0, 0.0, 80.0, 40.0);
        let tree = make_tree_with_widgets(vec![w1, w2_id]);
        let errors = validate_constraints(&tree);
        assert!(
            errors.is_empty(),
            "clean tree should have no errors, got {errors:?}"
        );
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
            errors
                .iter()
                .any(|e| matches!(e, ConstraintError::EqualSizeCycle { .. })),
            "should detect equal-size cycle, errors={errors:?}"
        );
    }

    #[test]
    fn h_align_leading_sets_x_zero() {
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
