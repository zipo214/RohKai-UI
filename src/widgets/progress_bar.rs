use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn default_instance() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::ProgressBar,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 200.0,
            h: 20.0,
        },
        // min/max define the data range; progress binding is in [min, max]
        props: WidgetProps {
            label: String::from("Progress"),
            min: 0.0,
            max: 1.0,
        },
        state_binding: Some(String::from("progress")),
    }
}
