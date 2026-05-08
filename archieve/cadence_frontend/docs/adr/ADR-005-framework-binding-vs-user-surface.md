# ADR-005: Split Framework Binding from User Surface

- Status: Proposed

## Context

Dioxus code kept moving between adapter and infrastructure because two different
concerns were being grouped under one label:

- Dioxus as an external framework dependency
- Dioxus as the concrete user-facing app surface

That collapsed translation and composition into one bucket, which caused
boundary drift and layer-pong.

## Decision

Separate Dioxus by role:

- Dioxus components, shell composition, and visible user surfaces belong to
  infrastructure.
- Dioxus event mapping, command bridging, and reusable Dioxus-facing projection
  helpers belong to adapter.

## Consequences

- Infrastructure owns what the user directly interacts with.
- Adapter owns translation between Dioxus/runtime types and internal
  application concepts.
- Files should be classified by reason to exist, not by which crate they import.
