```text
/goal /caveman ultra
Read AGENTS.md/CLAUDE.md. Work only in D:\dev\rohkai.
Run: pwsh -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1
Read svg-zero-dep skill. Read: docs/SVG_RENDERER_ROADMAP.md (Post-R8 lanes = truth),
src/canvas/svg_rasterizer.rs (XmlParser/parse_tag namespace stripping, SvgScene, SvgRenderReport),
src/svg_import.rs (scan_svg, ImportContext, SvgImportOutput report), src/panels/svg_report.rs (report UI),
src/codegen/export.rs (single-crate:: contract). Note: parser currently strips namespace prefixes.

Goal: implement R12 namespace model + malformed-document recovery + accessibility metadata end to end. No new
crates. Bounded, deterministic, diagnosed; preserve secure-static profile.

Before coding, derive + REPORT from code:
1. how parse_tag handles qualified names today (prefix stripping) and where xmlns scoping would attach
2. every hard-reject path in the parser/importer that mature engines recover from
3. where title/desc currently get dropped + how they'd surface in the R8 report panel + export
4. tests per path

Required:
1. Bounded namespace model: track xmlns/xmlns:* scoping per element; resolve qualified names to a small known
   set (svg, xlink) and treat foreign-namespace elements as skipped-with-diagnostic rather than mis-parsed.
   Keep the secure profile (still reject DOCTYPE/entities/scripts/external). Bounded scope-stack depth.
2. Malformed-document recovery: where safe, recover-and-diagnose instead of hard-fail — unclosed tags,
   stray text, mismatched close tags, junk attributes — producing partial output + a recovery diagnostic
   (not a whole-document ParseFailed). Keep hard-fail only for security gates. Add recovery counters to the
   report.
3. Accessibility metadata: parse <title>/<desc> (+ aria-label/role where present); surface them in the
   SvgRenderReport / SvgImportOutput report and the R8 properties panel; preserve on export. Bounded length.
4. both embedded sources std-only; single-crate:: export contract still passes; honor existing caps.

Tests:
1. namespace: xlink:href + a custom-namespace element (skipped+diagnosed, not mis-rendered); xmlns scoping
2. recovery: unclosed/mismatched tags + junk -> partial render + recovery diagnostic, no panic, deterministic
3. a11y: title/desc extracted into report + visible row; bounded length
4. security regression: DOCTYPE/entity/script/external still rejected
5. export parity: ignored all-built-in exported-project cargo check still passes

Verify (zero warnings): cargo fmt --check; cargo check; cargo test; cargo clippy --all-targets -- -D warnings;
pwsh scripts\validate-svg-import.ps1; pwsh scripts\check-text-encoding.ps1; cargo run (a11y row + recovery in report).

Docs: flip R12 tasks to [x] in SVG_RENDERER_ROADMAP.md + update XML model / a11y / recovery gap rows + the
maturity assessment (renderer-grade progress); update SVG_IMPORT.md + feature-evaluation; append CODE_COOP.md
handoff + DEVLOG.md entry (mark post-R8 lanes complete or note remaining).

Final report: path matrix, paths changed, tests, verification numbers, gaps only if excluded. Success:
bounded namespace model + malformed recovery + a11y metadata, secure profile intact, surfaced in the report,
tests green, zero warnings, no new deps. Post-R8 lanes complete.
```
