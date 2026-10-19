//! New rooms: a rectangle of open floor with a solid border ring, built
//! like imported rooms so the joining tools and collision treat it the same.
//!
//! Draw the open part next to another room's open floor, sharing an edge,
//! and the two grids overlap by two sectors as imported neighbours do, so
//! [`super::connect`] can open a doorway between them.
use super::*;
use crate::world::{Ambient, Sector};

/// What to build.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewRoom {
    /// The open floor, in world sector numbers: `x[0]..x[1]` by `z[0]..z[1]`.
    pub x: [i32; 2],
    pub z: [i32; 2],
    /// Floor and ceiling heights (y down, whole 256-unit steps).
    pub floor: i32,
    pub ceiling: i32,
    /// Floor, ceiling and wall textures.
    pub textures: [usize; 3],
    /// Vertex shade (128 is neutral).
    pub shade: i32,
    pub ambient: Ambient,
}

/// Add a room; returns its index.
pub fn new_room(level: &mut Level, spec: &NewRoom) -> Result<usize, String> {
    let (width, depth) = (spec.x[1] - spec.x[0], spec.z[1] - spec.z[0]);
    if width < 1 || depth < 1 {
        return Err("a room needs at least one open sector".into());
    }
    if spec.floor <= spec.ceiling {
        return Err("the floor must be below the ceiling".into());
    }
    if spec.floor % STEP != 0 || spec.ceiling % STEP != 0 {
        return Err("floor and ceiling heights come in whole 256-unit steps".into());
    }
    if spec.textures.iter().any(|&t| t >= level.textures.len()) {
        return Err("no such texture".into());
    }
    let (w, d) = (width as usize + 2, depth as usize + 2);
    let mut room = Room {
        origin: [(spec.x[0] - 1) * SECTOR, 0, (spec.z[0] - 1) * SECTOR],
        top: spec.ceiling,
        bottom: spec.floor,
        x_sectors: w,
        z_sectors: d,
        ambient: spec.ambient,
        ..Default::default()
    };
    let open = |x: i32, z: i32| x > 0 && z > 0 && x + 1 < w as i32 && z + 1 < d as i32;
    for x in 0..w as i32 {
        for z in 0..d as i32 {
            room.sectors.push(if open(x, z) {
                Sector { floor: spec.floor, ceiling: spec.ceiling, ..Default::default() }
            } else {
                Sector { floor: SOLID, ceiling: SOLID, ..Default::default() }
            });
        }
    }
    let [floor_texture, ceiling_texture, wall_texture] = spec.textures;
    let mut added = BTreeMap::new();
    for x in 1..=width {
        for z in 1..=depth {
            let (x0, z0) = (x * SECTOR, z * SECTOR);
            let square = |y| [[x0, y, z0], [x0 + SECTOR, y, z0], [x0 + SECTOR, y, z0 + SECTOR], [x0, y, z0 + SECTOR]];
            for (y, toward, texture) in
                [(spec.floor, [0, -1, 0], floor_texture), (spec.ceiling, [0, 1, 0], ceiling_texture)]
            {
                let c = square(y).map(|p| vertex_at(&mut room, p, spec.shade, &mut added));
                let c = facing(&room, c, toward);
                room.quads.push([c[0], c[1], c[2], c[3], texture]);
            }
            // Walls face in from the border ring, in the order -x, +x, -z, +z.
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                if open(x + dx, z + dz) {
                    continue;
                }
                let foot = square(spec.floor);
                let edge = match (dx, dz) {
                    (-1, _) => [foot[0], foot[3]],
                    (1, _) => [foot[1], foot[2]],
                    (_, -1) => [foot[0], foot[1]],
                    _ => [foot[3], foot[2]],
                };
                let top = |p: [i32; 3]| [p[0], spec.ceiling, p[2]];
                let c = [edge[0], edge[1], top(edge[1]), top(edge[0])]
                    .map(|p| vertex_at(&mut room, p, spec.shade, &mut added));
                let c = facing(&room, c, [-dx, 0, -dz]);
                room.quads.push([c[0], c[1], c[2], c[3], wall_texture]);
            }
        }
    }
    level.rooms.push(room);
    Ok(level.rooms.len() - 1)
}

/// The floor, ceiling and wall textures a room uses most, for building next
/// to it in the same style.
pub fn typical_textures(level: &Level, room: usize) -> [usize; 3] {
    let Some(r) = level.rooms.get(room) else { return [0; 3] };
    let mut counts: [BTreeMap<usize, usize>; 3] = Default::default();
    for q in &r.quads {
        let slot = match sector_face(r, q).map(|f| f.surface) {
            Some(Surface::Floor) => 0,
            Some(Surface::Ceiling) => 1,
            None => 2,
        };
        *counts[slot].entry(q[4]).or_default() += 1;
    }
    let any = r.quads.first().map_or(0, |q| q[4]);
    counts.map(|c| c.into_iter().max_by_key(|&(t, n)| (n, std::cmp::Reverse(t))).map_or(any, |(t, _)| t))
}

/// A room's middle vertex shade, so a new room next to it is lit alike.
pub fn typical_shade(level: &Level, room: usize) -> i32 {
    let Some(r) = level.rooms.get(room) else { return 128 };
    let mut shades: Vec<i32> = r.vertices.iter().map(|v| v[3]).collect();
    shades.sort_unstable();
    shades.get(shades.len() / 2).copied().unwrap_or(128)
}
