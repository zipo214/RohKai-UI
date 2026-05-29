use crate::codegen::widget_descriptor::WidgetDescriptor;
use crate::project::schema::{WidgetInstance, WidgetKind};
use crate::widgets;

// Sanity: palette CATEGORIES must cover every kind in ALL_KINDS.
// This const expression keeps ALL_KINDS referenced (it's the widget-addition protocol entry point).
const _PALETTE_KIND_COUNT: usize = widgets::ALL_KINDS.len();

/// Palette categories — order determines display order.
/// CollapsingHeader stores open/closed state in egui Memory (session-persistent).
const CATEGORIES: &[(&str, &[WidgetKind])] = &[
    (
        "Basic",
        &[
            WidgetKind::Button,
            WidgetKind::Label,
            WidgetKind::TextInput,
            WidgetKind::TextArea,
        ],
    ),
    (
        "Input",
        &[
            WidgetKind::Slider,
            WidgetKind::SpinBox,
            WidgetKind::Checkbox,
            WidgetKind::RadioButton,
            WidgetKind::ComboBox,
            WidgetKind::FontComboBox,
        ],
    ),
    (
        "Layout",
        &[
            WidgetKind::Frame,
            WidgetKind::GroupBox,
            WidgetKind::VLayout,
            WidgetKind::HLayout,
            WidgetKind::GridLayout,
            WidgetKind::ScrollArea,
            WidgetKind::TabWidget,
            WidgetKind::HorizontalSpacer,
            WidgetKind::VerticalSpacer,
        ],
    ),
    ("Display", &[WidgetKind::ProgressBar]),
];

/// Inner palette content.
/// Returns `(click_instance, drag_instance)`:
/// - `click_instance`: user clicked a palette item — caller places it at viewport centre
/// - `drag_instance`: user is dragging — caller puts it in `interaction.template_drag`
pub fn show_content(
    ui: &mut egui::Ui,
    descriptors: &[WidgetDescriptor],
) -> (Option<WidgetInstance>, Option<WidgetInstance>) {
    ui.heading("Palette");
    ui.separator();

    let mut click_add: Option<WidgetInstance> = None;
    let mut drag_add: Option<WidgetInstance> = None;

    // Built-in categories
    for &(cat_name, kinds) in CATEGORIES {
        egui::CollapsingHeader::new(cat_name)
            .default_open(true)
            .show(ui, |ui| {
                for kind in kinds {
                    let (c, d) = palette_button(ui, kind);
                    if c.is_some() {
                        click_add = c;
                    }
                    if d.is_some() {
                        drag_add = d;
                    }
                }
            });
    }

    // Descriptor-backed categories (grouped by category field)
    if !descriptors.is_empty() {
        ui.separator();
        // Collect unique category names in descriptor order
        let mut categories: Vec<&str> = Vec::new();
        for d in descriptors {
            if !categories.contains(&d.category.as_str()) {
                categories.push(&d.category);
            }
        }
        for cat_name in categories {
            egui::CollapsingHeader::new(cat_name)
                .default_open(true)
                .show(ui, |ui| {
                    for desc in descriptors.iter().filter(|d| d.category == cat_name) {
                        let (c, d_inst) = descriptor_palette_button(ui, desc);
                        if c.is_some() {
                            click_add = c;
                        }
                        if d_inst.is_some() {
                            drag_add = d_inst;
                        }
                    }
                });
        }
    }

    (click_add, drag_add)
}

fn palette_button(
    ui: &mut egui::Ui,
    kind: &WidgetKind,
) -> (Option<WidgetInstance>, Option<WidgetInstance>) {
    let label = format!("{kind:?}");
    let desired = egui::vec2(ui.available_width(), 22.0);
    let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());

    let accent = crate::canvas::interaction::kind_accent(kind);
    let vis = ui.style().interact(&resp);

    let (bg, border, text_col) = if resp.hovered() || resp.dragged() {
        let [r, g, b, _] = accent.to_array();
        let tinted = egui::Color32::from_rgb(
            (r as u32 * 4 / 20) as u8,
            (g as u32 * 4 / 20) as u8,
            (b as u32 * 4 / 20) as u8,
        );
        (tinted, egui::Stroke::new(1.5, accent), accent)
    } else {
        (vis.bg_fill, vis.bg_stroke, vis.text_color())
    };

    ui.painter().rect(rect, vis.rounding, bg, border);

    // Accent dot — left side
    let dot_center = egui::pos2(rect.min.x + 11.0, rect.center().y);
    ui.painter().circle_filled(dot_center, 4.0, accent);

    // Label — shifted right to clear the dot
    ui.painter().text(
        egui::pos2(rect.center().x + 8.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        &label,
        egui::FontId::proportional(13.0),
        text_col,
    );

    let resp = resp.on_hover_text("Click to add · Drag onto canvas");

    let mut click_add = None;
    let mut drag_add = None;
    if resp.clicked() {
        click_add = Some(widgets::default_for(kind));
    } else if resp.drag_started() {
        drag_add = Some(widgets::default_for(kind));
    }
    (click_add, drag_add)
}

fn descriptor_palette_button(
    ui: &mut egui::Ui,
    descriptor: &WidgetDescriptor,
) -> (Option<WidgetInstance>, Option<WidgetInstance>) {
    let [r, g, b] = descriptor.accent_color;
    let accent = egui::Color32::from_rgb(r, g, b);
    let desired = egui::vec2(ui.available_width(), 22.0);
    let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());

    let vis = ui.style().interact(&resp);
    let (bg, border, text_col) = if resp.hovered() || resp.dragged() {
        let tinted = egui::Color32::from_rgb(
            (r as u32 * 4 / 20) as u8,
            (g as u32 * 4 / 20) as u8,
            (b as u32 * 4 / 20) as u8,
        );
        (tinted, egui::Stroke::new(1.5, accent), accent)
    } else {
        (vis.bg_fill, vis.bg_stroke, vis.text_color())
    };

    ui.painter().rect(rect, vis.rounding, bg, border);

    // Accent dot
    let dot_center = egui::pos2(rect.min.x + 11.0, rect.center().y);
    ui.painter().circle_filled(dot_center, 4.0, accent);

    // Name label
    ui.painter().text(
        egui::pos2(rect.center().x + 8.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        &descriptor.name,
        egui::FontId::proportional(13.0),
        text_col,
    );

    let resp = resp.on_hover_text(format!(
        "{} · Click to add · Drag onto canvas",
        descriptor.id
    ));

    let mut click_add = None;
    let mut drag_add = None;
    if resp.clicked() {
        click_add = Some(widgets::default_for_descriptor(descriptor));
    } else if resp.drag_started() {
        drag_add = Some(widgets::default_for_descriptor(descriptor));
    }
    (click_add, drag_add)
}
