# Codex Handoff - RohKai Behavior System

## Context

Claude previously worked on introducing a visual behavior system into RohKai.

The existing project already contained:
- Widget bindings
- Event handler fields
- AppState generation
- Export codegen
- Rust Wiring (advanced infrastructure)

A gap existed between widget events and state mutation.

## Architectural Direction

Three layers are desired:

### Visual Behaviors
Beginner-facing wiring.

Examples:
- Button click -> increment progress
- Checkbox -> toggle bool
- Slider -> set value

### Action Graph
Structured application logic.

Examples:
- Branching
- Conditions
- Math
- Validation
- Function calls

### Global Rust Wiring
Advanced infrastructure.

Examples:
- Channels
- Async plumbing
- Iterator pipelines
- Trait implementations
- Custom Rust handlers

Rust Wiring should be reframed as Global Rust Wiring.

## Important Principle

Behavior Graph is the source of truth.

Recipes are NOT the source of truth.

Bad:
Wire -> generated Rust blob

Good:
Wire -> Recipe -> Typed Actions -> Codegen -> Rust

## Recipe Concept

Connector gesture should predict likely behaviors.

Examples:

Button Click + f32:
- Increment
- Reset
- Set value

Button Click + bool:
- Toggle

Checkbox Change + bool:
- Bind checked state

Recipes create typed actions.

## Long-Term Goal

Canvas wire:
Event -> Behavior

Behavior:
Action Graph

Action Graph:
Typed actions

Codegen:
Generates Rust

Global Rust Wiring:
Advanced escape hatch

The editor should feel like a hybrid of:
- Qt Designer
- Visual Studio event designer
- Node-RED
- Blueprint-style interaction authoring

while preserving RohKai's UiTree-centric architecture.
