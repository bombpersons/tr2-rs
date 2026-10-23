//! The original's screen text: its font, a sprite sequence in every level
//! file (object 255), laid out as T_DrawText `0x8005D104` lays it out.
//!
//! A line is a string at a point of the 368 by 240 screen, measured from
//! the left or right edge or the centre and from the top, middle or bottom
//! ([`Align`]); the point is the left end of the line's baseline. Each
//! character draws the font's sprite for it (the character map at
//! `0x80082F54`) and moves on by the sprite's width (`0x80082F04`) plus
//! one, a space by six. Digits keep to twelve-pixel cells. `(`, `)`, `$`
//! and `~` draw without moving on, over the next character. The width used
//! for centring and right alignment (`0x8005CC3C`) counts twelve for the
//! glyphs numbered like digit codes (w to z and 0 to 5), as the original
//! does, rounded down to even. Both tables are the same in `TOMB2.EXE`.
//!
//! [`ScreenText`] draws a line in the overlay with the active level's
//! font: one atlas sprite per character, as the original draws one sprite
//! primitive per character.
use bevy::prelude::*;
use engine::{
    render::{ActiveLevel, overlay::OverlayAnchor},
    world::{WorldGeometry, format::Sprite as LevelSprite},
};

/// The font's sprite sequence.
pub const FONT_OBJECT: u16 = 255;
/// Codes for the font's symbols that are not characters: the arrows (codes
/// 15 to 18 draw sprites 106 to 109; the passport's "Load Game" arrow), and
/// the Cross and Triangle buttons, which take the places of `>` and `;`.
pub mod symbol {
    pub const DOWN: char = '\u{0F}';
    pub const UP: char = '\u{10}';
    pub const LEFT: char = '\u{11}';
    pub const RIGHT: char = '\u{12}';
    pub const CROSS: char = '>';
    pub const TRIANGLE: char = ';';
}
/// The screen the original lays text out on.
pub const SCREEN: IVec2 = IVec2::new(0x170, 0xF0);
/// Gaps between letters and for a space (a text's +0x10 and +0x12, set by
/// T_Print `0x8005C7AC`).
const LETTER_SPACING: i32 = 1;
const WORD_SPACING: i32 = 6;
/// The cell a digit is centred in.
const DIGIT_CELL: i32 = 12;
/// Large button symbols (codes 0x7F..=0x81), drawn from another table; they
/// take sixteen pixels and are not drawn here.
const SYMBOL: i32 = 16;

/// The font's sprite for each character code below 0x7F (`0x80082F54`; codes
/// up to 18 are computed instead, see [`glyph`]).
const CHARACTERS: [u8; 127] = [
    12, 12, 7, 5, 7, 7, 7, 7, 7, 7, 7, 7, 16, 14, 14, 14, 16, 16, 16, 16, 16, 12, 14, 8, 8, 8, 8, 8, 8, 8, 0, 0, 0, 64,
    66, 76, 77, 74, 78, 79, 69, 70, 92, 75, 63, 71, 62, 68, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 73, 93, 94, 74, 95,
    65, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 80, 96, 81,
    97, 98, 99, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51,
    100, 101, 102, 67,
];

/// Each font sprite's width (`0x80082F04`).
const WIDTHS: [u8; 110] = [
    14, 11, 11, 11, 11, 11, 11, 13, 8, 11, 12, 11, 13, 13, 12, 11, 12, 12, 11, 12, 13, 13, 13, 12, 12, 11, 9, 9, 9, 9,
    9, 9, 9, 9, 5, 9, 9, 5, 12, 10, 9, 9, 9, 8, 9, 8, 9, 9, 11, 9, 9, 9, 12, 8, 10, 10, 10, 10, 10, 9, 10, 10, 5, 5, 5,
    11, 9, 10, 8, 6, 6, 7, 7, 3, 11, 8, 13, 16, 9, 4, 12, 12, 7, 5, 7, 7, 7, 7, 7, 7, 7, 7, 16, 14, 14, 14, 16, 16, 16,
    16, 16, 12, 14, 8, 8, 8, 8, 8, 8, 8,
];

/// The font sprite of character `code`, if it draws one.
fn glyph(code: u8) -> Option<usize> {
    match code {
        0..=10 => Some(usize::from(code) + 0x51),
        11..=18 => Some(usize::from(code) + 0x5B),
        0x13..0x20 | 0x20 | 0x7F.. => None,
        _ => Some(usize::from(CHARACTERS[usize::from(code)])),
    }
}

fn width_of(glyph: usize) -> i32 {
    WIDTHS.get(glyph).map_or(0, |&w| i32::from(w))
}

