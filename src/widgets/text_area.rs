use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::TextArea,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 200.0,
            h: 80.0,
        },
        props: WidgetProps {
            label: String::from("Text"),
            placeholder: String::from("Enter text…"),
            ..Default::default()
        },
        state_binding: Some(String::from("text_area")),
        ..Default::default()
    }
}
