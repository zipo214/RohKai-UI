//! Font shaping abstraction layer (P2-A).
//!
//! `ShaperEngine` is the trait contract.  Callers hold a `&dyn ShaperEngine`
//! or a concrete type; they never depend on rustybuzz directly.
//!
//! Two implementations ship:
//! - `RustyBuzzShaper` — full HarfBuzz shaping via rustybuzz (in-app only).
//! - `HersheyShaper` — std-only fixed-pitch fallback; safe to embed in exports.
//!
//! Future swap path: replace `RustyBuzzShaper` with a bespoke HarfBuzz port
//! that passes all 2 252 Unicode shaping tests without touching any caller.

pub mod hershey_shaper;
pub mod rustybuzz_shaper;

// Re-exported for convenience; not yet internally wired — suppressed until
// P2-A wiring lands in callers.
#[allow(unused_imports)]
pub use hershey_shaper::HersheyShaper;
#[allow(unused_imports)]
pub use rustybuzz_shaper::RustyBuzzShaper;

/// Per-glyph output produced by a shaping run.
///
/// Units are font design units (1/`units_per_em` of a typographic em).
/// For `HersheyShaper` the design unit is virtual (600 per UTF-8 byte).
// Not yet wired to in-app callers — P2-A scaffold.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphInfo {
    /// Byte offset of the source cluster in the input UTF-8 string.
    pub cluster: u32,
    /// Horizontal advance (how far the pen moves after this glyph).
    pub x_advance: i32,
    /// Horizontal pen offset applied before drawing this glyph.
    pub x_offset: i32,
    /// Vertical pen offset applied before drawing this glyph.
    pub y_offset: i32,
}

/// Trait contract for a font shaping engine.
///
/// Implementations must be `Send + Sync` so they can be held in shared
/// application state without wrapping.
// Not yet wired to in-app callers — P2-A scaffold.
#[allow(dead_code)]
pub trait ShaperEngine: Send + Sync {
    /// Shape `text` using the provided raw font bytes.
    ///
    /// Returns one `GlyphInfo` per output glyph in visual order.
    /// Engines that require font bytes return an empty `Vec` when `font_data`
    /// is empty/invalid/inapplicable; fontless fallback engines (e.g.
    /// `HersheyShaper`) may ignore `font_data` and still shape. Implementations
    /// must never panic.
    fn shape(&self, text: &str, font_data: &[u8]) -> Vec<GlyphInfo>;
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- HersheyShaper ---

    #[test]
    fn hershey_shape_ascii() {
        let s = HersheyShaper;
        let glyphs = s.shape("Hello", &[]);
        assert_eq!(glyphs.len(), 5, "expected one GlyphInfo per char");
        for g in &glyphs {
            assert!(
                g.x_advance > 0,
                "x_advance must be positive, got {}",
                g.x_advance
            );
        }
    }

    #[test]
    fn hershey_empty_string() {
        let s = HersheyShaper;
        assert!(s.shape("", &[]).is_empty());
    }

    #[test]
    fn hershey_is_send_sync() {
        fn assert_send_sync<T: ShaperEngine + Send + Sync>() {}
        assert_send_sync::<HersheyShaper>();
    }

    // --- RustyBuzzShaper ---

    #[test]
    fn rustybuzz_empty_font_does_not_panic() {
        let s = RustyBuzzShaper;
        // Empty font_data must return Vec::new(), not panic.
        let result = s.shape("text", &[]);
        // We just verify it doesn't panic; result may be empty.
        let _ = result;
    }

    #[test]
    fn rustybuzz_is_send_sync() {
        fn assert_send_sync<T: ShaperEngine + Send + Sync>() {}
        assert_send_sync::<RustyBuzzShaper>();
    }
}
