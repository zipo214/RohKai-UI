//! `RustyBuzzShaper` — live font shaper backed by rustybuzz.
//!
//! This is the in-app shaper implementation.  It must **not** be embedded
//! in exported projects; `HersheyShaper` is the fallback for those paths.
//!
//! If `font_data` is empty or cannot be parsed as a valid font face,
//! `shape` returns an empty `Vec` without panicking.

use super::{GlyphInfo, ShaperEngine};

/// Font shaper backed by rustybuzz (HarfBuzz shaping algorithm).
// Not yet wired to in-app callers — P2-A scaffold.
#[allow(dead_code)]
pub struct RustyBuzzShaper;

impl ShaperEngine for RustyBuzzShaper {
    fn shape(&self, text: &str, font_data: &[u8]) -> Vec<GlyphInfo> {
        // Guard: nothing to shape with an empty font or empty text.
        if font_data.is_empty() || text.is_empty() {
            return Vec::new();
        }

        let face = match rustybuzz::Face::from_slice(font_data, 0) {
            Some(f) => f,
            None => return Vec::new(),
        };

        let mut buf = rustybuzz::UnicodeBuffer::new();
        buf.push_str(text);

        let glyph_buf = rustybuzz::shape(&face, &[], buf);

        let infos = glyph_buf.glyph_infos();
        let positions = glyph_buf.glyph_positions();

        infos
            .iter()
            .zip(positions.iter())
            .map(|(info, pos)| GlyphInfo {
                cluster: info.cluster,
                x_advance: pos.x_advance,
                x_offset: pos.x_offset,
                y_offset: pos.y_offset,
            })
            .collect()
    }
}
