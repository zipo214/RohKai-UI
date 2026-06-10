# Custom Widget System Evaluation

## Scope

This covers `.rkwd` descriptors, advanced descriptor editor, guided descriptor
builder, custom widgets in the palette/canvas/properties/codegen/export, and the
future true Visual Widget Maker.

## Top-Class Expectation

Top-class extensibility lets beginners create useful reusable widgets visually,
while power users can define precise codegen, properties, events, dependencies,
and preview behavior. The source of truth should remain a descriptor model that
can be validated, shared, versioned, and tested.

## Current State

| Feature | Current Depth | What It Does Now | Gap |
|---|---:|---|---|
| Descriptor format | 3-4 | `.rkwd` defines metadata, props, state fields, templates, events, deps. | Needs versioning/migration and richer validation UX. |
| Descriptor editor | 3 | Full power-user editor with live preview. | Needs better template diagnostics and guided safety checks. |
| Guided builder | 2-3 | Beginner form over descriptor defaults/templates. | It is not a true WYSIWYG widget construction tool. |
| Palette integration | 3 | Loaded descriptors appear as custom widgets. | Needs hot reload health details and preview thumbnails. |
| Custom properties | 3 | Descriptor props appear in properties panel and codegen. | Needs typed enum UI, constraints, docs, and dependency conflict handling. |
| Future visual maker | 0-1 | Design doc exists. | Needs internal maker document, primitive canvas, deterministic descriptor generation. |

## The Three Layers

| Layer | User | Purpose | Current State |
|---|---|---|---|
| Guided Descriptor Builder | Beginner | Make simple Label/Button-like descriptors safely. | Implemented MVP. |
| Advanced Descriptor Editor | Power user | Edit descriptor schema and code templates directly. | Implemented. |
| Visual Widget Maker | Designer/builder | Construct reusable widgets from visual primitives. | Planned. |

## Utility

- Extensibility utility: very high.
- Authoring utility: high when builder/maker is approachable.
- Runtime utility: high when descriptors export reliably.
- Safety utility: high because descriptors can inject dependencies/codegen.

## Ideal State

| Capability | Ideal Behavior |
|---|---|
| Visual maker document | Internal primitive tree with rect/text/hit regions, exposed properties, and states. |
| Descriptor generation | Visual maker deterministically emits validated `.rkwd` descriptors. |
| Template safety | Tokens validated with clear errors, generated code preview, dependency policy checks. |
| Sharing | Bundle format with descriptor, preview, assets, version, examples, and tests. |
| Marketplace-ready metadata | Category, tags, screenshots, compatibility, dependencies, author, license. |
| Migration | Descriptor versions migrate forward safely. |

## Depth Measurements

| Measure | Target For Top-Class |
|---|---|
| Descriptor validity | Invalid descriptors explain every failure and cannot corrupt palette load. |
| Custom export compile | Projects using descriptors compile with generated dependencies. |
| Builder success | Beginner can create a reusable button/label widget in under 3 minutes. |
| Maker depth | User can visually compose a widget without raw Rust templates. |
| Dependency safety | Descriptor dependencies are explicit, deduped, and policy-checked. |

## Recommended Next Work

1. Implement `WidgetMakerDocument` and primitive vertical slice.
2. Add descriptor version field and migration tests.
3. Add generated descriptor compile/export fixture.
4. Add descriptor preview thumbnails and load-health panel.
5. Define `.rkwb` bundle format after maker primitives exist.

