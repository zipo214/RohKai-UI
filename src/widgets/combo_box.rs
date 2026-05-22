use crate::project::schema::{
    default_combo_options, Rect, WidgetInstance, WidgetKind, WidgetProps,
};
use uuid::Uuid;

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::ComboBox,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 150.0,
            h: 28.0,
        },
        props: WidgetProps {
            label: String::from("Select…"),
            options: default_combo_options(),
            ..Default::default()
        },
        state_binding: Some(String::from("combo_value")),
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
