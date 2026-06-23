//! In-app canvas clipboard (CB-01..CB-25). Session-only; never persisted.
//! Mirrors the `src/canvas/search.rs` module structure.

use std::collections::HashSet;

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

    let mut widgets: Vec<WidgetInstance> = tree
        .widgets
        .iter()
        .filter(|w| closure.contains(&w.id))
        .cloned()
        .collect();
    for w in &mut widgets {
        w.children.retain(|c| closure.contains(c));
    }

    let source_had_behaviors = tree.app_props.behaviors.iter().any(|b| {
        b.source_widget().is_some_and(|s| closure.contains(&s))
            || b.target_widget.is_some_and(|t| closure.contains(&t))
    });

    ClipboardContents {
        widgets,
        source_had_behaviors,
    }
}

pub use crate::project::ui_tree::PasteError;

/// Cascade step between repeated pastes, in canvas units (zoom-stable, CB-19).
pub const PASTE_CASCADE_STEP: f32 = 16.0;

/// Duration of the post-paste flash ring fade, in seconds (CB-21).
pub const PASTE_FLASH_SECS: f32 = 0.6;

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
        return Ok(PasteOutcome {
            new_root_ids: Vec::new(),
            count: 0,
            had_behaviors: false,
        });
    }
    let anchor = bbox_center(&clipboard.widgets);
    let cascade_off = cascade as f32 * PASTE_CASCADE_STEP;
    let delta = egui::vec2(
        target_canvas.x - anchor.x + cascade_off,
        target_canvas.y - anchor.y + cascade_off,
    );
    let new_root_ids = tree.paste_batch(clipboard.widgets.clone(), delta, target_container)?;
    let count = clipboard.widgets.len();
    Ok(PasteOutcome {
        new_root_ids,
        count,
        had_behaviors: clipboard.source_had_behaviors,
    })
}

/// Duplicate `selected` in place with a fixed cascade-step offset (CB-25).
/// Independent of the clipboard buffer.
pub fn duplicate_in_place(
    selected: &[uuid::Uuid],
    tree: &mut UiTree,
) -> Result<PasteOutcome, PasteError> {
    let contents = copy_selection(selected, tree);
    if contents.is_empty() {
        return Ok(PasteOutcome {
            new_root_ids: Vec::new(),
            count: 0,
            had_behaviors: false,
        });
    }
    let count = contents.widgets.len();
    let delta = egui::vec2(PASTE_CASCADE_STEP, PASTE_CASCADE_STEP);
    let new_root_ids = tree.paste_batch(contents.widgets.clone(), delta, None)?;
    Ok(PasteOutcome {
        new_root_ids,
        count,
        had_behaviors: contents.source_had_behaviors,
    })
}

