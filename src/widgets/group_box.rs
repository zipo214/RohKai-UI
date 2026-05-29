use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::GroupBox,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 200.0,
            h: 120.0,
        },
        props: WidgetProps {
            label: String::from("Group"),
            ..Default::default()
        },
        ..Default::default()
    }
}
