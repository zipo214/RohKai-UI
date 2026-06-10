//! Stage 10 data-view widgets: Table, ListView, TreeView.
use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn table_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::Table,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 280.0,
            h: 160.0,
        },
        props: WidgetProps {
            label: String::from("Table"),
            options: vec![
                "Column A".to_owned(),
                "Column B".to_owned(),
                "Column C".to_owned(),
            ],
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn list_view_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::ListView,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 200.0,
            h: 160.0,
        },
        props: WidgetProps {
            label: String::from("List"),
            options: vec![
                "Item 1".to_owned(),
                "Item 2".to_owned(),
                "Item 3".to_owned(),
            ],
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn tree_view_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::TreeView,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 200.0,
            h: 160.0,
        },
        props: WidgetProps {
            label: String::from("Tree"),
            options: vec![
                "Root".to_owned(),
                "Child A".to_owned(),
                "Child B".to_owned(),
            ],
            ..Default::default()
        },
        ..Default::default()
    }
}
