use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::Slider,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 200.0,
            h: 28.0,
        },
        props: WidgetProps {
            label: String::from("Value"),
            min: 0.0,
            max: 100.0,
            default_value: 50.0,
            ..Default::default()
        },
        state_binding: Some(String::from("slider_value")),
        ..Default::default()
    }
}
