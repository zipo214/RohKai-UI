//! Read-only canvas overlays for Stage 11 Rust-centric visualization.
//!
//! These never mutate the `UiTree`; they paint annotations on top of the canvas
//! derived from existing data (bindings via `field_collector`, per-widget
//! handler/error annotations).  Toggled from the View menu.

use crate::canvas::widget_instance::canvas_rect;
use crate::codegen::field_collector;
use crate::project::schema::{HandlerResult, WidgetInstance};
use crate::project::ui_tree::UiTree;

/// Colour for an AppState field badge, keyed by Rust type.
fn type_color(ty: &str) -> egui::Color32 {
    match ty {
        "String" => egui::Color32::from_rgb(96, 165, 250),
        "f32" | "f64" => egui::Color32::from_rgb(251, 146, 60),
        "bool" => egui::Color32::from_rgb(167, 139, 250),
        "u32" | "i32" | "usize" => egui::Color32::from_rgb(52, 211, 153),
        _ => egui::Color32::from_gray(180),
    }
}

/// Draw ownership badges: for each widget with a binding, show `→ field: Type`
/// at the widget's top-left, plus a legend of all AppState fields.
pub fn draw_ownership(
    painter: &egui::Painter,
    tree: &UiTree,
    origin: egui::Pos2,
    zoom: f32,
    panel_rect: egui::Rect,
) {
    // Map binding name → type from the field collector (single source of truth).
    let collected = field_collector::collect(tree);
    let type_of = |name: &str| -> String {
        collected
            .fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.ty.clone())
            .unwrap_or_else(|| "?".to_owned())
    };

    for w in &tree.widgets {
        let Some(binding) = w.state_binding.as_deref().filter(|b| !b.is_empty()) else {
            continue;
        };
        let rect = canvas_rect(w, origin, zoom);
        if !panel_rect.intersects(rect) {
            continue;
        }
        // Look up + display the effective field name (keyword bindings like
        // `type` collect as `type_value`), so the badge matches the legend and
        // the generated code rather than a raw, unbound name.
        let field = crate::codegen::rust::effective_field_binding(Some(binding))
            .unwrap_or_else(|| binding.to_owned());
        let ty = type_of(&field);
        let color = type_color(&ty);
        let text = format!("→ {field}: {ty}");
        let pos = egui::pos2(rect.min.x, rect.min.y - 12.0);
        // Background chip for readability.
        let galley = painter.layout_no_wrap(text.clone(), egui::FontId::monospace(9.5), color);
        let chip = egui::Rect::from_min_size(
            pos - egui::vec2(2.0, 1.0),
            galley.size() + egui::vec2(4.0, 2.0),
        );
        painter.rect_filled(chip, 2.0, egui::Color32::from_black_alpha(170));
        painter.galley(pos, galley, color);
    }

    // Legend — top-right of the panel.
    if !collected.fields.is_empty() {
        let mut lines: Vec<(String, egui::Color32)> =
            vec![("AppState fields".to_owned(), egui::Color32::from_gray(220))];
        for f in &collected.fields {
            lines.push((format!("{}: {}", f.name, f.ty), type_color(&f.ty)));
        }
        let line_h = 13.0;
        let w = 160.0;
        let h = line_h * lines.len() as f32 + 8.0;
        let legend = egui::Rect::from_min_size(
            egui::pos2(panel_rect.max.x - w - 8.0, panel_rect.min.y + 8.0),
            egui::vec2(w, h),
        );
        painter.rect_filled(legend, 4.0, egui::Color32::from_black_alpha(200));
        painter.rect_stroke(
            legend,
            4.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
            egui::StrokeKind::Inside,
        );
        for (i, (text, color)) in lines.iter().enumerate() {
            painter.text(
                egui::pos2(legend.min.x + 6.0, legend.min.y + 4.0 + i as f32 * line_h),
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::monospace(9.5),
                *color,
            );
        }
    }
}

/// Draw error-flow badges: annotate each handler-bearing widget with its
/// declared error mode (Result / Option). Plain handlers are not badged.
pub fn draw_error_flow(
    painter: &egui::Painter,
    tree: &UiTree,
    origin: egui::Pos2,
    zoom: f32,
    panel_rect: egui::Rect,
) {
    for w in &tree.widgets {
        if !has_handler(w) {
            continue;
        }
        let rect = canvas_rect(w, origin, zoom);
        if !panel_rect.intersects(rect) {
            continue;
        }
        let (text, color) = match w.handler_result {
            HandlerResult::Plain => continue,
            HandlerResult::Result => ("Result<()>", egui::Color32::from_rgb(248, 113, 113)),
            HandlerResult::Option => ("Option<()>", egui::Color32::from_rgb(251, 191, 36)),
        };
        let async_tag = if w.async_handler { " async" } else { "" };
        let label = format!("⚠ {text}{async_tag}");
        let pos = egui::pos2(rect.min.x, rect.max.y + 2.0);
        let galley = painter.layout_no_wrap(label, egui::FontId::monospace(9.5), color);
        let chip = egui::Rect::from_min_size(
            pos - egui::vec2(2.0, 1.0),
            galley.size() + egui::vec2(4.0, 2.0),
        );
        painter.rect_filled(chip, 2.0, egui::Color32::from_black_alpha(170));
        painter.galley(pos, galley, color);
    }
}

/// Whether a widget carries any event handler the stack will actually surface.
/// Consults the authoritative `WidgetKind::supported_events()` matrix so the
/// overlay never badges handlers that codegen/export ignore.
fn has_handler(w: &WidgetInstance) -> bool {
    use crate::project::schema::WidgetEvent;
    let events = w.kind.supported_events();
    let supports = |e: WidgetEvent| events.contains(&e);
    (supports(WidgetEvent::Click) && !w.on_click.is_empty())
        || (supports(WidgetEvent::DoubleClick) && !w.on_double_click.is_empty())
        || (supports(WidgetEvent::Change) && !w.on_change.is_empty())
        || (supports(WidgetEvent::LostFocus) && !w.on_lost_focus.is_empty())
        || (supports(WidgetEvent::DragStopped) && !w.on_drag_stopped.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::WidgetKind;

    #[test]
    fn type_colors_distinct_per_kind() {
        assert_ne!(type_color("String"), type_color("f32"));
        assert_ne!(type_color("bool"), type_color("String"));
        assert_eq!(type_color("weird"), type_color("alsoweird")); // fallback stable
    }

    #[test]
    fn has_handler_detects_click_and_change() {
        let mut w = WidgetInstance {
            kind: WidgetKind::Button,
            ..Default::default()
        };
        assert!(!has_handler(&w));
        w.on_click = "go".to_owned();
        assert!(has_handler(&w));

        let mut s = WidgetInstance {
            kind: WidgetKind::Slider,
            ..Default::default()
        };
        assert!(!has_handler(&s));
        s.on_change = "changed".to_owned();
        assert!(has_handler(&s));
    }
}
