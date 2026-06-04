use crate::project::schema::{
    default_combo_options, AppProps, Rect, WidgetInstance, WidgetKind, WidgetProps,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

pub const MIN_WIDGET_SIZE: f32 = 20.0;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UiTree {
    pub widgets: Vec<WidgetInstance>,
    /// App-level properties: window title, size, icon. Serialized with project.
    #[serde(default)]
    pub app_props: AppProps,
}

impl UiTree {
    pub fn add(&mut self, mut widget: WidgetInstance) {
        Self::repair_widget(&mut widget);
        self.make_binding_unique(&mut widget);
        self.widgets.push(widget);
    }

    fn make_binding_unique(&self, widget: &mut WidgetInstance) {
        let Some(base) = widget.state_binding.clone() else {
            return;
        };

        if !self
            .widgets
            .iter()
            .any(|w| w.state_binding.as_deref() == Some(base.as_str()))
        {
            return;
        }

        let mut index = 2;
        loop {
            let candidate = format!("{base}_{index}");
            if !self
                .widgets
                .iter()
                .any(|w| w.state_binding.as_deref() == Some(candidate.as_str()))
            {
                widget.state_binding = Some(candidate);
                return;
            }
            index += 1;
        }
    }

    pub fn remove(&mut self, id: Uuid) {
        let children: Vec<Uuid> = self
            .widgets
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.children.clone())
            .unwrap_or_default();
        // Remove this id from any parent's children list
        for w in &mut self.widgets {
            w.children.retain(|&cid| cid != id);
        }
        self.widgets.retain(|w| w.id != id);
        // Cascade: delete children of a removed Frame
        for child_id in children {
            self.remove(child_id);
        }
    }

    #[allow(dead_code)]
    pub fn parent_of(&self, id: Uuid) -> Option<Uuid> {
        self.widgets
            .iter()
            .find(|w| w.children.contains(&id))
            .map(|w| w.id)
    }

    /// Group selected widget IDs into a new Frame. Returns the new Frame's id.
    pub fn group(&mut self, selected: &[Uuid]) -> Option<Uuid> {
        if selected.len() < 2 {
            return None;
        }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for &id in selected {
            if let Some(w) = self.widgets.iter().find(|w| w.id == id) {
                min_x = min_x.min(w.rect.x);
                min_y = min_y.min(w.rect.y);
                max_x = max_x.max(w.rect.x + w.rect.w);
                max_y = max_y.max(w.rect.y + w.rect.h);
            }
        }
        if min_x == f32::MAX {
            return None;
        }
        const PAD: f32 = 8.0;
        let frame_id = Uuid::new_v4();
        let frame = WidgetInstance {
            id: frame_id,
            kind: WidgetKind::Frame,
            rect: Rect {
                x: (min_x - PAD).max(0.0),
                y: (min_y - PAD).max(0.0),
                w: (max_x - min_x) + PAD * 2.0,
                h: (max_y - min_y) + PAD * 2.0,
            },
            props: WidgetProps {
                label: String::from("Group"),
                ..Default::default()
            },
            state_binding: None,
            children: selected.to_vec(),
            ..Default::default()
        };
        let earliest = selected
            .iter()
            .filter_map(|&id| self.widgets.iter().position(|w| w.id == id))
            .min()
            .unwrap_or(self.widgets.len());
        self.widgets.insert(earliest, frame);
        Some(frame_id)
    }

    /// Ungroup a Frame: remove the Frame, return child IDs (children remain top-level).
    pub fn ungroup(&mut self, frame_id: Uuid) -> Vec<Uuid> {
        if let Some(idx) = self.widgets.iter().position(|w| w.id == frame_id) {
            let children = self.widgets[idx].children.clone();
            self.widgets.remove(idx);
            return children;
        }
        Vec::new()
    }

    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut WidgetInstance> {
        self.widgets.iter_mut().find(|w| w.id == id)
    }

    /// Remove every canvas widget while preserving app-level project properties.
    pub fn clear_widgets(&mut self) {
        self.widgets.clear();
    }

    pub fn validate_and_repair(&mut self) {
        let mut seen_ids = HashSet::new();
        let mut seen_bindings = HashSet::new();

        for widget in &mut self.widgets {
            if !seen_ids.insert(widget.id) {
                widget.id = Uuid::new_v4();
                seen_ids.insert(widget.id);
            }

            Self::repair_widget(widget);
            Self::repair_binding(widget, &mut seen_bindings);
        }

        // Remove stale child references (child UUIDs that no longer exist)
        let all_ids: HashSet<Uuid> = self.widgets.iter().map(|w| w.id).collect();
        for widget in &mut self.widgets {
            widget.children.retain(|id| all_ids.contains(id));
        }
    }

    fn repair_widget(widget: &mut WidgetInstance) {
        widget.rect.x = widget.rect.x.max(0.0);
        widget.rect.y = widget.rect.y.max(0.0);
        widget.rect.w = widget.rect.w.max(MIN_WIDGET_SIZE);
        widget.rect.h = widget.rect.h.max(MIN_WIDGET_SIZE);

        if widget.props.min > widget.props.max {
            std::mem::swap(&mut widget.props.min, &mut widget.props.max);
        }

        if !widget.props.default_value.is_finite() {
            widget.props.default_value = 0.5;
        }
        widget.props.default_value = widget
            .props
            .default_value
            .clamp(widget.props.min, widget.props.max);

        if widget.kind == WidgetKind::ComboBox && widget.props.options.is_empty() {
            widget.props.options = default_combo_options();
        }
    }

    fn repair_binding(widget: &mut WidgetInstance, seen_bindings: &mut HashSet<String>) {
        let Some(binding) = widget.state_binding.as_deref().map(str::trim) else {
            return;
        };

        if binding.is_empty() {
            widget.state_binding = None;
            return;
        }

        let base = binding.to_owned();
        if seen_bindings.insert(base.clone()) {
            widget.state_binding = Some(base);
            return;
        }

        let mut index = 2;
        loop {
            let candidate = format!("{base}_{index}");
            if seen_bindings.insert(candidate.clone()) {
                widget.state_binding = Some(candidate);
                return;
            }
            index += 1;
        }
    }

    /// Move widget to last position in the vec — drawn last = visually on top.
    pub fn bring_to_front(&mut self, id: Uuid) {
        if let Some(idx) = self.widgets.iter().position(|w| w.id == id) {
            let w = self.widgets.remove(idx);
            self.widgets.push(w);
            debug_assert_eq!(
                self.widgets.last().map(|w| w.id),
                Some(id),
                "bring_to_front: widget not at last position"
            );
        }
    }

    /// Move widget to index 0 — drawn first = visually behind everything.
    pub fn send_to_back(&mut self, id: Uuid) {
        if let Some(idx) = self.widgets.iter().position(|w| w.id == id) {
            let w = self.widgets.remove(idx);
            self.widgets.insert(0, w);
            debug_assert_eq!(
                self.widgets.first().map(|w| w.id),
                Some(id),
                "send_to_back: widget not at index 0"
            );
        }
    }

    /// Move a widget to an explicit draw-order index.
    ///
    /// Higher indices are drawn later and therefore appear visually above lower indices.
    pub fn move_to_index(&mut self, id: Uuid, to_idx: usize) {
        if let Some(from_idx) = self.widgets.iter().position(|w| w.id == id) {
            let widget = self.widgets.remove(from_idx);
            let target = to_idx.min(self.widgets.len());
            self.widgets.insert(target, widget);
            debug_assert_eq!(
                self.widgets.get(target).map(|w| w.id),
                Some(id),
                "move_to_index: widget not at target index"
            );
            self.validate_and_repair();
        }
    }

    /// Swap with next index — higher index = drawn later = more on top.
    pub fn bring_forward(&mut self, id: Uuid) {
        if let Some(idx) = self.widgets.iter().position(|w| w.id == id) {
            if idx + 1 < self.widgets.len() {
                self.widgets.swap(idx, idx + 1);
                debug_assert_eq!(self.widgets[idx + 1].id, id, "bring_forward: swap failed");
            }
        }
    }

    /// Swap with previous index — lower index = drawn earlier = more behind.
    pub fn send_back(&mut self, id: Uuid) {
        if let Some(idx) = self.widgets.iter().position(|w| w.id == id) {
            if idx > 0 {
                self.widgets.swap(idx, idx - 1);
                debug_assert_eq!(self.widgets[idx - 1].id, id, "send_back: swap failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widget(id: Uuid) -> WidgetInstance {
        WidgetInstance {
            id,
            ..Default::default()
        }
    }

    #[test]
    fn move_to_index_reorders_draw_order() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let c = Uuid::from_u128(3);
        let d = Uuid::from_u128(4);
        let mut tree = UiTree {
            widgets: vec![widget(a), widget(b), widget(c), widget(d)],
            ..Default::default()
        };

        tree.move_to_index(b, 3);
        assert_eq!(
            tree.widgets.iter().map(|w| w.id).collect::<Vec<_>>(),
            vec![a, c, d, b]
        );

        tree.move_to_index(b, 1);
        assert_eq!(
            tree.widgets.iter().map(|w| w.id).collect::<Vec<_>>(),
            vec![a, b, c, d]
        );
    }

    #[test]
    fn move_to_index_unknown_id_is_noop() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let mut tree = UiTree {
            widgets: vec![widget(a), widget(b)],
            ..Default::default()
        };

        tree.move_to_index(Uuid::from_u128(99), 0);
        assert_eq!(
            tree.widgets.iter().map(|w| w.id).collect::<Vec<_>>(),
            vec![a, b]
        );
    }

    #[test]
    fn clear_widgets_preserves_app_props() {
        let mut tree = UiTree {
            widgets: vec![widget(Uuid::from_u128(1)), widget(Uuid::from_u128(2))],
            ..Default::default()
        };
        tree.app_props.title = "Keep Me".to_owned();

        tree.clear_widgets();

        assert!(tree.widgets.is_empty());
        assert_eq!(tree.app_props.title, "Keep Me");
    }
}
