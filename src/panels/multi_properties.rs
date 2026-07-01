use crate::project::schema::{WidgetInstance, WidgetKind};
use crate::project::ui_tree::UiTree;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SharedValue<T> {
    Same(T),
    Mixed,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SharedProperties {
    pub label: SharedValue<String>,
    pub width: SharedValue<f32>,
    pub height: SharedValue<f32>,
    pub tooltip: SharedValue<Option<String>>,
    pub enabled: SharedValue<bool>,
    pub fg_color: SharedValue<Option<[u8; 3]>>,
    pub corner_radius: SharedValue<Option<f32>>,
}

pub(crate) fn collect_shared(widgets: &[&WidgetInstance]) -> SharedProperties {
    SharedProperties {
        label: shared_label(widgets),
        width: shared_by(widgets, |w| w.rect.w),
        height: shared_by(widgets, |w| w.rect.h),
        tooltip: shared_by(widgets, |w| w.tooltip.clone()),
        enabled: shared_by(widgets, |w| w.enabled.unwrap_or(true)),
        fg_color: shared_by(widgets, |w| w.fg_color),
        corner_radius: shared_by(widgets, |w| w.corner_radius),
    }
}

pub(crate) fn show(ui: &mut egui::Ui, tree: &mut UiTree, selected: &[Uuid]) {
    let widgets = selected_widgets(tree, selected);
    if widgets.is_empty() {
        ui.weak("No live selected widgets.");
        return;
    }

    let count = widgets.len();
    let first_width = widgets[0].rect.w;
    let first_height = widgets[0].rect.h;
    let first_label = widgets[0].props.label.clone();
    let shared = collect_shared(&widgets);
    drop(widgets);
    let mut did_mutate = false;

    ui.separator();
    ui.label(egui::RichText::new(format!("{count} widgets selected")).strong());
    ui.weak("Shared edits apply to all compatible selected widgets.");

    egui::Grid::new("multi_properties_geometry")
        .num_columns(4)
        .spacing([6.0, 4.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("W").small().weak());
            let mut width = shared_f32_or(&shared.width, first_width);
            if ui
                .add(
                    egui::DragValue::new(&mut width)
                        .speed(1.0)
                        .range(1.0..=10_000.0),
                )
                .on_hover_text(shared_hint(&shared.width))
                .changed()
            {
                apply_size(tree, selected, Some(width), None);
                did_mutate = true;
            }
            ui.label(egui::RichText::new("H").small().weak());
            let mut height = shared_f32_or(&shared.height, first_height);
            if ui
                .add(
                    egui::DragValue::new(&mut height)
                        .speed(1.0)
                        .range(1.0..=10_000.0),
                )
                .on_hover_text(shared_hint(&shared.height))
                .changed()
            {
                apply_size(tree, selected, None, Some(height));
                did_mutate = true;
            }
            ui.end_row();
        });
    show_mixed_note(ui, "Width", &shared.width);
    show_mixed_note(ui, "Height", &shared.height);

    show_multi_label(ui, tree, selected, &shared.label, &first_label);
    show_multi_tooltip(ui, tree, selected, &shared.tooltip);
    show_multi_enabled(ui, tree, selected, &shared.enabled);
    show_multi_fg_color(ui, tree, selected, &shared.fg_color);
    show_multi_corner_radius(ui, tree, selected, &shared.corner_radius);

    if did_mutate {
        tree.validate_and_repair();
    }
}

pub(crate) fn apply_label(tree: &mut UiTree, selected: &[Uuid], label: &str) {
    for id in selected {
        let Some(w) = tree.get_mut(*id) else {
            continue;
        };
        if is_label_capable(&w.kind) {
            w.props.label = label.to_owned();
        }
    }
}

pub(crate) fn apply_size(
    tree: &mut UiTree,
    selected: &[Uuid],
    width: Option<f32>,
    height: Option<f32>,
) {
    for id in selected {
        let Some(w) = tree.get_mut(*id) else {
            continue;
        };
        if let Some(width) = width {
            w.rect.w = width.max(1.0);
        }
        if let Some(height) = height {
            w.rect.h = height.max(1.0);
        }
    }
}

