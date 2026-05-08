//! Canvas technology translation belongs here.
//!
//! The future board migration should put draw-command mapping, rendering,
//! hit-testing, and input normalization in this module tree while leaving the
//! visible host component in `infrastructure::ui`.

pub mod draw_command;
pub mod hit_testing;
pub mod input_mapper;
pub mod nine_slice;
pub mod renderer;
pub mod sprite_atlas;
pub mod theme_bridge;

pub use draw_command::*;
pub use hit_testing::*;
pub use input_mapper::*;
pub use renderer::*;
pub use theme_bridge::*;
