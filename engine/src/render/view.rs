//! Presentation coordinate boundary, verified against original camera matrices.
//!
//! Rules use (source X, -source Y, source Z). Reflecting Z for the whole level
//! gives a right-handed render world. The original view at 0x8008aa60 faces +X
//! with screen-right along -Z; see docs/view-evidence.md.
use bevy::prelude::*;

pub const LEVEL_TO_RENDER: Vec3 = Vec3::new(1.0, 1.0, -1.0);

pub fn render_position(game_position: Vec3) -> Vec3 {
    game_position * LEVEL_TO_RENDER
}

/// Ordinary Bevy camera in render coordinates, independent of the level root.
pub fn camera_transform(eye: Vec3, target: Vec3) -> Transform {
    Transform::from_translation(render_position(eye)).looking_at(render_position(target), Vec3::Y)
}
