//! Project surfaces panel and reusable modal-dialog templates.

use crate::project::{
    document::{ActiveDocument, ModalDialogProps, ProjectDocument, SurfaceKind},
    schema::{Rect, WidgetInstance, WidgetKind, WidgetProps},
    ui_tree::UiTree,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogTemplate {
    Blank,
    OkCancel,
    Settings,
}

impl DialogTemplate {
    pub const ALL: [Self; 3] = [Self::Blank, Self::OkCancel, Self::Settings];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Blank => "Blank Dialog",
            Self::OkCancel => "OK / Cancel Dialog",
            Self::Settings => "Settings Dialog",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceAction {
    None,
    Activate(Uuid),
    Add(DialogTemplate),
    Duplicate(Uuid),
    Delete(Uuid),
    MoveUp(Uuid),
    MoveDown(Uuid),
}

pub fn show_content(
    ui: &mut egui::Ui,
    document: &ProjectDocument,
    active_surface: Uuid,
) -> SurfaceAction {
    let mut action = SurfaceAction::None;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Project Surfaces").strong());
        ui.menu_button("+", |ui| {
            for template in DialogTemplate::ALL {
                if ui.button(template.label()).clicked() {
                    action = SurfaceAction::Add(template);
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text("Create modal dialog");
    });
    ui.label(
        egui::RichText::new("Each surface is an independent editable form.")
            .small()
            .weak(),
    );
    ui.add_space(4.0);

    for surface in &document.surfaces {
        let is_root = surface.id == document.root_surface;
        let icon = match surface.kind {
            SurfaceKind::MainWindow => "▣",
            SurfaceKind::ModalDialog(_) => "□",
        };
        ui.horizontal(|ui| {
            if ui
                .selectable_label(
                    surface.id == active_surface,
                    format!("{icon} {}", surface.name),
                )
                .on_hover_text(&surface.props.title)
                .clicked()
            {
                action = SurfaceAction::Activate(surface.id);
            }
            if is_root {
                ui.label(egui::RichText::new("root").small().weak());
                return;
            }
            if ui.small_button("↑").on_hover_text("Move up").clicked() {
                action = SurfaceAction::MoveUp(surface.id);
            }
            if ui.small_button("↓").on_hover_text("Move down").clicked() {
                action = SurfaceAction::MoveDown(surface.id);
            }
            if ui
                .small_button("⧉")
                .on_hover_text("Duplicate surface")
                .clicked()
            {
                action = SurfaceAction::Duplicate(surface.id);
            }
            if ui
                .small_button("×")
                .on_hover_text("Delete surface")
                .clicked()
            {
                action = SurfaceAction::Delete(surface.id);
            }
        });
    }
    action
}

pub fn show_tabs(
    ui: &mut egui::Ui,
    document: &ProjectDocument,
    active_surface: Uuid,
) -> Option<Uuid> {
    let mut requested = None;
    egui::ScrollArea::horizontal()
        .id_salt("surface_tabs")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for surface in &document.surfaces {
                    let marker = match surface.kind {
                        SurfaceKind::MainWindow => "Main",
                        SurfaceKind::ModalDialog(_) => "Dialog",
                    };
                    if ui
                        .selectable_label(
                            surface.id == active_surface,
                            format!("{} · {marker}", surface.name),
                        )
                        .clicked()
                    {
                        requested = Some(surface.id);
                    }
                }
            });
        });
    requested
}