/// Characters drawn over the next one.
fn overlays(code: u8) -> bool {
    matches!(code, b'(' | b')' | b'$' | b'~')
}

/// The line's width for alignment (`0x8005CC3C`).
pub fn width(text: &str) -> i32 {
    let mut width = 0;
    for code in text.bytes() {
        if (11..0x20).contains(&code) || code >= 0x82 || overlays(code) {
            continue;
        }
        width += match code {
            b' ' => WORD_SPACING,
            0x7F..=0x81 => SYMBOL,
            _ => {
                let Some(glyph) = glyph(code) else { continue };
                // The original tests the glyph number here, not the code.
                if (0x30..0x3A).contains(&glyph) { DIGIT_CELL } else { width_of(glyph) + LETTER_SPACING }
            }
        };
    }
    (width - LETTER_SPACING) & !1
}

/// One character's sprite and where its point is along the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub glyph: usize,
    pub x: i32,
}

/// The sprites of a line and their points from its start (`0x8005D104`).
pub fn layout(text: &str) -> Vec<Placed> {
    let mut x = 0;
    let mut placed = vec![];
    for code in text.bytes() {
        match code {
            0x13..0x20 => {}
            b' ' => x += WORD_SPACING,
            0x7F..=0x81 => x += SYMBOL,
            _ => {
                let Some(glyph) = glyph(code) else { continue };
                let digit = code.is_ascii_digit();
                let indent = if digit { (DIGIT_CELL - width_of(glyph)) / 2 } else { 0 };
                x += indent;
                placed.push(Placed { glyph, x });
                if !overlays(code) {
                    x += if digit { DIGIT_CELL - indent } else { width_of(glyph) + LETTER_SPACING };
                }
            }
        }
    }
    placed
}

/// Where a line's point is measured from (a text's flags 0x10 and 0x80
/// across, 0x20 and 0x100 down).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Align {
    pub across: Edge,
    pub down: Edge,
}

/// An edge of the screen, or its middle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Edge {
    /// Left or top.
    #[default]
    Start,
    Middle,
    /// Right or bottom.
    End,
}

impl Align {
    pub const TOP_LEFT: Self = Self { across: Edge::Start, down: Edge::Start };
    pub const TOP: Self = Self { across: Edge::Middle, down: Edge::Start };
    pub const TOP_RIGHT: Self = Self { across: Edge::End, down: Edge::Start };
    pub const BOTTOM_LEFT: Self = Self { across: Edge::Start, down: Edge::End };
    pub const BOTTOM: Self = Self { across: Edge::Middle, down: Edge::End };
    pub const BOTTOM_RIGHT: Self = Self { across: Edge::End, down: Edge::End };
    pub const CENTRE: Self = Self { across: Edge::Middle, down: Edge::Middle };

    /// Where a line of `width` at `at` starts on the original's screen.
    pub fn start(self, at: IVec2, width: i32) -> IVec2 {
        let x = match self.across {
            Edge::Start => at.x,
            Edge::Middle => at.x + (SCREEN.x - width) / 2,
            Edge::End => at.x + SCREEN.x - width,
        };
        let y = match self.down {
            Edge::Start => at.y,
            Edge::Middle => at.y + SCREEN.y / 2,
            Edge::End => at.y + SCREEN.y,
        };
        IVec2::new(x, y)
    }

    /// The overlay anchor of a line of `width` at `at`: its start, kept to
    /// the edge it is measured from as the screen widens.
    fn anchor(self, at: IVec2, width: i32) -> OverlayAnchor {
        let side = |edge| match edge {
            Edge::Start => -1.0,
            Edge::Middle => 0.0,
            Edge::End => 1.0,
        };
        let x = match self.across {
            Edge::Start => at.x,
            Edge::Middle => at.x - width / 2,
            Edge::End => at.x - width,
        };
        OverlayAnchor::new(Vec2::new(side(self.across), -side(self.down)), Vec2::new(x as f32, -at.y as f32))
    }
}

/// A line of the original's text in the overlay.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[require(Transform, Visibility)]
pub struct ScreenText {
    pub text: String,
    pub at: IVec2,
    pub align: Align,
}

impl ScreenText {
    pub fn new(text: impl Into<String>, at: IVec2, align: Align) -> Self {
        Self { text: text.into(), at, align }
    }
}

/// A set of lines shown together: replace them all at once (with
/// `set_if_neq`) and they are drawn again only when they change.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
#[require(Transform, Visibility)]
pub struct ScreenLines(pub Vec<ScreenText>);

