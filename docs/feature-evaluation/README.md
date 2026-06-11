# RohKai Feature Evaluation

This folder evaluates RohKai by feature area. It is deliberately more concrete
than the roadmap: the roadmap says what we plan to build, while these documents
say how deep each feature is, how useful it is, how it should behave in an ideal
state, and how to measure whether it got there.

## How To Read This Folder

- `depth-model.md` defines the shared scoring language.
- Each area document compares RohKai to top-class expectations from tools such
  as Qt Designer, Lazarus/Delphi, Figma-style design tools, visual app builders,
  and modern code-aware IDEs.
- "Current state" describes the repo as of this evaluation, including known MVPs
  and stubs. It should not be used as marketing copy.
- "Ideal state" describes what top-of-class RohKai should become, not what is
  already true.

## Evaluation Areas

| Area | Document | Main Question |
|---|---|---|
| Depth vocabulary | `depth-model.md` | What does "real", "MVP", or "top-class" mean? |
| Product shell and navigation | `app-shell-navigation.md` | Can users find, organize, and control the app comfortably? |
| Canvas authoring | `canvas-authoring.md` | Is the visual designer precise, inspectable, and fast? |
| Widget and component catalog | `widgets-and-components.md` | Do widgets behave like real egui output and competitor-depth components? |
| Codegen, Lazare, and export | `codegen-lazare-export.md` | Does canvas/code/export stay one source of truth? |
| Rust-centric visual features | `rust-centric-visual-features.md` | How deep are Stage 11 overlays, Rust wiring, async, traits, iterators, and macro snippets? |
| Remaining roadmap items | `remaining-roadmap-items.md` | What is still unchecked, what exists nearby, and what exactly closes each gap? |
| SVG import and renderer | `svg-import-renderer.md` | How close are importer and renderer to mature SVG engines? |
| Custom widget system | `custom-widget-system.md` | Are descriptors, builder, and future maker distinct and useful? |
| Project infrastructure | `project-infrastructure.md` | Are files, assets, undo, project views, and templates robust? |
| Preferences, theming, and platform | `preferences-theming-platform.md` | Can users adapt RohKai and exported apps to their environment? |
| Testing and quality gates | `testing-quality.md` | How do we prove features work and stay working? |

## Global Product Depth Summary

| Feature Family | Current Depth | Top-Class Target |
|---|---:|---|
| Core canvas editing | 3-4 | 5 |
| Codegen/export loop | 4 | 5 |
| Lazare bidirectional code sync | 3 | 5 |
| Basic widget catalog | 3 | 5 |
| Advanced widgets/data views | 1-2 | 5 |
| Non-visual components | 1-2 | 5 |
| Custom descriptor system | 3 | 5 |
| True visual widget maker | 1-2 | 5 |
| SVG importer | 3 | 5 |
| SVG raster preview/export | 4 | 5 |
| Project infrastructure | 3 | 5 |
| Preferences/theming | 3 | 5 |
| Testing/verification | 3-4 | 5 |

RohKai's strongest differentiator is the live loop: `UiTree` drives canvas,
code preview, AppState, and export. Its largest depth gaps are model-bound data
widgets, runtime non-visual components, true formula/charting systems, the visual
widget maker, and full SVG/text rendering.
