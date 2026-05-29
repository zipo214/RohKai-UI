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
                .hint_text(default_handler_name(&comp.kind))
                .desired_width(120.0),
        );
    });

    let handler_name = if comp.handler.is_empty() {
        default_handler_name(&comp.kind).to_owned()
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
// Codegen helpers (used by egui_emitter / export)
// ---------------------------------------------------------------------------

/// Generate `(field_decl, default_assignment)` pairs for all components.
/// `field_decl` like `"    user_data: String,"`,
/// `default_assignment` like `"            user_data: String::new(),"`.
pub fn component_state_field_pairs(components: &[DesignComponent]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for comp in components {
        match comp.kind {
            ComponentKind::DataSource | ComponentKind::HttpRequest => {
                let name = sanitize_field_name(&comp.name);
                pairs.push((
                    format!("    {name}_data: String,"),
                    format!("            {name}_data: String::new(),"),
                ));
            }
            ComponentKind::StateMachine => {
                let name = sanitize_field_name(&comp.name);
                pairs.push((
                    format!("    {name}_state: usize,"),
                    format!("            {name}_state: 0,"),
                ));
            }
            ComponentKind::Timer | ComponentKind::Lifecycle => {}
        }
    }
    pairs
}

/// Generate update() body lines for all components.
pub fn component_update_lines(components: &[DesignComponent]) -> Vec<String> {
    let mut lines = Vec::new();
    for comp in components {
        let handler = if comp.handler.is_empty() {
            default_handler_name(&comp.kind).to_owned()
        } else {
            comp.handler.clone()
        };
        match comp.kind {
            ComponentKind::Timer => {
                let ms = comp.interval_ms.unwrap_or(1000);
                lines.push(format!(
                    "        // Timer '{}': fires every {ms}ms → self.{handler}()",
                    comp.name
                ));
            }
            ComponentKind::StateMachine => {
                lines.push(format!(
                    "        // StateMachine '{}': self.{handler}() drives state transitions",
                    comp.name
                ));
            }
            ComponentKind::HttpRequest => {
                lines.push(format!(
                    "        // HttpRequest '{}': call self.{handler}() to dispatch (use mpsc for async)",
                    comp.name
                ));
            }
            ComponentKind::DataSource | ComponentKind::Lifecycle => {}
        }
    }
    lines
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

fn default_handler_name(kind: &ComponentKind) -> &'static str {
    match kind {
        ComponentKind::Timer => "on_tick",
        ComponentKind::DataSource => "fetch_data",
        ComponentKind::Lifecycle => "on_startup",
        ComponentKind::StateMachine => "on_transition",
        ComponentKind::HttpRequest => "on_response",
    }
}

fn sanitize_field_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .to_lowercase()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::DesignComponent;

    fn make_timer(name: &str, handler: &str, interval_ms: u32) -> DesignComponent {
        DesignComponent {
            id: Uuid::new_v4(),
            kind: ComponentKind::Timer,
            name: name.to_owned(),
            interval_ms: Some(interval_ms),
            handler: handler.to_owned(),
        }
    }

    fn make_datasource(name: &str) -> DesignComponent {
        DesignComponent {
            id: Uuid::new_v4(),
            kind: ComponentKind::DataSource,
            name: name.to_owned(),
            interval_ms: None,
            handler: String::new(),
        }
    }

    #[test]
    fn timer_update_line_contains_handler() {
        let comp = make_timer("refresh", "on_refresh", 500);
        let lines = component_update_lines(&[comp]);
        assert!(lines.len() == 1);
        assert!(lines[0].contains("on_refresh"));
        assert!(lines[0].contains("500ms"));
    }

    #[test]
    fn datasource_emits_state_field() {
        let comp = make_datasource("user_list");
        let pairs = component_state_field_pairs(&[comp]);
        assert!(pairs.len() == 1);
        assert!(pairs[0].0.contains("user_list_data"));
        assert!(pairs[0].1.contains("String::new()"));
    }

    #[test]
    fn timer_emits_no_state_field() {
        let comp = make_timer("tick", "on_tick", 1000);
        let pairs = component_state_field_pairs(&[comp]);
        assert!(pairs.is_empty());
    }

    #[test]
    fn multiple_components_handled() {
        let components = vec![
            make_timer("clock", "on_clock", 1000),
            make_datasource("products"),
            DesignComponent {
                id: Uuid::new_v4(),
                kind: ComponentKind::Lifecycle,
                name: "app".to_owned(),
                interval_ms: None,
                handler: "on_startup".to_owned(),
            },
        ];
        let lines = component_update_lines(&components);
        assert_eq!(lines.len(), 1); // only Timer produces update lines
        let pairs = component_state_field_pairs(&components);
        assert_eq!(pairs.len(), 1); // only DataSource produces state fields
    }
}
