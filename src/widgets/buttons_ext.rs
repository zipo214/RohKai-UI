//! Stage 10 button-family widgets: ToolButton, CommandLinkButton, DialogButtonBox.
use crate::project::schema::{
    DialogButtonRole, DialogButtonSpec, Rect, WidgetInstance, WidgetKind, WidgetProps,
};
use uuid::Uuid;

pub fn tool_button_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::ToolButton,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 32.0,
            h: 32.0,
        },
        props: WidgetProps {
            label: String::from("⚙"),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn command_link_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::CommandLinkButton,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 220.0,
            h: 56.0,
        },
        props: WidgetProps {
            label: String::from("Continue"),
            placeholder: String::from("Proceed to the next step"),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn dialog_button_box_default() -> WidgetInstance {
    WidgetInstance {
        id: Uuid::new_v4(),
        kind: WidgetKind::DialogButtonBox,
        rect: Rect {
            x: 20.0,
            y: 20.0,
            w: 220.0,
            h: 36.0,
        },
        props: WidgetProps {
            label: String::from("Buttons"),
            options: vec!["OK".to_owned(), "Cancel".to_owned()],
            dialog_buttons: vec![
                DialogButtonSpec {
                    label: "OK".to_owned(),
                    role: DialogButtonRole::Accept,
                },
                DialogButtonSpec {
                    label: "Cancel".to_owned(),
                    role: DialogButtonRole::Reject,
                },
            ],
            ..Default::default()
        },
        ..Default::default()
    }
}
