//! The editor's colour themes, chosen from View > Theme or with `--theme`.
//! Only colours and corners change; sizes and spacing stay those of the
//! compact style in ui.rs.
use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, Stroke, Visuals};

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    /// Bronze and parchment on dark stone, square like the PlayStation menus.
    TombRaider,
    #[default]
    DarkModern,
    LightModern,
    /// Charcoal with orange highlights.
    Orange,
}

/// The colours a theme is built from.
struct Palette {
    /// Behind bars and panels.
    panel: Color32,
    /// Window backgrounds.
    window: Color32,
    /// Buttons, checkboxes and fields at rest.
    raised: Color32,
    hover: Color32,
    pressed: Color32,
    /// Window edges and separators.
    border: Color32,
    text: Color32,
    /// Text on hovered and pressed widgets.
    strong: Color32,
    /// Selected items and focus.
    accent: Color32,
    /// Text on the accent.
    on_accent: Color32,
    /// Text fields and scroll wells.
    well: Color32,
    /// Striped rows.
    stripe: Color32,
    warn: Color32,
    link: Color32,
    corner: u8,
}

impl Theme {
    pub const ALL: [Theme; 4] = [Theme::TombRaider, Theme::DarkModern, Theme::LightModern, Theme::Orange];

    pub fn name(self) -> &'static str {
        match self {
            Theme::TombRaider => "Tomb Raider",
            Theme::DarkModern => "Dark",
            Theme::LightModern => "Light",
            Theme::Orange => "Orange",
        }
    }

    /// A theme by name, ignoring case, spaces and dashes ("tomb-raider").
    pub fn parse(text: &str) -> Option<Self> {
        let key = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
        Self::ALL.into_iter().find(|t| key(t.name()) == key(text))
    }

    fn palette(self) -> Palette {
        let hex = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        match self {
            Theme::TombRaider => Palette {
                panel: hex(0x17130e),
                window: hex(0x1f1912),
                raised: hex(0x382c1d),
                hover: hex(0x4d3c24),
                pressed: hex(0x6a5127),
                border: hex(0x6b5330),
                text: hex(0xd6c59c),
                strong: hex(0xf3e4b9),
                accent: hex(0xa77a2c),
                on_accent: hex(0xfff3d0),
                well: hex(0x110e0a),
                stripe: hex(0x251e16),
                warn: hex(0xe3a33a),
                link: hex(0xe8bb5e),
                corner: 0,
            },
            Theme::DarkModern => Palette {
                panel: hex(0x1b1c1f),
                window: hex(0x222326),
                raised: hex(0x303237),
                hover: hex(0x3c3f45),
                pressed: hex(0x4b4f57),
                border: hex(0x37393e),
                text: hex(0xc8cbd0),
                strong: hex(0xf0f2f5),
                accent: hex(0x2e6bd6),
                on_accent: hex(0xffffff),
                well: hex(0x141517),
                stripe: hex(0x27282c),
                warn: hex(0xe5a03a),
                link: hex(0x6aa4ff),
                corner: 3,
            },
            Theme::LightModern => Palette {
                panel: hex(0xeceef1),
                window: hex(0xf9fafb),
                raised: hex(0xe1e4e8),
                hover: hex(0xd3d7dd),
                pressed: hex(0xc2c7cf),
                border: hex(0xc9cdd3),
                text: hex(0x2b2e34),
                strong: hex(0x0c0e11),
                accent: hex(0x2b6be0),
                on_accent: hex(0xffffff),
                well: hex(0xffffff),
                stripe: hex(0xf0f2f4),
                warn: hex(0xb05f0a),
                link: hex(0x1d5bcc),
                corner: 3,
            },
            Theme::Orange => Palette {
                panel: hex(0x1a1816),
                window: hex(0x221f1c),
                raised: hex(0x35302a),
                hover: hex(0x4b3a29),
                pressed: hex(0x7a4416),
                border: hex(0x4a3e33),
                text: hex(0xe6dacb),
                strong: hex(0xfff5ea),
                accent: hex(0xff7a1a),
                on_accent: hex(0x1c0f04),
                well: hex(0x141210),
                stripe: hex(0x29251f),
                warn: hex(0xffb24a),
                link: hex(0xff9a4a),
                corner: 3,
            },
        }
    }

    pub fn visuals(self) -> Visuals {
        let p = self.palette();
        let mut v = if self == Theme::LightModern { Visuals::light() } else { Visuals::dark() };
        let radius = CornerRadius::same(p.corner);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = p.window;
        w.noninteractive.weak_bg_fill = p.window;
        w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
        w.inactive.bg_fill = p.raised;
        w.inactive.weak_bg_fill = p.raised;
        w.inactive.bg_stroke = Stroke::NONE;
        w.inactive.fg_stroke = Stroke::new(1.0, p.text);
        w.hovered.bg_fill = p.hover;
        w.hovered.weak_bg_fill = p.hover;
        w.hovered.bg_stroke = Stroke::new(1.0, p.accent.gamma_multiply(0.8));
        w.hovered.fg_stroke = Stroke::new(1.5, p.strong);
        w.active.bg_fill = p.pressed;
        w.active.weak_bg_fill = p.pressed;
        w.active.bg_stroke = Stroke::new(1.0, p.accent);
        w.active.fg_stroke = Stroke::new(2.0, p.strong);
        w.open.bg_fill = p.raised;
        w.open.weak_bg_fill = p.raised;
        w.open.bg_stroke = Stroke::new(1.0, p.border);
        w.open.fg_stroke = Stroke::new(1.0, p.strong);
        for state in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
            state.corner_radius = radius;
        }
        v.selection.bg_fill = p.accent;
        v.selection.stroke = Stroke::new(1.0, p.on_accent);
        v.hyperlink_color = p.link;
        v.warn_fg_color = p.warn;
        v.faint_bg_color = p.stripe;
        v.extreme_bg_color = p.well;
        v.text_edit_bg_color = Some(p.well);
        v.code_bg_color = p.raised;
        v.window_fill = p.window;
        v.window_stroke = Stroke::new(1.0, p.border);
        v.panel_fill = p.panel;
        v.window_corner_radius = radius;
        v.menu_corner_radius = radius;
        v.window_shadow = egui::Shadow::NONE;
        v.popup_shadow = egui::Shadow::NONE;
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for theme in Theme::ALL {
            assert_eq!(Theme::parse(theme.name()), Some(theme));
        }
        assert_eq!(Theme::parse("tomb-raider"), Some(Theme::TombRaider));
        assert_eq!(Theme::parse("purple"), None);
    }

    #[test]
    fn only_light_is_light() {
        for theme in Theme::ALL {
            assert_eq!(theme.visuals().dark_mode, theme != Theme::LightModern);
        }
    }
}
