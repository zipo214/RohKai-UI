use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::Frame,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 200.0,
            h: 150.0,
        },
        props: WidgetProps {
            label: String::from("Group"),
            ..Default::default()
        },
        state_binding: None,
        children: Vec::new(),
        import_metadata: None,
        tooltip: None,
        enabled: None,
        fg_color: None,
        corner_radius: None,
        label_binding: None,
        custom_props: Vec::new(),
        event_handler: None,
    }
}