pub(crate) fn apply_tooltip(tree: &mut UiTree, selected: &[Uuid], tooltip: Option<String>) {
    for id in selected {
        if let Some(w) = tree.get_mut(*id) {
            w.tooltip = tooltip.clone().filter(|s| !s.is_empty());
        }
    }
}

pub(crate) fn apply_enabled(tree: &mut UiTree, selected: &[Uuid], enabled: bool) {
    for id in selected {
        if let Some(w) = tree.get_mut(*id) {
            w.enabled = if enabled { None } else { Some(false) };
        }
    }
}

pub(crate) fn apply_fg_color(tree: &mut UiTree, selected: &[Uuid], fg_color: Option<[u8; 3]>) {
    for id in selected {
        if let Some(w) = tree.get_mut(*id) {
            w.fg_color = fg_color;
        }
    }
}

pub(crate) fn apply_corner_radius(
    tree: &mut UiTree,
    selected: &[Uuid],
    corner_radius: Option<f32>,
) {
    for id in selected {
        if let Some(w) = tree.get_mut(*id) {
            w.corner_radius = corner_radius.filter(|r| *r > 0.0);
        }
    }
}

fn shared_label(widgets: &[&WidgetInstance]) -> SharedValue<String> {
    if widgets.is_empty() || widgets.iter().any(|w| !is_label_capable(&w.kind)) {
        return SharedValue::Unavailable;
    }
    shared_by(widgets, |w| w.props.label.clone())
}

fn shared_by<T, F>(widgets: &[&WidgetInstance], mut f: F) -> SharedValue<T>
where
    T: Clone + PartialEq,
    F: FnMut(&WidgetInstance) -> T,
{
    let Some(first) = widgets.first() else {
        return SharedValue::Unavailable;
    };
    let first_value = f(first);
    if widgets.iter().skip(1).any(|w| f(w) != first_value) {
        SharedValue::Mixed
    } else {
        SharedValue::Same(first_value)
    }
}

fn is_label_capable(kind: &WidgetKind) -> bool {
    matches!(
        kind,
        WidgetKind::Button
            | WidgetKind::Label
            | WidgetKind::Checkbox
            | WidgetKind::RadioButton
            | WidgetKind::Frame
            | WidgetKind::GroupBox
            | WidgetKind::ToolButton
            | WidgetKind::CommandLinkButton
            | WidgetKind::DialogButtonBox
            | WidgetKind::MathLabel
            | WidgetKind::FilePicker
            | WidgetKind::Custom(_)
    )
}

fn selected_widgets<'a>(tree: &'a UiTree, selected: &[Uuid]) -> Vec<&'a WidgetInstance> {
    selected
        .iter()
        .filter_map(|id| tree.widgets.iter().find(|w| w.id == *id))
        .collect()
}

fn shared_f32_or(shared: &SharedValue<f32>, fallback: f32) -> f32 {
    match shared {
        SharedValue::Same(value) => *value,
        SharedValue::Mixed | SharedValue::Unavailable => fallback,
    }
}

fn shared_hint<T>(shared: &SharedValue<T>) -> &'static str {
    match shared {
        SharedValue::Same(_) => "Shared value",
        SharedValue::Mixed => "Mixed value; editing overwrites selected widgets",
        SharedValue::Unavailable => "Unavailable for this selection",
    }
}

fn show_mixed_note<T>(ui: &mut egui::Ui, label: &str, shared: &SharedValue<T>) {
    if matches!(shared, SharedValue::Mixed) {
        ui.weak(format!("{label}: mixed"));
    }
}

fn show_multi_label(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    selected: &[Uuid],
    label: &SharedValue<String>,
    first_label: &str,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Label").small().weak());
        match label {
            SharedValue::Unavailable => {
                ui.weak("Not common to this selection");
            }
            SharedValue::Same(value) => {
                let mut text = value.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY))
                    .changed()
                {
                    apply_label(tree, selected, &text);
                    tree.validate_and_repair();
                }
            }
            SharedValue::Mixed => {
                let mut text = String::new();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut text)
                            .hint_text(format!("Mixed, first: {first_label}"))
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    apply_label(tree, selected, &text);
                    tree.validate_and_repair();
                }
            }
        }
    });
}

