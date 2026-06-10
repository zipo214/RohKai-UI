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
                "{} — {}\nRight-click to remove",
                component_kind_label(&comp.kind),
                comp.name
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
                .on_hover_text(format!("Add {} component", component_kind_label(&kind)))
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
pub fn show_config(ui: &mut egui::Ui, comp: &mut DesignComponent) {
    let (icon, color) = component_icon(&comp.kind);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{icon} {}", component_kind_label(&comp.kind)))
                .small()
                .strong()
                .color(color),
        );
    });

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Name").small().weak());
        ui.add(
            egui::TextEdit::singleline(&mut comp.name)
                .hint_text("my_timer")
                .desired_width(100.0),
        );
    });

    if comp.kind == ComponentKind::Timer {
        ui.horizontal(|ui| {
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
        });
    }

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Handler").small().weak());
        ui.add(
            egui::TextEdit::singleline(&mut comp.handler)
                .hint_text(crate::codegen::component_state::default_handler_name(
                    &comp.kind,
                ))
                .desired_width(120.0),
        );
    });

    let handler_name = if comp.handler.is_empty() {
        crate::codegen::component_state::default_handler_name(&comp.kind).to_owned()
    } else {
        comp.handler.clone()
    };
    ui.label(
        egui::RichText::new(format!("→ fn {}(&mut self)", handler_name))
            .monospace()
            .small()
            .color(egui::Color32::from_rgb(52, 211, 153)),
    );
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
