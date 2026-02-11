//! UI asset handles and loaders for images used throughout the editor.

use bevy::{
    image::{ImageLoaderSettings, ImageSampler},
    prelude::*,
};

use crate::editor::ui::styles::ButtonImages;

pub fn load_ui_image(assets: &AssetServer, path: &'static str) -> Handle<Image> {
    assets.load_with_settings(path, |settings: &mut ImageLoaderSettings| {
        // Use nearest sampling to preserve pixel art style for UI.
        settings.sampler = ImageSampler::nearest();
    })
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiButtonNormalAssets {
    #[dependency]
    pub off: Handle<Image>,
    #[dependency]
    pub on: Handle<Image>,
    #[dependency]
    pub hover: Handle<Image>,
    #[dependency]
    pub focus: Handle<Image>,
    #[dependency]
    pub disabled: Handle<Image>,
}

impl FromWorld for UiButtonNormalAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            off: load_ui_image(assets, "images/ui/button_normal/normal_off.png"),
            on: load_ui_image(assets, "images/ui/button_normal/normal_on.png"),
            hover: load_ui_image(assets, "images/ui/button_normal/normal_hover.png"),
            focus: load_ui_image(assets, "images/ui/button_normal/normal_focus.png"),
            disabled: load_ui_image(assets, "images/ui/button_normal/normal_disabled.png"),
        }
    }
}

impl From<&UiButtonNormalAssets> for ButtonImages {
    fn from(value: &UiButtonNormalAssets) -> Self {
        ButtonImages::new(value.off.clone(), value.hover.clone(), value.on.clone())
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiButtonLongAssets {
    #[dependency]
    pub off: Handle<Image>,
    #[dependency]
    pub on: Handle<Image>,
    #[dependency]
    pub hover: Handle<Image>,
    #[dependency]
    pub focus: Handle<Image>,
    #[dependency]
    pub disabled: Handle<Image>,
}

impl FromWorld for UiButtonLongAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            off: load_ui_image(assets, "images/ui/button_long/long_off.png"),
            on: load_ui_image(assets, "images/ui/button_long/long_on.png"),
            hover: load_ui_image(assets, "images/ui/button_long/long_hover.png"),
            focus: load_ui_image(assets, "images/ui/button_long/long_focus.png"),
            disabled: load_ui_image(assets, "images/ui/button_long/long_disable.png"),
        }
    }
}

impl From<&UiButtonLongAssets> for ButtonImages {
    fn from(value: &UiButtonLongAssets) -> Self {
        ButtonImages::new(value.off.clone(), value.hover.clone(), value.on.clone())
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiButtonBigAssets {
    #[dependency]
    pub normal: Handle<Image>,
    #[dependency]
    pub hover: Handle<Image>,
    #[dependency]
    pub pressed: Handle<Image>,
    #[dependency]
    pub focused: Handle<Image>,
    #[dependency]
    pub disabled: Handle<Image>,
}

impl FromWorld for UiButtonBigAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            normal: load_ui_image(assets, "images/ui/button_big/bouton_normal.png"),
            hover: load_ui_image(assets, "images/ui/button_big/bouton_hover.png"),
            pressed: load_ui_image(assets, "images/ui/button_big/bouton_pressed.png"),
            focused: load_ui_image(assets, "images/ui/button_big/bouton_focused.png"),
            disabled: load_ui_image(assets, "images/ui/button_big/bouton_disabled.png"),
        }
    }
}

impl From<&UiButtonBigAssets> for ButtonImages {
    fn from(value: &UiButtonBigAssets) -> Self {
        ButtonImages::new(
            value.normal.clone(),
            value.hover.clone(),
            value.pressed.clone(),
        )
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiButtonSkewedAssets {
    #[dependency]
    pub normal: Handle<Image>,
    #[dependency]
    pub hover: Handle<Image>,
    #[dependency]
    pub pressed: Handle<Image>,
    #[dependency]
    pub focused: Handle<Image>,
    #[dependency]
    pub disabled: Handle<Image>,
}

impl FromWorld for UiButtonSkewedAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            normal: load_ui_image(assets, "images/ui/button_skewed/button_back_normal.png"),
            hover: load_ui_image(assets, "images/ui/button_skewed/button_back_hover.png"),
            pressed: load_ui_image(assets, "images/ui/button_skewed/button_back_pressed.png"),
            focused: load_ui_image(assets, "images/ui/button_skewed/button_back_focused.png"),
            disabled: load_ui_image(assets, "images/ui/button_skewed/button_back_disabled.png"),
        }
    }
}