fn show_multi_tooltip(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    selected: &[Uuid],
    tooltip: &SharedValue<Option<String>>,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Tooltip").small().weak());
        let mut text = match tooltip {
            SharedValue::Same(Some(value)) => value.clone(),
            SharedValue::Same(None) | SharedValue::Mixed | SharedValue::Unavailable => {
                String::new()
            }
        };
        let hint = match tooltip {
            SharedValue::Mixed => "Mixed; edit to overwrite",
            SharedValue::Same(None) => "No tooltip",
            SharedValue::Unavailable => "Unavailable",
            SharedValue::Same(Some(_)) => "Hover text...",
        };
        if ui
            .add(
                egui::TextEdit::singleline(&mut text)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY),
            )
            .changed()
        {
            apply_tooltip(tree, selected, Some(text));
            tree.validate_and_repair();
        }
        if ui
            .small_button("✕")
            .on_hover_text("Clear tooltip for selected widgets")
            .clicked()
        {
            apply_tooltip(tree, selected, None);
            tree.validate_and_repair();
        }
    });
}

fn show_multi_enabled(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    selected: &[Uuid],
    enabled: &SharedValue<bool>,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Enabled").small().weak());
        match enabled {
            SharedValue::Same(value) => {
                let mut value = *value;
                if ui.checkbox(&mut value, "").changed() {
                    apply_enabled(tree, selected, value);
                    tree.validate_and_repair();
                }
            }
            SharedValue::Mixed => {
                ui.weak("Mixed");
                if ui.small_button("Enable all").clicked() {
                    apply_enabled(tree, selected, true);
                    tree.validate_and_repair();
                }
                if ui.small_button("Disable all").clicked() {
                    apply_enabled(tree, selected, false);
                    tree.validate_and_repair();
                }
            }
            SharedValue::Unavailable => {
                ui.weak("Unavailable");
            }
        }
    });
}

fn show_multi_fg_color(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    selected: &[Uuid],
    fg_color: &SharedValue<Option<[u8; 3]>>,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Fg color").small().weak());
        if matches!(fg_color, SharedValue::Mixed) {
            ui.weak("Mixed");
        }
        let mut c32 = match fg_color {
            SharedValue::Same(Some(c)) => egui::Color32::from_rgb(c[0], c[1], c[2]),
            SharedValue::Same(None) | SharedValue::Mixed | SharedValue::Unavailable => {
                egui::Color32::WHITE
            }
        };
        let before = c32;
        let resp = egui::color_picker::color_edit_button_srgba(
            ui,
            &mut c32,
            egui::color_picker::Alpha::Opaque,
        );
        if resp.changed() || c32 != before {
            let color = if c32 == egui::Color32::WHITE {
                None
            } else {
                Some([c32.r(), c32.g(), c32.b()])
            };
            apply_fg_color(tree, selected, color);
            tree.validate_and_repair();
        }
        if ui
            .small_button("✕")
            .on_hover_text("Reset fg color for selected widgets")
            .clicked()
        {
            apply_fg_color(tree, selected, None);
            tree.validate_and_repair();
        }
    });
}

