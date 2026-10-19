//! 2D overlays (HUD, menus, title screens) laid out in the original's screen
//! pixels and scaled with the screen's height.
//!
//! The original drew its 2D at one display size ([`ViewSettings::size`]).
//! When the PS1 screen is resized, an overlay camera
//! ([`overlay_projection`]) keeps that many units from top to bottom, so
//! overlays keep their size relative to the picture (2x resolution draws them
//! at 2x), while a wider screen adds units at the sides. [`OverlayAnchor`]
//! keeps an element at a screen edge or corner as the width changes. At the
//! original's size both are exactly one unit per pixel with the origin at the
//! centre, as a plain `Camera2d` would be.
use bevy::{camera::ScalingMode, prelude::*};

use super::ViewSettings;
use crate::psx::PsxScreen;

/// The projection for a `Camera2d` drawing overlays: [`ViewSettings::size`]'s
/// height in units, whatever the screen's size.
pub fn overlay_projection(settings: &ViewSettings) -> Projection {
    Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::FixedVertical { viewport_height: settings.size.y as f32 },
        ..OrthographicProjection::default_2d()
    })
}

/// The overlay's size in units on a `screen` of real pixels: the original's
/// height, and the width that keeps the screen's shape.
pub fn overlay_size(settings: &ViewSettings, screen: UVec2) -> Vec2 {
    let height = settings.size.y as f32;
    Vec2::new(height * screen.x as f32 / screen.y.max(1) as f32, height)
}

/// Keeps an overlay element at a point of the screen, in overlay units:
/// `at` is -1..=1 per axis (`(-1, 1)` top left, `(0, -1)` bottom centre),
/// scaled by half the overlay's size, plus `offset`. Sets the `Transform`'s
/// x and y; z, rotation and scale stay the game's. The overlay counterpart
/// of `psx::PsxAnchor`, which works in real screen pixels.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(Component)]
#[require(Transform)]
pub struct OverlayAnchor {
    pub at: Vec2,
    pub offset: Vec2,
}

impl OverlayAnchor {
    pub fn new(at: Vec2, offset: Vec2) -> Self {
        Self { at, offset }
    }
    /// The position on an overlay of `size` units, origin at its centre.
    pub fn position(&self, size: Vec2) -> Vec2 {
        self.at * size / 2.0 + self.offset
    }
}

pub(super) fn place(
    settings: Res<ViewSettings>,
    screen: Option<Res<PsxScreen>>,
    mut anchored: Query<(Ref<OverlayAnchor>, &mut Transform)>,
) {
    let Some(screen) = screen else { return };
    let size = overlay_size(&settings, screen.size());
    let resized = screen.is_changed() || settings.is_changed();
    for (anchor, mut transform) in &mut anchored {
        if resized || anchor.is_changed() {
            let at = anchor.position(size);
            (transform.translation.x, transform.translation.y) = (at.x, at.y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_keep_their_place_at_the_original_size_and_follow_the_edges() {
        let settings = ViewSettings { size: UVec2::new(320, 240), ..default() };
        let top_left = OverlayAnchor::new(Vec2::new(-1.0, 1.0), Vec2::new(8.0, -8.0));
        assert_eq!(top_left.position(overlay_size(&settings, UVec2::new(320, 240))), Vec2::new(-152.0, 112.0));
        // 2x: the same overlay units, so the same place on the picture.
        assert_eq!(top_left.position(overlay_size(&settings, UVec2::new(640, 480))), Vec2::new(-152.0, 112.0));
        // 16:9 at 240 lines: 426 units wide, still 8 from the left edge.
        assert_eq!(top_left.position(overlay_size(&settings, UVec2::new(426, 240))), Vec2::new(-205.0, 112.0));
    }
}
