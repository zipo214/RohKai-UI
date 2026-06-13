//! Shared viewport-safe sizing for widget-authoring windows.

const VIEWPORT_MARGIN: f32 = 16.0;

#[derive(Debug, Clone, Copy)]
pub struct AuthoringWindowBounds {
    pub default_pos: egui::Pos2,
    pub default_size: egui::Vec2,
    pub min_size: egui::Vec2,
    pub max_size: egui::Vec2,
}

pub fn authoring_window_bounds(
    viewport: egui::Rect,
    preferred_size: egui::Vec2,
    requested_min_size: egui::Vec2,
) -> AuthoringWindowBounds {
    let available = egui::vec2(
        (viewport.width() - VIEWPORT_MARGIN * 2.0).max(1.0),
        (viewport.height() - VIEWPORT_MARGIN * 2.0).max(1.0),
    );
    let max_size = available;
    let min_size = egui::vec2(
        requested_min_size.x.min(max_size.x),
        requested_min_size.y.min(max_size.y),
    );
    let default_size = egui::vec2(
        preferred_size.x.clamp(min_size.x, max_size.x),
        preferred_size.y.clamp(min_size.y, max_size.y),
    );
    let default_pos = egui::pos2(
        viewport.center().x - default_size.x * 0.5,
        viewport.center().y - default_size.y * 0.5,
    );

    AuthoringWindowBounds {
        default_pos,
        default_size,
        min_size,
        max_size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_stay_inside_normal_viewport() {
        let viewport = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 720.0));
        let bounds =
            authoring_window_bounds(viewport, egui::vec2(960.0, 620.0), egui::vec2(700.0, 460.0));
        let rect = egui::Rect::from_min_size(bounds.default_pos, bounds.default_size);

        assert!(viewport.contains_rect(rect));
        assert!(bounds.default_size.x <= bounds.max_size.x);
        assert!(bounds.default_size.y <= bounds.max_size.y);
        assert!(bounds.min_size.x <= bounds.max_size.x);
        assert!(bounds.min_size.y <= bounds.max_size.y);
    }

    #[test]
    fn tiny_viewport_reduces_minimum_without_overflow() {
        let viewport = egui::Rect::from_min_size(egui::pos2(20.0, 30.0), egui::vec2(420.0, 300.0));
        let bounds =
            authoring_window_bounds(viewport, egui::vec2(960.0, 620.0), egui::vec2(700.0, 460.0));
        let rect = egui::Rect::from_min_size(bounds.default_pos, bounds.default_size);

        assert!(viewport.contains_rect(rect));
        assert_eq!(bounds.min_size, bounds.max_size);
        assert_eq!(bounds.default_size, bounds.max_size);
    }
}
