//! Inspector panel bodies, one module per [`InspectorPanelKind`] variant.
//!
//! [`InspectorPanelKind`]: crate::application::editor::InspectorPanelKind

pub mod drawer;
mod instrument_shape;
mod name_filter;
pub mod placement_prompt;
pub mod project_overview;
pub mod sample_options;
pub mod selection;
pub mod sound;
mod sound_library;
pub mod tile_inspect;
mod tile_presentation;
pub mod timeline_event;

pub(crate) mod effect_tiles;
pub(crate) mod modulation_tiles;

mod flow;

mod flow_member_name;
mod musical_patterns;

pub mod tricks;
