#[cfg(target_arch = "wasm32")]
use manganis::{Asset, AssetOptions, asset};
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

pub(crate) const DEFAULT_KICK_SELECTOR: &str = "/kick/";
pub(crate) const LEGACY_BD_SOURCE: &str = "builtin://samples/bd.wav";

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WebBundledSampleEntry {
    pub key: &'static str,
    pub web_path: &'static str,
}

#[cfg(target_arch = "wasm32")]
pub(crate) const WEB_SAMPLE_ASSET_ROOT: Asset =
    asset!("/assets/generated_samples", AssetOptions::folder());

#[cfg(target_arch = "wasm32")]
include!(concat!(env!("OUT_DIR"), "/web_sample_catalog.rs"));

pub(crate) fn legacy_sample_source(source: &str) -> Option<&'static str> {
    match source {
        LEGACY_BD_SOURCE => Some(DEFAULT_KICK_SELECTOR),
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn bundled_web_sample_urls() -> Vec<(String, String)> {
    let root = WEB_SAMPLE_ASSET_ROOT
        .to_string()
        .trim_end_matches('/')
        .to_string();
    WEB_SAMPLE_ENTRIES
        .iter()
        .map(|entry| (entry.key.to_string(), format!("{root}/{}", entry.web_path)))
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn bundled_sample_root() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("samples");
    path.is_dir().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_bd_source_maps_to_default_kick_selector() {
        assert_eq!(
            legacy_sample_source(LEGACY_BD_SOURCE),
            Some(DEFAULT_KICK_SELECTOR)
        );
        assert_eq!(legacy_sample_source(DEFAULT_KICK_SELECTOR), None);
    }
}
