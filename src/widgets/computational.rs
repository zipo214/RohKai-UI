//! Stage 10 computational / IO widgets: MathLabel, FilePicker, Chart.
use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
use uuid::Uuid;

pub fn math_label_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::MathLabel,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 160.0,
            h: 28.0,
        },
        props: WidgetProps {
            label: String::from("Result"),
            ..Default::default()
        },
        state_binding: Some(String::from("computed_value")),
        ..Default::default()
    }
}

pub fn file_picker_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::FilePicker,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 240.0,
            h: 28.0,
        },
        props: WidgetProps {
            label: String::from("Browse…"),
            ..Default::default()
        },
        state_binding: Some(String::from("selected_path")),
        ..Default::default()
    }
}

pub fn chart_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::Chart,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 240.0,
            h: 160.0,
        },
        props: WidgetProps {
            label: String::from("Chart"),
            ..Default::default()
        },
        state_binding: Some(String::from("chart_values")),
        ..Default::default()
    }
}
