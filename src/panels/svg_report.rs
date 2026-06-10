//! R8: surface the SVG render report (fidelity, diagnostics, provenance) in the
//! properties panel for the selected SVG Image widget, plus a read-only source
//! viewer and a rendered-report / source toggle.
//!
//! `report_summary` is the pure, unit-tested mapping from `SvgRenderReport` to
//! display rows; `show_report` renders those rows (no new report computation
//! beyond the existing `rasterize_with_report`).

use crate::canvas::svg_rasterizer::{rasterize_with_report, SvgRenderFidelity, SvgRenderReport};

/// Fixed, small raster size for the in-panel report — deterministic and cheap,
/// independent of canvas zoom.
const REPORT_RASTER: u32 = 256;

/// Display-ready view of a render report (pure mapping; rendered by `show_report`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportSummary {
    pub fidelity: SvgRenderFidelity,
    /// Labelled summary rows (Fidelity / Rendered / Skipped / Warnings / Unsupported).
    pub rows: Vec<(String, String)>,
    /// `(code, message[+span])` per warning.
    pub warnings: Vec<(String, String)>,
    /// `(feature, message[+span])` per unsupported feature.
    pub unsupported: Vec<(String, String)>,
}

fn fidelity_label(fidelity: SvgRenderFidelity) -> &'static str {
    match fidelity {
        SvgRenderFidelity::High => "High",
        SvgRenderFidelity::Medium => "Medium",
        SvgRenderFidelity::Low => "Low",
    }
}

