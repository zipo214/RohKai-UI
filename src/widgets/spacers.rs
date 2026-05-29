use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn horizontal_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::HorizontalSpacer,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 60.0,
            h: 8.0,
        },
        props: WidgetProps {
            label: String::new(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn vertical_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::VerticalSpacer,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 8.0,
            h: 60.0,
        },
        props: WidgetProps {
            label: String::new(),
            ..Default::default()
        },
        ..Default::default()
    }
}
