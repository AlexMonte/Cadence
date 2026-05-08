# ADR-006: Canvas Host in Infrastructure, Renderer in Adapter

- Status: Proposed

## Context

The editor board needs a future canvas-based surface for dense spatial editing,
custom hit testing, and sprite-based rendering. Canvas introduces the same
ambiguity Dioxus did:

- the visible canvas element is part of the user-facing surface
- the rendering and input translation logic are technology binding

## Decision

- Put the visible board canvas host in `infrastructure::ui`.
- Put draw-command mapping, rendering, hit-testing, input normalization, theme
  bridging, and sprite helpers in `adapter::canvas`.

## Consequences

- Infrastructure remains the concrete surface the user interacts with.
- Adapter remains a translation layer instead of a second application layer.
- Application editor state stays independent from rendering technology.
