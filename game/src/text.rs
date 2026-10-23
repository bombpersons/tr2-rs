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
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use engine::{
    psx::{PsxBlend, PsxMaterial2d},
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
    /// A box drawn behind it.
    pub boxed: Option<TextBox>,
}

impl ScreenText {
    pub fn new(text: impl Into<String>, at: IVec2, align: Align) -> Self {
        Self { text: text.into(), at, align, boxed: None }
    }

    pub fn with_box(mut self, boxed: TextBox) -> Self {
        self.boxed = Some(boxed);
        self
    }
}

/// A box behind a line: a background (T_AddBackground `0x8005CA5C`) and an
/// outline (T_AddOutline `0x8005CB20`), centred under the line. Its width
/// is the one asked for plus four, its height 16 unless asked for, and its
/// top 15 above the baseline, as the GPU capture of the load requester
/// (`validation/save/title_load_gpu`) draws them. The background is four
/// half-transparent quarters meeting at a brighter centre; the outline,
/// opaque, shades from corner to edge middle to corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextBox {
    pub width: i32,
    pub height: Option<i32>,
    pub style: BoxStyle,
}

/// The original's background and outline colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxStyle {
    /// A requester's whole panel (`0x800828AC`, `0x800828EC`): dark green,
    /// edged grey, lit from the top left.
    Panel,
    /// A requester's heading (`0x80082910`, `0x80082950`): edged black.
    Heading,
    /// The chosen line (`0x800829D8`, `0x800829B4`): edged white above and
    /// below, green at the ends.
    Selected,
}

impl BoxStyle {
    /// The background's edge and centre colours.
    fn fill(self) -> ([u8; 3], [u8; 3]) {
        match self {
            BoxStyle::Panel => ([0, 32, 0], [0, 96, 0]),
            BoxStyle::Heading => ([0; 3], [16, 128, 56]),
            BoxStyle::Selected => ([0; 3], [28, 120, 64]),
        }
    }

    /// The outline's colours at the top left, top middle, top right, right
    /// middle, bottom right, bottom middle, bottom left and left middle.
    fn outline(self) -> [[u8; 3]; 8] {
        const GREEN: [u8; 3] = [56, 240, 128];
        match self {
            BoxStyle::Panel => [[96; 3], [128; 3], [32; 3], [0; 3], [0; 3], [32; 3], [64; 3], [64; 3]],
            BoxStyle::Heading => [[0; 3]; 8],
            BoxStyle::Selected => [[0; 3], [255; 3], [0; 3], GREEN, [0; 3], [255; 3], [0; 3], GREEN],
        }
    }

    /// How far behind its line it draws: the panel behind everything.
    fn depth(self) -> f32 {
        match self {
            BoxStyle::Panel => 0.3,
            BoxStyle::Heading | BoxStyle::Selected => 0.1,
        }
    }
}

impl TextBox {
    /// Its pixels, from the left end of the line's baseline: left, top,
    /// width and height (the outline's last column and row are at left +
    /// width and top + height).
    pub fn rect(&self, line_width: i32) -> IRect {
        let width = self.width + 4;
        let left = line_width / 2 - width / 2;
        let top = -15;
        IRect::new(left, top, left + width, top + self.height.unwrap_or(16))
    }

    /// The background's four quarters (drawn half transparent), each a
    /// quad of the original's pixel corners and their colours.
    pub fn background(&self, line_width: i32) -> Vec<Quad> {
        let r = self.rect(line_width);
        let (edge, centre) = self.style.fill();
        let (mx, my) = (r.center().x, r.center().y);
        let [l, t, ri, b] = [r.min.x, r.min.y, r.max.x, r.max.y];
        [
            ([[l, t], [mx, t], [l, my], [mx, my]], 3),
            ([[mx, t], [ri, t], [mx, my], [ri, my]], 2),
            ([[l, my], [mx, my], [l, b], [mx, b]], 1),
            ([[mx, my], [ri, my], [mx, b], [ri, b]], 0),
        ]
        .map(|(corners, centre_at)| {
            let mut colours = [edge; 4];
            colours[centre_at] = centre;
            Quad { corners, colours }
        })
        .to_vec()
    }

    /// The outline's eight lines, each a pixel wide over the pixels between
    /// its ends, shaded from end to end.
    pub fn outline(&self, line_width: i32) -> Vec<Quad> {
        let r = self.rect(line_width);
        let (mx, my) = (r.center().x, r.center().y);
        let [l, t, ri, b] = [r.min.x, r.min.y, r.max.x, r.max.y];
        let points = [[l, t], [mx, t], [ri, t], [ri, my], [ri, b], [mx, b], [l, b], [l, my]];
        let colours = self.style.outline();
        (0..8)
            .map(|i| {
                let (a, z) = (points[i], points[(i + 1) % 8]);
                let (ca, cz) = (colours[i], colours[(i + 1) % 8]);
                let ([x0, y0], [x1, y1]) = ([a[0].min(z[0]), a[1].min(z[1])], [a[0].max(z[0]) + 1, a[1].max(z[1]) + 1]);
                // The colour at the lower coordinate end first.
                let (low, high) = if a[0] + a[1] <= z[0] + z[1] { (ca, cz) } else { (cz, ca) };
                let colours = if y1 - y0 == 1 { [low, high, low, high] } else { [low, low, high, high] };
                Quad { corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]], colours }
            })
            .collect()
    }
}

/// A gouraud quad in the original's pixels (y down), corners in the GPU's
/// order: top left, top right, bottom left, bottom right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quad {
    pub corners: [[i32; 2]; 4],
    pub colours: [[u8; 3]; 4],
}

