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
        WidgetKind::Button | WidgetKind::Frame => None,
        WidgetKind::Label
        | WidgetKind::TextInput
        | WidgetKind::ComboBox
        | WidgetKind::RadioButton => Some(KindInfo {
            rust_type: "String",
            default_expr: "String::new()",
        }),
        WidgetKind::Slider | WidgetKind::ProgressBar => Some(KindInfo {
            rust_type: "f32",
            default_expr: "0.0",
        }),
        WidgetKind::Checkbox => Some(KindInfo {
            rust_type: "bool",
            default_expr: "false",
        }),
        WidgetKind::Image => None,
        WidgetKind::Custom(_) => None,
    }
}
