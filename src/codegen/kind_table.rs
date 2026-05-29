use crate::project::schema::WidgetKind;

/// Rust type and default-expression for a widget kind's state field.
pub struct KindInfo {
    pub rust_type: &'static str,
    pub default_expr: &'static str,
}

/// Returns `None` for kinds that carry no state (Button, Frame).
/// All other callers (state_emitter, export) delegate here instead of
/// maintaining their own per-kind match arms.
pub fn state_info(kind: &WidgetKind) -> Option<KindInfo> {
    match kind {
        // Stateless widgets
        WidgetKind::Button
        | WidgetKind::Frame
        | WidgetKind::Image
        | WidgetKind::HorizontalSpacer
        | WidgetKind::VerticalSpacer
        | WidgetKind::GroupBox
        | WidgetKind::VLayout
        | WidgetKind::HLayout
        | WidgetKind::ScrollArea
        | WidgetKind::Custom(_) => None,

        // String-state widgets
        WidgetKind::Label
        | WidgetKind::TextInput
        | WidgetKind::TextArea
        | WidgetKind::ComboBox
        | WidgetKind::FontComboBox
        | WidgetKind::RadioButton => Some(KindInfo {
            rust_type: "String",
            default_expr: "String::new()",
        }),

        // f32-state widgets
        WidgetKind::Slider | WidgetKind::ProgressBar | WidgetKind::SpinBox => Some(KindInfo {
            rust_type: "f32",
            default_expr: "0.0",
        }),

        // bool-state widgets
        WidgetKind::Checkbox => Some(KindInfo {
            rust_type: "bool",
            default_expr: "false",
        }),
    }
}
