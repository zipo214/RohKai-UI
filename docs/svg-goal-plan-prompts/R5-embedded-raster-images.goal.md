```text
/goal /caveman ultra
Read AGENTS.md/CLAUDE.md. Work only in D:\dev\rohkai.
Run: pwsh -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1
Read svg-zero-dep skill. Read: docs/SVG_RENDERER_ROADMAP.md (R5=truth),
src/svg_import.rs (<image> placeholder, max_image_data_uri_bytes cap, data: parsing at ~L1391),
src/canvas/svg_rasterizer.rs (DisplayList, PaintSampler, compositing/clip from R4, caps),
src/canvas/svg_golden.rs, src/codegen/export.rs (embedded copy + single-crate:: contract).
Precondition: R4 must be complete (image draw composites through the premultiplied/clip pipeline).

Goal: implement R5 embedded raster images end to end. First DECIDE and justify in the devlog:
implement zero-dependency PNG (and optionally baseline JPEG) decode for `data:` URIs in RohKai
source, OR keep image decode explicitly unsupported with clearer diagnostics/UI. No new crates either way.
Default recommendation: implement PNG `data:` decode now; JPEG baseline optional, else diagnosed.
Whatever ships must render real pixels in BOTH in-app and export-embedded rasterizers or be honestly
labeled unsupported. No "image supported" claim without real pixels.

Before coding, derive + REPORT from code:
1. how <image> currently imports (placeholder) and where data: bytes are bounded
2. pixel paths: in-app preview AND export.rs embedded copy (both change identically)
3. how a decoded bitmap would enter DisplayList + composite through R4 clip/premultiplied buffer
4. transforms/opacity/preserveAspectRatio applied to <image>; clip/mask interaction
5. which image diagnostics flip from placeholder->rendered vs stay unsupported
6. tests+goldens per path

If decode is chosen but a path won't be done now (e.g. JPEG), STOP and report; do not call PNG-only "images done".

Required (if implementing):
1. zero-dep PNG: chunk parse, zlib/inflate, all standard filter types, color types, alpha; interlace
   either supported or explicitly diagnosed+rejected; bounded decode memory/CPU + pixel caps
2. (optional) baseline JPEG: DCT/Huffman/quantization/YCbCr; if deferred, diagnose progressive+JPEG clearly
3. draw decoded bitmap via DisplayList: x/y/width/height, preserveAspectRatio, opacity, transforms;
   composite through R4 premultiplied buffer + active clip; failure -> placeholder + diagnostic, never panic
4. external href stays rejected (security); only inline data: allowed; honor data URI byte cap
5. both embedded sources std-only; single-crate:: export contract still passes

Tests:
1. goldens: small PNG data: (RGB + RGBA), image under a clip, image with opacity/transform
2. security/malformed: truncated PNG, oversized declared dims, bad CRC/filter, external href -> bounded fail+diagnostic
3. determinism + fidelity-score (rendered vs placeholder); memory/pixel cap test
4. export parity: ignored all-built-in exported-project cargo check still passes
5. invariant: every image case rendering in-app also renders in exported copy

Verify (zero warnings): cargo fmt --check; cargo check; cargo test; cargo clippy -- -D warnings;
pwsh scripts\validate-svg-import.ps1; pwsh scripts\check-text-encoding.ps1; cargo run (data: PNG Image previews real pixels).

Docs: flip only truly-done R5 tasks to [x] in SVG_RENDERER_ROADMAP.md + update limits/gap rows; record the
decode decision; update SVG_IMPORT.md + feature-evaluation; append CODE_COOP.md handoff + DEVLOG.md entry (next=R6).

Final report: decision+rationale, path matrix, paths changed, tests+goldens, verification numbers, gaps only if excluded before editing.
Success: chosen image policy renders real pixels (or is honestly unsupported) identically in-app and exported,
through R4 clip/compositing, with security/determinism/export-parity tests green, zero warnings. Next: R6.
```
