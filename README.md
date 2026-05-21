# Rohkai ^ρϗ

A pure Rust, native WYSIWYG GUI designer for egui applications.
No gap between what you design and what gets generated.

## The Name

*Rohkai* derives from *Rocaille* — the French Baroque ornamental style
defined by elaborate shell-and-stone window surrounds and decorative
frames. A Rocaille is an opening made meaningful by what surrounds it.

The deeper inspiration comes from the medieval cathedral craftsman's
*épure* — the 1:1-scale geometric drawing traced on the lodge floor
from which every vault, arch, and stone was cut. As architectural
historian Sara Galletti describes, épures were "traced on site" by the
*appareilleur* and "referred to throughout the execution process" —
they were not plans *of* the building, they *were* the building before
it existed in stone.¹

Research into the Lady Chapel vaults at Ely Cathedral by Webb and
Buchanan at the University of Liverpool further documents how medieval
masons "used a 2D tracing floor to experiment with ideas in plan"
before projecting vault geometry into three dimensions.²

Rohkai is the épure for your Rust UI. The canvas and the code are the
same object in two different states of matter.

The mark *^ρϗ* — rho and koppa, archaic Greek letters used as
craftsmen's marks — appears in every file Rohkai generates:

    // ^ρϗ Rohkai — generated interface

## What It Does

- Drag widgets onto a canvas
- See correct, position-aware egui Rust code generate live
- Export a complete, compilable Rust project
- `cargo run` the export — your designed UI launches as a standalone
  native app with no connection to Rohkai

## Inspirations

- **Lazarus / Delphi** — the tightest WYSIWYG-to-code loop ever built
  for native desktop applications. The form *was* the code. Nothing
  since has matched it.
- **Ply** (https://plyx.iz.rs) — pure Rust UI framework built from
  frustration with everything else. The right spirit.
- **egui** — immediate mode, stateless, redraws every frame. The right
  foundation.

## Built With

Pure Rust. No C FFI. No system toolkit bindings. Everything via Cargo.

## Status

Active development. Stages 0–5 complete.

---

> *I totally apologize if I can't read French and butchered the translations and history, citations are below.*

¹ Galletti, S. (2020). *Épures d'architecture: Geometric Constructions
for Vault Building in Philibert de L'Orme's Premier tome de
l'architecture (1567)*. Opus Incertum, 6, 76–89.
https://doi.org/10.13128/opus-12362

² Webb, N. & Buchanan, A. (2018). *Tracing the Past: a digital analysis
of the Lady Chapel vaults at Ely cathedral*. University of Liverpool.
https://livrepository.liverpool.ac.uk/3028581/
