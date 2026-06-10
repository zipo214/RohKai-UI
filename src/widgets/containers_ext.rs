//! Stage 10 container widgets: StackedWidget, ToolBox.
use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn stacked_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::StackedWidget,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 240.0,
            h: 160.0,
        },
        props: WidgetProps {
            label: String::from("Stack"),
            options: vec!["Page 1".to_owned(), "Page 2".to_owned()],
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn tool_box_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::ToolBox,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 220.0,
            h: 180.0,
        },
        props: WidgetProps {
            label: String::from("Tool Box"),
            options: vec!["Section A".to_owned(), "Section B".to_owned()],
            ..Default::default()
        },
        ..Default::default()
    }
}
