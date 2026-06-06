//! Golden renderer fixture harness.
//!
//! Renders a fixed set of small SVG fixtures through the zero-dependency
//! rasterizer and compares the output against stored "golden" ASCII
//! signatures.  Because image crates are a banned dependency, the golden
//! reference is a compact, human-diffable ASCII grid rather than PNG bytes:
//! each pixel maps to one character by alpha + dominant colour bucket.
//!
//! When a renderer change intentionally alters output, run the fixture and
//! paste the printed `actual` signature into the fixture's `golden` field.
//! Drift that is *not* intentional makes the corresponding test fail loudly.

use crate::canvas::svg_rasterizer::rasterize;

/// One golden fixture: a name, the SVG source, target raster size, and the
/// expected ASCII signature.
pub struct GoldenFixture {
    pub name: &'static str,
    pub svg: &'static str,
    pub width: u32,
    pub height: u32,
    pub golden: &'static str,
}

/// Produce a deterministic, human-diffable ASCII signature of a rendered SVG.
///
/// Each pixel becomes one character:
/// - `.` fully transparent
/// - `R` / `G` / `B` red / green / blue dominant (opaque)
/// - `W` bright / near-white, `K` dark / near-black
/// - `o` partially transparent (anti-aliased edge)
///
/// Rows are separated by `\n`.
pub fn render_signature(svg: &str, width: u32, height: u32) -> String {
    let image = match rasterize(svg, width, height) {
        Ok(img) => img,
        Err(e) => return format!("<render error: {e:?}>"),
    };
    let [w, h] = image.size;
    let mut out = String::with_capacity((w + 1) * h);
    for y in 0..h {
        for x in 0..w {
            let [r, g, b, a] = image.pixels[y * w + x].to_array();
            out.push(pixel_char(r, g, b, a));
        }
        if y + 1 < h {
            out.push('\n');
        }
    }
    out
}

fn pixel_char(r: u8, g: u8, b: u8, a: u8) -> char {
    if a == 0 {
        return '.';
    }
    if a < 200 {
        return 'o'; // anti-aliased / semi-transparent edge
    }
    let (r, g, b) = (r as i32, g as i32, b as i32);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    // Near-grayscale: classify by luminance.
    if max - min < 40 {
        return if max > 160 {
            'W'
        } else if max < 80 {
            'K'
        } else {
            'o'
        };
    }
    if r == max {
        'R'
    } else if g == max {
        'G'
    } else {
        'B'
    }
}

