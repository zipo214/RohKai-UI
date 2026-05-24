pub mod button;
pub mod checkbox;
pub mod combo_box;
pub mod frame;
pub mod label;
pub mod progress_bar;
pub mod radio_button;
pub mod slider;
pub mod text_input;

use crate::codegen::widget_descriptor::WidgetDescriptor;
use crate::project::schema::{WidgetInstance, WidgetKind};

pub fn default_for(kind: &WidgetKind) -> WidgetInstance {
    match kind {
        WidgetKind::Button => button::default_instance(),
        WidgetKind::Label => label::default_instance(),
        WidgetKind::TextInput => text_input::default_instance(),
        WidgetKind::Slider => slider::default_instance(),
        WidgetKind::Checkbox => checkbox::default_instance(),
        WidgetKind::Frame => frame::default_instance(),
        WidgetKind::ComboBox => combo_box::default_instance(),
        WidgetKind::RadioButton => radio_button::default_instance(),
        WidgetKind::ProgressBar => progress_bar::default_instance(),
        WidgetKind::Image => crate::project::schema::WidgetInstance {
            kind: WidgetKind::Image,
            rect: crate::project::schema::Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 200.0,
            },
            props: crate::project::schema::WidgetProps {
                label: "SVG Image".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        },
        WidgetKind::Custom(ref id) => {
            // Fallback for Custom without a descriptor — should not normally be
            // called via default_for; use default_for_descriptor instead.
            crate::project::schema::WidgetInstance {
                kind: WidgetKind::Custom(id.clone()),
                props: crate::project::schema::WidgetProps {
                    label: id.clone(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }
    }
}

/// Build a default `WidgetInstance` from a loaded descriptor.
pub fn default_for_descriptor(descriptor: &WidgetDescriptor) -> WidgetInstance {
    let [w, h] = descriptor.default_size;
    WidgetInstance {
        kind: WidgetKind::Custom(descriptor.id.clone()),
        rect: crate::project::schema::Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        },
        props: crate::project::schema::WidgetProps {
            label: descriptor
                .properties
                .iter()
                .find(|p| p.key == "label")
                .map(|p| p.default.clone())
                .unwrap_or_else(|| descriptor.name.clone()),
            ..Default::default()
        },
        descriptor_name: Some(descriptor.name.clone()),
        descriptor_accent: Some(descriptor.accent_color),
        descriptor_live_tpl: Some(descriptor.codegen.live_preview.clone()),
        descriptor_export_tpl: Some(descriptor.codegen.export.clone()),
        descriptor_props: crate::codegen::widget_descriptor::default_props(descriptor),
        descriptor_state_fields: descriptor
            .state_fields
            .iter()
            .map(|sf| {
                [
                    sf.key.clone(),
                    sf.rust_type.clone(),
                    sf.default_expr.clone(),
                ]
            })
            .collect(),
        // Deps are validated at descriptor-load time; format_cargo_dep is
        // called here again to produce the canonical TOML line.  Any dep that
        // failed validation was already rejected by load_from_widgets_dir, so
        // the unwrap_or fallback is a safety net only.
        descriptor_cargo_deps: descriptor
            .cargo_deps
            .iter()
            .filter_map(|dep| crate::codegen::widget_descriptor::format_cargo_dep(dep).ok())
            .collect(),
        ..Default::default()
    }
}

#[allow(dead_code)] // widget-addition protocol: step 3 of /new-widget — add variant here
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
