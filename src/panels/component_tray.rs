//! Design-time non-visual component tray.
//!
//! Shows Timers, DataSources, and Lifecycle hooks as clickable icon-chips
//! in a horizontal strip.  Lives in the left panel below the outline.
//! Components are stored in `AppProps.components` (persisted in project file).

use crate::project::schema::{ComponentKind, DesignComponent, StateDef, TransitionDef};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Action
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum TrayAction {
    /// Open config panel for this component.
    Select(Uuid),
    /// Add a new component of the given kind.
    Add(ComponentKind),
    /// Remove this component.
    Remove(Uuid),
    None,
}

// ---------------------------------------------------------------------------
// show_tray — horizontal icon strip
// ---------------------------------------------------------------------------

/// Renders the component tray icon bar.  Returns a `TrayAction`.
pub fn show_tray(
    ui: &mut egui::Ui,
    components: &[DesignComponent],
    selected: Option<Uuid>,
) -> TrayAction {
    let mut action = TrayAction::None;

    ui.horizontal_wrapped(|ui| {
        // Existing components
        for comp in components {
            let is_sel = selected == Some(comp.id);
            let (icon, color) = component_icon(&comp.kind);
            let chip = format!("{icon} {}", comp.name);
            let btn = ui.add(
                egui::Button::new(egui::RichText::new(&chip).small().color(if is_sel {
                    color
                } else {
                    egui::Color32::from_gray(200)
                }))
                .fill(if is_sel {
                    egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 40)
                } else {
                    egui::Color32::from_gray(40)
                }),
            );
            if btn.clicked() {
                action = TrayAction::Select(comp.id);
            }
            if btn.secondary_clicked() {
                action = TrayAction::Remove(comp.id);
            }
            btn.on_hover_text(format!(
                "{} — {}\n{}\nRight-click to remove",
                component_kind_label(&comp.kind),
                comp.name,
                describe_kind(&comp.kind),
            ));
        }

        // Add buttons
        ui.separator();
        for kind in [
            ComponentKind::Timer,
            ComponentKind::DataSource,
            ComponentKind::Lifecycle,
            ComponentKind::StateMachine,
            ComponentKind::HttpRequest,
        ] {
            let (icon, _) = component_icon(&kind);
            let label = format!("+ {icon}");
            if ui
                .small_button(&label)
                .on_hover_text(format!(
                    "Add {} component\n{}",
                    component_kind_label(&kind),
                    describe_kind(&kind)
                ))
                .clicked()
            {
                action = TrayAction::Add(kind);
            }
        }
    });

    action
}

// ---------------------------------------------------------------------------
// show_config — inline editor for selected component
// ---------------------------------------------------------------------------

