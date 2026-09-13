use bevy::prelude::*;
use load_up_macros::LoadAssetResource;

#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
struct BadAsset {
    texture: Handle<Image>,
}

fn main() {}
