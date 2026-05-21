use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: uuid::Uuid::new_v4(),
        kind: WidgetKind::TextInput,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 160.0,
            h: 28.0,
        },
        props: WidgetProps {
            label: String::from("text_field"),
            ..Default::default()
        },
        state_binding: Some(String::from("text_field")),
    }
}
