pub mod button;
pub mod combo_box;
pub mod frame;
pub mod label;
pub mod progress_bar;
pub mod radio_button;
pub mod slider;
pub mod text_input;

use crate::project::schema::{WidgetInstance, WidgetKind};

pub fn default_for(kind: &WidgetKind) -> WidgetInstance {
    match kind {
        WidgetKind::Button => button::default_instance(),
        WidgetKind::Label => label::default_instance(),
        WidgetKind::TextInput => text_input::default_instance(),
        WidgetKind::Slider => slider::default_instance(),
        WidgetKind::Checkbox => {
            let mut w = button::default_instance();
            w.kind = WidgetKind::Checkbox;
            w.props.label = String::from("Enable");
            w.state_binding = Some(String::from("is_enabled"));
            w
        }
        WidgetKind::Frame => frame::default_instance(),
        WidgetKind::ComboBox => combo_box::default_instance(),
        WidgetKind::RadioButton => radio_button::default_instance(),
        WidgetKind::ProgressBar => progress_bar::default_instance(),
    }
}

pub const ALL_KINDS: &[WidgetKind] = &[
    WidgetKind::Button,
    WidgetKind::Label,
    WidgetKind::TextInput,
    WidgetKind::Slider,
    WidgetKind::Checkbox,
    WidgetKind::Frame,
    WidgetKind::ComboBox,
    WidgetKind::RadioButton,
    WidgetKind::ProgressBar,
];
