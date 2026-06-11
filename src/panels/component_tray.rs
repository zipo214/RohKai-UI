//! Design-time non-visual component tray.
//!
//! Shows Timers, DataSources, and Lifecycle hooks as clickable icon-chips
//! in a horizontal strip.  Lives in the left panel below the outline.
//! Components are stored in `AppProps.components` (persisted in project file).

use crate::project::schema::{ComponentKind, DesignComponent};
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

/// Returns a one-sentence description of what a component kind does (and does not) at runtime.
pub fn describe_kind(kind: &ComponentKind) -> &'static str {
    match kind {
        ComponentKind::Timer => {
            "Generates a handler stub comment in update(). \
             Real tick scheduling requires a Rust clock + mpsc channel (not yet wired)."
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
            "Generates a usize AppState field for current state + a transition handler stub. \
             Transition logic and visual editor are deferred to Stage 13."
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
