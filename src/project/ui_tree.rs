use crate::project::schema::{AppProps, WidgetInstance};
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
        self.widgets.retain(|w| w.id != id);
    }

    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut WidgetInstance> {
        self.widgets.iter_mut().find(|w| w.id == id)
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
    }

    fn repair_widget(widget: &mut WidgetInstance) {
        widget.rect.x = widget.rect.x.max(0.0);
        widget.rect.y = widget.rect.y.max(0.0);
        widget.rect.w = widget.rect.w.max(MIN_WIDGET_SIZE);
        widget.rect.h = widget.rect.h.max(MIN_WIDGET_SIZE);

        if widget.props.min > widget.props.max {
            std::mem::swap(&mut widget.props.min, &mut widget.props.max);
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
            #[cfg(debug_assertions)]
            eprintln!(
                "bring_to_front: idx {} → {} (len {})",
                idx,
                self.widgets.len() - 1,
                self.widgets.len()
            );
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
            #[cfg(debug_assertions)]
            eprintln!("send_to_back: idx {} → 0 (len {})", idx, self.widgets.len());
            let w = self.widgets.remove(idx);
            self.widgets.insert(0, w);
            debug_assert_eq!(
                self.widgets.first().map(|w| w.id),
                Some(id),
                "send_to_back: widget not at index 0"
            );
        }
    }

    /// Swap with next index — higher index = drawn later = more on top.
    pub fn bring_forward(&mut self, id: Uuid) {
        if let Some(idx) = self.widgets.iter().position(|w| w.id == id) {
            if idx + 1 < self.widgets.len() {
                #[cfg(debug_assertions)]
                eprintln!("bring_forward: idx {} ↔ {}", idx, idx + 1);
                self.widgets.swap(idx, idx + 1);
                debug_assert_eq!(self.widgets[idx + 1].id, id, "bring_forward: swap failed");
            }
        }
    }

    /// Swap with previous index — lower index = drawn earlier = more behind.
    pub fn send_back(&mut self, id: Uuid) {
        if let Some(idx) = self.widgets.iter().position(|w| w.id == id) {
            if idx > 0 {
                #[cfg(debug_assertions)]
                eprintln!("send_back: idx {} ↔ {}", idx, idx - 1);
                self.widgets.swap(idx, idx - 1);
                debug_assert_eq!(self.widgets[idx - 1].id, id, "send_back: swap failed");
            }
        }
    }
}
