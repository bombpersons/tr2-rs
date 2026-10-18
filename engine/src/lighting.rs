//! Room and item lighting, ported from the PSX executable
//! (docs/lighting-evidence.md). Renderer-independent: presentation turns these
//! values into vertex colours and material lighting.
use bevy::prelude::*;

use crate::world::Level;

/// The GPU's neutral texture modulation: a shade byte of 128 is 1.0.
pub const NEUTRAL_SHADE: f32 = 128.0;

/// Room vertex colour, 0x800617b0/0x80062090: the low byte of the 16-bit
/// vertex shade drawn as grey. Returned as the encoded multiplier.
pub fn room_vertex_shade(shade: i32) -> f32 {
    (shade & 0xff) as f32 / NEUTRAL_SHADE
}

/// Result of S_CalculateLight 0x8006c6f4 for one item.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ItemLight {
    /// 0 (bright) .. 8191 (dark), after subtracting the light strength.
    pub ambient: i32,
    /// Halved brightest light contribution (0 when unlit).
    pub strength: i32,
    /// Unit vector from the brightest light to the item, original axes (y down).
    pub direction: Vec3,
}

impl ItemLight {
    /// Encoded vertex multiplier for a unit normal (original axes) as the
    /// object lighter 0x80064954 computes it: 255 - clamp(ambient +
    /// strength*(n.L), 0, 8191)/32, with 128 neutral. Its rotation, normal
    /// and light vector all use 16384 for one; the >>16 dot product and
    /// directional denominator cancel those scales (0x80064974..0x80064ABC).
    pub fn shade(&self, normal: Vec3) -> f32 {
        let v = self.ambient as f32 + self.strength as f32 * normal.dot(self.direction);
        (255.0 - v.clamp(0.0, 8191.0) / 32.0) / NEUTRAL_SHADE
    }
}

/// S_CalculateLight 0x8006c6f4 (static-light branch, room light mode 0): each
/// room light gives intensity*f^2/(d^2/4096 + f^2) with f^2 = falloff^2>>12;
/// the brightest wins and is halved. Dynamic lights (0x8008b240) and the
/// flickering light modes are not yet ported.
pub fn item_light(data: &Level, room: usize, point: IVec3) -> ItemLight {
    let Some(room) = data.rooms.get(room) else { return ItemLight::default() };
    let mut brightest = 0i64;
    let mut offset = IVec3::ZERO;
    for light in &room.lights {
        let d = point.as_i64vec3() - IVec3::from_array(light.position).as_i64vec3();
        let distance = (d.x * d.x + d.y * d.y + d.z * d.z) >> 12;
        let falloff = (i64::from(light.falloff) * i64::from(light.falloff)) >> 12;
        let intensity = i64::from(light.intensity);
        if distance + falloff == 0 {
            continue;
        }
        let shade = intensity * falloff / (distance + falloff);
        if brightest < shade {
            brightest = shade;
            offset = d.as_ivec3();
        }
    }
    let strength = (brightest >> 1) as i32;
    let ambient = room.ambient.shade;
    if strength == 0 {
        return ItemLight { ambient, strength: 0, direction: Vec3::ZERO };
    }
    ItemLight { ambient: ambient - strength, strength, direction: offset.as_vec3().normalize_or_zero() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_shade_uses_the_low_byte_with_128_neutral() {
        // A RAM room vertex 0x14520401 (first game) draws grey 0x52.
        assert_eq!(room_vertex_shade(0x1452), 82.0 / 128.0);
    }

    #[test]
    fn unlit_items_take_the_ambient_shade() {
        let light = ItemLight { ambient: 7691, strength: 0, direction: Vec3::ZERO };
        // RAM ambient 7691 at the avatar: 255 - 7691/32 = 14.66 (GPU greys 8..23).
        assert!((light.shade(Vec3::Y) * 128.0 - 14.66).abs() < 0.01);
    }
}