/// Quads as one mesh in a line's frame (y up), triangles 0 1 2 and 1 2 3
/// as the GPU splits a quad.
fn quads_mesh(quads: &[Quad]) -> Mesh {
    let (mut positions, mut colours, mut indices) = (vec![], vec![], vec![]);
    for quad in quads {
        let start = positions.len() as u32;
        for ([x, y], c) in quad.corners.into_iter().zip(quad.colours) {
            positions.push([x as f32, -y as f32, 0.0]);
            colours.push(Color::srgb_u8(c[0], c[1], c[2]).to_linear().to_f32_array());
        }
        indices.extend([0, 2, 1, 1, 2, 3].map(|i| start + i));
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
        .with_inserted_indices(Indices::U32(indices))
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
        app.init_resource::<BoxMaterials>().add_systems(Update, (load_font, show_lines, draw_text).chain());
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

/// One sprite of a line, or its box.
#[derive(Component)]
struct Character;

/// The boxes' materials: the background half transparent, the outline opaque.
#[derive(Resource)]
struct BoxMaterials {
    background: Handle<PsxMaterial2d>,
    outline: Handle<PsxMaterial2d>,
}

impl FromWorld for BoxMaterials {
    fn from_world(world: &mut World) -> Self {
        // Headless apps have no 2D materials plugin; the store alone will do.
        let mut materials = world.get_resource_or_init::<Assets<PsxMaterial2d>>();
        let mut add = |blend| materials.add(PsxMaterial2d { blend, ..default() });
        Self { background: add(PsxBlend::Half), outline: add(PsxBlend::Opaque) }
    }
}

/// Lays out lines that are new or changed, or all of them for a new font.
fn draw_text(
    mut commands: Commands,
    font: Option<Res<OriginalFont>>,
    lines: Query<(Entity, Ref<ScreenText>, Option<&Children>)>,
    characters: Query<(), With<Character>>,
    box_materials: Res<BoxMaterials>,
    mut meshes: ResMut<Assets<Mesh>>,
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
        let line_width = width(&line.text);
        commands.entity(entity).insert(line.align.anchor(line.at, line_width));
        if let Some(boxed) = line.boxed {
            let depth = -boxed.style.depth();
            for (quads, material) in [
                (boxed.background(line_width), &box_materials.background),
                (boxed.outline(line_width), &box_materials.outline),
            ] {
                commands.spawn((
                    Character,
                    Mesh2d(meshes.add(quads_mesh(&quads))),
                    MeshMaterial2d(material.clone()),
                    // The outline over its background.
                    Transform::from_xyz(
                        0.0,
                        0.0,
                        if material == &box_materials.outline { depth + 0.01 } else { depth },
                    ),
                    ChildOf(entity),
                ));
            }
        }
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

    /// The load requester's panel and chosen line as the GPU capture of the
    /// title's load list draws them (`validation/save/title_load_gpu`,
    /// commands 308..331): the panel's lower left quarter (316) and its
    /// outline's left upper line (308); the chosen line's quarters
    /// (328..331) and its top right line (326).
    #[test]
    fn boxes_draw_as_the_original_draws_them() {
        let at = |q: &Quad, start: IVec2| {
            q.corners.map(|[x, y]| [x + start.x, y + start.y]).into_iter().zip(q.colours).collect::<Vec<_>>()
        };
        let panel = TextBox { width: 296, height: Some(130), style: BoxStyle::Panel };
        let start = Align::BOTTOM.start(IVec2::new(0, -174), width(" "));
        let quarters = panel.background(width(" "));
        let lower_left = at(&quarters[2], start);
        let g = |v: u8| [0, v, 0];
        assert_eq!(lower_left, [([34, 116], g(32)), ([184, 116], g(96)), ([34, 181], g(32)), ([184, 181], g(32))]);
        // A line covers its end pixels: column 34, rows 51..=116, shaded
        // from 96 at the top to 64.
        let left_upper = at(&panel.outline(width(" "))[7], start);
        assert_eq!(left_upper, [([34, 51], [96; 3]), ([35, 51], [96; 3]), ([34, 117], [64; 3]), ([35, 117], [64; 3])]);
        let chosen = TextBox { width: 284, height: None, style: BoxStyle::Selected };
        let text = "Current Position";
        let start = Align::BOTTOM.start(IVec2::new(0, -126), width(text));
        let centre = [28, 120, 64];
        let quarters: Vec<_> = chosen.background(width(text)).iter().map(|q| at(q, start)).collect();
        assert_eq!(quarters[0], [([40, 99], [0; 3]), ([184, 99], [0; 3]), ([40, 107], [0; 3]), ([184, 107], centre)]);
        assert_eq!(
            quarters[3],
            [([184, 107], centre), ([328, 107], [0; 3]), ([184, 115], [0; 3]), ([328, 115], [0; 3])]
        );
        let top_right = at(&chosen.outline(width(text))[1], start);
        assert_eq!(
            top_right,
            [([184, 99], [255; 3]), ([329, 99], [0; 3]), ([184, 100], [255; 3]), ([329, 100], [0; 3])]
        );
    }

    /// The width counts twelve for the glyphs numbered like digit codes.
    #[test]
    fn width_keeps_the_originals_digit_test() {
        assert_eq!(glyph(b'w'), Some(48));
        assert_eq!(width("w"), (DIGIT_CELL - LETTER_SPACING) & !1);
        assert_eq!(width("6"), width_of(58) & !1);
    }
}
