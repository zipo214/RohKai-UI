//! RohKai library crate root.
//!
//! RohKai ships as a lib + bin: this library exposes the designer's modules so
//! integration tests in `tests/` (e.g. the cross-surface `fidelity_audit.rs`
//! parity harness) can exercise the real public API — codegen, schema, panels,
//! and SVG core — instead of only inline `#[cfg(test)]` modules. The `rohkai`
//! binary (`src/main.rs`) is a thin shell that constructs [`app::RohKaiApp`].
//!
//! [`project::document::ProjectDocument`] is the project source of truth.
//! Each surface owns one [`project::ui_tree::UiTree`], which remains the sole
//! canvas/codegen authority for that surface.

pub mod app;
pub mod canvas;
pub mod codegen;
pub mod panels;
pub mod project;
pub mod settings;
pub mod svg_core;
pub mod svg_import;
pub mod ui_colors;
pub mod widgets;
