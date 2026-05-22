use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
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
