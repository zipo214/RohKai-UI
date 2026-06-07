//! Codegen for design-time components (Timer / DataSource / StateMachine / …).
//!
//! Generates the `AppState` field declarations + defaults and the `update()`
//! body lines for components placed in the component tray. Lives in `codegen`
//! per the project rule that only this module emits Rust syntax strings; the
//! `panels::component_tray` UI calls into here rather than building syntax.

use crate::project::schema::{ComponentKind, DesignComponent};

/// Default handler name for a component kind (also used by the tray UI hint).
pub fn default_handler_name(kind: &ComponentKind) -> &'static str {
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

/// Generate `update()` body lines for all components.
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
                    "        // Timer '{}': design-time MVP; add runtime clock dispatch every {ms}ms to call self.{handler}()",
                    comp.name
                ));
            }
            ComponentKind::StateMachine => {
                lines.push(format!(
                    "        // StateMachine '{}': design-time MVP; implement self.{handler}() to change the generated state field",
                    comp.name
                ));
            }
            ComponentKind::HttpRequest => {
                lines.push(format!(
                    "        // HttpRequest '{}': design-time MVP; wire self.{handler}() to std::sync::mpsc or a user-approved HTTP crate",
                    comp.name
                ));
            }
            ComponentKind::DataSource | ComponentKind::Lifecycle => {}
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::DesignComponent;
    use uuid::Uuid;

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
        assert!(lines[0].contains("design-time MVP"));
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

    #[test]
    fn state_machine_update_line_is_documented_stub() {
        let comp = DesignComponent {
            id: Uuid::new_v4(),
            kind: ComponentKind::StateMachine,
            name: "flow".to_owned(),
            interval_ms: None,
            handler: "advance".to_owned(),
        };
        let lines = component_update_lines(&[comp]);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("StateMachine"));
        assert!(lines[0].contains("design-time MVP"));
        assert!(lines[0].contains("advance"));
    }

    #[test]
    fn http_request_update_line_is_documented_stub() {
        let comp = DesignComponent {
            id: Uuid::new_v4(),
            kind: ComponentKind::HttpRequest,
            name: "api".to_owned(),
            interval_ms: None,
            handler: "fetch_api".to_owned(),
        };
        let lines = component_update_lines(&[comp]);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("HttpRequest"));
        assert!(lines[0].contains("design-time MVP"));
        assert!(lines[0].contains("fetch_api"));
    }
}