fn show_multi_corner_radius(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    selected: &[Uuid],
    corner_radius: &SharedValue<Option<f32>>,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Radius").small().weak());
        if matches!(corner_radius, SharedValue::Mixed) {
            ui.weak("Mixed");
        }
        let mut radius = match corner_radius {
            SharedValue::Same(Some(value)) => *value,
            SharedValue::Same(None) | SharedValue::Mixed | SharedValue::Unavailable => 0.0,
        };
        if ui
            .add(
                egui::DragValue::new(&mut radius)
                    .range(0.0..=32.0_f32)
                    .speed(0.5)
                    .suffix(" px"),
            )
            .changed()
        {
            apply_corner_radius(tree, selected, Some(radius));
            tree.validate_and_repair();
        }
        if ui
            .small_button("✕")
            .on_hover_text("Reset rounding for selected widgets")
            .clicked()
        {
            apply_corner_radius(tree, selected, None);
            tree.validate_and_repair();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};
    use crate::project::ui_tree::UiTree;
    use uuid::Uuid;

    fn widget(id: u128, kind: WidgetKind, label: &str) -> WidgetInstance {
        WidgetInstance {
            id: Uuid::from_u128(id),
            kind,
            rect: Rect {
                x: id as f32,
                y: id as f32,
                w: 100.0,
                h: 30.0,
            },
            props: WidgetProps {
                label: label.to_owned(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn tree_with(widgets: Vec<WidgetInstance>) -> UiTree {
        UiTree {
            widgets,
            ..Default::default()
        }
    }

    #[test]
    fn shared_label_reports_same_mixed_and_unavailable() {
        let mut a = widget(1, WidgetKind::Button, "Save");
        let b = widget(2, WidgetKind::Checkbox, "Save");
        let c = widget(3, WidgetKind::RadioButton, "Cancel");
        let d = widget(4, WidgetKind::Slider, "Value");

        assert_eq!(
            collect_shared(&[&a, &b]).label,
            SharedValue::Same("Save".to_owned())
        );
        assert_eq!(collect_shared(&[&a, &c]).label, SharedValue::Mixed);
        assert_eq!(collect_shared(&[&a, &d]).label, SharedValue::Unavailable);

        a.kind = WidgetKind::Label;
        assert_eq!(
            collect_shared(&[&a]).label,
            SharedValue::Same("Save".to_owned())
        );
    }

    #[test]
    fn shared_size_reports_mixed_without_mutating() {
        let a = widget(1, WidgetKind::Button, "A");
        let mut b = widget(2, WidgetKind::Button, "B");
        b.rect.h = 44.0;
        let tree = tree_with(vec![a.clone(), b.clone()]);

        let shared = collect_shared(&[&a, &b]);

        assert_eq!(shared.width, SharedValue::Same(100.0));
        assert_eq!(shared.height, SharedValue::Mixed);
        assert_eq!(tree.widgets[0].rect.h, 30.0);
        assert_eq!(tree.widgets[1].rect.h, 44.0);
    }

    #[test]
    fn apply_label_touches_only_selected_label_capable_widgets() {
        let mut tree = tree_with(vec![
            widget(1, WidgetKind::Button, "A"),
            widget(2, WidgetKind::Slider, "Value"),
            widget(3, WidgetKind::Label, "C"),
            widget(4, WidgetKind::Button, "Unselected"),
        ]);
        let selected = [Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)];

        apply_label(&mut tree, &selected, "Shared");

        assert_eq!(tree.widgets[0].props.label, "Shared");
        assert_eq!(tree.widgets[1].props.label, "Value");
        assert_eq!(tree.widgets[2].props.label, "Shared");
        assert_eq!(tree.widgets[3].props.label, "Unselected");
    }

    #[test]
    fn apply_size_and_appearance_touch_only_selected_widgets() {
        let mut tree = tree_with(vec![
            widget(1, WidgetKind::Button, "A"),
            widget(2, WidgetKind::Label, "B"),
            widget(3, WidgetKind::Button, "C"),
        ]);
        let selected = [Uuid::from_u128(1), Uuid::from_u128(2)];

        apply_size(&mut tree, &selected, Some(140.0), Some(36.0));
        apply_tooltip(&mut tree, &selected, Some("Shared tip".to_owned()));
        apply_enabled(&mut tree, &selected, false);
        apply_fg_color(&mut tree, &selected, Some([1, 2, 3]));
        apply_corner_radius(&mut tree, &selected, Some(8.0));

        for w in &tree.widgets[..2] {
            assert_eq!(w.rect.w, 140.0);
            assert_eq!(w.rect.h, 36.0);
            assert_eq!(w.tooltip.as_deref(), Some("Shared tip"));
            assert_eq!(w.enabled, Some(false));
            assert_eq!(w.fg_color, Some([1, 2, 3]));
            assert_eq!(w.corner_radius, Some(8.0));
        }
        assert_eq!(tree.widgets[2].rect.w, 100.0);
        assert_eq!(tree.widgets[2].tooltip, None);
        assert_eq!(tree.widgets[2].enabled, None);
        assert_eq!(tree.widgets[2].fg_color, None);
        assert_eq!(tree.widgets[2].corner_radius, None);
    }
}
