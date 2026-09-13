use bevy::prelude::*;
use load_up_macros::LoadAssetResource;

#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
struct BadAsset {
    #[dependency(path = "a.png", fallback = "b.png", retry = 2)]
    texture: Handle<Image>,
}

fn main() {}
