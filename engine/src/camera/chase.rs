//! Chase camera, ported from the PSX executable (docs/chase-camera-evidence.md).
//!
//! Everything here works in original integer units (y down) so truncation in
//! the original's fixed-point arithmetic is reproduced: ideal eye 0x80019214,
//! smart shift 0x80018800 with candidate callback 0x80018594, line of sight
//! 0x8001fb54 (axis sweeps 0x8001fc70/0x80020078, clip 0x80020480) and eye
//! movement 0x80017e80.
use bevy::prelude::*;

use crate::world::{NavBox, WorldGeometry};

/// Floor/ceiling value the original uses for solid sectors.
const NO_HEIGHT: i32 = -32512;
/// Default chase distance (0x8001a23c resets 0x8008b294 every tick).
pub const CHASE_DISTANCE: i32 = 1536;

/// A point with the room the original tracks alongside it (GAME_VECTOR).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub room: usize,
}

impl Point {
    pub fn from_world(v: Vec3, room: usize) -> Self {
        let v = (v * 1024.0).round();
        Self { x: v.x as i32, y: -v.y as i32, z: v.z as i32, room }
    }
    pub fn world(self) -> Vec3 {
        Vec3::new(self.x as f32, -self.y as f32, self.z as f32) / 1024.0
    }
}

/// Fixed-point sine (0x80063f74): a quarter-wave table of
/// round(sin(i*pi/2048)*16384), verified equal to the RAM table at 0x80083ff8.
pub fn sin(angle: i32) -> i32 {
    let a = angle & 0xffff;
    let mut q = a & 0x7fff;
    if q >= 16384 {
        q = 0x8000 - q;
    }
    let value = ((q >> 4) as f64 * std::f64::consts::PI / 2048.0).sin() * 16384.0;
    let value = value.round() as i32;
    if a > 0x8000 { -value } else { value }
}

/// Fixed-point cosine (0x80063f70): sine of angle + 16384.
pub fn cos(angle: i32) -> i32 {
    sin(angle + 16384)
}

fn isqrt(value: i32) -> i32 {
    (value.max(0) as f64).sqrt() as i32
}

/// Level geometry as the camera reads it: `camera` mirrors flag 0x80087a80,
/// set for target/ideal/shift work and cleared when the eye moves.
#[derive(Clone, Copy)]
pub struct Space<'a> {
    pub world: &'a WorldGeometry,
    pub camera: bool,
}

/// GetFloor/GetHeight/GetCeiling for one point: the room the point resolves
/// to, its floor and ceiling (NO_HEIGHT for solid sectors).
fn sample(space: Space, x: i32, y: i32, z: i32, room: usize) -> (usize, i32, i32) {
    match space.world.floor_at_for(Vec3::new(x as f32, -y as f32, z as f32) / 1024.0, room, space.camera) {
        Some(hit) => (hit.room, (-hit.height * 1024.0).round() as i32, (-hit.ceiling * 1024.0).round() as i32),
        None => (room, NO_HEIGHT, NO_HEIGHT),
    }
}

fn blocked(floor: i32, ceiling: i32, y: i32) -> bool {
    floor < y || y < ceiling
}

