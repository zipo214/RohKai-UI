//! RohKai library crate root.
//!
//! RohKai ships as a lib + bin: this library exposes the designer's modules so
//! integration tests in `tests/` (e.g. the cross-surface `fidelity_audit.rs`
//! parity harness) can exercise the real public API — codegen, schema, panels,
//! and SVG core — instead of only inline `#[cfg(test)]` modules. The `rohkai`
//! binary (`src/main.rs`) is a thin shell that constructs [`app::RohKaiApp`].
//!
//! `UiTree` (in [`project::ui_tree`]) remains the single source of truth: the
//! canvas renders it and the codegen modules emit Rust from it.

pub mod app;
pub mod canvas;
pub mod codegen;
pub mod panels;
pub mod project;
pub mod settings;
pub mod svg_core;
pub mod svg_import;
pub mod widgets;
