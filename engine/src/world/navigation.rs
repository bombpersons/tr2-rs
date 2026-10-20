//! Local corridor targets over authored navigation boxes. Coordinates are
//! converted once at the boundary to the level's1024-units-per-sector grid.
//! General multi-box search and dynamic blockers are separate from this query.
use super::WorldGeometry;
use bevy::prelude::*;

/// Clearance and permitted floor changes for a local ground corridor.
/// Distances use the level's integer world units. `zone` selects an authored
/// movement-zone table; absent zone filtering preserves the flat query API.
#[derive(Clone, Copy, Debug, Default)]
pub struct CorridorLimits {
    pub margin: i32,
    pub max_step: i32,
    pub max_drop: i32,
    pub zone: Option<usize>,
}

impl WorldGeometry {
    /// A same-box or directly overlapping, flat-floor corridor target.
    /// Outside the destination on one axis, retain the current perpendicular
    /// coordinate, constrained inside the destination's clearance margins.
    /// This is the straight-corridor subset of PSX0x80015DD4..0x800161DC.
    /// Unsupported paths return None, so the game's controller owns fallback.
    pub fn adjacent_path_target(
        &self,
        origin: Vec3,
        room: usize,
        goal: Vec3,
        goal_room: usize,
        margin: i32,
    ) -> Option<Vec3> {
        self.adjacent_path_target_with_limits(origin, room, goal, goal_room, CorridorLimits { margin, ..default() })
    }

    /// The straight, directly connected corridor query with caller-supplied
    /// step/drop and zone limits. This chooses a steering target, not a floor
    /// movement response or a multi-box route. PSX0x800151F0..0x80015224
    /// filters reverse-search edges by zone and signed floor difference.
    pub fn adjacent_path_target_with_limits(
        &self,
        origin: Vec3,
        room: usize,
        goal: Vec3,
        goal_room: usize,
        limits: CorridorLimits,
    ) -> Option<Vec3> {
        let point = |p: Vec3| [(p.x * 1024.0).round() as i32, (p.z * 1024.0).round() as i32];
        let [x, z] = point(origin);
        let [gx, gz] = point(goal);
        let start = self.navigation_box(room, x, z)?;
        let end = self.navigation_box(goal_room, gx, gz)?;
        if start == end {
            return Some(goal);
        }
        let (a, b) = (self.data.boxes.get(start)?, self.data.boxes.get(end)?);
        // Native search expands from goal toward origin. In forward movement
        // terms the two bounds are a maximum rise and a maximum drop.
        let drop = b.floor - a.floor;
        if drop < -limits.max_step || drop > limits.max_drop || b.overlap & 0x4000 != 0 {
            return None;
        }
        if let Some(index) = limits.zone {
            let zones = self.data.zones.get(index)?;
            if zones.get(start)? != zones.get(end)? {
                return None;
            }
        }
        let mut adjacent = false;
        for &entry in self.data.overlaps.get((a.overlap & 0x3fff) as usize..)? {
            adjacent |= (entry & 0x7fff) as usize == end;
            if entry & 0x8000 != 0 {
                break;
            }
        }
        if !adjacent {
            return None;
        }
        let (xmin, xmax, zmin, zmax) = (b.x_min * 1024, b.x_max * 1024 - 1, b.z_min * 1024, b.z_max * 1024 - 1);
        let outside_x = x < xmin || x > xmax;
        let outside_z = z < zmin || z > zmax;
        if outside_x && outside_z {
            return None;
        }
        // Check containment geometrically, not merely by sector's box id:
        // overlapping boxes can contain the actor before that id changes.
        let margin = limits.margin;
        let inset = |value: i32, min: i32, max: i32| {
            if value < min + margin {
                min + margin
            } else if value > max - margin {
                max - margin
            } else {
                value
            }
        };
        Some(Vec3::new(
            if outside_z { inset(x, xmin, xmax) as f32 / 1024.0 } else { goal.x },
            goal.y,
            if outside_x { inset(z, zmin, zmax) as f32 / 1024.0 } else { goal.z },
        ))
    }
}
