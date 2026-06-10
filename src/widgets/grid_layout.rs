use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::GridLayout,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 240.0,
            h: 160.0,
        },
        props: WidgetProps {
            label: String::from("Grid"),
            ..Default::default()
        },
        ..Default::default()
    }
}