impl From<&UiButtonSkewedAssets> for ButtonImages {
    fn from(value: &UiButtonSkewedAssets) -> Self {
        ButtonImages::new(
            value.normal.clone(),
            value.hover.clone(),
            value.pressed.clone(),
        )
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiButtonArrowAssets {
    #[dependency]
    pub normal: Handle<Image>,
    #[dependency]
    pub hover: Handle<Image>,
    #[dependency]
    pub pressed: Handle<Image>,
    #[dependency]
    pub focused: Handle<Image>,
    #[dependency]
    pub disabled: Handle<Image>,
}

impl FromWorld for UiButtonArrowAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            normal: load_ui_image(assets, "images/ui/button_arrow/flechebutton_normal.png"),
            hover: load_ui_image(assets, "images/ui/button_arrow/flechebutton_hover.png"),
            pressed: load_ui_image(assets, "images/ui/button_arrow/flechebutton_on.png"),
            focused: load_ui_image(assets, "images/ui/button_arrow/flechebutton_focus.png"),
            disabled: load_ui_image(assets, "images/ui/button_arrow/flechebutton_disabledl.png"),
        }
    }
}

impl From<&UiButtonArrowAssets> for ButtonImages {
    fn from(value: &UiButtonArrowAssets) -> Self {
        ButtonImages::new(
            value.normal.clone(),
            value.hover.clone(),
            value.pressed.clone(),
        )
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiButtonTabAssets {
    #[dependency]
    pub normal: Handle<Image>,
    #[dependency]
    pub pressed: Handle<Image>,
    #[dependency]
    pub disabled: Handle<Image>,
}

impl FromWorld for UiButtonTabAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            normal: load_ui_image(assets, "images/ui/button_tab/normal.png"),
            pressed: load_ui_image(assets, "images/ui/button_tab/pressed.png"),
            disabled: load_ui_image(assets, "images/ui/button_tab/disabled.png"),
        }
    }
}

impl From<&UiButtonTabAssets> for ButtonImages {
    fn from(value: &UiButtonTabAssets) -> Self {
        ButtonImages::new(
            value.normal.clone(),
            value.normal.clone(),
            value.pressed.clone(),
        )
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiButtonTabsAssets {
    #[dependency]
    pub button: Handle<Image>,
    #[dependency]
    pub button_hover: Handle<Image>,
    #[dependency]
    pub button_on: Handle<Image>,
    #[dependency]
    pub button_focus: Handle<Image>,
    #[dependency]
    pub button_disabled: Handle<Image>,
    #[dependency]
    pub button_main: Handle<Image>,
    #[dependency]
    pub button_main_hover: Handle<Image>,
    #[dependency]
    pub button_main_on: Handle<Image>,
    #[dependency]
    pub button_main_focus: Handle<Image>,
    #[dependency]
    pub button_main_disabled: Handle<Image>,
}

impl FromWorld for UiButtonTabsAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            button: load_ui_image(assets, "images/ui/button_tabs/button.png"),
            button_hover: load_ui_image(assets, "images/ui/button_tabs/button_hover.png"),
            button_on: load_ui_image(assets, "images/ui/button_tabs/button_on.png"),
            button_focus: load_ui_image(assets, "images/ui/button_tabs/button_focus.png"),
            button_disabled: load_ui_image(assets, "images/ui/button_tabs/button_disabled.png"),
            button_main: load_ui_image(assets, "images/ui/button_tabs/button_main.png"),
            button_main_hover: load_ui_image(assets, "images/ui/button_tabs/button_main_hover.png"),
            button_main_on: load_ui_image(assets, "images/ui/button_tabs/button_main_on.png"),
            button_main_focus: load_ui_image(assets, "images/ui/button_tabs/button_main_focus.png"),
            button_main_disabled: load_ui_image(
                assets,
                "images/ui/button_tabs/button_main_disabled.png",
            ),
        }
    }
}

impl UiButtonTabsAssets {
    pub fn base_images(&self) -> ButtonImages {
        ButtonImages::new(
            self.button.clone(),
            self.button_hover.clone(),
            self.button_on.clone(),
        )
    }

