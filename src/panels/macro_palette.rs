//! Stage 11 — Macro palette.
//!
//! A window listing common Rust macros. Clicking one appends its snippet to the
//! live (Lazare) code buffer, where it round-trips through the normal parser and
//! can be edited in place.

/// A macro snippet: display name + the text inserted into the code buffer.
struct MacroSnippet {
    name: &'static str,
    desc: &'static str,
    snippet: &'static str,
}

const MACROS: &[MacroSnippet] = &[
    MacroSnippet {
        name: "vec!",
        desc: "Construct a Vec",
        snippet: "let items = vec![1, 2, 3];",
    },
    MacroSnippet {
        name: "format!",
        desc: "Build a String",
        snippet: "let text = format!(\"{} = {}\", label, value);",
    },
    MacroSnippet {
        name: "println!",
        desc: "Print to stdout",
        snippet: "println!(\"{:?}\", value);",
    },
    MacroSnippet {
        name: "dbg!",
        desc: "Debug-print and return value",
        snippet: "dbg!(&value);",
    },
    MacroSnippet {
        name: "assert!",
        desc: "Runtime assertion",
        snippet: "assert!(condition, \"message\");",
    },
    MacroSnippet {
        name: "todo!()",
        desc: "Unimplemented marker",
        snippet: "todo!(\"implement this\");",
    },
    MacroSnippet {
        name: "matches!",
        desc: "Pattern-match boolean",
        snippet: "let is_some = matches!(opt, Some(_));",
    },
];

/// Render the macro palette. Returns an optional snippet the caller should
/// append to the code buffer this frame.
pub fn show(ctx: &egui::Context, open: &mut bool) -> Option<String> {
    if !*open {
        return None;
    }
    let mut chosen: Option<String> = None;

    let screen = ctx.content_rect();
    let default_pos = egui::pos2(
        (screen.center().x - 180.0).max(screen.min.x + 20.0),
        (screen.center().y - 200.0).max(screen.min.y + 20.0),
    );

    egui::Window::new("Macro Palette")
        .id(egui::Id::new("macro_palette_window"))
        .open(open)
        .default_pos(default_pos)
        .default_size([360.0, 380.0])
        .resizable(true)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new("Click a macro to append its snippet to the code panel.")
                    .small()
                    .weak(),
            );
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("macro_palette_scroll")
                .show(ui, |ui| {
                    for m in MACROS {
                        ui.horizontal(|ui| {
                            if ui
                                .button(egui::RichText::new(m.name).monospace())
                                .on_hover_text(m.snippet)
                                .clicked()
                            {
                                chosen = Some(m.snippet.to_owned());
                            }
                            ui.label(egui::RichText::new(m.desc).small().weak());
                        });
                    }
                });
        });

    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macros_have_nonempty_snippets() {
        assert!(!MACROS.is_empty());
        for m in MACROS {
            assert!(!m.name.is_empty());
            assert!(!m.snippet.is_empty());
        }
    }

    #[test]
    fn known_macros_present() {
        let names: Vec<&str> = MACROS.iter().map(|m| m.name).collect();
        for expected in ["vec!", "format!", "println!"] {
            assert!(names.contains(&expected), "missing {expected}");
        }
    }
}
