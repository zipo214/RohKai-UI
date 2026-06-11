//! `HersheyShaper` — std-only fixed-pitch fallback shaper.
//!
//! This module must **never** import rustybuzz or any non-std crate.
//! It is used wherever the export-embedded path needs a shaper reference
//! (e.g. svg_rasterizer), and that source is kept std-only so it can be
//! embedded verbatim in exported projects.
//!
//! The advance heuristic: each UTF-8 byte contributes 600 design units.
//! That approximates Latin proportional spacing without any font file.

use super::{GlyphInfo, ShaperEngine};

/// Fixed-pitch fallback shaper.  No font data required.
// Not yet wired to in-app callers — P2-A scaffold.
#[allow(dead_code)]
pub struct HersheyShaper;

impl ShaperEngine for HersheyShaper {
    fn shape(&self, text: &str, _font_data: &[u8]) -> Vec<GlyphInfo> {
        let mut cluster: u32 = 0;
        text.chars()
            .map(|c| {
                let x_advance = (c.len_utf8() as i32) * 600;
                let info = GlyphInfo {
                    cluster,
                    x_advance,
                    x_offset: 0,
                    y_offset: 0,
                };
                cluster += c.len_utf8() as u32;
                info
            })
            .collect()
    }
}
