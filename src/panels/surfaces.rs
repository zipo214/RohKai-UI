//! Project surfaces panel and reusable modal-dialog templates.

use crate::project::{
    document::{ActiveDocument, ModalDialogProps, ProjectDocument, SurfaceKind},
    schema::{
        Behavior, DialogButtonRole, DialogButtonSpec, Rect, SurfaceEvent, VisualAction,
        WidgetInstance, WidgetKind, WidgetProps, is_dialog_action_widget,
    },
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
    Preview(Uuid),
    WireSelectedToOpen(Uuid),
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
                if ui
                    .small_button("▶")
                    .on_hover_text("Preview this surface")
                    .clicked()
                {
                    action = SurfaceAction::Preview(surface.id);
                }
                return;
            }
            if ui
                .small_button("▶")
                .on_hover_text("Preview this surface in isolation")
                .clicked()
            {
                action = SurfaceAction::Preview(surface.id);
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
                .small_button("↗")
                .on_hover_text("Open this dialog from the selected widget")
                .clicked()
            {
                action = SurfaceAction::WireSelectedToOpen(surface.id);
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

pub fn show_active_surface_properties(
    ui: &mut egui::Ui,
    active: &mut ActiveDocument,
    selected_behavior: &mut Option<Uuid>,
) {
    let active_id = active.active_surface_id();
    let persisted_name = active.active_surface().name.clone();
    let name_draft_id = egui::Id::new(("surface_name_draft", active_id));
    let mut surface_name = ui
        .data(|data| data.get_temp::<String>(name_draft_id))
        .unwrap_or_else(|| persisted_name.clone());
    ui.label(egui::RichText::new("Surface").strong());
    let name_response = ui
        .horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut surface_name)
        })
        .inner;
    if name_response.changed() {
        ui.data_mut(|data| data.insert_temp(name_draft_id, surface_name.clone()));
    }
    let commit_name = name_response.lost_focus()
        || (name_response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
    if commit_name {
        let _ = active.rename_surface(active_id, &surface_name);
        surface_name = active.active_surface().name.clone();
        ui.data_mut(|data| data.insert_temp(name_draft_id, surface_name.clone()));
    } else if !name_response.has_focus() && surface_name != persisted_name {
        surface_name.clone_from(&persisted_name);
        ui.data_mut(|data| data.insert_temp(name_draft_id, surface_name.clone()));
    }
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
        .filter(|widget| is_dialog_action_widget(&widget.kind))
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

    ui.separator();
    ui.label(egui::RichText::new("Lifecycle Behaviors").strong());
    ui.label(
        egui::RichText::new("Run visual actions when this dialog opens or closes.")
            .small()
            .weak(),
    );
    for event in SurfaceEvent::ALL {
        let existing: Vec<(Uuid, String)> = active
            .app_props
            .behaviors
            .iter()
            .filter(|behavior| {
                behavior.source_surface() == Some(active_id)
                    && behavior.surface_event() == Some(event)
            })
            .map(|behavior| (behavior.id, behavior.action.label().to_owned()))
            .collect();
        ui.horizontal(|ui| {
            ui.label(event.label());
            for (behavior, action) in &existing {
                if ui
                    .selectable_label(*selected_behavior == Some(*behavior), action)
                    .clicked()
                {
                    *selected_behavior = Some(*behavior);
                }
            }
            if ui.small_button("+").on_hover_text("Add behavior").clicked() {
                let behavior = Behavior::surface(
                    Uuid::new_v4(),
                    active_id,
                    event,
                    VisualAction::CallHandler {
                        handler: lifecycle_handler_name(&surface_name, event),
                    },
                );
                *selected_behavior = Some(behavior.id);
                active.app_props.behaviors.push(behavior);
            }
        });
    }

    let diagnostics: Vec<_> = active
        .snapshot()
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.surface.is_none() || diagnostic.surface == Some(active_id))
        .collect();
    if !diagnostics.is_empty() {
        ui.separator();
        ui.label(
            egui::RichText::new("Surface Diagnostics")
                .strong()
                .color(egui::Color32::from_rgb(248, 113, 113)),
        );
        for diagnostic in diagnostics {
            ui.label(
                egui::RichText::new(format!("• {}", diagnostic.message))
                    .small()
                    .color(egui::Color32::from_rgb(248, 113, 113)),
            );
        }
    }
}

fn lifecycle_handler_name(surface_name: &str, event: SurfaceEvent) -> String {
    let surface = crate::codegen::rust::effective_binding(surface_name);
    let event = match event {
        SurfaceEvent::Opened => "opened",
        SurfaceEvent::Accepted => "accepted",
        SurfaceEvent::Rejected => "rejected",
        SurfaceEvent::Closed => "closed",
    };
    format!("{surface}_{event}")
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

#[must_use]
pub fn modal_props(active: &ActiveDocument) -> Option<ModalDialogProps> {
    match &active.active_surface().kind {
        SurfaceKind::ModalDialog(props) => Some(props.clone()),
        SurfaceKind::MainWindow => None,
    }
}
