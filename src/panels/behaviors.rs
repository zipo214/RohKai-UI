//! Behaviors panel — edit the visual behavior graph (event → action wires).
//!
//! Shown inside the Properties tab.  All edits mutate
//! `tree.app_props.behaviors` directly: the UiTree stays the single source of
//! truth and the live code panel / export pick the change up on the next emit.

use crate::project::schema::{ValueExpr, VisualAction, WidgetEvent};
use crate::project::ui_tree::UiTree;
use uuid::Uuid;

/// Constructor used by the action-type picker to swap variants while keeping
/// the current state field.
type ActionConstructor = fn(String) -> VisualAction;

const TEAL: egui::Color32 = egui::Color32::from_rgb(52, 211, 153);

/// Editor for the behavior selected on the canvas (wire click or fresh drop).
/// Returns `true` when it rendered something.
pub fn show_selected(
    ui: &mut egui::Ui,
    tree: &mut UiTree,
    selected_behavior: &mut Option<Uuid>,
) -> bool {
    let Some(bid) = *selected_behavior else {
        return false;
    };
    let Some(idx) = tree.app_props.behaviors.iter().position(|b| b.id == bid) else {
        *selected_behavior = None;
        return false;
    };

    ui.label(egui::RichText::new("Behavior").strong().color(TEAL));

    // Source line + event picker (events derive from the source widget's
    // canonical supported_events — never re-listed).
    let source_id = tree.app_props.behaviors[idx].source_widget;
    let source_desc = tree
        .widgets
        .iter()
        .find(|w| w.id == source_id)
        .map(|w| format!("{:?} \u{201c}{}\u{201d}", w.kind, w.props.label))
        .unwrap_or_else(|| "(missing widget)".to_owned());
    ui.horizontal(|ui| {
        ui.label("Source:");
        ui.label(egui::RichText::new(source_desc).weak());
    });
    let source_events: Vec<WidgetEvent> = tree
        .widgets
        .iter()
        .find(|w| w.id == source_id)
        .map(|w| w.kind.supported_events().to_vec())
        .unwrap_or_default();

    // Recipe suggestions for this source→sink pair (the smart constructor).
    // Derived live from the recorded target widget's sink type via the
    // interaction matrix; the default was already applied on drop, alternates
    // are one click, and params stay editable below.  Event does not narrow the
    // set today, so computing it from the pre-frame event is exact.
    let target_id = tree.app_props.behaviors[idx].target_widget;
    let event_now = tree.app_props.behaviors[idx].event;
    let suggestions: Vec<crate::codegen::behavior_recipes::RecipeSuggestion> = target_id
        .and_then(|tid| tree.widgets.iter().find(|w| w.id == tid))
        .and_then(crate::codegen::behavior_recipes::sink_info_for)
        .map(|sink| crate::codegen::behavior_recipes::suggestions_for(event_now, &sink))
        .unwrap_or_default();

    {
        let b = &mut tree.app_props.behaviors[idx];
        ui.horizontal(|ui| {
            ui.label("Event:");
            egui::ComboBox::from_id_salt(("behavior_event", bid))
                .selected_text(b.event.label())
                .show_ui(ui, |ui| {
                    for ev in &source_events {
                        ui.selectable_value(&mut b.event, *ev, ev.label());
                    }
                });
        });

        // Suggested recipes (beginner-facing): one click swaps in a typed
        // action.  The raw Action picker below stays as the advanced escape.
        if !suggestions.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label("Suggested:");
                for s in &suggestions {
                    if ui.selectable_label(b.action == s.action, s.label).clicked() {
                        b.action = s.action.clone();
                    }
                }
            });
        }

        // Action type picker — switching preserves the field where possible.
        let current_field = b.action.field().unwrap_or("").to_owned();
        ui.horizontal(|ui| {
            ui.label("Action:");
            egui::ComboBox::from_id_salt(("behavior_action", bid))
                .selected_text(b.action.label())
                .show_ui(ui, |ui| {
                    let choices: [(&str, ActionConstructor); 5] = [
                        ("Set", |f| VisualAction::Set {
                            field: f,
                            value: ValueExpr::Number(0.0),
                        }),
                        ("Add", |f| VisualAction::Add {
                            field: f,
                            amount: 0.1,
                            min: Some(0.0),
                            max: Some(1.0),
                        }),
                        ("Subtract", |f| VisualAction::Subtract {
                            field: f,
                            amount: 0.1,
                            min: Some(0.0),
                            max: Some(1.0),
                        }),
                        ("Toggle", |f| VisualAction::Toggle { field: f }),
                        ("Call handler", |_| VisualAction::CallHandler {
                            handler: String::new(),
                        }),
                    ];
                    for (label, make) in choices {
                        if ui
                            .selectable_label(b.action.label() == label, label)
                            .clicked()
                        {
                            b.action = make(current_field.clone());
                        }
                    }
                });
        });

        show_action_editor(ui, &mut b.action);
    }

    if ui
        .button(
            egui::RichText::new("Delete behavior").color(egui::Color32::from_rgb(248, 113, 113)),
        )
        .clicked()
    {
        tree.app_props.behaviors.remove(idx);
        *selected_behavior = None;
    }
    ui.separator();
    true
}

