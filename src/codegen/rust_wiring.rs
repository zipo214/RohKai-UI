//! Stage 11 — Rust-centric wiring codegen.
//!
//! Pure functions that turn `AppProps.rust_wiring` and per-widget handler
//! annotations into Rust source: mpsc channel fields, iterator-pipeline
//! methods, trait-impl blocks, async handler-call wrappers, and handler
//! signatures with error-handling contracts.
//!
//! Everything here is std-only — no tokio, no new crates.  Async uses
//! `std::thread::spawn` + `std::sync::mpsc`, the project's approved
//! background-task pattern.

use crate::project::schema::{HandlerResult, IterOp, RustWiring};

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

/// `(field_decl, init_line)` pairs for each declared channel, e.g.
/// `("progress_tx: std::sync::mpsc::Sender<f32>", "let (progress_tx, progress_rx) = std::sync::mpsc::channel();")`.
pub fn channel_field_pairs(wiring: &RustWiring) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for ch in &wiring.channels {
        let name = sanitize(&ch.name);
        let ty = ch.ty.trim();
        out.push((
            format!("    {name}_tx: std::sync::mpsc::Sender<{ty}>,"),
            format!("        let ({name}_tx, {name}_rx) = std::sync::mpsc::channel::<{ty}>();"),
        ));
        out.push((
            format!("    {name}_rx: std::sync::mpsc::Receiver<{ty}>,"),
            String::new(),
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// Iterator pipelines
// ---------------------------------------------------------------------------

/// One method per pipeline:
/// `fn name(&self) -> Vec<_> { source.iter().map(...).filter(...).collect() }`.
pub fn iterator_methods(wiring: &RustWiring, indent: &str) -> String {
    let mut s = String::new();
    for p in &wiring.iterators {
        let name = sanitize(&p.name);
        let mut chain = format!("{}.iter()", p.source.trim());
        for op in &p.ops {
            match op {
                IterOp::Map(expr) => chain.push_str(&format!(".map(|x| {})", expr.trim())),
                IterOp::Filter(expr) => chain.push_str(&format!(".filter(|x| {})", expr.trim())),
            }
        }
        chain.push_str(".collect()");
        s.push_str(&format!(
            "{indent}#[allow(dead_code)]\n{indent}fn {name}(&self) -> Vec<_> {{\n{indent}    {chain}\n{indent}}}\n"
        ));
    }
    s
}

// ---------------------------------------------------------------------------
// Trait impls
// ---------------------------------------------------------------------------

/// `impl Trait for ExportedApp { method { body } }` blocks.
pub fn trait_impl_blocks(wiring: &RustWiring) -> String {
    let mut s = String::new();
    for t in &wiring.trait_impls {
        let tr = t.trait_name.trim();
        let method = t.method.trim();
        let body = t.body.trim();
        if tr.is_empty() || method.is_empty() {
            continue;
        }
        s.push_str(&format!(
            "\nimpl {tr} for ExportedApp {{\n    {method} {{\n        {body}\n    }}\n}}\n"
        ));
    }
    s
}

// ---------------------------------------------------------------------------
// Handler signature + call site
// ---------------------------------------------------------------------------

/// Signature for a handler stub, e.g. `fn on_save(&mut self) -> Result<(), String>`.
pub fn handler_signature(handler: &str, result: &HandlerResult) -> String {
    format!("fn {handler}(&mut self){}", result.return_suffix())
}

/// The body line(s) of a generated handler stub for a given result mode.
pub fn handler_stub_body(result: &HandlerResult, handler: &str) -> String {
    match result {
        HandlerResult::Plain => format!("        // TODO: implement {handler}"),
        HandlerResult::Result => {
            format!("        // TODO: implement {handler}\n        Ok(())")
        }
        HandlerResult::Option => {
            format!("        // TODO: implement {handler}\n        Some(())")
        }
    }
}

/// The call-site expression invoking `self.{handler}()`, honoring async + result.
///
/// `indent` is the leading whitespace for continuation lines.
/// - Plain sync:        `self.h();`
/// - Result sync:       `if let Err(e) = self.h() { eprintln!("{e}"); }`
/// - Option sync:       `let _ = self.h();`
/// - async (any):       `{ /* spawn */ std::thread::spawn(move || { /* self.h() — move owned state */ }); }`
///   (async note: the closure can't borrow `self`; we emit a guidance comment so
///   the generated code compiles and the user moves the needed data in.)
pub fn handler_call(
    handler: &str,
    async_handler: bool,
    result: &HandlerResult,
    indent: &str,
) -> String {
    if async_handler {
        return format!(
            "{indent}// '{handler}' runs on a background thread; move owned data into the closure.\n\
             {indent}std::thread::spawn(move || {{\n\
             {indent}    // TODO: background work for {handler}; report via an mpsc Sender.\n\
             {indent}}});"
        );
    }
    match result {
        HandlerResult::Plain => format!("{indent}self.{handler}();"),
        HandlerResult::Result => {
            format!("{indent}if let Err(e) = self.{handler}() {{\n{indent}    eprintln!(\"{handler}: {{e}}\");\n{indent}}}")
        }
        HandlerResult::Option => format!("{indent}let _ = self.{handler}();"),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let s = s.to_lowercase();
    if s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(true) {
        format!("c_{s}")
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{ChannelDef, IteratorPipeline, TraitImpl};
    use uuid::Uuid;

    fn wiring() -> RustWiring {
        RustWiring {
            channels: vec![ChannelDef {
                id: Uuid::nil(),
                name: "progress".to_owned(),
                ty: "f32".to_owned(),
            }],
            iterators: vec![IteratorPipeline {
                id: Uuid::nil(),
                name: "active".to_owned(),
                source: "self.state.items".to_owned(),
                ops: vec![
                    IterOp::Filter("x.active".to_owned()),
                    IterOp::Map("x.id".to_owned()),
                ],
            }],
            trait_impls: vec![TraitImpl {
                id: Uuid::nil(),
                trait_name: "MyBehavior".to_owned(),
                method: "fn run(&mut self)".to_owned(),
                body: "self.state.volume = 1.0;".to_owned(),
            }],
        }
    }

    #[test]
    fn channel_fields_emit_sender_receiver() {
        let pairs = channel_field_pairs(&wiring());
        let decls: Vec<&str> = pairs.iter().map(|(d, _)| d.as_str()).collect();
        assert!(decls
            .iter()
            .any(|d| d.contains("progress_tx: std::sync::mpsc::Sender<f32>")));
        assert!(decls
            .iter()
            .any(|d| d.contains("progress_rx: std::sync::mpsc::Receiver<f32>")));
        let inits: Vec<&str> = pairs.iter().map(|(_, i)| i.as_str()).collect();
        assert!(inits.iter().any(|i| i.contains("mpsc::channel::<f32>()")));
    }

    #[test]
    fn iterator_method_chains_ops_in_order() {
        let s = iterator_methods(&wiring(), "    ");
        assert!(s.contains("fn active(&self) -> Vec<_>"));
        // filter then map, in declared order
        let f = s.find(".filter(|x| x.active)").unwrap();
        let m = s.find(".map(|x| x.id)").unwrap();
        assert!(f < m, "ops must chain in declared order");
        assert!(s.contains(".collect()"));
    }

    #[test]
    fn trait_impl_block_well_formed() {
        let s = trait_impl_blocks(&wiring());
        assert!(s.contains("impl MyBehavior for ExportedApp"));
        assert!(s.contains("fn run(&mut self)"));
        assert!(s.contains("self.state.volume = 1.0;"));
    }

    #[test]
    fn handler_signature_reflects_result_mode() {
        assert_eq!(
            handler_signature("h", &HandlerResult::Plain),
            "fn h(&mut self)"
        );
        assert_eq!(
            handler_signature("h", &HandlerResult::Result),
            "fn h(&mut self) -> Result<(), String>"
        );
        assert_eq!(
            handler_signature("h", &HandlerResult::Option),
            "fn h(&mut self) -> Option<()>"
        );
    }

    #[test]
    fn handler_call_wraps_result_and_async() {
        assert_eq!(
            handler_call("h", false, &HandlerResult::Plain, ""),
            "self.h();"
        );
        assert!(handler_call("h", false, &HandlerResult::Result, "")
            .contains("if let Err(e) = self.h()"));
        assert!(handler_call("h", true, &HandlerResult::Plain, "").contains("std::thread::spawn"));
    }

    #[test]
    fn empty_wiring_emits_nothing() {
        let w = RustWiring::default();
        assert!(channel_field_pairs(&w).is_empty());
        assert!(iterator_methods(&w, "    ").is_empty());
        assert!(trait_impl_blocks(&w).is_empty());
    }
}
