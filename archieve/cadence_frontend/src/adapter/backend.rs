//! Backend bridge surface used by the frontend application.

#[cfg(not(target_arch = "wasm32"))]
mod native;
pub mod runtime;
pub mod types;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
pub use runtime::*;
pub use types::*;
#[cfg(target_arch = "wasm32")]
pub use web::*;
