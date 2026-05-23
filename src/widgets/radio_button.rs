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
            radio_value: String::from("option_a"),
            group_binding: String::from("selected_option"),
            ..Default::default()
        },
        state_binding: Some(String::from("selected_option")),
        ..Default::default()
    }
}