/// Compact list of behaviors whose source is `widget_id`; clicking one selects
/// its wire for editing.
pub fn show_for_widget(
    ui: &mut egui::Ui,
    tree: &UiTree,
    widget_id: Uuid,
    selected_behavior: &mut Option<Uuid>,
) {
    let wired: Vec<(Uuid, String)> = tree
        .app_props
        .behaviors
        .iter()
        .filter(|b| b.source_widget == widget_id)
        .map(|b| {
            let target = b
                .action
                .field()
                .map(str::to_owned)
                .or_else(|| match &b.action {
                    VisualAction::CallHandler { handler } => Some(handler.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            (
                b.id,
                format!("{} → {} {}", b.event.label(), b.action.label(), target),
            )
        })
        .collect();
    if wired.is_empty() {
        return;
    }
    ui.separator();
    ui.label(egui::RichText::new("Behaviors").strong().color(TEAL));
    ui.label(
        egui::RichText::new("Drag from a widget's right socket to a bound widget to add one.")
            .small()
            .weak(),
    );
    for (bid, label) in wired {
        if ui
            .selectable_label(*selected_behavior == Some(bid), label)
            .clicked()
        {
            *selected_behavior = Some(bid);
        }
    }
}

fn show_action_editor(ui: &mut egui::Ui, action: &mut VisualAction) {
    match action {
        VisualAction::Set { field, value } => {
            field_row(ui, field);
            ui.horizontal(|ui| {
                ui.label("Type:");
                let current = match value {
                    ValueExpr::Number(_) => "Number",
                    ValueExpr::Text(_) => "Text",
                    ValueExpr::Flag(_) => "Bool",
                };
                egui::ComboBox::from_id_salt("behavior_value_type")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(current == "Number", "Number").clicked() {
                            *value = ValueExpr::Number(0.0);
                        }
                        if ui.selectable_label(current == "Text", "Text").clicked() {
                            *value = ValueExpr::Text(String::new());
                        }
                        if ui.selectable_label(current == "Bool", "Bool").clicked() {
                            *value = ValueExpr::Flag(false);
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Value:");
                match value {
                    ValueExpr::Number(n) => {
                        ui.add(egui::DragValue::new(n).speed(0.05));
                    }
                    ValueExpr::Text(t) => {
                        ui.text_edit_singleline(t);
                    }
                    ValueExpr::Flag(b) => {
                        ui.checkbox(b, "");
                    }
                }
            });
        }
        VisualAction::Add {
            field,
            amount,
            min,
            max,
        }
        | VisualAction::Subtract {
            field,
            amount,
            min,
            max,
        } => {
            field_row(ui, field);
            ui.horizontal(|ui| {
                ui.label("Amount:");
                ui.add(egui::DragValue::new(amount).speed(0.01));
            });
            ui.horizontal(|ui| {
                let mut clamped = min.is_some() || max.is_some();
                if ui.checkbox(&mut clamped, "Clamp").changed() {
                    if clamped {
                        *min = Some(0.0);
                        *max = Some(1.0);
                    } else {
                        *min = None;
                        *max = None;
                    }
                }
                if let (Some(lo), Some(hi)) = (min.as_mut(), max.as_mut()) {
                    ui.add(egui::DragValue::new(lo).speed(0.05).prefix("min "));
                    ui.add(egui::DragValue::new(hi).speed(0.05).prefix("max "));
                }
            });
        }
        VisualAction::Toggle { field } => field_row(ui, field),
        VisualAction::CallHandler { handler } => {
            ui.horizontal(|ui| {
                ui.label("Handler:");
                ui.text_edit_singleline(handler);
            });
        }
    }
}

fn field_row(ui: &mut egui::Ui, field: &mut String) {
    ui.horizontal(|ui| {
        ui.label("State field:");
        ui.text_edit_singleline(field);
    });
}
