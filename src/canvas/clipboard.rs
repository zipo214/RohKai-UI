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
        b.source_widget()
            .map(|s| closure.contains(&s))
            .unwrap_or(false)
            || b.target_widget.map(|t| closure.contains(&t)).unwrap_or(false)
    });

    ClipboardContents {
        widgets,
        source_had_behaviors,
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
        let _ = Rect::default();
    }
}
