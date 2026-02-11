//! Shared asset loading helpers and resource-handle tracking.

use std::collections::VecDeque;

use bevy::{image::ImageSampler, prelude::*};

use crate::editor::screens::Screen;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ResourceHandles>();
    app.add_systems(
        Update,
        display_loading_debug.run_if(in_state(Screen::Loading)),
    );
    app.add_systems(PreUpdate, load_resource_assets);
}

pub trait LoadResource {
    fn load_resource<T: Resource + Asset + Clone + FromWorld>(&mut self) -> &mut Self;
}
fn display_loading_debug(resources: Res<ResourceHandles>) {
    let (loaded, total) = resources.get_progress();
    if loaded < total {
        log::info!("Waiting: {}, Finished: {}", total - loaded, loaded);
    }
}

impl LoadResource for App {
    fn load_resource<T: Resource + Asset + Clone + FromWorld>(&mut self) -> &mut Self {
        self.init_asset::<T>();
        let world = self.world_mut();
        let value = T::from_world(world);
        let assets = world.resource::<AssetServer>();
        let handle = assets.add(value);
        let mut handles = world.resource_mut::<ResourceHandles>();

        handles
            .waiting
            .push_back((handle.untyped(), |world, handle| {
                let assets = world.resource::<Assets<T>>();
                if let Some(value) = assets.get(handle.id().typed::<T>()) {
                    world.insert_resource(value.clone());
                }
            }));
        self
    }
}

/// A function that inserts a loaded resource.
type InsertLoadedResource = fn(&mut World, &UntypedHandle);

#[derive(Resource, Default)]
pub struct ResourceHandles {
    // Use a queue for waiting assets so they can be cycled through and moved to
    // `finished` one at a time.
    waiting: VecDeque<(UntypedHandle, InsertLoadedResource)>,
    finished: Vec<UntypedHandle>,
}

impl ResourceHandles {
    /// Returns true if all requested [`Asset`]s have finished loading and are available as [`Resource`]s.
    pub fn is_all_done(&self) -> bool {
        self.waiting.is_empty()
    }

    /// Returns the loading progress as (loaded_count, total_count).
    pub fn get_progress(&self) -> (usize, usize) {
        let total = self.waiting.len() + self.finished.len();
        let loaded = self.finished.len();
        (loaded, total)
    }
}

fn load_resource_assets(world: &mut World) {
    world.resource_scope(|world, mut resource_handles: Mut<ResourceHandles>| {
        world.resource_scope(|world, assets: Mut<AssetServer>| {
            let pending = resource_handles.waiting.len();
            for _ in 0..pending {
                let Some((handle, insert_fn)) = resource_handles.waiting.pop_front() else {
                    break;
                };
                if assets.is_loaded_with_dependencies(&handle) {
                    insert_fn(world, &handle);
                    resource_handles.finished.push(handle);
                } else {
                    resource_handles.waiting.push_back((handle, insert_fn));
                }
            }
        });
    });
}

/// Create a texture atlas with the given padding and sampling settings
/// from the individual sprites in the given folder.
pub fn create_texture_atlas(
    image_handle: Handle<Image>,
    padding: Option<UVec2>,
    sampling: Option<ImageSampler>,
    textures: &mut ResMut<Assets<Image>>,
) -> (TextureAtlasLayout, TextureAtlasSources, Handle<Image>) {
    // Build a texture atlas using the individual sprites
    let mut texture_atlas_builder = TextureAtlasBuilder::default();
    texture_atlas_builder.padding(padding.unwrap_or_default());

    // TextureAtlasBuilder::add_texture expects a reference to the Image, not a Handle<Image>.
    let image = textures
        .get(&image_handle)
        .expect("Image asset must be available in Assets<Image> to build a texture atlas");
    texture_atlas_builder.add_texture(Some(image_handle.id()), image);

    let (texture_atlas_layout, texture_atlas_sources, texture) = texture_atlas_builder
        .build()
        .expect("Texture atlas should build from loaded image inputs");
    let texture = textures.add(texture);

    // Update the sampling settings of the texture atlas
    let image = textures
        .get_mut(&texture)
        .expect("Newly added texture atlas image must be mutable in Assets<Image>");
    image.sampler = sampling.unwrap_or_default();

    (texture_atlas_layout, texture_atlas_sources, texture)
}

/// Create and spawn a sprite from a texture atlas
pub fn create_sprite_from_atlas(
    commands: &mut Commands,
    translation: (f32, f32, f32),
    atlas_texture: Handle<Image>,
    atlas_sources: TextureAtlasSources,
    atlas_handle: Handle<TextureAtlasLayout>,
    vendor_handle: &Handle<Image>,
) {
    commands.spawn((
        Transform {
            translation: Vec3::new(translation.0, translation.1, translation.2),
            scale: Vec3::splat(3.0),
            ..default()
        },
        Sprite::from_atlas_image(
            atlas_texture,
            atlas_sources
                .handle(atlas_handle, vendor_handle)
                .expect("Vendor image must exist in atlas sources"),
        ),
    ));
}
