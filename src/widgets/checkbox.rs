use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::Checkbox,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 100.0,
            h: 30.0,
        },
        props: WidgetProps {
            label: String::from("Enable"),
            ..Default::default()
        },
        state_binding: Some(String::from("is_enabled")),
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
