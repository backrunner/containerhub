use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}
impl Appearance {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "跟随系统",
            Self::Light => "浅色",
            Self::Dark => "深色",
        }
    }
    pub fn apply(self, window: &mut Window, cx: &mut App) {
        let mode = match self {
            Self::Light => ThemeMode::Light,
            Self::Dark => ThemeMode::Dark,
            Self::System => window.appearance().into(),
        };
        Theme::change(mode, Some(window), cx);
        let p = Palette::get(mode == ThemeMode::Dark);
        Theme::update(cx, |theme| {
            theme.font_size = px(13.);
            theme.radius = px(6.);
            theme.sheet.margin_top = px(36.);
            theme.colors.title_bar = p.surface;
            theme.colors.background = p.surface;
            theme.colors.foreground = p.text;
            theme.colors.border = p.border;
            theme.colors.primary = p.primary;
            theme.colors.primary_foreground = rgb(0xffffff).into();
            theme.colors.primary_hover = Hsla {
                l: p.primary.l * 0.92,
                ..p.primary
            };
            theme.colors.primary_active = Hsla {
                l: p.primary.l * 0.84,
                ..p.primary
            };
            // Buttons own independent semantic tokens; changing `primary` alone
            // leaves their default monochrome palette in place.
            theme.colors.button_primary = p.primary;
            theme.colors.button_primary_hover = theme.colors.primary_hover;
            theme.colors.button_primary_active = theme.colors.primary_active;
            theme.colors.button_primary_foreground = theme.colors.primary_foreground;
            theme.colors.button = p.surface;
            theme.colors.button_foreground = p.text;
            theme.colors.button_hover = p.selected;
            theme.colors.button_active = p.accent_surface;
            theme.colors.input = p.border;
            theme.colors.ring = p.primary;
            theme.colors.caret = p.primary;
            theme.colors.selection = p.primary.opacity(0.2);
            theme.colors.popover = p.surface;
            theme.colors.popover_foreground = p.text;
            theme.colors.secondary = p.sidebar;
            theme.colors.secondary_foreground = p.text;
            theme.colors.secondary_hover = p.selected;
            theme.colors.secondary_active = p.accent_surface;
            theme.colors.muted = p.canvas;
            theme.colors.muted_foreground = p.muted;
            theme.colors.accent = p.accent_surface;
            theme.colors.accent_foreground = p.accent;
        });
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub canvas: Hsla,
    pub surface: Hsla,
    pub sidebar: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub border: Hsla,
    pub primary: Hsla,
    pub accent: Hsla,
    pub accent_surface: Hsla,
    pub selected: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub danger: Hsla,
}
impl Palette {
    pub fn get(dark: bool) -> Self {
        static TOKENS: OnceLock<serde_json::Value> = OnceLock::new();
        let data = TOKENS.get_or_init(|| {
            serde_json::from_str(include_str!("../../../packages/design-tokens/themes.json"))
                .expect("valid embedded theme tokens")
        });
        let theme = &data[if dark { "dark" } else { "light" }];
        let c = |name: &str| {
            rgb(
                u32::from_str_radix(theme[name].as_str().expect("color token"), 16)
                    .expect("hex color"),
            )
            .into()
        };
        Self {
            canvas: c("canvas"),
            surface: c("surface"),
            sidebar: c("sidebar"),
            text: c("text"),
            muted: c("muted"),
            border: c("border"),
            primary: c("primary"),
            accent: c("accent"),
            accent_surface: c("accent_surface"),
            selected: c("selected"),
            success: c("success"),
            warning: c("warning"),
            danger: c("danger"),
        }
    }
}
