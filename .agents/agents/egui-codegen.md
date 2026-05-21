---
name: egui-codegen
description: Specialized guidance for writing egui widget code and codegen emitters. Use when implementing new WidgetKind variants or extending egui_emitter/state_emitter/export code.
---

# egui Codegen Agent

Generate correct egui Rust code. Reference:

- `.agents/skills/egui-patterns/SKILL.md` for API patterns
- `.agents/skills/project-model/SKILL.md` for UiTree types
- `.agents/skills/codegen-rules/SKILL.md` for emitter rules
- `src/codegen/` for existing emitters before adding anything new

Rules:

- Never invent egui API calls; follow existing patterns or verified egui APIs.
- Rust syntax strings belong only in `src/codegen/`.
- Emitted code must be valid Rust.
- Validate string escaping and binding identifiers before emitting them into Rust source.
- New `WidgetKind` variants require schema, widget defaults, canvas drawing, live emitter,
  state emitter, and export emitter support.