/// One axis sweep across sector boundaries. Returns 1 when clear, -1 when
/// the point inside the current sector is solid (end clipped there), 0 when
/// the far side of a boundary is solid (end clipped at the boundary).
fn axis_los(world: Space, start: Point, end: &mut Point, along_z: bool) -> i32 {
    let (main0, main1) = if along_z { (start.z, end.z) } else { (start.x, end.x) };
    let delta = main1 - main0;
    if delta == 0 {
        return 1;
    }
    let (cross0, cross1) = if along_z { (start.x, end.x) } else { (start.z, end.z) };
    let step_cross = ((cross1 - cross0) << 10) / delta;
    let step_y = ((end.y - start.y) << 10) / delta;
    let point = |main: i32, cross: i32, y: i32| if along_z { (cross, y, main) } else { (main, y, cross) };
    let mut room = start.room;
    let forward = delta > 0;
    let mut main = if forward { main0 | 1023 } else { main0 & !1023 };
    let offset = main - main0;
    let mut cross = cross0 + (step_cross.wrapping_mul(offset) >> 10);
    let mut y = start.y + (step_y.wrapping_mul(offset) >> 10);
    while if forward { main < main1 } else { main > main1 } {
        let (x0, y0, z0) = point(main, cross, y);
        let (r, floor, ceiling) = sample(world, x0, y0, z0, room);
        room = r;
        if blocked(floor, ceiling, y) {
            *end = Point { x: x0, y: y0, z: z0, room };
            return -1;
        }
        let near_room = room;
        let next = if forward { main + 1 } else { main - 1 };
        let (x1, y1, z1) = point(next, cross, y);
        let (r, floor, ceiling) = sample(world, x1, y1, z1, room);
        room = r;
        if blocked(floor, ceiling, y) {
            *end = Point { x: x0, y: y0, z: z0, room: near_room };
            return 0;
        }
        if forward {
            main += 1024;
            cross += step_cross;
            y += step_y;
        } else {
            main -= 1024;
            cross -= step_cross;
            y -= step_y;
        }
    }
    end.room = room;
    1
}

/// ClipTarget 0x80020480: pull the end back to the floor or ceiling it
/// crosses along the line. False when clipped.
fn clip_target(world: Space, start: Point, end: &mut Point) -> bool {
    let (_, floor, ceiling) = sample(world, end.x, end.y, end.z, end.room);
    let (dx, dy, dz) = (end.x - start.x, end.y - start.y, end.z - start.z);
    let limit = if floor < end.y && start.y < floor {
        floor
    } else if end.y < ceiling && ceiling < start.y {
        ceiling
    } else {
        return true;
    };
    end.y = limit;
    end.x = start.x + dx.wrapping_mul(limit - start.y) / dy;
    end.z = start.z + dz.wrapping_mul(limit - start.y) / dy;
    false
}

/// LOS 0x8001fb54: true when clear; otherwise `end` is clipped to the hit.
pub fn line_of_sight(world: Space, start: Point, end: &mut Point) -> bool {
    let z_first = (end.x - start.x).abs() >= (end.z - start.z).abs();
    let first = axis_los(world, start, end, z_first);
    let second = axis_los(world, start, end, !z_first);
    if second == 0 {
        return false;
    }
    let (room, _, _) = sample(world, end.x, end.y, end.z, end.room);
    end.room = room;
    clip_target(world, start, end) && first == 1 && second == first
}

/// 0x80018768: the sector when the point is between its floor and ceiling.
fn open_box(world: Space, x: i32, y: i32, z: i32, room: usize) -> Option<Option<NavBox>> {
    let (room, floor, ceiling) = sample(world, x, y, z, room);
    if floor < y || y < ceiling {
        return None;
    }
    Some(world.world.navigation_box(room, x, z).and_then(|b| world.world.data.boxes.get(b).copied()))
}

/// Candidate callback 0x80018594: put the eye on the violated box edge at the
/// chase distance (`distance2` = horizontal distance squared) where possible.
#[allow(clippy::too_many_arguments)]
fn edge_candidate(
    main: &mut i32,
    cross: &mut i32,
    target_main: i32,
    target_cross: i32,
    limit: i32,
    cross_a: i32,
    other: i32,
    cross_b: i32,
    distance2: i32,
) {
    let a0 = (target_main - limit).pow(2);
    let a1 = (target_cross - cross_a).pow(2);
    let b1 = (target_cross - cross_b).pow(2);
    let o0 = (target_main - other).pow(2);
    let (corner_b, corner_a, opposite_a) = (a0 + b1, a0 + a1, o0 + a1);
    if distance2 < corner_a {
        *main = limit;
        if distance2 - a0 >= 0 {
            let r = isqrt(distance2 - a0);
            *cross = if cross_a < cross_b { target_cross - r } else { target_cross + r };
        }
    } else if corner_a > 0x1c639 {
        *main = limit;
        *cross = cross_a;
    } else if distance2 < corner_b {
        *main = limit;
        if distance2 - a0 >= 0 {
            let r = isqrt(distance2 - a0);
            *cross = if cross_a < cross_b { target_cross + r } else { target_cross - r };
        }
    } else if 2 * distance2 < opposite_a {
        if 2 * distance2 - a1 >= 0 {
            let r = isqrt(2 * distance2 - a1);
            *main = if limit < other { target_main + r } else { target_main - r };
            *cross = cross_a;
        }
    } else if opposite_a < corner_b {
        *main = limit;
        *cross = cross_b;
    } else {
        *main = other;
        *cross = cross_a;
    }
}