/// Gives each changed [`ScreenLines`] one child line per entry.
fn show_lines(
    mut commands: Commands,
    blocks: Query<(Entity, Ref<ScreenLines>, Option<&Children>)>,
    lines: Query<(), With<ScreenText>>,
) {
    for (entity, block, children) in &blocks {
        if !block.is_changed() {
            continue;
        }
        for &child in children.into_iter().flatten() {
            if lines.contains(child) {
                commands.entity(child).despawn();
            }
        }
        for line in block.0.iter().filter(|l| !l.text.is_empty()) {
            commands.spawn((line.clone(), ChildOf(entity)));
        }
    }
}

/// The active level's font: its atlas and the font's sprites.
#[derive(Resource, Clone)]
pub struct OriginalFont {
    pub level: String,
    pub atlas: Handle<Image>,
    pub glyphs: Vec<LevelSprite>,
}

pub struct TextPlugin;

impl Plugin for TextPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (load_font, show_lines, draw_text).chain());
    }
}

/// Takes the font from each level as it becomes active.
fn load_font(
    mut commands: Commands,
    server: Res<AssetServer>,
    level: Option<Res<ActiveLevel>>,
    world: Option<Res<WorldGeometry>>,
    font: Option<Res<OriginalFont>>,
) {
    let (Some(level), Some(world)) = (level, world) else { return };
    if font.is_some_and(|f| f.level == level.name) {
        return;
    }
    let Some(glyphs) = world.data.sprite_sequence(FONT_OBJECT) else { return };
    commands.insert_resource(OriginalFont {
        level: level.name.clone(),
        atlas: server.load(format!("levels/{}/atlas.png", level.name)),
        glyphs: glyphs.to_vec(),
    });
}

/// One sprite of a line.
#[derive(Component)]
struct Character;

/// Lays out lines that are new or changed, or all of them for a new font.
fn draw_text(
    mut commands: Commands,
    font: Option<Res<OriginalFont>>,
    lines: Query<(Entity, Ref<ScreenText>, Option<&Children>)>,
    characters: Query<(), With<Character>>,
) {
    let Some(font) = font else { return };
    for (entity, line, children) in &lines {
        if !line.is_changed() && !font.is_changed() {
            continue;
        }
        for &child in children.into_iter().flatten() {
            if characters.contains(child) {
                commands.entity(child).despawn();
            }
        }
        commands.entity(entity).insert(line.align.anchor(line.at, width(&line.text)));
        for placed in layout(&line.text) {
            let Some(sprite) = font.glyphs.get(placed.glyph) else { continue };
            let [x, y, w, h] = sprite.atlas.map(|v| v as f32);
            let [left, top, right, bottom] = sprite.rect.map(|v| v as f32);
            commands.spawn((
                Character,
                Sprite {
                    image: font.atlas.clone(),
                    rect: Some(Rect::new(x, y, x + w, y + h)),
                    custom_size: Some(Vec2::new(right - left, bottom - top)),
                    ..default()
                },
                Transform::from_xyz(placed.x as f32 + (left + right) / 2.0, -(top + bottom) / 2.0, 0.0),
                ChildOf(entity),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lines from the GPU capture of the open ring
    /// (`validation/inventory/gpu_ring`): where each starts and each
    /// character's point from there (its sprite's left edge in the capture
    /// less the sprite's left offset; `;` draws one pixel left of its point).
    #[test]
    fn lines_lay_out_as_the_original_draws_them() {
        let xs = |text: &str| layout(text).iter().map(|p| p.x).collect::<Vec<_>>();
        assert_eq!(Align::TOP.start(IVec2::new(0, 26), width("INVENTORY")), IVec2::new(127, 26));
        assert_eq!(xs("INVENTORY"), [0, 9, 23, 37, 49, 63, 76, 89, 102]);
        assert_eq!(Align::BOTTOM.start(IVec2::new(0, -16), width("Statistics")), IVec2::new(139, 224));
        assert_eq!(xs("Statistics"), [0, 12, 21, 31, 40, 46, 56, 65, 71, 81]);
        assert_eq!(Align::BOTTOM_RIGHT.start(IVec2::new(-20, -15), width("]")), IVec2::new(336, 225));
        assert_eq!(Align::BOTTOM_RIGHT.start(IVec2::new(-25, -40), width("; Go Back")), IVec2::new(253, 200));
        assert_eq!(xs("; Go Back"), [0, 21, 33, 49, 61, 71, 81]);
    }

    /// The width counts twelve for the glyphs numbered like digit codes.
    #[test]
    fn width_keeps_the_originals_digit_test() {
        assert_eq!(glyph(b'w'), Some(48));
        assert_eq!(width("w"), (DIGIT_CELL - LETTER_SPACING) & !1);
        assert_eq!(width("6"), width_of(58) & !1);
    }
}
