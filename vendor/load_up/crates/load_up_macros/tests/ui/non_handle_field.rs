use bevy::prelude::*;
use load_up_macros::LoadAssetResource;

#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
struct BadAsset {
    #[dependency(path = "a.png")]
    label: String,
}

fn main() {}
