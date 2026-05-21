use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: uuid::Uuid::new_v4(),
        kind: WidgetKind::Button,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 100.0,
            h: 30.0,
        },
        props: WidgetProps {
            label: String::from("Button"),
            ..Default::default()
        },
        state_binding: None,
    }
}
