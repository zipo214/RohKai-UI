//! Code generation from Visual Widget Maker compositions.
//!
//! Converts `WidgetMakerDoc` primitives to egui Rust template strings.
//! Lives in `codegen/` to satisfy the invariant that Rust syntax strings
//! are produced only within this module tree.

use crate::canvas::widget_maker::{MakerPrimKind, MakerPrimitive, WidgetMakerDoc};
use crate::codegen::rust::string_literal;

/// Generate the `live_preview` template string from the maker doc.
pub fn gen_live_preview(doc: &WidgetMakerDoc) -> String {
    let mut lines = vec!["    {".to_owned()];
    lines.push("        let _painter = ui.painter();".to_owned());
    lines.push("        let _outer = ui.max_rect();".to_owned());
    for prim in &doc.primitives {
        lines.extend(prim_to_egui_lines(prim));
    }
    lines.push("    }".to_owned());
    lines.join("\n")
}

/// Generate the `export` template string from the maker doc.
pub fn gen_export_template(doc: &WidgetMakerDoc) -> String {
    let mut lines = vec!["                {".to_owned()];
    lines.push("                    let _painter = ui.painter();".to_owned());
    lines.push("                    let _outer = ui.max_rect();".to_owned());
    for prim in &doc.primitives {
        lines.extend(prim_to_egui_lines(prim));
    }
    lines.push("                }".to_owned());
    lines.join("\n")
}

fn prim_to_egui_lines(prim: &MakerPrimitive) -> Vec<String> {
    let [r, g, b] = prim.fill;
    let color = format!("egui::Color32::from_rgb({r}, {g}, {b})");
    let sub_rect = format!(
        "egui::Rect::from_min_size(\
            _outer.min + egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}), \
            egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}))",
        prim.x, prim.y, prim.w, prim.h
    );
    match prim.kind {
        MakerPrimKind::Rect => vec![format!(
            "        _painter.rect_filled({sub_rect}, {:.1}, {color});",
            prim.corner_radius
        )],
        MakerPrimKind::Outline => vec![format!(
            "        _painter.rect_stroke({sub_rect}, {:.1}, egui::Stroke::new(1.0, {color}));",
            prim.corner_radius
        )],
        MakerPrimKind::Ellipse => vec![format!(
            "        _painter.circle_filled({sub_rect}.center(), \
                {sub_rect}.width().min({sub_rect}.height()) * 0.5, {color});"
        )],
        MakerPrimKind::Text => {
            let text_lit = if prim.use_label_token {
                // Template token — double-braces are intentional in the output
                "\"{{label}}\"".to_owned()
            } else {
                // string_literal() uses Debug formatting: handles \, \n, \t, " correctly
                string_literal(&prim.text_content)
            };
            let tc = format!("egui::Color32::from_rgb({r}, {g}, {b})");
            vec![format!(
                "        ui.put({sub_rect}, egui::Label::new(egui::RichText::new({text_lit}).size({:.1}).color({tc})).wrap(false));",
                prim.font_size
            )]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::widget_maker::{MakerPrimKind, MakerPrimitive, WidgetMakerDoc};

    fn text_prim(content: &str) -> MakerPrimitive {
        MakerPrimitive {
            kind: MakerPrimKind::Text,
            text_content: content.to_owned(),
            use_label_token: false,
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            fill: [255, 255, 255],
            font_size: 14.0,
            ..Default::default()
        }
    }

    fn doc_with(prim: MakerPrimitive) -> WidgetMakerDoc {
        let mut doc = WidgetMakerDoc::new_with_defaults();
        doc.primitives = vec![prim];
        doc
    }

    #[test]
    fn text_prim_escapes_backslash() {
        let out = gen_export_template(&doc_with(text_prim("path\\to\\file")));
        assert!(
            out.contains("path\\\\to\\\\file"),
            "backslashes must be escaped: {out}"
        );
        assert!(
            !out.contains("path\\to\\file") || out.contains("path\\\\"),
            "raw backslash in output"
        );
    }

    #[test]
    fn text_prim_escapes_double_quote() {
        let out = gen_export_template(&doc_with(text_prim("say \"hello\"")));
        assert!(
            out.contains("\\\"hello\\\""),
            "quotes must be escaped: {out}"
        );
    }

    #[test]
    fn label_token_uses_double_braces() {
        let mut prim = text_prim("ignored");
        prim.use_label_token = true;
        let out = gen_live_preview(&doc_with(prim));
        assert!(
            out.contains("{{label}}"),
            "label token must use double braces: {out}"
        );
    }
}
