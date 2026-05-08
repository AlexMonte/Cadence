//! Sample lookup and decode bindings owned by Cadence.

mod builtin;
mod catalog;
mod loader;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use self::builtin::bundled_sample_root;
#[cfg(target_arch = "wasm32")]
pub(crate) use self::builtin::bundled_web_sample_urls;
pub(crate) use self::builtin::{DEFAULT_KICK_SELECTOR, legacy_sample_source};
pub(crate) use self::catalog::{PathSampleCatalog, ResolvedSample, SampleCatalogEntry};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use self::loader::load_sample;
#[cfg(target_arch = "wasm32")]
pub(crate) use self::loader::load_sample_bytes;
