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
            name: "antialiased_diagonal_fill",
            svg: r##"<svg viewBox="0 0 8 8"><path d="M1 1 L7 3 L2 7 Z" fill="#ff0000"/></svg>"##,
            width: 8,
            height: 8,
            golden: "........\n.ooo....\n.oRRRoo.\n.oRRRRo.\n.oRRRo..\n.oRoo...\n.ooo....\n........",
        },
        GoldenFixture {
            name: "dashed_round_cap_stroke",
            svg: r##"<svg viewBox="0 0 12 4"><line x1="1" y1="2" x2="11" y2="2" stroke="#0000ff" stroke-width="2" stroke-linecap="round" stroke-dasharray="3 2"/></svg>"##,
            width: 12,
            height: 4,
            golden: "............\noBBBooBBBo..\noBBBooBBBo..\n............",
        },
        GoldenFixture {
            name: "evenodd_self_intersection",
            svg: r##"<svg viewBox="0 0 10 10"><path d="M5 .5 L7.65 9 L.7 3.75 L9.3 3.75 L2.35 9 Z" fill="#00ff00" fill-rule="evenodd"/></svg>"##,
            width: 10,
            height: 10,
            golden: "....oo....\n....oo....\n....oo....\noooooooooo\n.oGo..oGo.\n..oo..oo..\n...GooG...\n..oGooGo..\n..oo..oo..\n..........",
        },
        GoldenFixture {
            name: "opacity_bucket",
            svg: r##"<svg viewBox="0 0 4 4"><rect width="4" height="4" fill="#00ff00" opacity="0.5"/></svg>"##,
            width: 4,
            height: 4,
            golden: "oooo\noooo\noooo\noooo",
        },
        GoldenFixture {
            name: "single_stop_gradient_extends_solid_color",
            svg: r##"<svg viewBox="0 0 4 4"><defs><linearGradient id="g"><stop offset="0" stop-color="#f00"/></linearGradient></defs><rect width="4" height="4" fill="url(#g)"/></svg>"##,
            width: 4,
            height: 4,
            golden: "RRRR\nRRRR\nRRRR\nRRRR",
        },
        GoldenFixture {
            name: "linear_gradient_red_to_blue",
            svg: r##"<svg viewBox="0 0 8 2"><defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><rect width="8" height="2" fill="url(#g)"/></svg>"##,
            width: 8,
            height: 2,
            golden: "RRRRBBBB\nRRRRBBBB",
        },
        GoldenFixture {
            name: "linear_gradient_repeat_spread",
            svg: r##"<svg viewBox="0 0 8 2"><defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="0" x2="2" spreadMethod="repeat"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><rect width="8" height="2" fill="url(#g)"/></svg>"##,
            width: 8,
            height: 2,
            golden: "RBRBRBRB\nRBRBRBRB",
        },
        GoldenFixture {
            name: "radial_gradient_red_to_green",
            svg: r##"<svg viewBox="0 0 5 5"><defs><radialGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="green"/></radialGradient></defs><rect width="5" height="5" fill="url(#g)"/></svg>"##,
            width: 5,
            height: 5,
            golden: "GGGGG\nGRRRG\nGRRRG\nGRRRG\nGGGGG",
        },
        GoldenFixture {
            // R4: clip-path now renders visibly clipped (left half) rather than
            // being diagnosed-only.  The clipPath unions a 2x4 rect over a 4x4
            // fill, so the right two columns are clipped out.
            name: "rect_clip_path_applied",
            svg: r##"<svg viewBox="0 0 4 4"><clipPath id="c"><rect width="2" height="4"/></clipPath><rect width="4" height="4" fill="#ff0000" clip-path="url(#c)"/></svg>"##,
            width: 4,
            height: 4,
            golden: "RR..\nRR..\nRR..\nRR..",
        },
        GoldenFixture {
            // R4: clipPath with a nonzero-rule path (triangle) clips the fill.
            name: "path_clip_nonzero",
            svg: r##"<svg viewBox="0 0 8 8"><clipPath id="c"><path d="M0 0 L8 0 L0 8 Z"/></clipPath><rect width="8" height="8" fill="#00ff00" clip-path="url(#c)"/></svg>"##,
            width: 8,
            height: 8,
            golden: "GGGGGGGo\nGGGGGGo.\nGGGGGo..\nGGGGo...\nGGGo....\nGGo.....\nGo......\no.......",
        },
        GoldenFixture {
            // R4: objectBoundingBox clip units scale a [0,1] clip rect to the
            // referencing shape's bounding box (left half of a 4x4 fill).
            name: "object_bounding_box_clip",
            svg: r##"<svg viewBox="0 0 4 4"><clipPath id="c" clipPathUnits="objectBoundingBox"><rect width="0.5" height="1"/></clipPath><rect width="4" height="4" fill="#0000ff" clip-path="url(#c)"/></svg>"##,
            width: 4,
            height: 4,
            golden: "BB..\nBB..\nBB..\nBB..",
        },
        GoldenFixture {
            // R4: nested <svg> overflow clipping — the inner content overflows
            // its 2x2 viewport but is clipped to it.
            name: "nested_svg_overflow_clip",
            svg: r##"<svg viewBox="0 0 4 4"><svg x="0" y="0" width="2" height="2"><rect width="4" height="4" fill="#ff0000"/></svg></svg>"##,
            width: 4,
            height: 4,
            golden: "RR..\nRR..\n....\n....",
        },
        GoldenFixture {
            // R4: evenodd clip-rule punches a hole — outer 8x8 minus inner 4x4.
            name: "path_clip_evenodd_hole",
            svg: r##"<svg viewBox="0 0 8 8"><clipPath id="c"><path d="M0 0 H8 V8 H0 Z M2 2 H6 V6 H2 Z" clip-rule="evenodd"/></clipPath><rect width="8" height="8" fill="#ff0000" clip-path="url(#c)"/></svg>"##,
            width: 8,
            height: 8,
            golden: "RRRRRRRR\nRRRRRRRR\nRR....RR\nRR....RR\nRR....RR\nRR....RR\nRRRRRRRR\nRRRRRRRR",
        },
        GoldenFixture {
            // R4: transformed clipPath child — a 2x4 clip rect translated right
            // by 1 user unit clips columns 1..3 of a 4x4 fill.
            name: "transformed_clip_child",
            svg: r##"<svg viewBox="0 0 4 4"><clipPath id="c"><rect width="2" height="4" transform="translate(1,0)"/></clipPath><rect width="4" height="4" fill="#00ff00" clip-path="url(#c)"/></svg>"##,
            width: 4,
            height: 4,
            golden: ".GG.\n.GG.\n.GG.\n.GG.",
        },
        GoldenFixture {
            // R4: a translucent group with two overlapping fills composites once
            // at group opacity — the overlap is the same bucket as the rest
            // ('o' partial), proving no double-darkening.
            name: "translucent_group_no_double_darken",
            svg: r##"<svg viewBox="0 0 6 4"><g opacity="0.5"><rect width="4" height="4" fill="#ff0000"/><rect x="2" width="4" height="4" fill="#ff0000"/></g></svg>"##,
            width: 6,
            height: 4,
            golden: "oooooo\noooooo\noooooo\noooooo",
        },
        GoldenFixture {
            // R7: luminance mask — white mask content over the left half keeps it
            // visible; the right half (no mask content = black) is masked out.
            name: "luminance_mask_left_half",
            svg: r##"<svg viewBox="0 0 4 4"><mask id="m"><rect width="2" height="4" fill="#ffffff"/></mask><rect width="4" height="4" fill="#ff0000" mask="url(#m)"/></svg>"##,
            width: 4,
            height: 4,
            golden: "RR..\nRR..\nRR..\nRR..",
        },
        GoldenFixture {
            // R7: feOffset shifts the blue strip two user units to the right.
            name: "feoffset_shifts_right",
            svg: r##"<svg viewBox="0 0 4 4"><filter id="f"><feOffset dx="2" dy="0"/></filter><rect width="2" height="4" fill="#0000ff" filter="url(#f)"/></svg>"##,
            width: 4,
            height: 4,
            golden: "..BB\n..BB\n..BB\n..BB",
        },
        GoldenFixture {
            // R7: feFlood (green) + feMerge with SourceGraphic (red 2x2 on top).
            name: "feflood_femerge",
            svg: r##"<svg viewBox="0 0 4 4"><filter id="f"><feFlood flood-color="#00ff00" result="bg"/><feMerge><feMergeNode in="bg"/><feMergeNode in="SourceGraphic"/></feMerge></filter><rect width="2" height="2" fill="#ff0000" filter="url(#f)"/></svg>"##,
            width: 4,
            height: 4,
            golden: "RRGG\nRRGG\nGGGG\nGGGG",
        },
        GoldenFixture {
            // Geometry coverage: axis-aligned <polygon> fills crisply (Poly path).
            name: "polygon_square_fill",
            svg: r##"<svg viewBox="0 0 4 4"><polygon points="0,0 4,0 4,4 0,4" fill="#ff0000"/></svg>"##,
            width: 4,
            height: 4,
            golden: "RRRR\nRRRR\nRRRR\nRRRR",
        },
        // -------------------------------------------------------------------
        // R8.1: curated W3C SVG 1.1 sub-corpus filling feature gaps so every
        // supported feature has a visual golden (paint/shape/structure/mask).
        // -------------------------------------------------------------------
        GoldenFixture {
            // W3C paint-server: `currentColor` resolves from inherited `color`.
            name: "w3c_current_color_inherits",
            svg: r##"<svg viewBox="0 0 4 4" color="#ff0000"><rect width="4" height="4" fill="currentColor"/></svg>"##,
            width: 4,
            height: 4,
            golden: "RRRR\nRRRR\nRRRR\nRRRR",
        },
        GoldenFixture {
            // W3C color: functional rgb() notation parses to a solid paint.
            name: "w3c_rgb_func_color",
            svg: r##"<svg viewBox="0 0 4 4"><rect width="4" height="4" fill="rgb(0,0,255)"/></svg>"##,
            width: 4,
            height: 4,
            golden: "BBBB\nBBBB\nBBBB\nBBBB",
        },
        GoldenFixture {
            // W3C opacity: fill-opacity buckets to partial alpha (not full).
            name: "w3c_fill_opacity_half",
            svg: r##"<svg viewBox="0 0 4 4"><rect width="4" height="4" fill="#00ff00" fill-opacity="0.5"/></svg>"##,
            width: 4,
            height: 4,
            golden: "oooo\noooo\noooo\noooo",
        },
        GoldenFixture {
            // W3C structure: <use> instances a referenced rect, offset by x.
            name: "w3c_use_references_rect",
            svg: r##"<svg viewBox="0 0 4 4"><defs><rect id="r" width="2" height="4" fill="#ff0000"/></defs><use href="#r" x="2" y="0"/></svg>"##,
            width: 4,
            height: 4,
            golden: "..RR\n..RR\n..RR\n..RR",
        },
        GoldenFixture {
            // W3C coordinate systems: nested group transforms concatenate, placing
            // a 2x2 fill in the bottom-right quadrant.
            name: "w3c_nested_group_transform",
            svg: r##"<svg viewBox="0 0 4 4"><g transform="translate(2,0)"><g transform="translate(0,2)"><rect width="2" height="2" fill="#0000ff"/></g></g></svg>"##,
            width: 4,
            height: 4,
            golden: "....\n....\n..BB\n..BB",
        },
        GoldenFixture {
            // W3C basic shapes: <polyline> stroked horizontally.
            name: "w3c_polyline_stroke",
            svg: r##"<svg viewBox="0 0 4 4"><polyline points="0,2 4,2" fill="none" stroke="#0000ff" stroke-width="2"/></svg>"##,
            width: 4,
            height: 4,
            golden: "....\nBBBB\nBBBB\n....",
        },
        GoldenFixture {
            // W3C basic shapes: <circle> fill (anti-aliased disc).
            name: "w3c_circle_fill",
            svg: r##"<svg viewBox="0 0 8 8"><circle cx="4" cy="4" r="3" fill="#ff0000"/></svg>"##,
            width: 8,
            height: 8,
            golden: "........\n.ooRRoo.\n.oRRRRo.\n.RRRRRR.\n.RRRRRR.\n.oRRRRo.\n.ooRRoo.\n........",
        },
        GoldenFixture {
            // W3C basic shapes: <ellipse> fill (wide AA disc).
            name: "w3c_ellipse_fill",
            svg: r##"<svg viewBox="0 0 8 4"><ellipse cx="4" cy="2" rx="3" ry="1.5" fill="#00ff00"/></svg>"##,
            width: 8,
            height: 4,
            golden: ".oooooo.\n.oGGGGo.\n.oGGGGo.\n.oooooo.",
        },
        GoldenFixture {
            // W3C masking: mask-type="alpha" keys on the mask's ALPHA channel, so
            // an opaque BLACK left half stays visible (luminance would mask it out).
            name: "w3c_alpha_mask_left_half",
            svg: r##"<svg viewBox="0 0 4 4"><mask id="m" mask-type="alpha"><rect width="2" height="4" fill="#000000"/></mask><rect width="4" height="4" fill="#ff0000" mask="url(#m)"/></svg>"##,
            width: 4,
            height: 4,
            golden: "RR..\nRR..\nRR..\nRR..",
        },
        GoldenFixture {
            // R9: vector-effect="non-scaling-stroke" — a width-2 stroke under a
            // 4x group scale stays ~2px in device space (cols 3-4) instead of
            // scaling to ~8px.
            name: "r9_non_scaling_stroke",
            svg: r##"<svg viewBox="0 0 8 8"><g transform="scale(4)"><line x1="1" y1="0" x2="1" y2="2" stroke="#0000ff" stroke-width="2" vector-effect="non-scaling-stroke"/></g></svg>"##,
            width: 8,
            height: 8,
            golden: "...BB...\n...BB...\n...BB...\n...BB...\n...BB...\n...BB...\n...BB...\n...BB...",
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