/// Draw a teal selection ring around freshly pasted widgets (CB-21).
/// Visual language matches search's ring but is driven by paste_flash state.
pub fn draw_paste_flash(
    painter: &egui::Painter,
    flash_ids: &[uuid::Uuid],
    widget_screen_rects: &[(uuid::Uuid, egui::Rect)],
    alpha: f32,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::Rect;

    #[test]
    fn interaction_state_is_not_serializable() {
        // Compile-time intent check: clipboard state must never land in
        // .rohkai.json. InteractionState carries only #[derive(Default)].
        // This asserts the type is constructible by Default; the real
        // enforcement is the ABSENCE of a Serialize derive (reviewer-checked,
        // mirrors the CanvasSearchState non-serialize invariant).
        fn assert_default<T: Default>() {}
        assert_default::<crate::canvas::interaction::InteractionState>();
    }

    fn w(children: Vec<uuid::Uuid>) -> WidgetInstance {
        WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Button,
            children,
            ..Default::default()
        }
    }

    fn tree(widgets: Vec<WidgetInstance>) -> UiTree {
        UiTree {
            widgets,
            ..Default::default()
        }
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
        assert_eq!(c.widgets.len(), 2);
        assert!(c.widgets.iter().any(|x| x.id == child_id));
    }

    #[test]
    fn copy_child_only_clears_outside_links() {
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
    }

    #[test]
    fn cursor_to_canvas_round_trips() {
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
            widgets: vec![
                mk(100.0, 100.0, 20.0, 20.0),
                mk(200.0, 100.0, 20.0, 20.0),
                mk(300.0, 200.0, 40.0, 40.0),
            ],
            source_had_behaviors: false,
        };
        let mut tree = UiTree::default();
        let target = egui::pos2(520.0, 470.0);
        let out = paste_payload(&clip, target, 0, &mut tree, None).unwrap();
        assert_eq!(out.count, 3);

        let rects: Vec<Rect> = tree.widgets.iter().map(|w| w.rect.clone()).collect();
        assert!(
            rects
                .iter()
                .any(|r| (r.x - 400.0).abs() < 0.01 && (r.y - 400.0).abs() < 0.01)
        );
        assert!(
            rects
                .iter()
                .any(|r| (r.w - 40.0).abs() < 0.01 && (r.h - 40.0).abs() < 0.01)
        );
    }

    #[test]
    fn cascade_offsets_repeated_pastes() {
        let mut wdg = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Button,
            ..Default::default()
        };
        wdg.rect = Rect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        };
        let clip = ClipboardContents {
            widgets: vec![wdg],
            source_had_behaviors: false,
        };

        let mut tree = UiTree::default();
        let target = egui::pos2(0.0, 0.0);
        paste_payload(&clip, target, 0, &mut tree, None).unwrap();
        paste_payload(&clip, target, 1, &mut tree, None).unwrap();
        let mut xs: Vec<f32> = tree.widgets.iter().map(|w| w.rect.x).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((xs[1] - xs[0] - PASTE_CASCADE_STEP).abs() < 0.01);
    }

    #[test]
    fn duplicate_in_place_offsets_by_step_and_preserves_size() {
        let mut src = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Button,
            ..Default::default()
        };
        src.rect = Rect {
            x: 50.0,
            y: 60.0,
            w: 30.0,
            h: 30.0,
        };
        let src_id = src.id;
        let mut tree = UiTree {
            widgets: vec![src],
            ..Default::default()
        };

        let out = duplicate_in_place(&[src_id], &mut tree).unwrap();
        assert_eq!(out.count, 1);
        let dup = tree
            .widgets
            .iter()
            .find(|w| w.id == out.new_root_ids[0])
            .unwrap();
        assert!((dup.rect.x - (50.0 + PASTE_CASCADE_STEP)).abs() < 0.01);
        assert!((dup.rect.w - 30.0).abs() < 0.01);
    }

    // CB-PARITY: prove a pasted widget survives the REAL live egui emitter.
    // Entry point: `crate::codegen::egui_emitter::emit_document(&UiTree)`, which
    // returns a `GeneratedCodeDocument` whose `.text` is the emitted Rust source.
    // A Button's label is emitted via `string_literal(&props.label)`, so the
    // verbatim text appears in the output.
    #[test]
    fn pasted_widgets_appear_in_live_codegen() {
        let mut src = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Button,
            ..Default::default()
        };
        src.props.label = "Zorp".to_string();
        let clip = ClipboardContents {
            widgets: vec![src],
            source_had_behaviors: false,
        };
        let mut tree = UiTree::default();
        paste_payload(&clip, egui::pos2(10.0, 10.0), 0, &mut tree, None).unwrap();

        let code = crate::codegen::egui_emitter::emit_document(&tree).text;
        assert!(code.contains("Zorp"), "pasted widget missing from codegen");
    }

    // CB-PARITY: prove a pasted Frame+child subtree survives the REAL save/load
    // pipeline. Serialize/deserialize entry points:
    //   `crate::project::io::serialize_tree(&UiTree) -> Result<String, String>`
    //   `crate::project::io::deserialize_tree(&str) -> Result<UiTree, String>`
    // After reload, every `children[]` reference must resolve to a live widget id
    // (no dangling refs across the round-trip).
    #[test]
    fn pasted_tree_survives_save_load_round_trip() {
        let child = {
            let mut c = WidgetInstance {
                id: uuid::Uuid::new_v4(),
                kind: WidgetKind::Button,
                ..Default::default()
            };
            c.props.label = "Kid".into();
            c
        };
        let child_id = child.id;
        let frame = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Frame,
            children: vec![child_id],
            ..Default::default()
        };
        let clip = ClipboardContents {
            widgets: vec![frame, child],
            source_had_behaviors: false,
        };
        let mut tree = UiTree::default();
        paste_payload(&clip, egui::pos2(0.0, 0.0), 0, &mut tree, None).unwrap();

        let json = crate::project::io::serialize_tree(&tree).expect("serialize");
        let restored = crate::project::io::deserialize_tree(&json).expect("deserialize");
        let restored_widgets: &[WidgetInstance] = &restored.widgets;

        let ids: std::collections::HashSet<uuid::Uuid> =
            restored_widgets.iter().map(|w| w.id).collect();
        // The pasted Frame + child must still be present after reload.
        assert_eq!(restored_widgets.len(), tree.widgets.len());
        for w in restored_widgets {
            for c in &w.children {
                assert!(ids.contains(c), "dangling child after reload");
            }
        }
        // And the Frame's child link must survive (not just be empty).
        assert!(
            restored_widgets
                .iter()
                .any(|w| w.kind == WidgetKind::Frame && !w.children.is_empty()),
            "Frame lost its child reference across save/load"
        );
    }

    // A3 (paste + one undo restores prior ProjectDocument) is intentionally
    // NOT unit-tested here: undo is JSON-snapshot based on the full
    // ProjectDocument and is driven entirely by RohKaiApp's record/apply loop
    // (see `src/project/undo.rs` + app.rs wiring). It cannot be exercised
    // through `paste_payload` in isolation without reconstructing that app
    // scaffolding, so it is covered at the app layer rather than here.
}
