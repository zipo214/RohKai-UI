//! Canvas subsystem — rendering, interaction, and image processing for the design surface.
//!
//! `interaction` drives drag/select/resize/rubber-band/z-order.
//! `overlays` renders smart guides, guide lines, and pixel rulers.
//! `svg_rasterizer` is the zero-dependency SVG renderer (R0–R12 complete).
//! `widget_maker` holds the `WidgetMakerDoc` primitive composition model.
//! `shaper` provides the `ShaperEngine` trait + `RustyBuzzShaper` / `HersheyShaper` impls.

pub mod interaction;
pub mod overlays;
pub mod preview;
pub mod rulers;
pub mod search;
pub mod shaper;
#[cfg(test)]
pub mod svg_golden;
// `svg_rasterizer` is embedded verbatim into edition-2021 exported projects
// (see `codegen::export` `include_str!`). Edition-2024 `if let` let-chains are a
// compile error under 2021, so its nested `if let` blocks must NOT be collapsed.
// Silence the lint here; the `all_builtin_widgets_export_cargo_check` test guards
// against any 2024-only syntax actually reaching an export.
#[allow(clippy::collapsible_if)]
pub mod svg_rasterizer;
pub mod widget_instance;
pub mod widget_maker;
