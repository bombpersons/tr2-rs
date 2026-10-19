//! GetCollisionInfo 0x8001B018: four floor/ceiling probes around a body and
//! the contact class and push-out shift they imply, in the original's
//! integer, y-down units. Callers convert at this boundary and then apply
//! their state's own response.
//!
//! Static-mesh collision (0x8001BA24) and the slope/lava pit flags are not
//! ported; the callers here clear those flags.
use super::units;
use bevy::prelude::*;
use engine::{
    camera::chase::{cos, sin},
    world::WorldGeometry,
};

/// GetHeight's sentinel for a solid or missing sector.
pub(super) const NO_HEIGHT: i32 = -32512;

/// Original integer coordinates (y down) of a level position.
pub(super) fn original(p: Vec3) -> IVec3 {
    IVec3::new((p.x * 1024.0).round() as i32, (-p.y * 1024.0).round() as i32, (p.z * 1024.0).round() as i32)
}
pub(super) fn level(p: IVec3) -> Vec3 {
    Vec3::new(units(p.x as f32), units(-p.y as f32), units(p.z as f32))
}

/// Floor relative to the probe line, ceiling relative to the body's top.
#[derive(Clone, Copy, Debug)]
pub(super) struct Probe {
    pub floor: i32,
    pub ceiling: i32,
}

/// coll_type (coll+0x62).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Contact {
    None,
    Front,
    Left,
    Right,
    Top,
    TopFront,
    Clamp,
}

/// The caller's coll+0x30..0x3C.
pub(super) struct Limits {
    pub radius: i32,
    pub bad_pos: i32,
    pub bad_neg: i32,
    pub bad_ceiling: i32,
}

pub(super) struct CollisionInfo {
    pub mid: Probe,
    pub contact: Contact,
    /// Applied to the item by ShiftItem 0x8001C09C.
    pub shift: IVec3,
}

/// GetGridShift 0x8001B9EC: push `from` back into `to`'s sector, one unit inside.
fn grid_shift(from: i32, to: i32) -> i32 {
    match (from >> 10).cmp(&(to >> 10)) {
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Less => 1025 - (from & 1023),
        std::cmp::Ordering::Greater => -(from & 1023) - 1,
    }
}

/// `at` is the probe line (floors are relative to it); the body's top is
/// `height` above it. `old` is the position before this tick's movement.
pub(super) fn info(
    world: &WorldGeometry,
    at: IVec3,
    old: IVec3,
    room: usize,
    height: i32,
    facing: i32,
    limits: &Limits,
) -> CollisionInfo {
    let top = at.y - height;
    // Rooms are resolved 160 above the body; each probe starts from the room
    // the previous one resolved to.
    let mut room = room;
    let mut probe = |x: i32, z: i32| match world
        .floor_at(Vec3::new(units(x as f32), units((160 - top) as f32), units(z as f32)), room)
    {
        Some(hit) => {
            room = hit.room;
            Probe {
                floor: (-hit.height * 1024.0).round() as i32 - at.y,
                ceiling: (-hit.ceiling * 1024.0).round() as i32 - top,
            }
        }
        None => Probe { floor: NO_HEIGHT, ceiling: NO_HEIGHT },
    };
    let r = limits.radius;
    let quadrant = ((facing + 8192) & 0xffff) >> 14;
    let across = |trig: i32| (trig * r) >> 14;
    // Front, left and right offsets by cardinal quadrant; the front probe's
    // transverse offset follows the sine table.
    let [front_at, left_at, right_at] = match quadrant {
        0 => [[across(sin(facing)), r], [-r, r], [r, r]],
        1 => [[r, across(cos(facing))], [r, r], [r, -r]],
        2 => [[across(sin(facing)), -r], [r, -r], [-r, -r]],
        _ => [[-r, across(cos(facing))], [-r, -r], [-r, r]],
    };
    let mid = probe(at.x, at.z);
    let front = probe(at.x + front_at[0], at.z + front_at[1]);
    let left = probe(at.x + left_at[0], at.z + left_at[1]);
    let right = probe(at.x + right_at[0], at.z + right_at[1]);
    let mut shift = IVec3::ZERO;
    let undo = old - at;
    let done = |contact, shift| CollisionInfo { mid, contact, shift };
    if mid.floor == NO_HEIGHT {
        return done(Contact::Front, undo);
    }
    if mid.floor - mid.ceiling <= 0 {
        return done(Contact::Clamp, undo);
    }
    let mut contact = Contact::None;
    if mid.ceiling >= 0 {
        shift.y = mid.ceiling;
        contact = Contact::Top;
    }
    let bad = |p: &Probe| p.floor > limits.bad_pos || p.floor < limits.bad_neg;
    let along_x = quadrant % 2 == 0;
    if bad(&front) || front.ceiling > limits.bad_ceiling {
        if along_x {
            shift.x = undo.x;
            shift.z = grid_shift(at.z + front_at[1], at.z);
        } else {
            shift.x = grid_shift(at.x + front_at[0], at.x);
            shift.z = undo.z;
        }
        return done(Contact::Front, shift);
    }
    if front.ceiling >= limits.bad_ceiling {
        return done(Contact::TopFront, undo);
    }
    for (side, offset, kind) in [(left, left_at, Contact::Left), (right, right_at, Contact::Right)] {
        if bad(&side) {
            if along_x {
                shift.x = grid_shift(at.x + offset[0], at.x + front_at[0]);
            } else {
                shift.z = grid_shift(at.z + offset[1], at.z + front_at[1]);
            }
            return done(kind, shift);
        }
    }
    done(contact, shift)
}