/// The golden fixture set.  Kept intentionally small so signatures are
/// reviewable inline.
pub fn fixtures() -> Vec<GoldenFixture> {
    vec![
        GoldenFixture {
            name: "solid_red_fill",
            svg: r##"<svg viewBox="0 0 4 4"><rect width="4" height="4" fill="#ff0000"/></svg>"##,
            width: 4,
            height: 4,
            golden: "RRRR\nRRRR\nRRRR\nRRRR",
        },
        GoldenFixture {
            name: "half_green_right",
            svg: r##"<svg viewBox="0 0 4 4"><rect x="2" width="2" height="4" fill="#00ff00"/></svg>"##,
            width: 4,
            height: 4,
            golden: "..GG\n..GG\n..GG\n..GG",
        },
        GoldenFixture {
            name: "blue_top_strip",
            svg: r##"<svg viewBox="0 0 4 4"><rect width="4" height="2" fill="#0000ff"/></svg>"##,
            width: 4,
            height: 4,
            golden: "BBBB\nBBBB\n....\n....",
        },
        GoldenFixture {
            name: "translated_red_square",
            svg: r##"<svg viewBox="0 0 4 4"><rect width="2" height="2" fill="#ff0000" transform="translate(2,2)"/></svg>"##,
            width: 4,
            height: 4,
            golden: "....\n....\n..RR\n..RR",
        },
        GoldenFixture {
            name: "path_rect_fill",
            svg: r##"<svg viewBox="0 0 4 4"><path d="M0 0H4V4H0Z" fill="#ff0000"/></svg>"##,
            width: 4,
            height: 4,
            golden: "RRRR\nRRRR\nRRRR\nRRRR",
        },
        GoldenFixture {
            name: "nonzero_same_winding_contours",
            svg: r##"<svg viewBox="0 0 6 6"><path d="M0 0H6V6H0Z M2 2H4V4H2Z" fill="#ff0000"/></svg>"##,
            width: 6,
            height: 6,
            golden: "RRRRRR\nRRRRRR\nRRRRRR\nRRRRRR\nRRRRRR\nRRRRRR",
        },
        GoldenFixture {
            name: "evenodd_same_winding_hole",
            svg: r##"<svg viewBox="0 0 6 6"><path d="M0 0H6V6H0Z M2 2H4V4H2Z" fill="#ff0000" fill-rule="evenodd"/></svg>"##,
            width: 6,
            height: 6,
            golden: "RRRRRR\nRRRRRR\nRR..RR\nRR..RR\nRRRRRR\nRRRRRR",
        },
        GoldenFixture {
            name: "solid_stroke_line",
            svg: r##"<svg viewBox="0 0 4 4"><line x1="0" y1="2" x2="4" y2="2" stroke="#0000ff" stroke-width="2"/></svg>"##,
            width: 4,
            height: 4,
            golden: "....\nBBBB\nBBBB\n....",
        },
        GoldenFixture {
            name: "opacity_bucket",
            svg: r##"<svg viewBox="0 0 4 4"><rect width="4" height="4" fill="#00ff00" opacity="0.5"/></svg>"##,
            width: 4,
            height: 4,
            golden: "oooo\noooo\noooo\noooo",
        },
        GoldenFixture {
            name: "unsupported_gradient_stays_transparent",
            svg: r##"<svg viewBox="0 0 4 4"><defs><linearGradient id="g"><stop offset="0" stop-color="#f00"/></linearGradient></defs><rect width="4" height="4" fill="url(#g)"/></svg>"##,
            width: 4,
            height: 4,
            golden: "....\n....\n....\n....",
        },
        GoldenFixture {
            name: "unsupported_clip_diagnosed_not_applied",
            svg: r##"<svg viewBox="0 0 4 4"><clipPath id="c"><rect width="2" height="4"/></clipPath><rect width="4" height="4" fill="#ff0000" clip-path="url(#c)"/></svg>"##,
            width: 4,
            height: 4,
            golden: "RRRR\nRRRR\nRRRR\nRRRR",
        },
        GoldenFixture {
            name: "unsafe_external_href_rejected",
            svg: r##"<svg viewBox="0 0 4 4"><image href="https://example.invalid/a.png" width="4" height="4"/></svg>"##,
            width: 4,
            height: 4,
            golden: "<render error: ForbiddenContent>",
        },
        GoldenFixture {
            name: "empty_svg_all_transparent",
            svg: r##"<svg viewBox="0 0 4 4"></svg>"##,
            width: 4,
            height: 4,
            golden: "....\n....\n....\n....",
        },
    ]
}

// ---------------------------------------------------------------------------
// Tests — the golden harness itself
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_golden_fixtures_match() {
        let mut failures = Vec::new();
        for fx in fixtures() {
            let actual = render_signature(fx.svg, fx.width, fx.height);
            if actual != fx.golden {
                failures.push(format!(
                    "FIXTURE '{}' drifted:\n--- expected ---\n{}\n--- actual ---\n{}\n",
                    fx.name, fx.golden, actual
                ));
            }
        }
        assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    }

    #[test]
    fn signature_is_deterministic() {
        let fx = &fixtures()[0];
        let a = render_signature(fx.svg, fx.width, fx.height);
        let b = render_signature(fx.svg, fx.width, fx.height);
        assert_eq!(a, b, "signature must be stable across runs");
    }

    #[test]
    fn signature_dimensions_match_request() {
        // 6x3 raster → 3 rows, each 6 chars (plus 2 row separators).
        let sig = render_signature(
            r##"<svg viewBox="0 0 6 3"><rect width="6" height="3" fill="#ffffff"/></svg>"##,
            6,
            3,
        );
        let rows: Vec<&str> = sig.split('\n').collect();
        assert_eq!(rows.len(), 3);
        for row in rows {
            assert_eq!(row.chars().count(), 6);
        }
    }

    #[test]
    fn pixel_char_buckets() {
        assert_eq!(pixel_char(0, 0, 0, 0), '.'); // transparent
        assert_eq!(pixel_char(255, 0, 0, 255), 'R');
        assert_eq!(pixel_char(0, 255, 0, 255), 'G');
        assert_eq!(pixel_char(0, 0, 255, 255), 'B');
        assert_eq!(pixel_char(255, 255, 255, 255), 'W');
        assert_eq!(pixel_char(0, 0, 0, 255), 'K');
        assert_eq!(pixel_char(255, 0, 0, 120), 'o'); // semi-transparent
    }
}
