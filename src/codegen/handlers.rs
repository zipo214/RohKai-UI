//! Handler resolution helpers — single source of truth for click/change handler names.
//!
//! Both `egui_emitter` and `export` use the same priority order:
//! explicit `on_click`/`on_change` field → `event_handler` fallback → `None`.

use crate::project::schema::WidgetInstance;

/// Return the effective click handler name for `w`, or `None` if unset.
pub fn resolve_click_handler(w: &WidgetInstance) -> Option<&str> {
    if !w.on_click.is_empty() {
        return Some(w.on_click.as_str());
    }
    w.event_handler.as_deref().filter(|s| !s.is_empty())
}

/// Return the effective change handler name for `w`, or `None` if unset.
pub fn resolve_change_handler(w: &WidgetInstance) -> Option<&str> {
    if !w.on_change.is_empty() {
        return Some(w.on_change.as_str());
    }
    w.event_handler.as_deref().filter(|s| !s.is_empty())
}
