use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn vlayout_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::VLayout,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 200.0,
            h: 200.0,
        },
        props: WidgetProps {
            label: String::from("VLayout"),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn hlayout_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::HLayout,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 300.0,
            h: 80.0,
        },
        props: WidgetProps {
            label: String::from("HLayout"),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn scroll_area_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::ScrollArea,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 200.0,
            h: 150.0,
        },
        props: WidgetProps {
            label: String::from("Scroll Area"),
            ..Default::default()
        },
        ..Default::default()
    }
}
