use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::FontComboBox,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 160.0,
            h: 28.0,
        },
        props: WidgetProps {
            label: String::from("Proportional"),
            ..Default::default()
        },
        state_binding: Some(String::from("selected_font")),
        ..Default::default()
    }
}
