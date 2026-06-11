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
pub mod shaper;
#[cfg(test)]
pub mod svg_golden;
pub mod svg_rasterizer;
pub mod widget_instance;
pub mod widget_maker;
