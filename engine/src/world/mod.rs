//! The level: rooms on a sector grid joined by portals, floor descriptions
//! with slopes and triggers, models, animations and placements, and the
//! collision queries over them.
//!
//! The asset is [`Level`] ([`format`] describes the file and its units).
//! Rules work in Bevy's level space: y up, metres; [`position`] and
//! [`units`] convert from the file's integer world units.
use std::sync::Arc;

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};

pub mod format;
mod navigation;
pub use format::{
    Ambient, Animation, Command, FixedCamera, FloorInfo, Level, Light, Mesh as ObjectMesh, Model, NavBox, Placement, Pose, Room,
    Sector, Sound, StaticMesh, StaticPlacement, Texture, Trigger, TriggerKind,
};

/// World units per metre (and per sector).
pub const UNITS_PER_METRE: f32 = 1024.0;

/// World units (1/1024 m) as metres.
pub const fn units(value: f32) -> f32 {
    value / UNITS_PER_METRE
}

/// A file position (world units, y down) in level space (metres, y up).
pub fn position(v: [i32; 3]) -> Vec3 {
    Vec3::new(v[0] as f32, -v[1] as f32, v[2] as f32) / UNITS_PER_METRE
}

/// A binary angle (65536 to a turn) in radians.
pub fn angle(v: u16) -> f32 {
    f32::from(v) * std::f32::consts::TAU / 65536.0
}

/// Loads `level.json` files of the engine's format.
#[derive(Default, TypePath)]
pub struct LevelLoader;

impl AssetLoader for LevelLoader {
    type Asset = Level;
    type Settings = ();
    type Error = Box<dyn std::error::Error + Send + Sync>;
    async fn load(&self, reader: &mut dyn Reader, _: &(), _: &mut LoadContext<'_>) -> Result<Level, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let level: Level = serde_json::from_slice(&bytes)?;
        if level.format != format::FORMAT_VERSION {
            return Err(format!("level format {} (this engine reads {})", level.format, format::FORMAT_VERSION).into());
        }
        Ok(level)
    }
    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}

/// The loaded level, for rules.
#[derive(Resource, Clone)]
pub struct WorldGeometry {
    pub data: Arc<Level>,
}

