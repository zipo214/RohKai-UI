use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::TabWidget,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 260.0,
            h: 180.0,
        },
        props: WidgetProps {
            label: String::from("Tab Widget"),
            options: vec!["Tab 1".to_owned(), "Tab 2".to_owned(), "Tab 3".to_owned()],
            ..Default::default()
        },
        ..Default::default()
    }
}
