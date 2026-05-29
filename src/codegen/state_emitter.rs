use crate::codegen::field_collector;
use crate::project::ui_tree::UiTree;

/// Walk the UiTree and emit an AppState struct with the required fields.
pub fn emit(tree: &UiTree) -> String {
    let collected = field_collector::collect(tree);

    // Design-time component state fields (DataSource etc.)
    let component_pairs =
        crate::panels::component_tray::component_state_field_pairs(&tree.app_props.components);

    let mut out = String::from("// Generated AppState - do not edit manually\nstruct AppState {\n");
    for f in &collected.fields {
        out.push_str(&format!("    {}: {},\n", f.name, f.ty));
    }
    for (decl, _) in &component_pairs {
        out.push_str(decl);
        out.push('\n');
    }
    for w in &collected.warnings {
        out.push_str("    // ");
        out.push_str(w);
        out.push('\n');
    }
    out.push_str("}\n\n");
    out.push_str("impl Default for AppState {\n    fn default() -> Self {\n        Self {\n");
    for f in &collected.fields {
        out.push_str(&format!("            {}: {},\n", f.name, f.default_expr));
    }
    for (_, default_line) in &component_pairs {
        out.push_str(default_line);
        out.push('\n');
    }
    out.push_str("        }\n    }\n}\n");
    out
}
