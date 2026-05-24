use crate::codegen::field_collector;
use crate::project::ui_tree::UiTree;

/// Walk the UiTree and emit an AppState struct with the required fields.
pub fn emit(tree: &UiTree) -> String {
    let collected = field_collector::collect(tree);

    let mut out = String::from("// Generated AppState - do not edit manually\nstruct AppState {\n");
    for f in &collected.fields {
        out.push_str(&format!("    {}: {},\n", f.name, f.ty));
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
    out.push_str("        }\n    }\n}\n");
    out
}