/// Smart shift 0x80018800: keep the ideal eye inside the open navigation
/// space around the target, sliding it along walls rather than through them.
pub fn shift_ideal(world: Space, target: Point, eye: Point, ideal: &mut Point, distance2: i32) {
    line_of_sight(world, target, ideal);
    let Some(target_box) = world.world.navigation_box(target.room, target.x, target.z).and_then(|b| world.world.data.boxes.get(b))
    else {
        return;
    };
    let bounds = |b: &NavBox| [b.z_min << 10, (b.z_max << 10) - 1, b.x_min << 10, (b.x_max << 10) - 1];
    let [mut zmin, mut zmax, mut xmin, mut xmax] = bounds(target_box);
    let outside = ideal.z < zmin || ideal.z > zmax || ideal.x < xmin || ideal.x > xmax;
    if outside && let Some(own) = world.world.navigation_box(ideal.room, ideal.x, ideal.z).and_then(|b| world.world.data.boxes.get(b)) {
        [zmin, zmax, xmin, xmax] = bounds(own);
    }
    // Extend through open neighbours; a solid neighbour fixes the edge.
    let (ix, iy, iz, room) = (ideal.x, ideal.y, ideal.z, ideal.room);
    let probe = (iz - 1024) | 1023;
    let z_low = open_box(world, ix, iy, probe, room);
    match z_low {
        Some(Some(b)) if b.z_min << 10 < zmin => zmin = b.z_min << 10,
        None => zmin = probe,
        _ => {}
    }
    let probe = (iz + 1024) & !1023;
    let z_high = open_box(world, ix, iy, probe, room);
    match z_high {
        Some(Some(b)) if (b.z_max << 10) - 1 > zmax => zmax = (b.z_max << 10) - 1,
        None => zmax = probe,
        _ => {}
    }
    let probe = (ix - 1024) | 1023;
    let x_low = open_box(world, probe, iy, iz, room);
    match x_low {
        Some(Some(b)) if b.x_min << 10 < xmin => xmin = b.x_min << 10,
        None => xmin = probe,
        _ => {}
    }
    let probe = (ix + 1024) & !1023;
    let x_high = open_box(world, probe, iy, iz, room);
    match x_high {
        Some(Some(b)) if (b.x_max << 10) - 1 > xmax => xmax = (b.x_max << 10) - 1,
        None => xmax = probe,
        _ => {}
    }
    let (zmin, zmax, xmin, xmax) = (zmin + 256, zmax - 256, xmin + 256, xmax - 256);
    let (mut a, mut b) = (*ideal, *ideal);
    let mut prefer_a = None;
    let mut shift_z = |a: &mut Point, b: &mut Point, limit: i32, other: i32| {
        edge_candidate(&mut a.z, &mut a.x, target.z, target.x, limit, xmin, other, xmax, distance2);
        edge_candidate(&mut b.z, &mut b.x, target.z, target.x, limit, xmax, other, xmin, distance2);
    };
    let try_z = |a: &mut Point, b: &mut Point, shift: &mut dyn FnMut(&mut Point, &mut Point, i32, i32)| {
        if ideal.z < zmin && z_low.is_none() {
            shift(a, b, zmin, zmax);
            true
        } else if ideal.z > zmax && z_high.is_none() {
            shift(a, b, zmax, zmin);
            true
        } else {
            false
        }
    };
    let mut shift_x = |a: &mut Point, b: &mut Point, limit: i32, other: i32| {
        edge_candidate(&mut a.x, &mut a.z, target.x, target.z, limit, zmin, other, zmax, distance2);
        edge_candidate(&mut b.x, &mut b.z, target.x, target.z, limit, zmax, other, zmin, distance2);
    };
    let try_x = |a: &mut Point, b: &mut Point, shift: &mut dyn FnMut(&mut Point, &mut Point, i32, i32)| {
        if ideal.x < xmin && x_low.is_none() {
            shift(a, b, xmin, xmax);
            true
        } else if ideal.x > xmax && x_high.is_none() {
            shift(a, b, xmax, xmin);
            true
        } else {
            false
        }
    };
    if (ideal.x - target.x).abs() < (ideal.z - target.z).abs() {
        if try_z(&mut a, &mut b, &mut shift_z) {
            prefer_a = Some(eye.x < target.x);
        } else if try_x(&mut a, &mut b, &mut shift_x) {
            prefer_a = Some(ideal.z < target.z);
        }
    } else if try_x(&mut a, &mut b, &mut shift_x) {
        prefer_a = Some(eye.z < target.z);
    } else if try_z(&mut a, &mut b, &mut shift_z) {
        prefer_a = Some(ideal.x < target.x);
    }
    let Some(prefer_a) = prefer_a else { return };
    let chosen = if prefer_a {
        if line_of_sight(world, target, &mut a) { a } else { b }
    } else if line_of_sight(world, target, &mut b) {
        b
    } else {
        a
    };
    (ideal.x, ideal.y, ideal.z) = (chosen.x, chosen.y, chosen.z);
    ideal.room = sample(world, ideal.x, ideal.y, ideal.z, ideal.room).0;
}

