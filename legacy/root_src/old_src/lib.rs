pub(crate) mod adapter;
pub(crate) mod application;
pub(crate) mod domain;
pub mod infrastructure;

#[cfg(target_arch = "wasm32")]
pub mod web;

pub use infrastructure::dto::SharedAppState;
