use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::RadioButton,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 140.0,
            h: 28.0,
        },
        props: WidgetProps {
            label: String::from("Option A"),
            min: 0.0,
            max: 100.0,
        },
        state_binding: Some(String::from("radio_value")),
    }
}
