//! Canonical UI sizing, text, and style presets used by editor widgets.

use bevy::prelude::*;

use crate::editor::ui::palette::{
    BUTTON_BACKGROUND, BUTTON_HOVERED_BACKGROUND, BUTTON_PRESSED_BACKGROUND, BUTTON_TEXT,
    HEADER_TEXT, LABEL_TEXT, ThemeColors, theme_color,
};

pub const BORDER_4: BorderRect = BorderRect::all(4.0);
pub const BORDER_8: BorderRect = BorderRect::all(8.0);
pub const BORDER_10: BorderRect = BorderRect::all(10.0);
pub const BORDER_12: BorderRect = BorderRect::all(12.0);

#[derive(Clone, Reflect)]
pub struct TextStyle {
    pub font: Handle<Font>,
    pub font_size: f32,
    pub color: Color,
}

impl TextStyle {
    pub fn text_font(&self) -> TextFont {
        TextFont {
            font: self.font.clone(),
            font_size: self.font_size,
            ..default()
        }
    }

    pub fn with_font(mut self, font: Handle<Font>) -> Self {
        self.font = font;
        self
    }
}

#[derive(Clone, Reflect)]
pub struct TextStyles {
    pub label: TextStyle,
    pub header: TextStyle,
    pub button: TextStyle,
}

#[derive(Clone, Reflect)]
pub struct ButtonColors {
    pub none: Color,
    pub hovered: Color,
    pub pressed: Color,
    pub active: Color,
}

#[derive(Clone, Reflect)]
pub struct ButtonStyle {
    pub size: Vec2,
    pub border: BorderRect,
    pub text: TextStyle,
    pub colors: ButtonColors,
    pub align_items: AlignItems,
    pub justify_content: JustifyContent,
    pub padding: UiRect,
    pub border_radius: BorderRadius,
}

impl ButtonStyle {
    pub fn node(&self) -> Node {
        Node {
            width: px(self.size.x),
            height: px(self.size.y),
            align_items: self.align_items,
            justify_content: self.justify_content,
            padding: self.padding,
            border_radius: self.border_radius,
            ..default()
        }
    }

    pub fn with_size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    pub fn with_border(mut self, border: BorderRect) -> Self {
        self.border = border;
        self
    }
}

#[derive(Clone, Reflect)]
pub struct ButtonStyles {
    pub normal: ButtonStyle,
    pub long: ButtonStyle,
    pub big: ButtonStyle,
    pub skewed: ButtonStyle,
    pub arrow: ButtonStyle,
    pub tab: ButtonStyle,
    pub tabs: ButtonStyle,
    pub tabs_main: ButtonStyle,
    pub check: ButtonStyle,
    pub checkbox: ButtonStyle,
    pub small: ButtonStyle,
}

#[derive(Clone, Reflect)]
pub struct PanelStyle {
    pub size: Vec2,
    pub border: BorderRect,
    pub padding: UiRect,
}

impl PanelStyle {
    pub fn node(&self) -> Node {
        Node {
            width: px(self.size.x),
            height: px(self.size.y),
            padding: self.padding,
            ..default()
        }
    }

    pub fn with_size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }
}

#[derive(Clone, Reflect)]
pub struct PanelStyles {
    pub panel: PanelStyle,
    pub panel_off: PanelStyle,
    pub panel_on: PanelStyle,
    pub panel_v2: PanelStyle,
    pub panel_v2_off: PanelStyle,
    pub panel_v2_on: PanelStyle,
    pub panel_menu: PanelStyle,
    pub panel_options: PanelStyle,
}

#[derive(Clone, Reflect)]
pub struct RangeStyle {
    pub track_size: Vec2,
    pub grabber_size: Vec2,
    pub border: BorderRect,
}

#[derive(Clone, Reflect)]
pub struct ToggleStyle {
    pub checkbox_size: Vec2,
    pub checkbox_small_size: Vec2,
    pub checkbox_round_size: Vec2,
    pub check_button_size: Vec2,
    pub check_button_border: BorderRect,
    pub check_tile_size: Vec2,
    pub check_tile_border: BorderRect,
}

#[derive(Clone, Reflect)]
pub struct IconStyle {
    pub size: Vec2,
}

#[derive(Clone, Reflect)]
pub struct ComponentStyle {
    pub symetric_size: Vec2,
    pub symetric_border: BorderRect,
    pub slice_size: Vec2,
    pub chevron_size: Vec2,
    pub switch_base_size: Vec2,
    pub switch_head_size: Vec2,
}

#[derive(Clone, Reflect)]
pub struct BackgroundStyle {
    pub background_size: Vec2,
    pub title_size: Vec2,
}

#[derive(Clone, Reflect)]
pub struct FxStyle {
    pub light_blue_size: Vec2,
}

#[derive(Resource, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiStyles {
    pub text: TextStyles,
    pub buttons: ButtonStyles,
    pub panels: PanelStyles,
    pub range: RangeStyle,
    pub toggles: ToggleStyle,
    pub icons: IconStyle,
    pub components: ComponentStyle,
    pub background: BackgroundStyle,
    pub fx: FxStyle,
}

