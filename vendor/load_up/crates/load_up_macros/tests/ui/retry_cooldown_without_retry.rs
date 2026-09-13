use bevy::prelude::*;
use load_up_macros::LoadAssetResource;

#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
struct BadAsset {
    #[dependency(path = "a.png", retry_cooldown = 1.0)]
    texture: Handle<Image>,
}

fn main() {}
