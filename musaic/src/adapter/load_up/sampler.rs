use std::collections::HashSet;

use bevy::{image::ImageSampler, prelude::*};

/// Integer upscale for UI sprites (32px art → 64px on screen).
pub const PIXEL_UI_SCALE: f32 = 2.0;

/// Integer upscale for board center icons (optional; mesh scale still applies).
pub const PIXEL_TILE_ICON_SCALE: f32 = 2.0;

#[derive(Resource, Default)]
pub struct NearestSamplerApplied(pub HashSet<AssetId<Image>>);

/// Apply nearest-neighbor filtering when the image asset is available.
pub fn ensure_nearest_sampler(
    images: &mut Assets<Image>,
    applied: &mut NearestSamplerApplied,
    handle: &Handle<Image>,
) {
    let id = handle.id();
    if applied.0.contains(&id) {
        return;
    }
    let Some(image) = images.get_mut(handle) else {
        return;
    };
    image.sampler = ImageSampler::nearest();
    applied.0.insert(id);
}

pub fn ensure_nearest_samplers(
    images: &mut Assets<Image>,
    applied: &mut NearestSamplerApplied,
    handles: impl IntoIterator<Item = Handle<Image>>,
) {
    for handle in handles {
        ensure_nearest_sampler(images, applied, &handle);
    }
}

/// Source texel size in UI points (integer scale, no fractional pixels).
pub fn pixel_ui_display_size(images: &Assets<Image>, handle: &Handle<Image>) -> Option<Vec2> {
    let image = images.get(handle)?;
    let size = image.size();
    if size.x == 0 || size.y == 0 {
        return None;
    }
    Some(Vec2::new(
        (size.x as f32 * PIXEL_UI_SCALE).round(),
        (size.y as f32 * PIXEL_UI_SCALE).round(),
    ))
}
