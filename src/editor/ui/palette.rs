//! Central color palette and themed color helpers for editor UI.

use bevy::prelude::*;

/// #e0e0e0 - Light gray for standard text
pub const LABEL_TEXT: Color = theme_color_with_temp(ThemeColors::SomberWhite, Temperature::Ambient);

/// #ffffff - Bright white for headers
pub const HEADER_TEXT: Color =
    theme_color_with_temp(ThemeColors::SomberWhite, Temperature::Ambient);

/// #f5f5f5 - Off-white for button text
pub const BUTTON_TEXT: Color =
    theme_color_with_temp(ThemeColors::SomberWhite, Temperature::Highlight);
/// #1c2030 - Dark blue-gray for button background
pub const BUTTON_BACKGROUND: Color =
    theme_color_with_temp(ThemeColors::Slate, Temperature::Ambient);
/// #252d45 - Slightly lighter for hover state
pub const BUTTON_HOVERED_BACKGROUND: Color =
    theme_color_with_temp(ThemeColors::SomberWhite, Temperature::Highlight);
/// #131824 - Darker for pressed state
pub const BUTTON_PRESSED_BACKGROUND: Color =
    theme_color_with_temp(ThemeColors::SomberWhite, Temperature::Shadow);

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum ThemeColors {
    SomberWhite,
    Slate,
    OliveGrey,
    Rose,
    GoldenYellow,
    NeonLime,
    ElectricCyan,
    LemonYellow,
    SolarYellow,
    Peach,
    HotPink,
    Mint,
    SkyBlue,
    SteelBlue,
    Magenta,
    Coral,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Temperature {
    Highlight,
    Ambient,
    Shadow,
    DeepShadow,
}

pub const fn theme_color_with_temp(color: ThemeColors, temp: Temperature) -> Color {
    use Temperature::*;
    use ThemeColors::*;

    match (color, temp) {
        // SomberWhite - Neutral grays with purple undertones
        (SomberWhite, Highlight) => Color::srgb(252.0 / 255.0, 252.0 / 255.0, 252.0 / 255.0), // #FCFCFC | hsla(0, 0%, 99%)
        (SomberWhite, Ambient) => Color::srgb(195.0 / 255.0, 203.0 / 255.0, 219.0 / 255.0), // #C3CBDB | hsla(220, 24%, 81%)
        (SomberWhite, Shadow) => Color::srgb(98.0 / 255.0, 80.0 / 255.0, 126.0 / 255.0), // #62507E | hsla(263, 22%, 40%)
        (SomberWhite, DeepShadow) => Color::srgb(37.0 / 255.0, 42.0 / 255.0, 50.0 / 255.0), // #252A32 | hsla(217, 15%, 17%)

        // Slate - Deep blue-grays
        (Slate, Highlight) => Color::srgb(66.0 / 255.0, 69.0 / 255.0, 86.0 / 255.0), // #424556 | hsla(231, 13%, 30%)
        (Slate, Ambient) => Color::srgb(37.0 / 255.0, 42.0 / 255.0, 50.0 / 255.0), // #252A32 | hsla(217, 15%, 17%)
        (Slate, Shadow) => Color::srgb(10.0 / 255.0, 11.0 / 255.0, 15.0 / 255.0), // #0A0B0F | hsla(228, 20%, 5%)
        (Slate, DeepShadow) => Color::srgb(5.0 / 255.0, 6.0 / 255.0, 8.0 / 255.0), // #050608 | hsla(220, 23%, 3%)

        // OliveGrey - Muted earth tones
        (OliveGrey, Highlight) => Color::srgb(136.0 / 255.0, 140.0 / 255.0, 120.0 / 255.0), // #888C78 | hsla(72, 8%, 51%)
        (OliveGrey, Ambient) => Color::srgb(88.0 / 255.0, 86.0 / 255.0, 81.0 / 255.0), // #585651 | hsla(43, 4%, 33%)
        (OliveGrey, Shadow) => Color::srgb(50.0 / 255.0, 34.0 / 255.0, 46.0 / 255.0), // #32222E | hsla(315, 19%, 16%)
        (OliveGrey, DeepShadow) => Color::srgb(28.0 / 255.0, 20.0 / 255.0, 32.0 / 255.0), // #1C1420 | hsla(280, 23%, 10%)

        // Rose - Warm pinks and magentas
        (Rose, Highlight) => Color::srgb(1.0, 143.0 / 255.0, 143.0 / 255.0), // #FF8F8F | hsla(0, 100%, 78%)
        (Rose, Ambient) => Color::srgb(1.0, 34.0 / 255.0, 69.0 / 255.0), // #FF2245 | hsla(351, 100%, 59%)
        (Rose, Shadow) => Color::srgb(156.0 / 255.0, 5.0 / 255.0, 101.0 / 255.0), // #9C0565 | hsla(322, 94%, 32%)
        (Rose, DeepShadow) => Color::srgb(77.0 / 255.0, 4.0 / 255.0, 60.0 / 255.0), // #4D043C | hsla(314, 90%, 16%)

        // GoldenYellow - Warm yellows to reds
        (GoldenYellow, Highlight) => Color::srgb(1.0, 216.0 / 255.0, 0.0), // #FFD800 | hsla(51, 100%, 50%)
        (GoldenYellow, Ambient) => Color::srgb(1.0, 144.0 / 255.0, 0.0), // #FF9000 | hsla(34, 100%, 50%)
        (GoldenYellow, Shadow) => Color::srgb(191.0 / 255.0, 0.0, 0.0), // #BF0000 | hsla(0, 100%, 37%)
        (GoldenYellow, DeepShadow) => Color::srgb(94.0 / 255.0, 0.0, 0.0), // #5E0000 | hsla(0, 100%, 18%)

        // NeonLime - Bright greens
        (NeonLime, Highlight) => Color::srgb(229.0 / 255.0, 1.0, 5.0 / 255.0), // #E5FF05 | hsla(62, 100%, 52%)
        (NeonLime, Ambient) => Color::srgb(167.0 / 255.0, 237.0 / 255.0, 0.0), // #A7ED00 | hsla(78, 100%, 46%)
        (NeonLime, Shadow) => Color::srgb(10.0 / 255.0, 93.0 / 255.0, 69.0 / 255.0), // #0A5D45 | hsla(163, 81%, 20%)
        (NeonLime, DeepShadow) => Color::srgb(4.0 / 255.0, 44.0 / 255.0, 33.0 / 255.0), // #042C21 | hsla(164, 83%, 9%)

        // ElectricCyan - Bright blues and cyans
        (ElectricCyan, Highlight) => Color::srgb(0.0, 1.0, 240.0 / 255.0), // #00FFF0 | hsla(176, 100%, 50%)
        (ElectricCyan, Ambient) => Color::srgb(0.0, 185.0 / 255.0, 1.0), // #00B9FF | hsla(197, 100%, 50%)
        (ElectricCyan, Shadow) => Color::srgb(22.0 / 255.0, 100.0 / 255.0, 197.0 / 255.0), // #1664C5 | hsla(213, 80%, 43%)
        (ElectricCyan, DeepShadow) => Color::srgb(10.0 / 255.0, 44.0 / 255.0, 104.0 / 255.0), // #0A2C68 | hsla(218, 83%, 22%)

        // LemonYellow - Bright yellows to warm reds
        (LemonYellow, Highlight) => Color::srgb(1.0, 232.0 / 255.0, 34.0 / 255.0), // #FFE822 | hsla(47, 100%, 59%)
        (LemonYellow, Ambient) => Color::srgb(1.0, 169.0 / 255.0, 57.0 / 255.0), // #FFA939 | hsla(27, 100%, 62%)
        (LemonYellow, Shadow) => Color::srgb(229.0 / 255.0, 35.0 / 255.0, 62.0 / 255.0), // #E5233E | hsla(352, 79%, 52%)
        (LemonYellow, DeepShadow) => Color::srgb(120.0 / 255.0, 17.0 / 255.0, 30.0 / 255.0), // #78111E | hsla(352, 75%, 27%)

        // SolarYellow - Pure yellows to browns
        (SolarYellow, Highlight) => Color::srgb(1.0, 252.0 / 255.0, 0.0), // #FFFC00 | hsla(59, 100%, 50%)
        (SolarYellow, Ambient) => Color::srgb(235.0 / 255.0, 183.0 / 255.0, 10.0 / 255.0), // #EBB70A | hsla(46, 92%, 48%)
        (SolarYellow, Shadow) => Color::srgb(145.0 / 255.0, 88.0 / 255.0, 22.0 / 255.0), // #915816 | hsla(32, 74%, 33%)
        (SolarYellow, DeepShadow) => Color::srgb(64.0 / 255.0, 38.0 / 255.0, 10.0 / 255.0), // #40260A | hsla(31, 73%, 15%)

        // Peach - Warm oranges and corals
        (Peach, Highlight) => Color::srgb(1.0, 179.0 / 255.0, 91.0 / 255.0), // #FFB35B | hsla(21, 100%, 73%)
        (Peach, Ambient) => Color::srgb(215.0 / 255.0, 126.0 / 255.0, 75.0 / 255.0), // #D77E4B | hsla(22, 64%, 57%)
        (Peach, Shadow) => Color::srgb(121.0 / 255.0, 61.0 / 255.0, 78.0 / 255.0), // #793D4E | hsla(343, 33%, 36%)
        (Peach, DeepShadow) => Color::srgb(56.0 / 255.0, 28.0 / 255.0, 36.0 / 255.0), // #381C24 | hsla(343, 33%, 16%)

        // HotPink - Vibrant magentas and pinks
        (HotPink, Highlight) => Color::srgb(1.0, 112.0 / 255.0, 223.0 / 255.0), // #FF70DF | hsla(314, 100%, 73%)
        (HotPink, Ambient) => Color::srgb(1.0, 34.0 / 255.0, 169.0 / 255.0), // #FF22A9 | hsla(324, 100%, 59%)
        (HotPink, Shadow) => Color::srgb(69.0 / 255.0, 6.0 / 255.0, 75.0 / 255.0), // #45064B | hsla(295, 85%, 16%)
        (HotPink, DeepShadow) => Color::srgb(31.0 / 255.0, 3.0 / 255.0, 34.0 / 255.0), // #1F0322 | hsla(294, 84%, 7%)

        // Mint - Cool greens and teals
        (Mint, Highlight) => Color::srgb(204.0 / 255.0, 1.0, 245.0 / 255.0), // #CCFFF5 | hsla(160, 100%, 87%)
        (Mint, Ambient) => Color::srgb(109.0 / 255.0, 247.0 / 255.0, 177.0 / 255.0), // #6DF7B1 | hsla(150, 89%, 70%)
        (Mint, Shadow) => Color::srgb(1.0 / 255.0, 118.0 / 255.0, 135.0 / 255.0), // #017687 | hsla(188, 99%, 27%)
        (Mint, DeepShadow) => Color::srgb(0.0, 56.0 / 255.0, 64.0 / 255.0), // #003840 | hsla(188, 100%, 13%)

        // SkyBlue - Light blues to deep blues
        (SkyBlue, Highlight) => Color::srgb(123.0 / 255.0, 213.0 / 255.0, 243.0 / 255.0), // #7BD5F3 | hsla(195, 83%, 72%)
        (SkyBlue, Ambient) => Color::srgb(108.0 / 255.0, 136.0 / 255.0, 1.0), // #6C88FF | hsla(228, 100%, 69%)
        (SkyBlue, Shadow) => Color::srgb(61.0 / 255.0, 46.0 / 255.0, 147.0 / 255.0), // #3D2E93 | hsla(249, 52%, 38%)
        (SkyBlue, DeepShadow) => Color::srgb(29.0 / 255.0, 21.0 / 255.0, 68.0 / 255.0), // #1D1544 | hsla(250, 53%, 17%)

        // SteelBlue - Muted blues and grays
        (SteelBlue, Highlight) => Color::srgb(133.0 / 255.0, 163.0 / 255.0, 199.0 / 255.0), // #85A3C7 | hsla(213, 39%, 65%)
        (SteelBlue, Ambient) => Color::srgb(103.0 / 255.0, 108.0 / 255.0, 173.0 / 255.0), // #676CAD | hsla(236, 31%, 54%)
        (SteelBlue, Shadow) => Color::srgb(50.0 / 255.0, 55.0 / 255.0, 81.0 / 255.0), // #323751 | hsla(230, 24%, 26%)
        (SteelBlue, DeepShadow) => Color::srgb(24.0 / 255.0, 26.0 / 255.0, 38.0 / 255.0), // #181A26 | hsla(231, 23%, 12%)

        // Magenta - Vibrant purples and magentas
        (Magenta, Highlight) => Color::srgb(1.0, 89.0 / 255.0, 190.0 / 255.0), // #FF59BE | hsla(325, 100%, 67%)
        (Magenta, Ambient) => Color::srgb(197.0 / 255.0, 26.0 / 255.0, 234.0 / 255.0), // #C51AEA | hsla(289, 86%, 51%)
        (Magenta, Shadow) => Color::srgb(51.0 / 255.0, 22.0 / 255.0, 133.0 / 255.0), // #331685 | hsla(256, 72%, 30%)
        (Magenta, DeepShadow) => Color::srgb(24.0 / 255.0, 10.0 / 255.0, 63.0 / 255.0), // #180A3F | hsla(256, 73%, 14%)

        // Coral - Warm oranges and reds
        (Coral, Highlight) => Color::srgb(251.0 / 255.0, 149.0 / 255.0, 133.0 / 255.0), // #FB9585 | hsla(8, 93%, 75%)
        (Coral, Ambient) => Color::srgb(233.0 / 255.0, 116.0 / 255.0, 97.0 / 255.0), // #E97461 | hsla(8, 75%, 65%)
        (Coral, Shadow) => Color::srgb(147.0 / 255.0, 39.0 / 255.0, 143.0 / 255.0), // #93278F | hsla(302, 58%, 36%)
        (Coral, DeepShadow) => Color::srgb(69.0 / 255.0, 18.0 / 255.0, 67.0 / 255.0), // #451243 | hsla(302, 59%, 17%)
    }
}

pub const fn theme_color(color: ThemeColors) -> Color {
    theme_color_with_temp(color, Temperature::Ambient)
}

#[allow(dead_code)]
pub trait ThemeColorPalette {
    fn theme_color(color: ThemeColors) -> Color {
        Self::theme_color_with_temp(color, Temperature::Ambient)
    }

    fn theme_color_with_temp(color: ThemeColors, temp: Temperature) -> Color;
}

impl ThemeColorPalette for Color {
    fn theme_color_with_temp(color: ThemeColors, temp: Temperature) -> Color {
        theme_color_with_temp(color, temp)
    }
}