/// Renders the config editor for the selected component.
///
/// Organised into three sections:
///  1. Kind header + runtime status badge
///  2. Identity (name) + kind-specific fields (Timer interval)
///  3. Handler + generated code preview
pub fn show_config(ui: &mut egui::Ui, comp: &mut DesignComponent) {
    let (icon, color) = component_icon(&comp.kind);

    // --- Section 1: kind header + design-time status badge ---
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{icon} {}", component_kind_label(&comp.kind)))
                .small()
                .strong()
                .color(color),
        );
        ui.label(
            egui::RichText::new("design-time stub")
                .small()
                .color(egui::Color32::from_rgb(156, 163, 175))
                .italics(),
        )
        .on_hover_text(describe_kind(&comp.kind));
    });
    ui.label(
        egui::RichText::new(describe_kind(&comp.kind))
            .small()
            .weak(),
    );

    ui.separator();

    // --- Section 2: Identity + kind-specific ---
    ui.label(egui::RichText::new("Identity").small().strong());
    egui::Grid::new(("comp_identity", comp.id))
        .num_columns(2)
        .spacing([4.0, 2.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Name").small().weak());
            ui.add(
                egui::TextEdit::singleline(&mut comp.name)
                    .hint_text("my_component")
                    .desired_width(130.0),
            );
            ui.end_row();

            if comp.kind == ComponentKind::Timer {
                ui.label(egui::RichText::new("Interval").small().weak());
                let mut ms = comp.interval_ms.unwrap_or(1000);
                if ui
                    .add(
                        egui::DragValue::new(&mut ms)
                            .speed(10.0)
                            .range(16..=60_000)
                            .suffix(" ms"),
                    )
                    .changed()
                {
                    comp.interval_ms = Some(ms);
                }
                ui.end_row();
            }
        });

    ui.separator();

    // --- Section 3: Handler ---
    ui.label(egui::RichText::new("Handler").small().strong());
    let default_handler = crate::codegen::component_state::default_handler_name(&comp.kind);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("fn").small().weak().monospace());
        ui.add(
            egui::TextEdit::singleline(&mut comp.handler)
                .hint_text(default_handler)
                .desired_width(120.0),
        );
        ui.label(
            egui::RichText::new("(&mut self)")
                .small()
                .weak()
                .monospace(),
        );
    });

    let handler_name = if comp.handler.is_empty() {
        default_handler.to_owned()
    } else {
        comp.handler.clone()
    };

    // --- Section 4 (StateMachine only): states + transitions table ----------
    if comp.kind == ComponentKind::StateMachine {
        ui.separator();
        show_state_machine_editor(ui, comp);
    }

    // Generated code preview
    ui.separator();
    ui.label(egui::RichText::new("Generated").small().strong());

    // State field (if any)
    let pairs =
        crate::codegen::component_state::component_state_field_pairs(std::slice::from_ref(comp));
    if !pairs.is_empty() {
        for (decl, _) in &pairs {
            ui.label(
                egui::RichText::new(format!("field: {}", decl.trim()))
                    .monospace()
                    .small()
                    .color(egui::Color32::from_rgb(96, 165, 250)),
            );
        }
    }

    // Update body comment
    let update_lines =
        crate::codegen::component_state::component_update_lines(std::slice::from_ref(comp));
    if !update_lines.is_empty() {
        for line in &update_lines {
            ui.label(
                egui::RichText::new(line.trim())
                    .monospace()
                    .small()
                    .color(egui::Color32::from_rgb(52, 211, 153)),
            );
        }
    } else {
        // Lifecycle / DataSource — show what handler stub will look like
        ui.label(
            egui::RichText::new(format!("→ fn {handler_name}(&mut self)"))
                .monospace()
                .small()
                .color(egui::Color32::from_rgb(52, 211, 153)),
        );
    }
}

// ---------------------------------------------------------------------------
// State machine editor (table UI for states + transitions)
// ---------------------------------------------------------------------------

