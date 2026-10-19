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

/// A short-lived point light (muzzle flashes, explosions), in level units
/// with y down. The original keeps up to ten per frame in a table filled by
/// 0x80028A28 and cleared every frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub struct DynamicLight {
    pub position: IVec3,
    /// Brightness at the centre is `1 << intensity` (8191 = darkest shade).
    pub intensity: u32,
    /// Reach: the light ends `1 << falloff` units away, fading with the
    /// squared distance.
    pub falloff: u32,
}

impl DynamicLight {
    /// Brightness at `point`: `2^intensity - d^2 / 2^(2*falloff - intensity)`
    /// inside the cube and sphere of radius `2^falloff`, else none
    /// (0x8006CB08..0x8006CBC8 for items, 0x8006CCBC for static meshes).
    pub fn at(&self, point: IVec3) -> Option<i32> {
        let reach = 1i64 << self.falloff;
        let d = (point - self.position).as_i64vec3();
        if d.abs().max_element() > reach {
            return None;
        }
        let squared = d.length_squared();
        let shift = (2 * self.falloff).saturating_sub(self.intensity);
        (squared <= reach * reach).then(|| ((1i64 << self.intensity) - (squared >> shift)) as i32)
    }

    /// A room vertex's grey shade byte lit by this light (0x8006CE30):
    /// brightness / 32 is added, clamped to 0..=255. Shade 255 is left alone.
    pub fn light_room_vertex(&self, shade: u8, point: IVec3) -> u8 {
        match self.at(point) {
            Some(light) if shade != 0xff => (i32::from(shade) + ((light << 16) >> 21)).clamp(0, 255) as u8,
            _ => shade,
        }
    }

    /// Whether the light can reach a room's floor area at all: the original
    /// skips rooms whose sectors, less the outer walls, its cube misses
    /// (0x8006CF0C..0x8006CF68). `origin` is the room's, `sectors` its x/z
    /// sector counts.
    pub fn reaches_room(&self, origin: IVec3, sectors: [i32; 2]) -> bool {
        let reach = 1i32 << self.falloff;
        let (x, z) = (self.position.x - origin.x, self.position.z - origin.z);
        x - reach <= (sectors[0] - 1) * 1024
            && z - reach <= (sectors[1] - 1) * 1024
            && x + reach > 1023
            && z + reach > 1023
    }
}

/// This frame's [`DynamicLight`]s. Games replace them every frame; the
/// renderer lights items and (with a room mesh policy) room vertices.
#[derive(Resource, Reflect, Clone, Debug, Default, PartialEq)]
#[reflect(Resource)]
pub struct DynamicLights(pub Vec<DynamicLight>);

/// S_CalculateLight 0x8006c6f4 (room light mode 0): each room light gives
/// intensity*f^2/(d^2/4096 + f^2) with f^2 = falloff^2>>12; the brightest
/// wins. Each dynamic light in reach adds its brightness and, when brighter
/// than the best so far, takes over the direction. The sum is halved. The
/// flickering light modes are not yet ported.
pub fn item_light(data: &Level, room: usize, point: IVec3) -> ItemLight {
    item_light_with(data, room, point, &[])
}

/// [`item_light`] with this frame's dynamic lights.
pub fn item_light_with(data: &Level, room: usize, point: IVec3, dynamic: &[DynamicLight]) -> ItemLight {
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
    let mut total = brightest;
    let mut best = brightest;
    for light in dynamic {
        if let Some(shade) = light.at(point) {
            let shade = i64::from(shade);
            if best < shade {
                best = shade;
                offset = point - light.position;
            }
            total += shade;
        }
    }
    let strength = (total >> 1) as i32;
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
    fn dynamic_lights_fade_with_squared_distance_to_their_reach() {
        let light = DynamicLight { position: IVec3::new(1000, 0, 0), intensity: 12, falloff: 11 };
        assert_eq!(light.at(light.position), Some(4096));
        // 4096 - 1024^2 >> 10.
        assert_eq!(light.at(IVec3::new(2024, 0, 0)), Some(3072));
        assert_eq!(light.at(IVec3::new(3048, 0, 0)), Some(0));
        assert_eq!(light.at(IVec3::new(3049, 0, 0)), None, "outside the cube");
        assert_eq!(light.at(IVec3::new(2600, 0, 1300)), None, "inside the cube, outside the sphere");
        // Rooms add a 32nd, clamped, and leave full-bright vertices alone.
        assert_eq!(light.light_room_vertex(100, light.position), 228);
        assert_eq!(light.light_room_vertex(200, light.position), 255);
        assert_eq!(light.light_room_vertex(255, light.position), 255);
        assert!(light.reaches_room(IVec3::ZERO, [3, 3]));
        assert!(!light.reaches_room(IVec3::new(4096, 0, 0), [3, 3]));
    }

    #[test]
    fn unlit_items_take_the_ambient_shade() {
        let light = ItemLight { ambient: 7691, strength: 0, direction: Vec3::ZERO };
        // RAM ambient 7691 at the avatar: 255 - 7691/32 = 14.66 (GPU greys 8..23).
        assert!((light.shade(Vec3::Y) * 128.0 - 14.66).abs() < 0.01);
    }
}