/// Map a render report to display rows + diagnostic lines (with byte-span
/// provenance where present). Pure — the panel only renders the result.
pub fn report_summary(report: &SvgRenderReport) -> ReportSummary {
    let span = |s: Option<crate::canvas::svg_rasterizer::SvgRenderSource>| {
        s.map(|s| format!(" [bytes {}..{}]", s.byte_start, s.byte_end))
            .unwrap_or_default()
    };
    let mut rows = vec![
        (
            "Fidelity".to_owned(),
            fidelity_label(report.fidelity).to_owned(),
        ),
        (
            "Rendered".to_owned(),
            report.rendered_element_count.to_string(),
        ),
        (
            "Skipped".to_owned(),
            report.skipped_element_count.to_string(),
        ),
        ("Warnings".to_owned(), report.warning_count.to_string()),
        (
            "Unsupported".to_owned(),
            report.unsupported_feature_count.to_string(),
        ),
    ];
    // R12: accessibility metadata + recovery, shown only when present.
    if let Some(title) = &report.title {
        rows.push(("Title".to_owned(), title.clone()));
    }
    if let Some(desc) = &report.desc {
        rows.push(("Description".to_owned(), desc.clone()));
    }
    if report.recovered_error_count > 0 {
        rows.push((
            "Recovered".to_owned(),
            report.recovered_error_count.to_string(),
        ));
    }
    ReportSummary {
        fidelity: report.fidelity,
        rows,
        warnings: report
            .warnings
            .iter()
            .map(|w| (w.code.clone(), format!("{}{}", w.message, span(w.source))))
            .collect(),
        unsupported: report
            .unsupported_features
            .iter()
            .map(|u| {
                (
                    u.feature.clone(),
                    format!("{}{}", u.message, span(u.source)),
                )
            })
            .collect(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SvgView {
    Report,
    Source,
}

/// Render the fidelity report + diagnostics and a read-only source viewer for an
/// SVG Image widget, with a rendered/source toggle.
pub fn show_report(ui: &mut egui::Ui, svg_source: &str) {
    let id = ui.id().with("svg_report_view");
    let mut view = ui.data_mut(|d| d.get_temp::<SvgView>(id).unwrap_or(SvgView::Report));
    ui.horizontal(|ui| {
        ui.selectable_value(&mut view, SvgView::Report, "Rendered report");
        ui.selectable_value(&mut view, SvgView::Source, "SVG source");
    });
    ui.data_mut(|d| d.insert_temp(id, view));

    match view {
        SvgView::Report => match rasterize_with_report(svg_source, REPORT_RASTER, REPORT_RASTER) {
            Ok(out) => {
                let summary = report_summary(&out.report);
                let color = match summary.fidelity {
                    SvgRenderFidelity::High => egui::Color32::from_rgb(52, 211, 153),
                    SvgRenderFidelity::Medium => egui::Color32::from_rgb(251, 191, 36),
                    SvgRenderFidelity::Low => egui::Color32::from_rgb(248, 113, 113),
                };
                for (label, value) in &summary.rows {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("{label}:")).small().weak());
                        let txt = egui::RichText::new(value).small();
                        ui.label(if label == "Fidelity" {
                            txt.color(color)
                        } else {
                            txt
                        });
                    });
                }
                if !summary.warnings.is_empty() {
                    egui::CollapsingHeader::new(format!("Warnings ({})", summary.warnings.len()))
                        .id_salt("svg_report_warnings")
                        .show(ui, |ui| {
                            for (code, msg) in &summary.warnings {
                                ui.label(egui::RichText::new(format!("{code}: {msg}")).small());
                            }
                        });
                }
                if !summary.unsupported.is_empty() {
                    egui::CollapsingHeader::new(format!(
                        "Unsupported ({})",
                        summary.unsupported.len()
                    ))
                    .id_salt("svg_report_unsupported")
                    .show(ui, |ui| {
                        for (feat, msg) in &summary.unsupported {
                            ui.label(egui::RichText::new(format!("{feat}: {msg}")).small());
                        }
                    });
                }
            }
            Err(e) => {
                ui.label(
                    egui::RichText::new(format!("SVG render rejected: {e:?}"))
                        .small()
                        .color(egui::Color32::from_rgb(248, 113, 113)),
                );
            }
        },
        SvgView::Source => {
            egui::ScrollArea::vertical()
                .id_salt("svg_report_source")
                .max_height(200.0)
                .show(ui, |ui| {
                    let mut src = svg_source.to_owned();
                    ui.add(
                        egui::TextEdit::multiline(&mut src)
                            .font(egui::FontId::monospace(10.0))
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::svg_rasterizer::{
        SvgRenderSource, SvgRenderUnsupportedFeature, SvgRenderWarning,
    };

    fn report(fidelity: SvgRenderFidelity) -> SvgRenderReport {
        SvgRenderReport {
            requested_width: 256,
            requested_height: 256,
            output_width: 256,
            output_height: 256,
            rendered_element_count: 3,
            skipped_element_count: 1,
            warning_count: 1,
            unsupported_feature_count: 1,
            warnings: vec![SvgRenderWarning {
                code: "paint.unresolved_server".to_owned(),
                message: "missing paint".to_owned(),
                source: Some(SvgRenderSource {
                    node_id: 2,
                    byte_start: 10,
                    byte_end: 20,
                }),
            }],
            unsupported_features: vec![SvgRenderUnsupportedFeature {
                feature: "pattern".to_owned(),
                message: "patterns are diagnosed".to_owned(),
                source: None,
            }],
            fidelity,
            title: None,
            desc: None,
            recovered_error_count: 0,
        }
    }

    #[test]
    fn summary_rows_map_all_report_fields() {
        let s = report_summary(&report(SvgRenderFidelity::Medium));
        assert_eq!(s.fidelity, SvgRenderFidelity::Medium);
        assert_eq!(
            s.rows,
            vec![
                ("Fidelity".to_owned(), "Medium".to_owned()),
                ("Rendered".to_owned(), "3".to_owned()),
                ("Skipped".to_owned(), "1".to_owned()),
                ("Warnings".to_owned(), "1".to_owned()),
                ("Unsupported".to_owned(), "1".to_owned()),
            ]
        );
    }

    #[test]
    fn summary_includes_diagnostics_with_provenance() {
        let s = report_summary(&report(SvgRenderFidelity::Low));
        assert_eq!(s.warnings.len(), 1);
        assert_eq!(s.warnings[0].0, "paint.unresolved_server");
        assert!(s.warnings[0].1.contains("bytes 10..20"));
        assert_eq!(s.unsupported.len(), 1);
        assert_eq!(s.unsupported[0].0, "pattern");
        // No source span on this unsupported feature → no byte annotation.
        assert!(!s.unsupported[0].1.contains("bytes"));
    }

    #[test]
    fn a11y_and_recovery_rows_appear_only_when_present() {
        let mut r = report(SvgRenderFidelity::High);
        // Absent by default.
        let base = report_summary(&r);
        assert!(!base.rows.iter().any(|(k, _)| k == "Title"));
        assert!(!base.rows.iter().any(|(k, _)| k == "Recovered"));
        // Present when set.
        r.title = Some("Chart".to_owned());
        r.desc = Some("Quarterly revenue".to_owned());
        r.recovered_error_count = 2;
        let s = report_summary(&r);
        assert!(s.rows.contains(&("Title".to_owned(), "Chart".to_owned())));
        assert!(s
            .rows
            .contains(&("Description".to_owned(), "Quarterly revenue".to_owned())));
        assert!(s.rows.contains(&("Recovered".to_owned(), "2".to_owned())));
    }

    #[test]
    fn fidelity_labels_cover_all_levels() {
        assert_eq!(fidelity_label(SvgRenderFidelity::High), "High");
        assert_eq!(fidelity_label(SvgRenderFidelity::Medium), "Medium");
        assert_eq!(fidelity_label(SvgRenderFidelity::Low), "Low");
    }
}
