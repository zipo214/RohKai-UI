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
/// `fn name(&self) -> impl IntoIterator + '_ { source.iter().map(...).collect::<Vec<_>>() }`.
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
        chain.push_str(".collect::<Vec<_>>()");
        s.push_str(&format!(
            "{indent}#[allow(dead_code)]\n{indent}fn {name}(&self) -> impl IntoIterator + '_ {{\n{indent}    {chain}\n{indent}}}\n"
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
        if is_simple_ident(tr) {
            s.push_str(&format!("\ntrait {tr} {{\n    {method};\n}}\n"));
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
/// - async (any):       `self.h();` — calls the generated launcher, which spawns
///   a worker thread, sends completion through an mpsc channel, and is drained in
///   `update()` (see [`async_launcher_method`], [`async_drain_block`]).
/// - Plain sync:        `self.h();`
/// - Result sync:       `if let Err(e) = self.h() { eprintln!("{e}"); }`
/// - Option sync:       `let _ = self.h();`
pub fn handler_call(
    handler: &str,
    async_handler: bool,
    result: &HandlerResult,
    indent: &str,
) -> String {
    if async_handler {
        // The launcher is `fn {handler}(&mut self)` and guards against double-launch.
        return format!("{indent}self.{handler}();");
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
// Async task contract (std-only: std::thread + std::sync::mpsc)
//
// For each async handler we generate, on `ExportedApp`:
//   - `{h}_rx: Option<Receiver<MSG>>`  (set while a task is in flight)
//   - `{h}_running: bool`              (status — guards double-launch, drives UI)
//   - `{h}_error: Option<String>`      (Result mode only — last failure)
// plus a free `fn {h}_worker() -> MSG` (runs off the UI thread; no `&mut self`),
// a launcher method `fn {h}(&mut self)` that spawns + sends, and a drain block in
// `update()` that `try_recv`s completion and records status/error.
// MSG is `()` (Plain), `Result<(), String>` (Result), or `Option<()>` (Option).
// ---------------------------------------------------------------------------

/// The channel message type carrying a worker's completion for `result`.
pub fn async_msg_type(result: &HandlerResult) -> &'static str {
    match result {
        HandlerResult::Plain => "()",
        HandlerResult::Result => "Result<(), String>",
        HandlerResult::Option => "Option<()>",
    }
}

/// `ExportedApp` struct field declarations for one async handler (4-space indent).
pub fn async_struct_fields(handler: &str, result: &HandlerResult) -> Vec<String> {
    let msg = async_msg_type(result);
    let mut fields = vec![
        format!("    {handler}_rx: Option<std::sync::mpsc::Receiver<{msg}>>,"),
        format!("    {handler}_running: bool,"),
    ];
    if matches!(result, HandlerResult::Result) {
        fields.push(format!("    {handler}_error: Option<String>,"));
    }
    fields
}

/// `ExportedApp::default` field initializers for one async handler (12-space indent).
pub fn async_default_fields(handler: &str, result: &HandlerResult) -> Vec<String> {
    let mut fields = vec![
        format!("            {handler}_rx: None,"),
        format!("            {handler}_running: false,"),
    ];
    if matches!(result, HandlerResult::Result) {
        fields.push(format!("            {handler}_error: None,"));
    }
    fields
}

/// The launcher method `fn {h}(&mut self) {{ … }}` (4-space outer indent).
/// Spawns a worker thread and stores the receiver; guards against double-launch.
pub fn async_launcher_method(handler: &str, result: &HandlerResult) -> String {
    let msg = async_msg_type(result);
    let clear_error = if matches!(result, HandlerResult::Result) {
        format!("        self.{handler}_error = None;\n")
    } else {
        String::new()
    };
    format!(
        "    fn {handler}(&mut self) {{\n\
         \x20       if self.{handler}_running {{\n\
         \x20           return;\n\
         \x20       }}\n\
         \x20       self.{handler}_running = true;\n\
         {clear_error}\
         \x20       let (tx, rx) = std::sync::mpsc::channel::<{msg}>();\n\
         \x20       std::thread::spawn(move || {{\n\
         \x20           let _ = tx.send({handler}_worker());\n\
         \x20       }});\n\
         \x20       self.{handler}_rx = Some(rx);\n\
         \x20   }}\n"
    )
}

/// The module-level worker free function `fn {h}_worker() -> MSG`.
/// Runs off the UI thread; takes no `&mut self`.
pub fn async_worker_fn(handler: &str, result: &HandlerResult) -> String {
    match result {
        HandlerResult::Plain => format!(
            "fn {handler}_worker() {{\n    // TODO: background work for {handler} (no &mut self; runs off the UI thread).\n}}\n"
        ),
        HandlerResult::Result => format!(
            "fn {handler}_worker() -> Result<(), String> {{\n    // TODO: background work for {handler} (no &mut self; runs off the UI thread).\n    Ok(())\n}}\n"
        ),
        HandlerResult::Option => format!(
            "fn {handler}_worker() -> Option<()> {{\n    // TODO: background work for {handler} (no &mut self; runs off the UI thread).\n    Some(())\n}}\n"
        ),
    }
}

/// The `update()` drain block for one async handler (8-space indent).
/// Borrow-safe: captures the message by value, then mutates `self`.
pub fn async_drain_block(handler: &str, result: &HandlerResult) -> String {
    let store_error = if matches!(result, HandlerResult::Result) {
        format!(
            "            if let Some(Err(e)) = &{handler}_msg {{\n\
             \x20               self.{handler}_error = Some(e.clone());\n\
             \x20           }}\n"
        )
    } else {
        String::new()
    };
    format!(
        "        let {handler}_msg = self.{handler}_rx.as_ref().and_then(|rx| rx.try_recv().ok());\n\
         \x20       if {handler}_msg.is_some() {{\n\
         \x20           self.{handler}_running = false;\n\
         {store_error}\
         \x20           self.{handler}_rx = None;\n\
         \x20       }}\n"
    )
}

/// Emit a `ctx.request_repaint_after(...)` guard so the exported app repaints
/// promptly while any async task is in flight — no user input required.
///
/// Emitted inside `update()` immediately after all drain blocks.  The guard is
/// conditional so the app returns to event-driven repainting once all tasks
/// complete.
pub fn async_repaint_block(handlers: &[(String, HandlerResult)], indent: &str) -> String {
    if handlers.is_empty() {
        return String::new();
    }
    let cond = handlers
        .iter()
        .map(|(h, _)| format!("self.{h}_running"))
        .collect::<Vec<_>>()
        .join(" || ");
    format!(
        "{indent}if {cond} {{\n\
         {indent}    ctx.request_repaint_after(std::time::Duration::from_millis(16));\n\
         {indent}}}\n"
    )
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
    let s = if s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(true) {
        format!("c_{s}")
    } else {
        s
    };
    if crate::codegen::rust::RUST_KEYWORDS.contains(&s.as_str()) {
        format!("{s}_ch")
    } else {
        s
    }
}

fn is_simple_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
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
        assert!(s.contains("fn active(&self) -> impl IntoIterator + '_"));
        // filter then map, in declared order
        let f = s.find(".filter(|x| x.active)").unwrap();
        let m = s.find(".map(|x| x.id)").unwrap();
        assert!(f < m, "ops must chain in declared order");
        assert!(s.contains(".collect::<Vec<_>>()"));
    }

    #[test]
    fn trait_impl_block_well_formed() {
        let s = trait_impl_blocks(&wiring());
        assert!(s.contains("trait MyBehavior"));
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
        // Async call site invokes the launcher (which does the spawning).
        assert_eq!(
            handler_call("h", true, &HandlerResult::Plain, ""),
            "self.h();"
        );
    }

    #[test]
    fn empty_wiring_emits_nothing() {
        let w = RustWiring::default();
        assert!(channel_field_pairs(&w).is_empty());
        assert!(iterator_methods(&w, "    ").is_empty());
        assert!(trait_impl_blocks(&w).is_empty());
    }

    #[test]
    fn async_msg_type_per_mode() {
        assert_eq!(async_msg_type(&HandlerResult::Plain), "()");
        assert_eq!(async_msg_type(&HandlerResult::Result), "Result<(), String>");
        assert_eq!(async_msg_type(&HandlerResult::Option), "Option<()>");
    }

    #[test]
    fn async_struct_fields_include_error_only_for_result() {
        let plain = async_struct_fields("h", &HandlerResult::Plain);
        assert!(plain
            .iter()
            .any(|f| f.contains("h_rx: Option<std::sync::mpsc::Receiver<()>>")));
        assert!(plain.iter().any(|f| f.contains("h_running: bool")));
        assert!(!plain.iter().any(|f| f.contains("h_error")));

        let res = async_struct_fields("h", &HandlerResult::Result);
        assert!(res.iter().any(|f| f.contains("h_error: Option<String>")));
    }

    #[test]
    fn async_launcher_spawns_and_guards() {
        let m = async_launcher_method("on_save", &HandlerResult::Result);
        assert!(m.contains("fn on_save(&mut self)"));
        assert!(m.contains("if self.on_save_running {"));
        assert!(m.contains("self.on_save_running = true;"));
        assert!(m.contains("self.on_save_error = None;")); // Result clears error
        assert!(m.contains("std::thread::spawn(move ||"));
        assert!(m.contains("let _ = tx.send(on_save_worker());"));
        assert!(m.contains("self.on_save_rx = Some(rx);"));
    }

    #[test]
    fn async_worker_returns_msg_type() {
        assert!(async_worker_fn("h", &HandlerResult::Plain).contains("fn h_worker() {"));
        assert!(async_worker_fn("h", &HandlerResult::Result)
            .contains("fn h_worker() -> Result<(), String>"));
        assert!(
            async_worker_fn("h", &HandlerResult::Option).contains("fn h_worker() -> Option<()>")
        );
    }

    #[test]
    fn async_repaint_block_single_handler() {
        let handlers = vec![("h".to_owned(), HandlerResult::Plain)];
        let block = async_repaint_block(&handlers, "        ");
        assert!(block.contains("if self.h_running {"));
        assert!(block.contains("request_repaint_after(std::time::Duration::from_millis(16))"));
    }

    #[test]
    fn async_repaint_block_multi_handler_uses_or() {
        let handlers = vec![
            ("h1".to_owned(), HandlerResult::Plain),
            ("h2".to_owned(), HandlerResult::Result),
        ];
        let block = async_repaint_block(&handlers, "        ");
        assert!(block.contains("self.h1_running || self.h2_running"));
    }

    #[test]
    fn async_repaint_block_empty_emits_nothing() {
        assert!(async_repaint_block(&[], "        ").is_empty());
    }

    #[test]
    fn async_drain_uses_try_recv_and_stores_error_for_result() {
        let d = async_drain_block("on_save", &HandlerResult::Result);
        assert!(d.contains("self.on_save_rx.as_ref().and_then(|rx| rx.try_recv().ok())"));
        assert!(d.contains("self.on_save_running = false;"));
        assert!(d.contains("if let Some(Err(e)) = &on_save_msg {"));
        assert!(d.contains("self.on_save_error = Some(e.clone());"));
        assert!(d.contains("self.on_save_rx = None;"));

        let plain = async_drain_block("h", &HandlerResult::Plain);
        assert!(!plain.contains("_error"));
    }

    #[test]
    fn sanitize_keyword_channel_name_gets_suffix() {
        // "type" is a Rust keyword — must not appear as bare identifier
        let pairs = channel_field_pairs(&RustWiring {
            channels: vec![ChannelDef {
                id: Uuid::nil(),
                name: "type".to_owned(),
                ty: "String".to_owned(),
            }],
            iterators: vec![],
            trait_impls: vec![],
        });
        let decls: Vec<&str> = pairs.iter().map(|(d, _)| d.as_str()).collect();
        // Should NOT contain "type_tx" (keyword), should contain "type_ch_tx" or similar
        assert!(
            !decls.iter().any(|d| d.contains(" type_tx:")),
            "keyword 'type' must be sanitized in channel name"
        );
    }

    #[test]
    fn sanitize_digit_leading_channel_name() {
        let pairs = channel_field_pairs(&RustWiring {
            channels: vec![ChannelDef {
                id: Uuid::nil(),
                name: "123data".to_owned(),
                ty: "u32".to_owned(),
            }],
            iterators: vec![],
            trait_impls: vec![],
        });
        let decls: Vec<&str> = pairs.iter().map(|(d, _)| d.as_str()).collect();
        // Must not start with a digit
        assert!(
            !decls.iter().any(|d| {
                let trimmed = d.trim_start();
                trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
            }),
            "digit-leading channel name must be prefixed"
        );
    }
}
