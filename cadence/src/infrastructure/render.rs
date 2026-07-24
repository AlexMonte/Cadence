//! Backend-agnostic host-facing projection/render contract.
//!
//! `RendererCore` is score-first. Hosts that need the full projected meaning
//! of a transport window should prefer `projected_output()`. Thin
//! `projected_mosaic()` output remains available as a lossy compatibility view.

pub use crate::application::renderer_core::RendererCore;
