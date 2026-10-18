//! The original's view: projection, display window and fog distances.
use bevy::prelude::*;
use engine::render::ViewSettings;

/// Tomb Raider II's level camera. Evidence: docs/original-evidence.md.
///
/// - FOV: `0x80065cb8` stores the GTE projection distance H=228 (RAM
///   `0x8008c6d4`, Home and Wall checkpoints); on 240 lines the vertical FOV is
///   `2*atan(120/228)` = 55.5170812°.
/// - Size: the projection centre is 192x120 (`0x80065c48`/`0x80065c90`), so
///   the projected view is 384x240; the display window shows its first 368
///   columns (native captures are 368x240), cropping the right edge.
/// - Near/far and fog: provisional, chosen by visual comparison, not read
///   from the binary.
pub fn view_settings() -> ViewSettings {
    ViewSettings {
        fov: 55.517_082_f32.to_radians(),
        near: 0.05,
        far: 80.0,
        fog_start: 12.0,
        fog_end: 25.0,
        full_size: UVec2::new(384, 240),
        size: crate::SCREEN,
    }
}
