use crate::project::schema::{
    AppProps, Rect, WidgetInstance, WidgetKind, WidgetProps, default_combo_options,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub const MIN_WIDGET_SIZE: f32 = 20.0;

/// Error from an attempted atomic paste (CB-04). No tree mutation occurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteError {
    /// The staged graph would create a cycle, a self-child, or a
    /// widget owned by two parents.
    InvalidGraph,
}

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
        // Cascade: delete children of a removed container/layout.
        for child_id in children {
            self.remove(child_id);
        }
        self.prune_stale_behaviors();
    }

    /// Drop behaviors whose source widget no longer exists, and clear dangling
    /// `target_widget` presentation refs. The action's field stays valid even
    /// when the target widget is gone, so only the wire endpoint is cleared.
    fn prune_stale_behaviors(&mut self) {
        let live: HashSet<Uuid> = self.widgets.iter().map(|w| w.id).collect();
        self.app_props.behaviors.retain(|b| {
            b.source_widget()
                .is_none_or(|source| live.contains(&source))
        });
        for b in &mut self.app_props.behaviors {
            if b.target_widget.is_some_and(|t| !live.contains(&t)) {
                b.target_widget = None;
            }
        }
    }

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
        let selected_set: HashSet<Uuid> = selected.iter().copied().collect();
        let selected_order = self.owned_order_or_selection_order(selected);
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for &id in &selected_order {
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
        let common_parent = self.common_parent_for(&selected_order);
        let parent_insert_idx = common_parent.and_then(|pid| {
            self.widgets
                .iter()
                .find(|w| w.id == pid)
                .and_then(|w| w.children.iter().position(|id| selected_set.contains(id)))
        });
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
            children: selected_order.clone(),
            ..Default::default()
        };
        for w in &mut self.widgets {
            w.children.retain(|id| !selected_set.contains(id));
        }
        let earliest = selected_order
            .iter()
            .filter_map(|&id| self.widgets.iter().position(|w| w.id == id))
            .min()
            .unwrap_or(self.widgets.len());
        self.widgets.insert(earliest, frame);
        if let Some(pid) = common_parent
            && let Some(parent) = self.get_mut(pid)
        {
            let insert_idx = parent_insert_idx.unwrap_or(parent.children.len());
            parent
                .children
                .insert(insert_idx.min(parent.children.len()), frame_id);
        }
        self.reflow_layouts();
        Some(frame_id)
    }

    /// Ungroup a Frame: remove the Frame, return child IDs.
    ///
    /// If the Frame itself is owned by a layout/container, its children replace
    /// it in that parent's child list. Otherwise children remain top-level.
    pub fn ungroup(&mut self, frame_id: Uuid) -> Vec<Uuid> {
        let parent_id = self.parent_of(frame_id);
        let parent_insert_idx = parent_id.and_then(|pid| {
            self.widgets
                .iter()
                .find(|w| w.id == pid)
                .and_then(|w| w.children.iter().position(|id| *id == frame_id))
        });
        if let Some(idx) = self.widgets.iter().position(|w| w.id == frame_id) {
            let children = self.widgets[idx].children.clone();
            self.widgets.remove(idx);
            if let Some(pid) = parent_id
                && let Some(parent) = self.get_mut(pid)
            {
                parent.children.retain(|id| *id != frame_id);
                let insert_idx = parent_insert_idx.unwrap_or(parent.children.len());
                for (offset, child_id) in children.iter().enumerate() {
                    parent
                        .children
                        .insert((insert_idx + offset).min(parent.children.len()), *child_id);
                }
            }
            self.reflow_layouts();
            return children;
        }
        Vec::new()
    }

    fn common_parent_for(&self, selected: &[Uuid]) -> Option<Uuid> {
        let mut parents = selected.iter().map(|id| self.parent_of(*id));
        let first = parents.next().flatten()?;
        parents.all(|p| p == Some(first)).then_some(first)
    }

    fn owned_order_or_selection_order(&self, selected: &[Uuid]) -> Vec<Uuid> {
        let selected_set: HashSet<Uuid> = selected.iter().copied().collect();
        if let Some(parent_id) = self.common_parent_for(selected)
            && let Some(parent) = self.widgets.iter().find(|w| w.id == parent_id)
        {
            let ordered: Vec<Uuid> = parent
                .children
                .iter()
                .copied()
                .filter(|id| selected_set.contains(id))
                .collect();
            if ordered.len() == selected.len() {
                return ordered;
            }
        }
        selected.to_vec()
    }

    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut WidgetInstance> {
        self.widgets.iter_mut().find(|w| w.id == id)
    }

    pub fn move_child_within_parent(
        &mut self,
        parent_id: Uuid,
        child_id: Uuid,
        to_idx: usize,
    ) -> bool {
        let Some(parent) = self.get_mut(parent_id) else {
            return false;
        };
        let Some(from_idx) = parent.children.iter().position(|id| *id == child_id) else {
            return false;
        };
        let child = parent.children.remove(from_idx);
        let target = to_idx.min(parent.children.len());
        parent.children.insert(target, child);
        self.reflow_layouts();
        true
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
                    && !self.is_descendant_of(w.id, child_id)
            })
            .map(|w| w.id);

        for w in &mut self.widgets {
            w.children.retain(|&id| id != child_id);
        }

        if let Some(pid) = parent_id
            && let Some(parent) = self.get_mut(pid)
            && !parent.children.contains(&child_id)
        {
            parent.children.push(child_id);
        }

        self.reflow_layouts();
        parent_id
    }

    fn is_descendant_of(&self, possible_descendant: Uuid, ancestor: Uuid) -> bool {
        let mut stack = self
            .widgets
            .iter()
            .find(|w| w.id == ancestor)
            .map(|w| w.children.clone())
            .unwrap_or_default();
        let mut seen = HashSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            if id == possible_descendant {
                return true;
            }
            if let Some(widget) = self.widgets.iter().find(|w| w.id == id) {
                stack.extend(widget.children.iter().copied());
            }
        }
        false
    }

    /// Reflow direct children inside each layout container using absolute canvas rects.
    ///
    /// This intentionally keeps child `Rect`s absolute so existing hit testing,
    /// selection, save/load, and child codegen can share one coordinate model.
    pub fn reflow_layouts(&mut self) {
        let mut parent_ids: Vec<(usize, Uuid)> = self
            .widgets
            .iter()
            .filter(|w| is_layout_container(&w.kind))
            .map(|widget| {
                let mut depth = 0usize;
                let mut current = widget.id;
                while let Some(parent) = self.parent_of(current) {
                    depth += 1;
                    current = parent;
                    if depth > 64 {
                        break;
                    }
                }
                (depth, widget.id)
            })
            .collect();
        parent_ids.sort_by_key(|(depth, _)| *depth);

        for (_, parent_id) in parent_ids {
            let Some((kind, parent_rect, padding, spacing, stretch, grid_columns, child_ids)) =
                self.widgets
                    .iter()
                    .find(|widget| widget.id == parent_id)
                    .map(|widget| {
                        (
                            widget.kind.clone(),
                            widget.rect.clone(),
                            widget.props.inner_margin.max(0.0),
                            widget.props.layout_spacing.max(0.0),
                            widget.props.layout_stretch,
                            widget.props.grid_columns.clamp(1, GRID_LAYOUT_MAX_COLUMNS),
                            widget.children.clone(),
                        )
                    })
            else {
                continue;
            };
            match kind {
                WidgetKind::VLayout => self.reflow_vlayout_children(
                    &parent_rect,
                    padding,
                    spacing,
                    stretch,
                    &child_ids,
                ),
                WidgetKind::HLayout => self.reflow_hlayout_children(
                    &parent_rect,
                    padding,
                    spacing,
                    stretch,
                    &child_ids,
                ),
                WidgetKind::GridLayout => self.reflow_gridlayout_children(
                    &parent_rect,
                    padding,
                    spacing,
                    stretch,
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
        stretch: bool,
        child_ids: &[Uuid],
    ) {
        let flex_count = child_ids
            .iter()
            .filter(|id| {
                self.widgets
                    .iter()
                    .any(|w| w.id == **id && w.kind == WidgetKind::VerticalSpacer)
            })
            .count();
        let fixed_h: f32 = child_ids
            .iter()
            .filter_map(|id| self.widgets.iter().find(|w| w.id == *id))
            .filter(|w| w.kind != WidgetKind::VerticalSpacer)
            .map(|w| w.rect.h.max(MIN_WIDGET_SIZE))
            .sum();
        let total_spacing = spacing * child_ids.len().saturating_sub(1) as f32;
        let available_h = (parent_rect.h - padding * 2.0 - total_spacing).max(MIN_WIDGET_SIZE);
        let flex_h = if flex_count > 0 {
            ((available_h - fixed_h) / flex_count as f32).max(MIN_WIDGET_SIZE)
        } else {
            MIN_WIDGET_SIZE
        };
        let mut y = parent_rect.y + padding;
        let child_x = parent_rect.x + padding;
        let stretch_w = (parent_rect.w - padding * 2.0).max(MIN_WIDGET_SIZE);

        for child_id in child_ids {
            if let Some(child) = self.get_mut(*child_id) {
                child.rect.x = child_x;
                child.rect.y = y;
                child.rect.w = if stretch {
                    stretch_w
                } else {
                    child.rect.w.max(MIN_WIDGET_SIZE).min(stretch_w)
                };
                child.rect.h = if child.kind == WidgetKind::VerticalSpacer {
                    flex_h
                } else {
                    child.rect.h.max(MIN_WIDGET_SIZE)
                };
                y += child.rect.h + spacing;
            }
        }
    }

    fn reflow_hlayout_children(
        &mut self,
        parent_rect: &Rect,
        padding: f32,
        spacing: f32,
        stretch: bool,
        child_ids: &[Uuid],
    ) {
        let flex_count = child_ids
            .iter()
            .filter(|id| {
                self.widgets
                    .iter()
                    .any(|w| w.id == **id && w.kind == WidgetKind::HorizontalSpacer)
            })
            .count();
        let fixed_w: f32 = child_ids
            .iter()
            .filter_map(|id| self.widgets.iter().find(|w| w.id == *id))
            .filter(|w| w.kind != WidgetKind::HorizontalSpacer)
            .map(|w| w.rect.w.max(MIN_WIDGET_SIZE))
            .sum();
        let total_spacing = spacing * child_ids.len().saturating_sub(1) as f32;
        let available_w = (parent_rect.w - padding * 2.0 - total_spacing).max(MIN_WIDGET_SIZE);
        let flex_w = if flex_count > 0 {
            ((available_w - fixed_w) / flex_count as f32).max(MIN_WIDGET_SIZE)
        } else {
            MIN_WIDGET_SIZE
        };
        let child_y = parent_rect.y + padding;
        let stretch_h = (parent_rect.h - padding * 2.0).max(MIN_WIDGET_SIZE);
        let mut x = parent_rect.x + padding;

        for child_id in child_ids {
            if let Some(child) = self.get_mut(*child_id) {
                child.rect.x = x;
                child.rect.y = child_y;
                child.rect.w = if child.kind == WidgetKind::HorizontalSpacer {
                    flex_w
                } else {
                    child.rect.w.max(MIN_WIDGET_SIZE)
                };
                child.rect.h = if stretch {
                    stretch_h
                } else {
                    child.rect.h.max(MIN_WIDGET_SIZE).min(stretch_h)
                };
                x += child.rect.w + spacing;
            }
        }
    }

    fn reflow_gridlayout_children(
        &mut self,
        parent_rect: &Rect,
        padding: f32,
        spacing: f32,
        stretch: bool,
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
                child.rect.w = if stretch {
                    child_w
                } else {
                    child.rect.w.max(MIN_WIDGET_SIZE).min(child_w)
                };
                child.rect.h = if stretch {
                    child_h
                } else {
                    child.rect.h.max(MIN_WIDGET_SIZE).min(child_h)
                };
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

        // Child ownership is tree-shaped: no stale references, no duplicates,
        // no self-edges/cycles, and no child owned by more than one parent.
        let all_ids: HashSet<Uuid> = self.widgets.iter().map(|w| w.id).collect();
        let mut claimed_children = HashSet::new();
        let mut retained_children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for widget in &mut self.widgets {
            let parent_id = widget.id;
            let mut clean = Vec::with_capacity(widget.children.len());
            for child_id in widget.children.iter().copied() {
                if !all_ids.contains(&child_id) || child_id == parent_id {
                    continue;
                }
                if claimed_children.contains(&child_id) {
                    continue;
                }
                if would_create_child_cycle(parent_id, child_id, &retained_children) {
                    continue;
                }
                claimed_children.insert(child_id);
                clean.push(child_id);
            }
            widget.children = clean.clone();
            retained_children.insert(parent_id, clean);
        }

        self.prune_stale_behaviors();
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
        if let Some(idx) = self.widgets.iter().position(|w| w.id == id)
            && idx + 1 < self.widgets.len()
        {
            self.widgets.swap(idx, idx + 1);
            debug_assert_eq!(self.widgets[idx + 1].id, id, "bring_forward: swap failed");
        }
    }

    /// Swap with previous index — lower index = drawn earlier = more behind.
    pub fn send_back(&mut self, id: Uuid) {
        if let Some(idx) = self.widgets.iter().position(|w| w.id == id)
            && idx > 0
        {
            self.widgets.swap(idx, idx - 1);
            debug_assert_eq!(self.widgets[idx - 1].id, id, "send_back: swap failed");
        }
    }

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
        target_container: Option<Uuid>,
    ) -> Result<Vec<Uuid>, PasteError> {
        let id_map: HashMap<Uuid, Uuid> =
            staged.iter().map(|w| (w.id, Uuid::new_v4())).collect();

        let child_ids: HashSet<Uuid> =
            staged.iter().flat_map(|w| w.children.iter().copied()).collect();
        let old_roots: Vec<Uuid> =
            staged.iter().map(|w| w.id).filter(|id| !child_ids.contains(id)).collect();

        let staged_set: HashSet<Uuid> = staged.iter().map(|w| w.id).collect();
        let mut remapped: Vec<WidgetInstance> = Vec::with_capacity(staged.len());
        for w in &staged {
            let mut c = w.clone();
            c.id = id_map[&w.id];
            c.children = w
                .children
                .iter()
                .filter(|cid| staged_set.contains(cid))
                .map(|cid| id_map[cid])
                .collect();
            c.rect.x += anchor_delta.x;
            c.rect.y += anchor_delta.y;
            remapped.push(c);
        }

        Self::validate_staged(&remapped)?;

        let new_roots: Vec<Uuid> = old_roots.iter().map(|old| id_map[old]).collect();
        for w in remapped {
            self.widgets.push(w);
        }

        if let Some(container) = target_container {
            for root in &new_roots {
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
        let mut indeg: HashMap<Uuid, usize> = staged.iter().map(|w| (w.id, 0)).collect();
        for w in staged {
            for c in &w.children {
                if *c == w.id {
                    return Err(PasteError::InvalidGraph);
                }
                if let Some(n) = indeg.get_mut(c) {
                    *n += 1;
                }
            }
        }
        if indeg.values().any(|n| *n > 1) {
            return Err(PasteError::InvalidGraph);
        }

        let child_map: HashMap<Uuid, &Vec<Uuid>> =
            staged.iter().map(|w| (w.id, &w.children)).collect();
        let mut indeg2 = indeg.clone();
        let mut stack: Vec<Uuid> =
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
            return Err(PasteError::InvalidGraph);
        }
        Ok(())
    }
}

fn would_create_child_cycle(
    parent_id: Uuid,
    child_id: Uuid,
    retained_children: &HashMap<Uuid, Vec<Uuid>>,
) -> bool {
    let mut stack = vec![child_id];
    let mut seen = HashSet::new();
    while let Some(id) = stack.pop() {
        if id == parent_id {
            return true;
        }
        if !seen.insert(id) {
            continue;
        }
        if let Some(children) = retained_children.get(&id) {
            stack.extend(children.iter().copied());
        }
    }
    false
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
    fn attach_to_layout_transfers_existing_frame_child_ownership() {
        let frame_id = Uuid::from_u128(0x710);
        let layout_id = Uuid::from_u128(0x711);
        let child_id = Uuid::from_u128(0x712);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: frame_id,
                    kind: WidgetKind::Frame,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 200.0,
                        h: 160.0,
                    },
                    children: vec![child_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: layout_id,
                    kind: WidgetKind::VLayout,
                    rect: Rect {
                        x: 30.0,
                        y: 30.0,
                        w: 120.0,
                        h: 100.0,
                    },
                    ..Default::default()
                },
                WidgetInstance {
                    id: child_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 40.0,
                        y: 45.0,
                        w: 80.0,
                        h: 30.0,
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        assert_eq!(
            tree.attach_to_layout_at(child_id, (60.0, 50.0)),
            Some(layout_id)
        );

        let frame = tree.widgets.iter().find(|w| w.id == frame_id).unwrap();
        assert!(
            frame.children.is_empty(),
            "old Frame owner must detach child"
        );
        let layout = tree.widgets.iter().find(|w| w.id == layout_id).unwrap();
        assert_eq!(layout.children, vec![child_id]);
        assert_eq!(tree.parent_of(child_id), Some(layout_id));
    }

    #[test]
    fn nested_layouts_reflow_parent_before_child_regardless_of_storage_order() {
        let outer_id = Uuid::from_u128(0x701);
        let inner_id = Uuid::from_u128(0x702);
        let leaf_id = Uuid::from_u128(0x703);
        let mut tree = UiTree {
            // Deliberately store the nested layout before its parent. Reflow
            // must use ownership depth, not Vec order or stale snapshots.
            widgets: vec![
                WidgetInstance {
                    id: inner_id,
                    kind: WidgetKind::HLayout,
                    rect: Rect {
                        x: 500.0,
                        y: 500.0,
                        w: 100.0,
                        h: 80.0,
                    },
                    children: vec![leaf_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: leaf_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 600.0,
                        y: 600.0,
                        w: 40.0,
                        h: 24.0,
                    },
                    ..Default::default()
                },
                WidgetInstance {
                    id: outer_id,
                    kind: WidgetKind::VLayout,
                    rect: Rect {
                        x: 100.0,
                        y: 50.0,
                        w: 300.0,
                        h: 220.0,
                    },
                    children: vec![inner_id],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        tree.reflow_layouts();

        let inner = tree.widgets.iter().find(|w| w.id == inner_id).unwrap();
        assert_eq!(inner.rect.x, 108.0);
        assert_eq!(inner.rect.y, 58.0);
        assert_eq!(inner.rect.w, 284.0);
        let leaf = tree.widgets.iter().find(|w| w.id == leaf_id).unwrap();
        assert_eq!(leaf.rect.x, inner.rect.x + 8.0);
        assert_eq!(leaf.rect.y, inner.rect.y + 8.0);
        assert!(leaf.rect.x < 400.0, "leaf used stale inner-layout rect");
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
        assert_eq!(left.rect.w, 80.0);
        assert_eq!(left.rect.h, 44.0);
        assert_eq!(right.rect.x, 144.0);
        assert_eq!(right.rect.y, 48.0);
        assert_eq!(right.rect.w, 80.0);
        assert_eq!(right.rect.h, 44.0);
    }

    #[test]
    fn hlayout_horizontal_spacer_flexes_between_fixed_children() {
        let parent_id = Uuid::from_u128(33);
        let left_id = Uuid::from_u128(34);
        let spacer_id = Uuid::from_u128(35);
        let right_id = Uuid::from_u128(36);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::HLayout,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 300.0,
                        h: 60.0,
                    },
                    children: vec![left_id, spacer_id, right_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: left_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 50.0,
                        h: 30.0,
                    },
                    ..Default::default()
                },
                WidgetInstance {
                    id: spacer_id,
                    kind: WidgetKind::HorizontalSpacer,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 20.0,
                        h: 20.0,
                    },
                    ..Default::default()
                },
                WidgetInstance {
                    id: right_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 70.0,
                        h: 30.0,
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        tree.reflow_layouts();

        let left = tree.widgets.iter().find(|w| w.id == left_id).unwrap();
        let spacer = tree.widgets.iter().find(|w| w.id == spacer_id).unwrap();
        let right = tree.widgets.iter().find(|w| w.id == right_id).unwrap();
        assert_eq!(left.rect.x, 8.0);
        assert_eq!(left.rect.w, 50.0);
        assert_eq!(spacer.rect.x, 64.0);
        assert_eq!(spacer.rect.w, 152.0);
        assert_eq!(right.rect.x, 222.0);
        assert_eq!(right.rect.w, 70.0);
    }

    #[test]
    fn vlayout_vertical_spacer_flexes_between_fixed_children() {
        let parent_id = Uuid::from_u128(37);
        let top_id = Uuid::from_u128(38);
        let spacer_id = Uuid::from_u128(39);
        let bottom_id = Uuid::from_u128(40);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::VLayout,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 200.0,
                        h: 300.0,
                    },
                    children: vec![top_id, spacer_id, bottom_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: top_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 80.0,
                        h: 30.0,
                    },
                    ..Default::default()
                },
                WidgetInstance {
                    id: spacer_id,
                    kind: WidgetKind::VerticalSpacer,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 20.0,
                        h: 20.0,
                    },
                    ..Default::default()
                },
                WidgetInstance {
                    id: bottom_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 80.0,
                        h: 40.0,
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        tree.reflow_layouts();

        let top = tree.widgets.iter().find(|w| w.id == top_id).unwrap();
        let spacer = tree.widgets.iter().find(|w| w.id == spacer_id).unwrap();
        let bottom = tree.widgets.iter().find(|w| w.id == bottom_id).unwrap();
        assert_eq!(top.rect.y, 8.0);
        assert_eq!(top.rect.h, 30.0);
        assert_eq!(spacer.rect.y, 44.0);
        assert_eq!(spacer.rect.h, 202.0);
        assert_eq!(bottom.rect.y, 252.0);
        assert_eq!(bottom.rect.h, 40.0);
    }

    #[test]
    fn layout_stretch_false_preserves_child_size_hints() {
        let parent_id = Uuid::from_u128(70);
        let child_id = Uuid::from_u128(71);
        let grid_id = Uuid::from_u128(72);
        let grid_child_id = Uuid::from_u128(73);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::VLayout,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 200.0,
                        h: 100.0,
                    },
                    props: WidgetProps {
                        layout_stretch: false,
                        ..Default::default()
                    },
                    children: vec![child_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: child_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 60.0,
                        h: 30.0,
                    },
                    ..Default::default()
                },
                WidgetInstance {
                    id: grid_id,
                    kind: WidgetKind::GridLayout,
                    rect: Rect {
                        x: 0.0,
                        y: 120.0,
                        w: 200.0,
                        h: 100.0,
                    },
                    props: WidgetProps {
                        layout_stretch: false,
                        ..Default::default()
                    },
                    children: vec![grid_child_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: grid_child_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 70.0,
                        h: 24.0,
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        tree.reflow_layouts();

        let child = tree.widgets.iter().find(|w| w.id == child_id).unwrap();
        assert_eq!(child.rect.w, 60.0);
        assert_eq!(child.rect.h, 30.0);
        let grid_child = tree.widgets.iter().find(|w| w.id == grid_child_id).unwrap();
        assert_eq!(grid_child.rect.w, 70.0);
        assert_eq!(grid_child.rect.h, 24.0);
    }

    #[test]
    fn grouping_layout_children_replaces_them_with_frame_in_parent() {
        let parent_id = Uuid::from_u128(80);
        let a = Uuid::from_u128(81);
        let b = Uuid::from_u128(82);
        let c = Uuid::from_u128(83);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::HLayout,
                    children: vec![a, b, c],
                    ..Default::default()
                },
                widget(a),
                widget(b),
                widget(c),
            ],
            ..Default::default()
        };

        let frame_id = tree.group(&[b, a]).expect("group");

        let parent = tree.widgets.iter().find(|w| w.id == parent_id).unwrap();
        assert_eq!(parent.children, vec![frame_id, c]);
        let frame = tree.widgets.iter().find(|w| w.id == frame_id).unwrap();
        assert_eq!(frame.kind, WidgetKind::Frame);
        assert_eq!(frame.children, vec![a, b]);
        assert!(
            !tree
                .widgets
                .iter()
                .any(|w| w.id != frame_id && w.children.contains(&a))
        );
    }

    #[test]
    fn ungrouping_frame_inside_layout_restores_children_to_parent() {
        let parent_id = Uuid::from_u128(90);
        let frame_id = Uuid::from_u128(91);
        let a = Uuid::from_u128(92);
        let b = Uuid::from_u128(93);
        let c = Uuid::from_u128(94);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::VLayout,
                    children: vec![c, frame_id],
                    ..Default::default()
                },
                widget(c),
                WidgetInstance {
                    id: frame_id,
                    kind: WidgetKind::Frame,
                    children: vec![a, b],
                    ..Default::default()
                },
                widget(a),
                widget(b),
            ],
            ..Default::default()
        };

        let children = tree.ungroup(frame_id);

        assert_eq!(children, vec![a, b]);
        assert!(tree.widgets.iter().all(|w| w.id != frame_id));
        let parent = tree.widgets.iter().find(|w| w.id == parent_id).unwrap();
        assert_eq!(parent.children, vec![c, a, b]);
    }

    #[test]
    fn move_child_within_parent_reorders_grid_slots_and_reflows() {
        let parent_id = Uuid::from_u128(100);
        let a = Uuid::from_u128(101);
        let b = Uuid::from_u128(102);
        let c = Uuid::from_u128(103);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::GridLayout,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 220.0,
                        h: 80.0,
                    },
                    props: WidgetProps {
                        grid_columns: 2,
                        ..Default::default()
                    },
                    children: vec![a, b, c],
                    ..Default::default()
                },
                widget(a),
                widget(b),
                widget(c),
            ],
            ..Default::default()
        };

        assert!(tree.move_child_within_parent(parent_id, c, 0));

        let parent = tree.widgets.iter().find(|w| w.id == parent_id).unwrap();
        assert_eq!(parent.children, vec![c, a, b]);
        let moved = tree.widgets.iter().find(|w| w.id == c).unwrap();
        assert_eq!(moved.rect.x, 8.0);
        assert_eq!(moved.rect.y, 8.0);
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

    // --- Cline Rec 6: z-order ---

    #[test]
    fn bring_to_front_moves_to_end() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let c = Uuid::from_u128(3);
        let mut tree = UiTree {
            widgets: vec![widget(a), widget(b), widget(c)],
            ..Default::default()
        };
        tree.bring_to_front(a);
        assert_eq!(tree.widgets.last().map(|w| w.id), Some(a));
    }

    #[test]
    fn send_to_back_moves_to_start() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let c = Uuid::from_u128(3);
        let mut tree = UiTree {
            widgets: vec![widget(a), widget(b), widget(c)],
            ..Default::default()
        };
        tree.send_to_back(c);
        assert_eq!(tree.widgets.first().map(|w| w.id), Some(c));
    }

    // --- Cline Rec 6: group / ungroup ---

    #[test]
    fn group_creates_frame_with_children() {
        let a = Uuid::from_u128(10);
        let b = Uuid::from_u128(11);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: a,
                    rect: Rect {
                        x: 50.0,
                        y: 50.0,
                        w: 80.0,
                        h: 40.0,
                    },
                    ..Default::default()
                },
                WidgetInstance {
                    id: b,
                    rect: Rect {
                        x: 150.0,
                        y: 50.0,
                        w: 80.0,
                        h: 40.0,
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let frame_id = tree.group(&[a, b]);
        assert!(frame_id.is_some(), "group should return a frame id");
        let frame = tree
            .widgets
            .iter()
            .find(|w| w.id == frame_id.unwrap())
            .unwrap();
        assert_eq!(frame.kind, WidgetKind::Frame);
        assert!(frame.children.contains(&a));
        assert!(frame.children.contains(&b));
    }

    #[test]
    fn group_fails_with_less_than_two() {
        let a = Uuid::from_u128(20);
        let mut tree = UiTree {
            widgets: vec![widget(a)],
            ..Default::default()
        };
        assert_eq!(tree.group(&[a]), None);
        assert_eq!(tree.group(&[]), None);
    }

    #[test]
    fn ungroup_removes_frame_returns_children() {
        let a = Uuid::from_u128(30);
        let b = Uuid::from_u128(31);
        let frame_id = Uuid::from_u128(32);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: frame_id,
                    kind: WidgetKind::Frame,
                    children: vec![a, b],
                    ..Default::default()
                },
                widget(a),
                widget(b),
            ],
            ..Default::default()
        };
        let returned = tree.ungroup(frame_id);
        assert!(returned.contains(&a));
        assert!(returned.contains(&b));
        assert!(!tree.widgets.iter().any(|w| w.id == frame_id));
    }

    // --- Cline Rec 6: remove cascade ---

    #[test]
    fn remove_cascades_to_children() {
        let parent = Uuid::from_u128(40);
        let child = Uuid::from_u128(41);
        let grandchild = Uuid::from_u128(42);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent,
                    children: vec![child],
                    ..Default::default()
                },
                WidgetInstance {
                    id: child,
                    children: vec![grandchild],
                    ..Default::default()
                },
                widget(grandchild),
            ],
            ..Default::default()
        };
        tree.remove(parent);
        let ids: Vec<Uuid> = tree.widgets.iter().map(|w| w.id).collect();
        assert!(!ids.contains(&parent));
        assert!(!ids.contains(&child));
        assert!(!ids.contains(&grandchild));
    }

    // --- Cline Rec 6: validate_and_repair ---

    #[test]
    fn validate_and_repair_fixes_duplicate_ids() {
        let dup = Uuid::from_u128(50);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: dup,
                    ..Default::default()
                },
                WidgetInstance {
                    id: dup,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        tree.validate_and_repair();
        let ids: Vec<Uuid> = tree.widgets.iter().map(|w| w.id).collect();
        assert_eq!(ids[0], dup, "first keeps original id");
        assert_ne!(ids[1], dup, "second must be reassigned");
    }

    #[test]
    fn behaviors_pruned_when_source_widget_removed() {
        use crate::project::schema::{Behavior, ValueExpr, VisualAction, WidgetEvent};
        let btn = Uuid::from_u128(0x200);
        let bar = Uuid::from_u128(0x201);
        let mut tree = UiTree {
            widgets: vec![widget(btn), widget(bar)],
            ..Default::default()
        };
        tree.app_props.behaviors = vec![
            Behavior::widget(
                Uuid::from_u128(0x210),
                btn,
                WidgetEvent::Click,
                Some(bar),
                VisualAction::Add {
                    field: "progress".to_owned(),
                    amount: 0.1,
                    min: Some(0.0),
                    max: Some(1.0),
                },
            ),
            Behavior::widget(
                Uuid::from_u128(0x211),
                bar,
                WidgetEvent::Click,
                None,
                VisualAction::Set {
                    field: "name".to_owned(),
                    value: ValueExpr::Text(String::new()),
                },
            ),
        ];

        // Removing the target widget keeps the behavior but clears the wire
        // endpoint; removing the source widget drops the behavior entirely.
        tree.remove(bar);
        assert_eq!(tree.app_props.behaviors.len(), 1);
        assert_eq!(tree.app_props.behaviors[0].source_widget(), Some(btn));
        assert_eq!(tree.app_props.behaviors[0].target_widget, None);

        tree.remove(btn);
        assert!(tree.app_props.behaviors.is_empty());
    }

    #[test]
    fn validate_and_repair_prunes_stale_behavior_sources() {
        use crate::project::schema::{Behavior, VisualAction, WidgetEvent};
        let live = Uuid::from_u128(0x220);
        let ghost = Uuid::from_u128(0x221);
        let mut tree = UiTree {
            widgets: vec![widget(live)],
            ..Default::default()
        };
        tree.app_props.behaviors = vec![Behavior::widget(
            Uuid::from_u128(0x222),
            ghost,
            WidgetEvent::Click,
            Some(ghost),
            VisualAction::Toggle {
                field: "dark".to_owned(),
            },
        )];
        tree.validate_and_repair();
        assert!(
            tree.app_props.behaviors.is_empty(),
            "behavior with missing source widget must be pruned"
        );
    }

    #[test]
    fn validate_and_repair_removes_stale_children() {
        let parent = Uuid::from_u128(60);
        let ghost = Uuid::from_u128(61); // never in widgets vec
        let mut tree = UiTree {
            widgets: vec![WidgetInstance {
                id: parent,
                children: vec![ghost],
                ..Default::default()
            }],
            ..Default::default()
        };
        tree.validate_and_repair();
        let parent_w = tree.widgets.iter().find(|w| w.id == parent).unwrap();
        assert!(
            parent_w.children.is_empty(),
            "stale child ref must be removed"
        );
    }

    #[test]
    fn validate_and_repair_removes_duplicate_parents_and_cycles() {
        let a = Uuid::from_u128(0x801);
        let b = Uuid::from_u128(0x802);
        let c = Uuid::from_u128(0x803);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: a,
                    children: vec![b, b, c],
                    ..Default::default()
                },
                WidgetInstance {
                    id: b,
                    children: vec![a],
                    ..Default::default()
                },
                WidgetInstance {
                    id: c,
                    children: vec![b],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        tree.validate_and_repair();

        let a_children = &tree.widgets.iter().find(|w| w.id == a).unwrap().children;
        assert_eq!(a_children, &vec![b, c], "same parent dedupes children");
        let b_children = &tree.widgets.iter().find(|w| w.id == b).unwrap().children;
        assert!(
            b_children.is_empty(),
            "cycle edge back to parent is removed"
        );
        let c_children = &tree.widgets.iter().find(|w| w.id == c).unwrap().children;
        assert!(
            c_children.is_empty(),
            "second parent claim for an already-owned child is removed"
        );
    }
}

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

        assert_eq!(new_roots.len(), 1);
        let new_frame_id = new_roots[0];
        assert_ne!(new_frame_id, frame_id);
        let new_frame = tree.widgets.iter().find(|x| x.id == new_frame_id).unwrap();
        assert_eq!(new_frame.children.len(), 1);
        let new_child_id = new_frame.children[0];
        assert_ne!(new_child_id, child_id);
        assert!(tree.widgets.iter().any(|x| x.id == new_child_id));
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
        assert!(xs.contains(&(125.0, 105.0)));
        assert!(xs.contains(&(135.0, 115.0)));
    }

    #[test]
    fn cycle_in_staged_graph_aborts_with_no_mutation() {
        let mut a = w(vec![]);
        let mut b = w(vec![]);
        a.children = vec![b.id];
        b.children = vec![a.id];
        let staged = vec![a, b];

        let mut tree = UiTree::default();
        let before = tree.widgets.len();
        let result = tree.paste_batch(staged, egui::vec2(0.0, 0.0), None);
        assert!(matches!(result, Err(PasteError::InvalidGraph)));
        assert_eq!(tree.widgets.len(), before);
    }

    #[test]
    fn duplicate_parent_in_staged_graph_aborts() {
        let child = w(vec![]);
        let p1 = w(vec![child.id]);
        let p2 = w(vec![child.id]);
        let staged = vec![p1, p2, child];

        let mut tree = UiTree::default();
        let result = tree.paste_batch(staged, egui::vec2(0.0, 0.0), None);
        assert!(matches!(result, Err(PasteError::InvalidGraph)));
    }
}