    pub fn main_images(&self) -> ButtonImages {
        ButtonImages::new(
            self.button_main.clone(),
            self.button_main_hover.clone(),
            self.button_main_on.clone(),
        )
    }
}

impl From<&UiButtonTabsAssets> for ButtonImages {
    fn from(value: &UiButtonTabsAssets) -> Self {
        value.base_images()
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiButtonCheckAssets {
    #[dependency]
    pub off: Handle<Image>,
    #[dependency]
    pub on: Handle<Image>,
    #[dependency]
    pub hover: Handle<Image>,
    #[dependency]
    pub focus: Handle<Image>,
    #[dependency]
    pub disabled: Handle<Image>,
    #[dependency]
    pub void: Handle<Image>,
}

impl FromWorld for UiButtonCheckAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            off: load_ui_image(assets, "images/ui/button_check/check_off.png"),
            on: load_ui_image(assets, "images/ui/button_check/check_on.png"),
            hover: load_ui_image(assets, "images/ui/button_check/check_hover.png"),
            focus: load_ui_image(assets, "images/ui/button_check/check_focus.png"),
            disabled: load_ui_image(assets, "images/ui/button_check/check_disable.png"),
            void: load_ui_image(assets, "images/ui/button_check/check_void.png"),
        }
    }
}

impl From<&UiButtonCheckAssets> for ButtonImages {
    fn from(value: &UiButtonCheckAssets) -> Self {
        ButtonImages::new(value.off.clone(), value.hover.clone(), value.on.clone())
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiButtonCheckboxAssets {
    #[dependency]
    pub check_off: Handle<Image>,
    #[dependency]
    pub check_on: Handle<Image>,
    #[dependency]
    pub small_check_off: Handle<Image>,
    #[dependency]
    pub small_check_on: Handle<Image>,
    #[dependency]
    pub small_check_round_off: Handle<Image>,
    #[dependency]
    pub small_check_round_on: Handle<Image>,
}

impl FromWorld for UiButtonCheckboxAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            check_off: load_ui_image(assets, "images/ui/button_checkbox/check_off.png"),
            check_on: load_ui_image(assets, "images/ui/button_checkbox/check_on.png"),
            small_check_off: load_ui_image(assets, "images/ui/button_checkbox/small_check_off.png"),
            small_check_on: load_ui_image(assets, "images/ui/button_checkbox/small_check_on.png"),
            small_check_round_off: load_ui_image(
                assets,
                "images/ui/button_checkbox/small_check_round_off.png",
            ),
            small_check_round_on: load_ui_image(
                assets,
                "images/ui/button_checkbox/small_check_round_on.png",
            ),
        }
    }
}

impl From<&UiButtonCheckboxAssets> for ButtonImages {
    fn from(value: &UiButtonCheckboxAssets) -> Self {
        ButtonImages::new(
            value.check_off.clone(),
            value.check_on.clone(),
            value.check_on.clone(),
        )
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiPanelAssets {
    #[dependency]
    pub panel: Handle<Image>,
    #[dependency]
    pub panel_off: Handle<Image>,
    #[dependency]
    pub panel_on: Handle<Image>,
    #[dependency]
    pub panel_v2: Handle<Image>,
    #[dependency]
    pub panel_v2_off: Handle<Image>,
    #[dependency]
    pub panel_v2_on: Handle<Image>,
    #[dependency]
    pub panel_menu: Handle<Image>,
    #[dependency]
    pub options: Handle<Image>,
}

impl FromWorld for UiPanelAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            panel: load_ui_image(assets, "images/ui/panel/panel.png"),
            panel_off: load_ui_image(assets, "images/ui/panel/panel_off.png"),
            panel_on: load_ui_image(assets, "images/ui/panel/panel_on.png"),
            panel_v2: load_ui_image(assets, "images/ui/panel/panel_v2.png"),
            panel_v2_off: load_ui_image(assets, "images/ui/panel/panel_v2_off.png"),
            panel_v2_on: load_ui_image(assets, "images/ui/panel/panel_v2_on.png"),
            panel_menu: load_ui_image(assets, "images/ui/panel/panel_menu.png"),
            options: load_ui_image(assets, "images/ui/panel/options.png"),
        }
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiRangeAssets {
    #[dependency]
    pub bar_empty: Handle<Image>,
    #[dependency]
    pub bar_full: Handle<Image>,
    #[dependency]
    pub grabber_off: Handle<Image>,
    #[dependency]
    pub grabber_on: Handle<Image>,
}

impl FromWorld for UiRangeAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            bar_empty: load_ui_image(assets, "images/ui/range/barre_vide.png"),
            bar_full: load_ui_image(assets, "images/ui/range/barre_pleine.png"),
            grabber_off: load_ui_image(assets, "images/ui/range/grabber_off.png"),
            grabber_on: load_ui_image(assets, "images/ui/range/grabber_on.png"),
        }
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiComponentAssets {
    #[dependency]
    pub button_sliced_bottom_left: Handle<Image>,
    #[dependency]
    pub button_sliced_bottom_right: Handle<Image>,
    #[dependency]
    pub button_sliced_top_left: Handle<Image>,
    #[dependency]
    pub button_sliced_top_right: Handle<Image>,
    #[dependency]
    pub button_symetric: Handle<Image>,
    #[dependency]
    pub button_symetric_sliced: Handle<Image>,
    #[dependency]
    pub chevron_left: Handle<Image>,
    #[dependency]
    pub chevron_right: Handle<Image>,
    #[dependency]
    pub switch_base: Handle<Image>,
    #[dependency]
    pub switch_head: Handle<Image>,
}