pub fn show_active_surface_properties(ui: &mut egui::Ui, active: &mut ActiveDocument) {
    let active_id = active.active_surface_id();
    let mut surface_name = active.active_surface().name.clone();
    ui.label(egui::RichText::new("Surface").strong());
    ui.horizontal(|ui| {
        ui.label("Name");
        if ui.text_edit_singleline(&mut surface_name).changed() {
            let _ = active.rename_surface(active_id, &surface_name);
        }
    });
    ui.separator();

    egui::Grid::new(("surface_props", active_id))
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Title");
            ui.text_edit_singleline(&mut active.app_props.title);
            ui.end_row();
            ui.label("Width");
            ui.add(
                egui::DragValue::new(&mut active.app_props.win_w)
                    .range(100.0..=16_384.0)
                    .speed(1.0),
            );
            ui.end_row();
            ui.label("Height");
            ui.add(
                egui::DragValue::new(&mut active.app_props.win_h)
                    .range(100.0..=16_384.0)
                    .speed(1.0),
            );
            ui.end_row();
            ui.label("Resizable");
            ui.checkbox(&mut active.app_props.resizable, "");
            ui.end_row();
        });

    let SurfaceKind::ModalDialog(mut modal) = active.active_surface().kind.clone() else {
        ui.label(
            egui::RichText::new("Main application surface")
                .small()
                .weak(),
        );
        return;
    };
    ui.separator();
    ui.label(egui::RichText::new("Modal Policy").strong());
    let mut changed = false;
    changed |= ui
        .checkbox(&mut modal.reject_on_escape, "Escape rejects")
        .changed();
    changed |= ui
        .checkbox(&mut modal.close_on_backdrop, "Backdrop click closes")
        .changed();

    let buttons: Vec<(Uuid, String)> = active
        .widgets
        .iter()
        .filter(|widget| {
            matches!(
                widget.kind,
                WidgetKind::Button
                    | WidgetKind::ToolButton
                    | WidgetKind::CommandLinkButton
                    | WidgetKind::DialogButtonBox
            )
        })
        .map(|widget| (widget.id, widget.props.label.clone()))
        .collect();
    changed |= button_role_picker(
        ui,
        "Default button",
        &buttons,
        &mut modal.default_button,
        ("surface_default_button", active_id),
    );
    changed |= button_role_picker(
        ui,
        "Reject button",
        &buttons,
        &mut modal.reject_button,
        ("surface_reject_button", active_id),
    );
    if changed {
        let _ = active.set_modal_dialog_props(modal);
    }
}

fn button_role_picker(
    ui: &mut egui::Ui,
    label: &str,
    buttons: &[(Uuid, String)],
    selected: &mut Option<Uuid>,
    id: impl std::hash::Hash,
) -> bool {
    let before = *selected;
    let selected_label = selected
        .and_then(|id| buttons.iter().find(|(button_id, _)| *button_id == id))
        .map(|(_, label)| label.as_str())
        .unwrap_or("None");
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(id)
            .selected_text(selected_label)
            .show_ui(ui, |ui| {
                ui.selectable_value(selected, None, "None");
                for (button_id, button_label) in buttons {
                    ui.selectable_value(selected, Some(*button_id), button_label);
                }
            });
    });
    before != *selected
}

#[must_use]
pub fn dialog_template_tree(template: DialogTemplate) -> UiTree {
    let mut tree = UiTree::default();
    match template {
        DialogTemplate::Blank => {}
        DialogTemplate::OkCancel => tree.add(dialog_button_box(220.0)),
        DialogTemplate::Settings => {
            tree.add(WidgetInstance {
                kind: WidgetKind::Label,
                rect: Rect {
                    x: 28.0,
                    y: 32.0,
                    w: 100.0,
                    h: 28.0,
                },
                props: WidgetProps {
                    label: "Name".to_owned(),
                    ..Default::default()
                },
                ..Default::default()
            });
            tree.add(WidgetInstance {
                kind: WidgetKind::TextInput,
                rect: Rect {
                    x: 132.0,
                    y: 32.0,
                    w: 300.0,
                    h: 28.0,
                },
                state_binding: Some("name".to_owned()),
                props: WidgetProps {
                    placeholder: "Enter a name".to_owned(),
                    ..Default::default()
                },
                ..Default::default()
            });
            tree.add(dialog_button_box(220.0));
        }
    }
    tree
}

fn dialog_button_box(y: f32) -> WidgetInstance {
    WidgetInstance {
        kind: WidgetKind::DialogButtonBox,
        rect: Rect {
            x: 232.0,
            y,
            w: 200.0,
            h: 32.0,
        },
        props: WidgetProps {
            label: "Dialog actions".to_owned(),
            options: vec!["OK".to_owned(), "Cancel".to_owned()],
            ..Default::default()
        },
        ..Default::default()
    }
}

#[must_use]
pub fn modal_props(active: &ActiveDocument) -> Option<ModalDialogProps> {
    match &active.active_surface().kind {
        SurfaceKind::ModalDialog(props) => Some(props.clone()),
        SurfaceKind::MainWindow => None,
    }
}
