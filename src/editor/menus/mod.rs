//! The game's menus and transitions between them.

mod load_menu;

mod project_hub;
mod settings;
mod text_draft;
pub use project_hub::ProjectHubNode;
pub use text_draft::{TextDraftCommitted, TextDraftState};

use bevy::{
    image::{ImageLoaderSettings, ImageSampler},
    prelude::*,
};

use crate::editor::ui::buttons::ButtonAsset;
use crate::editor::ui::styles::ButtonImages;
use crate::shared::resources::LoadResource;

pub(super) fn plugin(app: &mut App) {
    app.init_state::<Menu>();
    app.load_resource::<MenuAssets>();

    app.add_plugins((
        load_menu::plugin,
        project_hub::plugin,
        settings::plugin,
        text_draft::plugin,
    ));
}

#[derive(States, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub enum Menu {
    #[default]
    None,
    LoadProject,
    Settings,
    Credits,
    ProjectHub,
    TextDraft,
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct MenuAssets {
    #[dependency]
    pub menu_button_normal: Handle<Image>,
    #[dependency]
    pub menu_button_hover: Handle<Image>,
    #[dependency]
    pub menu_button_pressed: Handle<Image>,
    #[dependency]
    pub menu_button_focused: Handle<Image>,
    #[dependency]
    pub menu_button_disabled: Handle<Image>,
    #[dependency]
    menu_background: Handle<Image>,
}

impl FromWorld for MenuAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            menu_button_normal: load_nearest_image(
                assets,
                "images/ui/button_normal/normal_off.png",
            ),
            menu_button_hover: load_nearest_image(
                assets,
                "images/ui/button_normal/normal_hover.png",
            ),
            menu_button_pressed: load_nearest_image(
                assets,
                "images/ui/button_normal/normal_on.png",
            ),
            menu_button_focused: load_nearest_image(
                assets,
                "images/ui/button_normal/normal_focus.png",
            ),
            menu_button_disabled: load_nearest_image(
                assets,
                "images/ui/button_normal/normal_disabled.png",
            ),
            menu_background: load_nearest_image(assets, "images/ui/background.png"),
        }
    }
}

fn load_nearest_image(assets: &AssetServer, path: &'static str) -> Handle<Image> {
    assets.load_with_settings(path, |settings: &mut ImageLoaderSettings| {
        // Preserve pixel-art look for menu textures.
        settings.sampler = ImageSampler::nearest();
    })
}

impl ButtonAsset<MenuAssets> for MenuAssets {
    fn pressed(&self) -> Handle<Image> {
        self.menu_button_pressed.clone()
    }

    fn hovered(&self) -> Handle<Image> {
        self.menu_button_hover.clone()
    }

    fn none(&self) -> Handle<Image> {
        self.menu_button_disabled.clone()
    }
}

impl From<&MenuAssets> for ButtonImages {
    fn from(value: &MenuAssets) -> Self {
        ButtonImages::new(
            value.menu_button_normal.clone(),
            value.menu_button_hover.clone(),
            value.menu_button_pressed.clone(),
        )
    }
}