impl FromWorld for UiComponentAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            button_sliced_bottom_left: load_ui_image(
                assets,
                "images/ui/components/button_sliced_bottom_left.png",
            ),
            button_sliced_bottom_right: load_ui_image(
                assets,
                "images/ui/components/button_sliced_bottom_right.png",
            ),
            button_sliced_top_left: load_ui_image(
                assets,
                "images/ui/components/button_sliced_top_left.png",
            ),
            button_sliced_top_right: load_ui_image(
                assets,
                "images/ui/components/button_sliced_top_right.png",
            ),
            button_symetric: load_ui_image(assets, "images/ui/components/button_symetric.png"),
            button_symetric_sliced: load_ui_image(
                assets,
                "images/ui/components/button_symetric_sliced.png",
            ),
            chevron_left: load_ui_image(assets, "images/ui/components/chevron_left.png"),
            chevron_right: load_ui_image(assets, "images/ui/components/chevron_right.png"),
            switch_base: load_ui_image(assets, "images/ui/components/switch_base.png"),
            switch_head: load_ui_image(assets, "images/ui/components/switch_head.png"),
        }
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiIconAssets {
    #[dependency]
    pub arrow_down: Handle<Image>,
    #[dependency]
    pub arrow_right: Handle<Image>,
    #[dependency]
    pub arrow_up: Handle<Image>,
    #[dependency]
    pub arrows: Handle<Image>,
    #[dependency]
    pub close_icon: Handle<Image>,
    #[dependency]
    pub file: Handle<Image>,
    #[dependency]
    pub folder: Handle<Image>,
    #[dependency]
    pub hide: Handle<Image>,
    #[dependency]
    pub inventory_icon: Handle<Image>,
    #[dependency]
    pub little_arrow_down: Handle<Image>,
    #[dependency]
    pub menu_icon: Handle<Image>,
    #[dependency]
    pub minus: Handle<Image>,
    #[dependency]
    pub more: Handle<Image>,
    #[dependency]
    pub refresh: Handle<Image>,
    #[dependency]
    pub resizer: Handle<Image>,
    #[dependency]
    pub settings_icon: Handle<Image>,
    #[dependency]
    pub size_reset: Handle<Image>,
    #[dependency]
    pub snap: Handle<Image>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum UiIconId {
    ArrowDown,
    ArrowRight,
    ArrowUp,
    Arrows,
    Close,
    File,
    Folder,
    Hide,
    Inventory,
    LittleArrowDown,
    Menu,
    Minus,
    More,
    Refresh,
    Resizer,
    Settings,
    SizeReset,
    Snap,
}

impl UiIconAssets {
    pub fn get(&self, id: UiIconId) -> Handle<Image> {
        match id {
            UiIconId::ArrowDown => self.arrow_down.clone(),
            UiIconId::ArrowRight => self.arrow_right.clone(),
            UiIconId::ArrowUp => self.arrow_up.clone(),
            UiIconId::Arrows => self.arrows.clone(),
            UiIconId::Close => self.close_icon.clone(),
            UiIconId::File => self.file.clone(),
            UiIconId::Folder => self.folder.clone(),
            UiIconId::Hide => self.hide.clone(),
            UiIconId::Inventory => self.inventory_icon.clone(),
            UiIconId::LittleArrowDown => self.little_arrow_down.clone(),
            UiIconId::Menu => self.menu_icon.clone(),
            UiIconId::Minus => self.minus.clone(),
            UiIconId::More => self.more.clone(),
            UiIconId::Refresh => self.refresh.clone(),
            UiIconId::Resizer => self.resizer.clone(),
            UiIconId::Settings => self.settings_icon.clone(),
            UiIconId::SizeReset => self.size_reset.clone(),
            UiIconId::Snap => self.snap.clone(),
        }
    }
}

impl FromWorld for UiIconAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            arrow_down: load_ui_image(assets, "images/ui/icons/arrow_down.png"),
            arrow_right: load_ui_image(assets, "images/ui/icons/arrow_right.png"),
            arrow_up: load_ui_image(assets, "images/ui/icons/arrow_up.png"),
            arrows: load_ui_image(assets, "images/ui/icons/arrows.png"),
            close_icon: load_ui_image(assets, "images/ui/icons/close_icon.png"),
            file: load_ui_image(assets, "images/ui/icons/file.png"),
            folder: load_ui_image(assets, "images/ui/icons/folder.png"),
            hide: load_ui_image(assets, "images/ui/icons/hide.png"),
            inventory_icon: load_ui_image(assets, "images/ui/icons/inventory_icon.png"),
            little_arrow_down: load_ui_image(assets, "images/ui/icons/little_arrow_down.png"),
            menu_icon: load_ui_image(assets, "images/ui/icons/menu_icon.png"),
            minus: load_ui_image(assets, "images/ui/icons/minus.png"),
            more: load_ui_image(assets, "images/ui/icons/more.png"),
            refresh: load_ui_image(assets, "images/ui/icons/refresh.png"),
            resizer: load_ui_image(assets, "images/ui/icons/resizer.png"),
            settings_icon: load_ui_image(assets, "images/ui/icons/settings_icon.png"),
            size_reset: load_ui_image(assets, "images/ui/icons/size_reset.png"),
            snap: load_ui_image(assets, "images/ui/icons/snap.png"),
        }
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiKeybindingIconAssets {
    #[dependency]
    pub attack: Handle<Image>,
    #[dependency]
    pub axis0_minus_1: Handle<Image>,
    #[dependency]
    pub axis01: Handle<Image>,
    #[dependency]
    pub axis1_minus_1: Handle<Image>,
    #[dependency]
    pub axis11: Handle<Image>,
    #[dependency]
    pub axis2_minus_1: Handle<Image>,
    #[dependency]
    pub axis21: Handle<Image>,
    #[dependency]
    pub axis3_minus_1: Handle<Image>,
    #[dependency]
    pub axis31: Handle<Image>,
    #[dependency]
    pub button0: Handle<Image>,
    #[dependency]
    pub button1: Handle<Image>,
    #[dependency]
    pub button2: Handle<Image>,
    #[dependency]
    pub button3: Handle<Image>,
    #[dependency]
    pub button4: Handle<Image>,
    #[dependency]
    pub button5: Handle<Image>,
    #[dependency]
    pub button6: Handle<Image>,
    #[dependency]
    pub button7: Handle<Image>,
    #[dependency]
    pub button8: Handle<Image>,
    #[dependency]
    pub button9: Handle<Image>,
    #[dependency]
    pub button10: Handle<Image>,
    #[dependency]
    pub button11: Handle<Image>,
    #[dependency]
    pub button12: Handle<Image>,
    #[dependency]
    pub button13: Handle<Image>,
    #[dependency]
    pub button14: Handle<Image>,
    #[dependency]
    pub button15: Handle<Image>,
    #[dependency]
    pub dash: Handle<Image>,
    #[dependency]
    pub interaction: Handle<Image>,
    #[dependency]
    pub mouse1: Handle<Image>,
    #[dependency]
    pub mouse2: Handle<Image>,
    #[dependency]
    pub mouse3: Handle<Image>,
    #[dependency]
    pub mouse4: Handle<Image>,
    #[dependency]
    pub mouse5: Handle<Image>,
    #[dependency]
    pub move_down: Handle<Image>,
    #[dependency]
    pub move_left: Handle<Image>,
    #[dependency]
    pub move_right: Handle<Image>,
    #[dependency]
    pub move_up: Handle<Image>,
    #[dependency]
    pub move_ow: Handle<Image>,
    #[dependency]
    pub move_ow_down: Handle<Image>,
    #[dependency]
    pub move_ow_left: Handle<Image>,
    #[dependency]
    pub move_ow_right: Handle<Image>,
    #[dependency]
    pub move_ow_up: Handle<Image>,
    #[dependency]
    pub ultime: Handle<Image>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum UiKeyIconId {
    Attack,
    Axis0Minus1,
    Axis01,
    Axis1Minus1,
    Axis11,
    Axis2Minus1,
    Axis21,
    Axis3Minus1,
    Axis31,
    Button0,
    Button1,
    Button2,
    Button3,
    Button4,
    Button5,
    Button6,
    Button7,
    Button8,
    Button9,
    Button10,
    Button11,
    Button12,
    Button13,
    Button14,
    Button15,
    Dash,
    Interaction,
    Mouse1,
    Mouse2,
    Mouse3,
    Mouse4,
    Mouse5,
    MoveDown,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveOw,
    MoveOwDown,
    MoveOwLeft,
    MoveOwRight,
    MoveOwUp,
    Ultime,
}

impl UiKeybindingIconAssets {
    pub fn get(&self, id: UiKeyIconId) -> Handle<Image> {
        match id {
            UiKeyIconId::Attack => self.attack.clone(),
            UiKeyIconId::Axis0Minus1 => self.axis0_minus_1.clone(),
            UiKeyIconId::Axis01 => self.axis01.clone(),
            UiKeyIconId::Axis1Minus1 => self.axis1_minus_1.clone(),
            UiKeyIconId::Axis11 => self.axis11.clone(),
            UiKeyIconId::Axis2Minus1 => self.axis2_minus_1.clone(),
            UiKeyIconId::Axis21 => self.axis21.clone(),
            UiKeyIconId::Axis3Minus1 => self.axis3_minus_1.clone(),
            UiKeyIconId::Axis31 => self.axis31.clone(),
            UiKeyIconId::Button0 => self.button0.clone(),
            UiKeyIconId::Button1 => self.button1.clone(),
            UiKeyIconId::Button2 => self.button2.clone(),
            UiKeyIconId::Button3 => self.button3.clone(),
            UiKeyIconId::Button4 => self.button4.clone(),
            UiKeyIconId::Button5 => self.button5.clone(),
            UiKeyIconId::Button6 => self.button6.clone(),
            UiKeyIconId::Button7 => self.button7.clone(),
            UiKeyIconId::Button8 => self.button8.clone(),
            UiKeyIconId::Button9 => self.button9.clone(),
            UiKeyIconId::Button10 => self.button10.clone(),
            UiKeyIconId::Button11 => self.button11.clone(),
            UiKeyIconId::Button12 => self.button12.clone(),
            UiKeyIconId::Button13 => self.button13.clone(),
            UiKeyIconId::Button14 => self.button14.clone(),
            UiKeyIconId::Button15 => self.button15.clone(),
            UiKeyIconId::Dash => self.dash.clone(),
            UiKeyIconId::Interaction => self.interaction.clone(),
            UiKeyIconId::Mouse1 => self.mouse1.clone(),
            UiKeyIconId::Mouse2 => self.mouse2.clone(),
            UiKeyIconId::Mouse3 => self.mouse3.clone(),
            UiKeyIconId::Mouse4 => self.mouse4.clone(),
            UiKeyIconId::Mouse5 => self.mouse5.clone(),
            UiKeyIconId::MoveDown => self.move_down.clone(),
            UiKeyIconId::MoveLeft => self.move_left.clone(),
            UiKeyIconId::MoveRight => self.move_right.clone(),
            UiKeyIconId::MoveUp => self.move_up.clone(),
            UiKeyIconId::MoveOw => self.move_ow.clone(),
            UiKeyIconId::MoveOwDown => self.move_ow_down.clone(),
            UiKeyIconId::MoveOwLeft => self.move_ow_left.clone(),
            UiKeyIconId::MoveOwRight => self.move_ow_right.clone(),
            UiKeyIconId::MoveOwUp => self.move_ow_up.clone(),
            UiKeyIconId::Ultime => self.ultime.clone(),
        }
    }
}

impl FromWorld for UiKeybindingIconAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            attack: load_ui_image(assets, "images/ui/icons/keybinding/attack.png"),
            axis0_minus_1: load_ui_image(assets, "images/ui/icons/keybinding/axis0-1.png"),
            axis01: load_ui_image(assets, "images/ui/icons/keybinding/axis01.png"),
            axis1_minus_1: load_ui_image(assets, "images/ui/icons/keybinding/axis1-1.png"),
            axis11: load_ui_image(assets, "images/ui/icons/keybinding/axis11.png"),
            axis2_minus_1: load_ui_image(assets, "images/ui/icons/keybinding/axis2-1.png"),
            axis21: load_ui_image(assets, "images/ui/icons/keybinding/axis21.png"),
            axis3_minus_1: load_ui_image(assets, "images/ui/icons/keybinding/axis3-1.png"),
            axis31: load_ui_image(assets, "images/ui/icons/keybinding/axis31.png"),
            button0: load_ui_image(assets, "images/ui/icons/keybinding/button0.png"),
            button1: load_ui_image(assets, "images/ui/icons/keybinding/button1.png"),
            button2: load_ui_image(assets, "images/ui/icons/keybinding/button2.png"),
            button3: load_ui_image(assets, "images/ui/icons/keybinding/button3.png"),
            button4: load_ui_image(assets, "images/ui/icons/keybinding/button4.png"),
            button5: load_ui_image(assets, "images/ui/icons/keybinding/button5.png"),
            button6: load_ui_image(assets, "images/ui/icons/keybinding/button6.png"),
            button7: load_ui_image(assets, "images/ui/icons/keybinding/button7.png"),
            button8: load_ui_image(assets, "images/ui/icons/keybinding/button8.png"),
            button9: load_ui_image(assets, "images/ui/icons/keybinding/button9.png"),
            button10: load_ui_image(assets, "images/ui/icons/keybinding/button10.png"),
            button11: load_ui_image(assets, "images/ui/icons/keybinding/button11.png"),
            button12: load_ui_image(assets, "images/ui/icons/keybinding/button12.png"),
            button13: load_ui_image(assets, "images/ui/icons/keybinding/button13.png"),
            button14: load_ui_image(assets, "images/ui/icons/keybinding/button14.png"),
            button15: load_ui_image(assets, "images/ui/icons/keybinding/button15.png"),
            dash: load_ui_image(assets, "images/ui/icons/keybinding/dash.png"),
            interaction: load_ui_image(assets, "images/ui/icons/keybinding/interaction.png"),
            mouse1: load_ui_image(assets, "images/ui/icons/keybinding/mouse1.png"),
            mouse2: load_ui_image(assets, "images/ui/icons/keybinding/mouse2.png"),
            mouse3: load_ui_image(assets, "images/ui/icons/keybinding/mouse3.png"),
            mouse4: load_ui_image(assets, "images/ui/icons/keybinding/mouse4.png"),
            mouse5: load_ui_image(assets, "images/ui/icons/keybinding/mouse5.png"),
            move_down: load_ui_image(assets, "images/ui/icons/keybinding/move_down.png"),
            move_left: load_ui_image(assets, "images/ui/icons/keybinding/move_left.png"),
            move_right: load_ui_image(assets, "images/ui/icons/keybinding/move_right.png"),
            move_up: load_ui_image(assets, "images/ui/icons/keybinding/move_up.png"),
            move_ow: load_ui_image(assets, "images/ui/icons/keybinding/move_ow.png"),
            move_ow_down: load_ui_image(assets, "images/ui/icons/keybinding/move_ow_down.png"),
            move_ow_left: load_ui_image(assets, "images/ui/icons/keybinding/move_ow_left.png"),
            move_ow_right: load_ui_image(assets, "images/ui/icons/keybinding/move_ow_right.png"),
            move_ow_up: load_ui_image(assets, "images/ui/icons/keybinding/move_ow_up.png"),
            ultime: load_ui_image(assets, "images/ui/icons/keybinding/ultime.png"),
        }
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiBackgroundAssets {
    #[dependency]
    pub background: Handle<Image>,
    #[dependency]
    pub title: Handle<Image>,
}

impl FromWorld for UiBackgroundAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            background: load_ui_image(assets, "images/ui/background.png"),
            title: load_ui_image(assets, "images/ui/title.png"),
        }
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiFxAssets {
    #[dependency]
    pub light_blue: Handle<Image>,
}

impl FromWorld for UiFxAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            light_blue: load_ui_image(assets, "images/ui/fx/light_blue.png"),
        }
    }
}