#[derive(Clone, Reflect)]
pub struct ButtonImages {
    pub none: Handle<Image>,
    pub hovered: Handle<Image>,
    pub pressed: Handle<Image>,
}

impl ButtonImages {
    pub fn new(none: Handle<Image>, hovered: Handle<Image>, pressed: Handle<Image>) -> Self {
        Self {
            none,
            hovered,
            pressed,
        }
    }
}

impl Default for UiStyles {
    fn default() -> Self {
        let button_text = TextStyle {
            font: Handle::default(),
            font_size: 40.0,
            color: BUTTON_TEXT,
        };
        let label_text = TextStyle {
            font: Handle::default(),
            font_size: 24.0,
            color: LABEL_TEXT,
        };
        let header_text = TextStyle {
            font: Handle::default(),
            font_size: 40.0,
            color: HEADER_TEXT,
        };

        let button_colors = ButtonColors {
            none: BUTTON_BACKGROUND,
            hovered: BUTTON_HOVERED_BACKGROUND,
            pressed: BUTTON_PRESSED_BACKGROUND,
            active: theme_color(ThemeColors::Peach),
        };

        let center_align = AlignItems::Center;
        let center_justify = JustifyContent::Center;
        let button_style = |size: Vec2, border: BorderRect| ButtonStyle {
            size,
            border,
            text: button_text.clone(),
            colors: button_colors.clone(),
            align_items: center_align,
            justify_content: center_justify,
            padding: UiRect::all(px(0.0)),
            border_radius: BorderRadius::default(),
        };

        let mut button_normal = button_style(Vec2::new(300.0, 80.0), BorderRect::all(22.0));
        button_normal.border_radius = BorderRadius::MAX;

        let button_long = button_style(Vec2::new(96.0, 32.0), BORDER_8);
        let button_big = button_style(Vec2::new(241.0, 52.0), BORDER_10);
        let button_skewed = button_style(Vec2::new(201.0, 50.0), BORDER_10);
        let button_arrow = button_style(Vec2::new(32.0, 32.0), BORDER_8);
        let button_tab = button_style(Vec2::new(16.0, 16.0), BORDER_4);
        let button_tabs = button_style(Vec2::new(64.0, 32.0), BORDER_8);
        let button_tabs_main = button_style(Vec2::new(48.0, 32.0), BORDER_8);
        let button_check = button_style(Vec2::new(64.0, 32.0), BORDER_8);
        let button_checkbox = button_style(Vec2::new(16.0, 16.0), BORDER_4);
        let button_small = button_style(Vec2::new(30.0, 30.0), BorderRect::all(18.0));

        let panel_style = |size: Vec2, border: BorderRect| PanelStyle {
            size,
            border,
            padding: UiRect::all(px(0.0)),
        };
        let panel_base = panel_style(Vec2::new(320.0, 200.0), BORDER_4);
        let panel_menu = panel_style(Vec2::new(256.0, 400.0), BORDER_12);
        let panel_options = panel_style(Vec2::new(472.0, 240.0), BORDER_12);

        Self {
            text: TextStyles {
                label: label_text,
                header: header_text,
                button: button_text.clone(),
            },
            buttons: ButtonStyles {
                normal: button_normal,
                long: button_long,
                big: button_big,
                skewed: button_skewed,
                arrow: button_arrow,
                tab: button_tab,
                tabs: button_tabs,
                tabs_main: button_tabs_main,
                check: button_check,
                checkbox: button_checkbox,
                small: button_small,
            },
            panels: PanelStyles {
                panel: panel_base.clone(),
                panel_off: panel_base.clone(),
                panel_on: panel_base.clone(),
                panel_v2: panel_base.clone(),
                panel_v2_off: panel_base.clone(),
                panel_v2_on: panel_base,
                panel_menu,
                panel_options,
            },
            range: RangeStyle {
                track_size: Vec2::new(120.0, 16.0),
                grabber_size: Vec2::new(16.0, 16.0),
                border: BORDER_8,
            },
            toggles: ToggleStyle {
                checkbox_size: Vec2::new(16.0, 16.0),
                checkbox_small_size: Vec2::new(10.0, 10.0),
                checkbox_round_size: Vec2::new(10.0, 10.0),
                check_button_size: Vec2::new(64.0, 32.0),
                check_button_border: BORDER_8,
                check_tile_size: Vec2::new(32.0, 32.0),
                check_tile_border: BORDER_4,
            },
            icons: IconStyle {
                size: Vec2::new(16.0, 16.0),
            },
            components: ComponentStyle {
                symetric_size: Vec2::new(80.0, 80.0),
                symetric_border: BORDER_12,
                slice_size: Vec2::new(80.0, 80.0),
                chevron_size: Vec2::new(38.0, 53.0),
                switch_base_size: Vec2::new(160.0, 80.0),
                switch_head_size: Vec2::new(64.0, 64.0),
            },
            background: BackgroundStyle {
                background_size: Vec2::new(1920.0, 1080.0),
                title_size: Vec2::new(135.0, 54.0),
            },
            fx: FxStyle {
                light_blue_size: Vec2::new(128.0, 128.0),
            },
        }
    }
}

impl UiStyles {
    pub fn kenney_defaults() -> Self {
        Self::default()
    }
}
