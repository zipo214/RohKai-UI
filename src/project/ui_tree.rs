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

    /// Attach a widget to the topmost layout container whose rect contains `canvas_point`.
    ///
    /// If no layout contains the point, the widget is detached from any layout
    /// parent. The widget itself remains in `widgets`; `children` is an ownership
    /// relation used by canvas/codegen/export, not nested storage.
    pub fn attach_to_layout_at(
        &mut self,
        child_id: Uuid,
        canvas_point: (f32, f32),
    ) -> Option<Uuid> {
        let parent_id = self
            .widgets
            .iter()
            .rev()
            .find(|w| {
                w.id != child_id
                    && is_layout_container(&w.kind)
                    && rect_contains_point(&w.rect, canvas_point)
            })
            .map(|w| w.id);

        for w in &mut self.widgets {
            if is_layout_container(&w.kind) {
                w.children.retain(|&id| id != child_id);
            }
        }

        if let Some(pid) = parent_id {
            if let Some(parent) = self.get_mut(pid) {
                parent.children.push(child_id);
            }
        }

        self.reflow_layouts();
        parent_id
    }

    /// Reflow direct children inside each layout container using absolute canvas rects.
    ///
    /// This intentionally keeps child `Rect`s absolute so existing hit testing,
    /// selection, save/load, and child codegen can share one coordinate model.
    pub fn reflow_layouts(&mut self) {
        let parents: Vec<(WidgetKind, Rect, f32, f32, usize, Vec<Uuid>)> = self
            .widgets
            .iter()
            .filter(|w| is_layout_container(&w.kind))
            .map(|w| {
                (
                    w.kind.clone(),
                    w.rect.clone(),
                    w.props.inner_margin.max(0.0),
                    w.props.layout_spacing.max(0.0),
                    w.props.grid_columns.clamp(1, GRID_LAYOUT_MAX_COLUMNS),
                    w.children.clone(),
                )
            })
            .collect();

        for (kind, parent_rect, padding, spacing, grid_columns, child_ids) in parents {
            match kind {
                WidgetKind::VLayout => {
                    self.reflow_vlayout_children(&parent_rect, padding, spacing, &child_ids)
                }
                WidgetKind::HLayout => {
                    self.reflow_hlayout_children(&parent_rect, padding, spacing, &child_ids)
                }
                WidgetKind::GridLayout => self.reflow_gridlayout_children(
                    &parent_rect,
                    padding,
                    spacing,
                    grid_columns,
                    &child_ids,
                ),
                _ => {}
            }
        }
    }

    fn reflow_vlayout_children(
        &mut self,
        parent_rect: &Rect,
        padding: f32,
        spacing: f32,
        child_ids: &[Uuid],
    ) {
        let mut y = parent_rect.y + padding;
        let child_x = parent_rect.x + padding;
        let child_w = (parent_rect.w - padding * 2.0).max(MIN_WIDGET_SIZE);
        let max_bottom = parent_rect.y + parent_rect.h - padding;

        for child_id in child_ids {
            if let Some(child) = self.get_mut(*child_id) {
                child.rect.x = child_x;
                child.rect.y = y.min(max_bottom);
                child.rect.w = child_w;
                child.rect.h = child.rect.h.max(MIN_WIDGET_SIZE);
                y += child.rect.h + spacing;
            }
        }
    }

    fn reflow_hlayout_children(
        &mut self,
        parent_rect: &Rect,
        padding: f32,
        spacing: f32,
        child_ids: &[Uuid],
    ) {
        let count = child_ids.len().max(1) as f32;
        let total_spacing = spacing * (count - 1.0);
        let available_w = (parent_rect.w - padding * 2.0 - total_spacing).max(MIN_WIDGET_SIZE);
        let child_w = (available_w / count).max(MIN_WIDGET_SIZE);
        let child_y = parent_rect.y + padding;
        let child_h = (parent_rect.h - padding * 2.0).max(MIN_WIDGET_SIZE);
        let max_right = parent_rect.x + parent_rect.w - padding;
        let mut x = parent_rect.x + padding;

        for child_id in child_ids {
            if let Some(child) = self.get_mut(*child_id) {
                child.rect.x = x.min(max_right);
                child.rect.y = child_y;
                child.rect.w = child_w;
                child.rect.h = child_h;
                x += child_w + spacing;
            }
        }
    }

    fn reflow_gridlayout_children(
        &mut self,
        parent_rect: &Rect,
        padding: f32,
        spacing: f32,
        grid_columns: usize,
        child_ids: &[Uuid],
    ) {
        let columns = grid_columns
            .clamp(1, GRID_LAYOUT_MAX_COLUMNS)
            .min(child_ids.len().max(1));
        let rows = child_ids.len().div_ceil(columns).max(1);
        let columns_f = columns as f32;
        let rows_f = rows as f32;
        let total_x_spacing = spacing * (columns_f - 1.0);
        let total_y_spacing = spacing * (rows_f - 1.0);
        let available_w = (parent_rect.w - padding * 2.0 - total_x_spacing).max(MIN_WIDGET_SIZE);
        let available_h = (parent_rect.h - padding * 2.0 - total_y_spacing).max(MIN_WIDGET_SIZE);
        let child_w = (available_w / columns_f).max(MIN_WIDGET_SIZE);
        let child_h = (available_h / rows_f).max(MIN_WIDGET_SIZE);
        let start_x = parent_rect.x + padding;
        let start_y = parent_rect.y + padding;

        for (idx, child_id) in child_ids.iter().enumerate() {
            if let Some(child) = self.get_mut(*child_id) {
                let col = idx % columns;
                let row = idx / columns;
                child.rect.x = start_x + col as f32 * (child_w + spacing);
                child.rect.y = start_y + row as f32 * (child_h + spacing);
                child.rect.w = child_w;
                child.rect.h = child_h;
            }
        }
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

        self.reflow_layouts();
    }

    fn repair_widget(widget: &mut WidgetInstance) {
        widget.rect.x = widget.rect.x.max(0.0);
        widget.rect.y = widget.rect.y.max(0.0);
        widget.rect.w = widget.rect.w.max(MIN_WIDGET_SIZE);
        widget.rect.h = widget.rect.h.max(MIN_WIDGET_SIZE);

        if widget.props.min > widget.props.max {
            std::mem::swap(&mut widget.props.min, &mut widget.props.max);
        }

        if !widget.props.inner_margin.is_finite() {
            widget.props.inner_margin = 8.0;
        }
        widget.props.inner_margin = widget.props.inner_margin.clamp(0.0, 128.0);

        if !widget.props.layout_spacing.is_finite() {
            widget.props.layout_spacing = 6.0;
        }
        widget.props.layout_spacing = widget.props.layout_spacing.clamp(0.0, 128.0);
        widget.props.grid_columns = widget.props.grid_columns.clamp(1, GRID_LAYOUT_MAX_COLUMNS);

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

fn rect_contains_point(rect: &Rect, (x, y): (f32, f32)) -> bool {
    x >= rect.x && x <= rect.x + rect.w && y >= rect.y && y <= rect.y + rect.h
}

const GRID_LAYOUT_MAX_COLUMNS: usize = 12;

fn is_layout_container(kind: &WidgetKind) -> bool {
    matches!(
        kind,
        WidgetKind::VLayout | WidgetKind::HLayout | WidgetKind::GridLayout
    )
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

    #[test]
    fn attach_to_vlayout_reflows_child_inside_parent() {
        let parent_id = Uuid::from_u128(10);
        let child_id = Uuid::from_u128(11);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::VLayout,
                    rect: Rect {
                        x: 100.0,
                        y: 50.0,
                        w: 220.0,
                        h: 300.0,
                    },
                    ..Default::default()
                },
                WidgetInstance {
                    id: child_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 120.0,
                        y: 90.0,
                        w: 80.0,
                        h: 32.0,
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        assert_eq!(
            tree.attach_to_layout_at(child_id, (140.0, 100.0)),
            Some(parent_id)
        );

        let parent = tree.widgets.iter().find(|w| w.id == parent_id).unwrap();
        assert_eq!(parent.children, vec![child_id]);
        let child = tree.widgets.iter().find(|w| w.id == child_id).unwrap();
        assert_eq!(child.rect.x, 108.0);
        assert_eq!(child.rect.y, 58.0);
        assert_eq!(child.rect.w, 204.0);
    }

    #[test]
    fn attach_to_stack_layout_detaches_when_dropped_outside() {
        let parent_id = Uuid::from_u128(20);
        let child_id = Uuid::from_u128(21);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::VLayout,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 200.0,
                        h: 200.0,
                    },
                    children: vec![child_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: child_id,
                    kind: WidgetKind::Button,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        assert_eq!(tree.attach_to_layout_at(child_id, (300.0, 300.0)), None);

        let parent = tree.widgets.iter().find(|w| w.id == parent_id).unwrap();
        assert!(parent.children.is_empty());
    }

    #[test]
    fn attach_to_hlayout_reflows_children_horizontally() {
        let parent_id = Uuid::from_u128(30);
        let left_id = Uuid::from_u128(31);
        let right_id = Uuid::from_u128(32);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::HLayout,
                    rect: Rect {
                        x: 50.0,
                        y: 40.0,
                        w: 240.0,
                        h: 60.0,
                    },
                    ..Default::default()
                },
                WidgetInstance {
                    id: left_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 60.0,
                        y: 50.0,
                        w: 80.0,
                        h: 30.0,
                    },
                    ..Default::default()
                },
                WidgetInstance {
                    id: right_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 180.0,
                        y: 50.0,
                        w: 80.0,
                        h: 30.0,
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        assert_eq!(
            tree.attach_to_layout_at(left_id, (80.0, 60.0)),
            Some(parent_id)
        );
        assert_eq!(
            tree.attach_to_layout_at(right_id, (210.0, 60.0)),
            Some(parent_id)
        );

        let parent = tree.widgets.iter().find(|w| w.id == parent_id).unwrap();
        assert_eq!(parent.children, vec![left_id, right_id]);
        let left = tree.widgets.iter().find(|w| w.id == left_id).unwrap();
        let right = tree.widgets.iter().find(|w| w.id == right_id).unwrap();
        assert_eq!(left.rect.x, 58.0);
        assert_eq!(left.rect.y, 48.0);
        assert_eq!(left.rect.w, 109.0);
        assert_eq!(left.rect.h, 44.0);
        assert_eq!(right.rect.x, 173.0);
        assert_eq!(right.rect.y, 48.0);
        assert_eq!(right.rect.w, 109.0);
        assert_eq!(right.rect.h, 44.0);
    }

    #[test]
    fn attach_to_gridlayout_reflows_children_row_major() {
        let parent_id = Uuid::from_u128(40);
        let ids = [
            Uuid::from_u128(41),
            Uuid::from_u128(42),
            Uuid::from_u128(43),
            Uuid::from_u128(44),
        ];
        let mut widgets = vec![WidgetInstance {
            id: parent_id,
            kind: WidgetKind::GridLayout,
            rect: Rect {
                x: 50.0,
                y: 40.0,
                w: 328.0,
                h: 230.0,
            },
            props: WidgetProps {
                layout_spacing: 10.0,
                grid_columns: 2,
                ..Default::default()
            },
            ..Default::default()
        }];
        widgets.extend(ids.iter().map(|id| WidgetInstance {
            id: *id,
            kind: WidgetKind::Button,
            rect: Rect {
                x: 60.0,
                y: 50.0,
                w: 80.0,
                h: 30.0,
            },
            ..Default::default()
        }));
        let mut tree = UiTree {
            widgets,
            ..Default::default()
        };

        for id in ids {
            assert_eq!(tree.attach_to_layout_at(id, (80.0, 60.0)), Some(parent_id));
        }

        let parent = tree.widgets.iter().find(|w| w.id == parent_id).unwrap();
        assert_eq!(parent.children, ids);
        let expected = [(58.0, 48.0), (219.0, 48.0), (58.0, 160.0), (219.0, 160.0)];
        for (id, (x, y)) in ids.iter().zip(expected) {
            let child = tree.widgets.iter().find(|w| w.id == *id).unwrap();
            assert_eq!(child.rect.x, x);
            assert_eq!(child.rect.y, y);
            assert_eq!(child.rect.w, 151.0);
            assert_eq!(child.rect.h, 102.0);
        }
    }
}
