use crate::project::schema::WidgetInstance;

/// Convert a WidgetInstance's stored rect to an egui::Rect in screen space.
/// `origin` is where canvas (0,0) maps in screen space; `zoom` scales sizes.
pub fn canvas_rect(w: &WidgetInstance, origin: egui::Pos2, zoom: f32) -> egui::Rect {
    egui::Rect::from_min_size(
        origin + egui::vec2(w.rect.x, w.rect.y) * zoom,
        egui::vec2(w.rect.w, w.rect.h) * zoom,
    )
}