fn show_state_machine_editor(ui: &mut egui::Ui, comp: &mut DesignComponent) {
    let sm = &mut comp.state_machine;

    // --- States section ---
    ui.label(egui::RichText::new("States").small().strong());

    let mut remove_state: Option<usize> = None;
    egui::Grid::new(("sm_states", comp.id))
        .num_columns(4)
        .spacing([4.0, 2.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Name").small().weak());
            ui.label(egui::RichText::new("Entry action").small().weak());
            ui.label(egui::RichText::new("Exit action").small().weak());
            ui.label(""); // remove button column
            ui.end_row();

            for (i, state) in sm.states.iter_mut().enumerate() {
                ui.add(
                    egui::TextEdit::singleline(&mut state.name)
                        .hint_text("name")
                        .desired_width(60.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut state.entry_action)
                        .hint_text("on_enter()")
                        .desired_width(80.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut state.exit_action)
                        .hint_text("on_exit()")
                        .desired_width(80.0),
                );
                if ui.small_button("✕").on_hover_text("Remove state").clicked() {
                    remove_state = Some(i);
                }
                ui.end_row();
            }
        });

    if let Some(i) = remove_state {
        // Removing a state must also drop dangling references to it, or the
        // state machine persists an invalid graph (orphan initial_state /
        // transitions).
        let removed_name = sm.states.remove(i).name;
        if sm.initial_state == removed_name {
            sm.initial_state = sm
                .states
                .first()
                .map(|s| s.name.clone())
                .unwrap_or_default();
        }
        sm.transitions
            .retain(|tr| tr.from != removed_name && tr.to != removed_name);
    }

    if ui.small_button("+ Add state").clicked() {
        sm.states.push(StateDef::default());
    }

    // Initial state selector
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Initial state").small().weak());
        egui::ComboBox::from_id_salt(("sm_initial", comp.id))
            .selected_text(if sm.initial_state.is_empty() {
                "(none)".to_owned()
            } else {
                sm.initial_state.clone()
            })
            .show_ui(ui, |ui| {
                let names: Vec<String> = sm.states.iter().map(|s| s.name.clone()).collect();
                for name in &names {
                    ui.selectable_value(&mut sm.initial_state, name.clone(), name);
                }
            });
    });

    ui.add_space(4.0);

    // --- Transitions section ---
    ui.label(egui::RichText::new("Transitions").small().strong());

    let mut remove_trans: Option<usize> = None;
    egui::Grid::new(("sm_transitions", comp.id))
        .num_columns(5)
        .spacing([4.0, 2.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("From").small().weak());
            ui.label(egui::RichText::new("To").small().weak());
            ui.label(egui::RichText::new("Guard").small().weak());
            ui.label(egui::RichText::new("Action").small().weak());
            ui.label(""); // remove button
            ui.end_row();

            for (i, tr) in sm.transitions.iter_mut().enumerate() {
                ui.add(
                    egui::TextEdit::singleline(&mut tr.from)
                        .hint_text("from")
                        .desired_width(55.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut tr.to)
                        .hint_text("to")
                        .desired_width(55.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut tr.guard)
                        .hint_text("bool expr")
                        .desired_width(75.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut tr.action)
                        .hint_text("code")
                        .desired_width(75.0),
                );
                if ui
                    .small_button("✕")
                    .on_hover_text("Remove transition")
                    .clicked()
                {
                    remove_trans = Some(i);
                }
                ui.end_row();
            }
        });

    if let Some(i) = remove_trans {
        sm.transitions.remove(i);
    }

    if ui.small_button("+ Add transition").clicked() {
        sm.transitions.push(TransitionDef::default());
    }
}

/// Returns a one-sentence description of what a component kind does (and does not) at runtime.
pub fn describe_kind(kind: &ComponentKind) -> &'static str {
    match kind {
        ComponentKind::Timer => {
            "Spawns a background thread on startup that sends a tick every interval_ms \
             via std::sync::mpsc. The designer drains the channel each frame and calls \
             ctx.request_repaint_after so timer-driven UIs stay live."
        }
        ComponentKind::DataSource => {
            "Generates a String AppState field for fetched data. \
             Actual network/file fetching requires an HTTP crate (user-approved at Stage 13)."
        }
        ComponentKind::Lifecycle => {
            "Generates an on_startup / on_shutdown handler stub. \
             Wire the stub to eframe App lifecycle hooks in the exported project."
        }
        ComponentKind::StateMachine => {
            "Defines named states, entry/exit actions, and guarded transitions. \
             Generates a current_state field in AppState + a transition handler stub."
        }
        ComponentKind::HttpRequest => {
            "Generates a response String AppState field + a handler stub comment. \
             Actual HTTP requires a user-approved crate (reqwest / ureq) at Stage 13."
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn component_icon(kind: &ComponentKind) -> (&'static str, egui::Color32) {
    match kind {
        ComponentKind::Timer => ("⏱", egui::Color32::from_rgb(251, 191, 36)),
        ComponentKind::DataSource => ("🗄", egui::Color32::from_rgb(96, 165, 250)),
        ComponentKind::Lifecycle => ("⚙", egui::Color32::from_rgb(167, 139, 250)),
        ComponentKind::StateMachine => ("⮌", egui::Color32::from_rgb(52, 211, 153)),
        ComponentKind::HttpRequest => ("🌐", egui::Color32::from_rgb(96, 200, 220)),
    }
}

fn component_kind_label(kind: &ComponentKind) -> &'static str {
    match kind {
        ComponentKind::Timer => "Timer",
        ComponentKind::DataSource => "Data Source",
        ComponentKind::Lifecycle => "Lifecycle",
        ComponentKind::StateMachine => "State Machine",
        ComponentKind::HttpRequest => "HTTP Request",
    }
}