/// Ideal chase eye 0x80019214: `distance` behind the target along the
/// actor's heading plus the requested angle, raised by the elevation.
pub fn ideal_eye(target: Point, eye_room: usize, yaw: i32, pitch: i32, distance: i32) -> (Point, i32) {
    let elevation = pitch.clamp(-15470, 15470);
    let horizontal = (distance * cos(elevation)) >> 14;
    let y = target.y + ((distance * sin(elevation)) >> 14);
    let angle = yaw as i16 as i32;
    let x = target.x - ((horizontal * sin(angle)) >> 14);
    let z = target.z - ((horizontal * cos(angle)) >> 14);
    (Point { x, y, z, room: eye_room }, horizontal * horizontal)
}

/// MoveCamera 0x80017e80: close 1/divisor of the gap, re-clamp a buried eye
/// by line of sight, then return the render offset that keeps the view 256
/// inside the floor and ceiling (the stored eye height is not changed).
pub fn move_eye(world: Space, target: Point, eye: &mut Point, ideal: Point, divisor: i32) -> i32 {
    let world = Space { camera: false, ..world };
    let divisor = divisor.max(1);
    eye.x += (ideal.x - eye.x) / divisor;
    eye.z += (ideal.z - eye.z) / divisor;
    eye.y += (ideal.y - eye.y) / divisor;
    eye.room = ideal.room;
    let (room, floor, _) = sample(world, eye.x, eye.y, eye.z, eye.room);
    eye.room = room;
    let mut low = floor - 256;
    if eye.y >= low && ideal.y >= low {
        line_of_sight(world, target, eye);
        let (room, floor, _) = sample(world, eye.x, eye.y, eye.z, eye.room);
        eye.room = room;
        low = floor - 256;
    }
    let (_, _, ceiling) = sample(world, eye.x, eye.y, eye.z, eye.room);
    let mut high = ceiling + 256;
    if low < high {
        high = (low + high) >> 1;
        low = high;
    }
    let offset = if eye.y > low {
        low - eye.y
    } else if eye.y < high {
        high - eye.y
    } else {
        0
    };
    eye.room = sample(world, eye.x, eye.y + offset, eye.z, eye.room).0;
    offset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_table_matches_quarter_wave_samples_from_ram() {
        // RAM 0x80083ff8: entries 0..4 and the peak.
        assert_eq!([sin(0), sin(16), sin(32), sin(48), sin(64)], [0, 25, 50, 75, 101]);
        assert_eq!(sin(16384), 16384);
        assert_eq!(sin(-16384), -16384);
        assert_eq!(cos(0), 16384);
    }
}
