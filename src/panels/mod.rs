//! UI panels — each module owns one panel or floating window.
//!
//! `palette` / `properties` / `code_preview` are the three primary side panels.
//! `widget_maker_panel` is the Visual Widget Maker floating window.
//! `svg_report` shows fidelity and diagnostic output for SVG Image widgets.
//! `db_panel` is the Stage 13 Database floating window.
//! All panels are pure immediate-mode egui; no retained widget state outside `RohKaiApp`.

pub mod code_preview;
pub mod component_tray;
pub mod db_panel;
pub mod descriptor_editor;
pub mod macro_palette;
pub mod outline;
pub mod palette;
pub mod project_tree;
pub mod properties;
pub mod rust_wiring;
pub mod shortcuts;
pub mod svg_report;
pub mod templates;
pub mod widget_builder;
pub mod widget_maker_panel;
pub mod window_bounds;