impl WorldGeometry {
    pub fn new(level: Level) -> Self {
        Self { data: Arc::new(level) }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FloorHit {
    pub height: f32,
    pub ceiling: f32,
    pub room: usize,
    pub water: bool,
    pub death: bool,
}

static NO_FLOOR_INFO: FloorInfo =
    FloorInfo { portal: None, floor_slope: [0, 0], ceiling_slope: [0, 0], kill: false, climb: None, triggers: Vec::new() };

impl Level {
    /// What a sector's floor does besides having a height.
    pub fn floor_info(&self, sector: &Sector) -> &FloorInfo {
        sector.floor_info.and_then(|i| self.floor_infos.get(i)).unwrap_or(&NO_FLOOR_INFO)
    }

    /// The model drawn for object type `kind`.
    pub fn model(&self, kind: u16) -> Option<&Model> {
        self.models.iter().find(|m| m.id == kind)
    }
}

impl WorldGeometry {
    pub fn slope_at(&self, pos: Vec3, room: usize) -> [i8; 2] {
        let Some(hit) = self.floor_at(pos, room) else { return [0, 0] };
        let x = (pos.x * 1024.0).floor() as i32;
        let z = (pos.z * 1024.0).floor() as i32;
        let mut room = hit.room;
        for _ in 0..64 {
            let Some(s) = self.sector(room, x, z) else { return [0, 0] };
            match s.below {
                Some(below) if below != room => room = below,
                _ => return self.data.floor_info(s).floor_slope,
            }
        }
        [0, 0]
    }

    /// Navigation box of the sector containing (x, z) in `room`'s own grid,
    /// as the camera's box lookup reads it (no portal walk).
    pub fn navigation_box(&self, room: usize, x: i32, z: i32) -> Option<usize> {
        self.sector(room, x, z).and_then(|s| s.nav_box)
    }

    fn sector(&self, room: usize, x: i32, z: i32) -> Option<&Sector> {
        let r = self.data.rooms.get(room)?;
        let sx = (x - r.origin[0]).div_euclid(1024);
        let sz = (z - r.origin[2]).div_euclid(1024);
        if sx < 0 || sz < 0 || sx as usize >= r.x_sectors || sz as usize >= r.z_sectors {
            return None;
        }
        r.sectors.get(sx as usize * r.z_sectors + sz as usize)
    }

    /// Sector portal and height rules are documented in docs/format-notes.md.
    pub fn floor_at(&self, pos: Vec3, room: usize) -> Option<FloorHit> {
        self.floor_at_for(pos, room, false)
    }

    /// `camera`: the camera's view of the floor (the original's flag
    /// 0x80087a80), under which GetHeight (0x8001ea8c) ignores floor slopes
    /// with a tilt of 3 or more on either axis and GetCeiling (0x8001f850)
    /// ceiling slopes beyond 2, so steep ramps read as their flat height.
    pub fn floor_at_for(&self, pos: Vec3, room: usize, camera: bool) -> Option<FloorHit> {
        let x = (pos.x * 1024.0).floor() as i32;
        let y = (-pos.y * 1024.0).floor() as i32;
        let z = (pos.z * 1024.0).floor() as i32;
        let mut room = room;
        let rooms = self.data.rooms.len();
        // Portals form cycles. Bound traversal, rather than recurse on bad assets.
        for _ in 0..rooms.min(64) {
            let sector = self.sector(room, x, z)?;
            let info = self.data.floor_info(sector);
            if let Some(next) = info.portal.filter(|&r| r != room && r < rooms) {
                room = next;
                continue;
            }
            if let Some(below) = sector.below.filter(|&r| y >= sector.floor && r != room) {
                room = below;
                continue;
            }
            if let Some(above) = sector.above.filter(|&r| y < sector.ceiling && r != room) {
                room = above;
                continue;
            }
            break;
        }
        let current_room = room;
        let water = self.data.rooms.get(room)?.water;
        // The collision floor is at the bottom of a room stack, even when the
        // player is still in its top room. Keep current_room for rendering/water.
        for _ in 0..64 {
            let s = self.sector(room, x, z)?;
            match s.below {
                Some(below) if below != room => room = below,
                _ => break,
            }
        }
        let s = self.sector(room, x, z)?;
        if s.floor <= format::SOLID {
            return None;
        }
        let info = self.data.floor_info(s);
        let mut slope = info.floor_slope;
        if camera && slope.iter().any(|t| t.unsigned_abs() >= 3) {
            slope = [0, 0];
        }
        let slant = |slope: [i8; 2]| -> i32 {
            let [sx, sz] = slope.map(i32::from);
            let dx = x.rem_euclid(1024);
            let dz = z.rem_euclid(1024);
            // Arithmetic shift of the signed product: positive tilts truncate,
            // negative tilts round away from zero (both confirmed by the first
            // game's recorded climbs and ramps).
            let rise = |t: i32, d: i32| if t > 0 { -((t * (1023 - d)) >> 2) } else { (t * d) >> 2 };
            rise(sx, dx) + rise(sz, dz)
        };
        let mut ceiling_room = current_room;
        for _ in 0..64 {
            let top = self.sector(ceiling_room, x, z)?;
            match top.above {
                Some(above) if above != ceiling_room => ceiling_room = above,
                _ => break,
            }
        }
        let top = self.sector(ceiling_room, x, z)?;
        let mut ceiling_slopes = self.data.floor_info(top).ceiling_slope;
        if camera && ceiling_slopes.iter().any(|t| t.unsigned_abs() > 2) {
            ceiling_slopes = [0, 0];
        }
        let [sx, sz] = ceiling_slopes.map(i32::from);
        let dx = x.rem_euclid(1024);
        let dz = z.rem_euclid(1024);
        let ceiling = top.ceiling - ((sx * if sx < 0 { dx - 1023 } else { dx }) >> 2) + ((sz * if sz > 0 { dz - 1023 } else { dz }) >> 2);
        Some(FloorHit {
            height: -(s.floor - slant(slope)) as f32 / 1024.0,
            ceiling: -ceiling as f32 / 1024.0,
            room: current_room,
            water,
            death: info.kill,
        })
    }

    /// Height of the water surface over (x, z), level metres, or `None`
    /// without water there. Inside a water room it is the ceiling of the
    /// highest water sector in the stack; in a dry room it is the floor of
    /// the lowest dry sector above water (PSX 0x8001E6BC). Only horizontal
    /// portals are followed to find the starting room.
    pub fn water_surface(&self, pos: Vec3, room: usize) -> Option<f32> {
        let x = (pos.x * 1024.0).floor() as i32;
        let z = (pos.z * 1024.0).floor() as i32;
        let rooms = &self.data.rooms;
        let mut room = room;
        for _ in 0..rooms.len().min(64) {
            let sector = self.sector(room, x, z)?;
            match self.data.floor_info(sector).portal.filter(|&r| r != room && r < rooms.len()) {
                Some(next) => room = next,
                None => break,
            }
        }
        let mut sector = self.sector(room, x, z)?;
        if rooms.get(room)?.water {
            for _ in 0..64 {
                match sector.above.filter(|&r| rooms.get(r).is_some_and(|r| r.water)) {
                    Some(above) => sector = self.sector(above, x, z)?,
                    None => break,
                }
            }
            return Some(-sector.ceiling as f32 / 1024.0);
        }
        for _ in 0..64 {
            let below = sector.below?;
            if rooms.get(below)?.water {
                return Some(-sector.floor as f32 / 1024.0);
            }
            sector = self.sector(below, x, z)?;
        }
        None
    }

    /// Select rooms connected by nearby portals. Geometry remains real world
    /// meshes; this only prevents distant rooms showing through open portals.
    pub fn visible_rooms(&self, room: usize, pos: Vec3) -> Vec<bool> {
        let mut visible = vec![false; self.data.rooms.len()];
        // Visit shortest portal paths first. A depth-first walk can reach a
        // room at the depth limit through a cycle, mark it visited, then
        // suppress a shorter path that would reveal its neighbours.
        let mut pending = std::collections::VecDeque::from([(room, 0)]);
        while let Some((id, depth)) = pending.pop_front() {
            if id >= visible.len() || visible[id] {
                continue;
            }
            visible[id] = true;
            if depth >= 8 {
                continue;
            }
            for p in &self.data.rooms[id].portals {
                let r = &self.data.rooms[id];
                let near = p.vertices.iter().any(|&v| {
                    (position([v[0] + r.origin[0], v[1], v[2] + r.origin[2]]) - pos).length_squared() < 28.0 * 28.0
                });
                if near {
                    pending.push_back((p.room, depth + 1));
                }
            }
        }
        visible
    }

    /// Floor description of the sector under `position` in `room`, located
    /// the way floor triggers are: coordinates truncate toward zero, and no
    /// portal is followed.
    pub fn trigger_floor(&self, position: Vec3, room: usize) -> Option<&FloorInfo> {
        let room = self.data.rooms.get(room)?;
        let x = (position.x * 1024.0) as i32;
        let z = (position.z * 1024.0) as i32;
        let sx = (x - room.origin[0]).div_euclid(1024);
        let sz = (z - room.origin[2]).div_euclid(1024);
        if sx < 0 || sz < 0 || sx as usize >= room.x_sectors || sz as usize >= room.z_sectors {
            return None;
        }
        let sector = room.sectors.get(sx as usize * room.z_sectors + sz as usize)?;
        Some(self.data.floor_info(sector))
    }

    /// Bounds of every mesh vertex of the model of object type `kind`, in its
    /// own space (y up, metres).
    pub fn model_bounds(&self, kind: u16) -> Option<(Vec3, Vec3)> {
        let model = self.data.model(kind)?;
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for &mesh in &model.meshes {
            if let Some(mesh) = self.data.meshes.get(mesh) {
                for v in &mesh.vertices {
                    let p = position([v[0], v[1], v[2]]);
                    min = min.min(p);
                    max = max.max(p);
                }
            }
        }
        (min.is_finite() && max.is_finite()).then_some((min, max))
    }
}

/// A sampled sector trace: false when the line from `from` to `to` passes
/// through a floor, a ceiling or a solid sector.
pub fn clear_line(world: Option<&WorldGeometry>, from: Vec3, to: Vec3, room: usize) -> bool {
    let Some(world) = world else {
        return true;
    };
    let count = ((from.distance(to) * 8.0).ceil().max(1.0) as usize).min(512);
    let mut room = room;
    for i in 0..=count {
        let point = from.lerp(to, i as f32 / count as f32);
        let Some(hit) = world.floor_at(point, room) else {
            return false;
        };
        if point.y <= hit.height || point.y >= hit.ceiling {
            return false;
        }
        room = hit.room;
    }
    true
}
