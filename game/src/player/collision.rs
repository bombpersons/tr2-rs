//! GetCollisionInfo 0x8001B018: four floor/ceiling probes around a body and
//! the contact class and push-out shift they imply, in the original's
//! integer, y-down units. Callers convert at this boundary and then apply
//! their state's own response.
//!
//! Static-mesh collision (0x8001BA24) and the lava pit flag are not ported;
//! the slope flags are [`Slopes`].
use super::units;
use bevy::prelude::*;
use engine::{
    animation::AnimationState,
    camera::chase::{cos, heading, sin},
    world::WorldGeometry,
};

/// Per-tick body collision context. Object pushes can advance the accepted
/// origin before the state-specific floor response (PSX0x8001CC48..0x8001CC6C).
#[derive(Component, Default)]
pub(crate) struct CollisionFrame {
    pub origin: Vec3,
    pub(super) roll_heading: Option<f32>,
    pub(super) ground: Option<GroundMove>,
    pub(super) walk: Option<WalkMove>,
}

/// A ground move waiting for its state collision until object pushes have
/// run.
pub(super) struct GroundMove {
    /// Lara's position before the move.
    pub old: Vec3,
    /// `CollisionFrame::origin` before the pushes; a push advances it.
    pub origin: Vec3,
    /// The pose, goal and step flag before this tick's animation step, which
    /// collision may replace.
    pub pose: AnimationState,
    pub goal: usize,
    pub stepped: bool,
}

/// A walk step waiting for the walk collision (0x8003A62C) until object
/// pushes have run.
pub(super) struct WalkMove {
    /// Lara's position before the move.
    pub old: Vec3,
    /// `CollisionFrame::origin` before the pushes; a push advances it.
    pub origin: Vec3,
    /// The pose before this tick's animation step.
    pub previous: AnimationState,
}

pub(super) fn remember(mut players: Query<(&super::Player, &mut CollisionFrame)>) {
    for (player, mut frame) in &mut players {
        frame.origin = player.position;
        frame.roll_heading = None;
        frame.ground = None;
        frame.walk = None;
    }
}

/// A binary angle (65536 per turn) as radians in [0, 2pi).
pub(super) fn angle_radians(angle: i32) -> f32 {
    (angle & 0xffff) as f32 * std::f32::consts::TAU / 65536.0
}

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

pub(crate) struct CollisionInfo {
    pub mid: Probe,
    pub front: Probe,
    pub left: Probe,
    pub right: Probe,
    pub contact: Contact,
    /// Applied to the item by ShiftItem 0x8001C09C.
    pub shift: IVec3,
}

/// Item pushes validate the body along the push direction, including its two
/// leading corners. PSX0x8001CBC8..0x8001CC44 uses height762, radius100,
/// floor limits+32512/-384 and ceiling0, then rejects every contact class.
/// It restores the last accepted position rather than applying the wall shift.
pub(crate) fn item_push_is_clear(world: &WorldGeometry, candidate: Vec3, origin: Vec3, room: usize) -> bool {
    let at = original(candidate);
    let old = original(origin);
    let delta = at - old;
    let facing = heading(delta.x, delta.z);
    info(world, at, old, room, 762, facing, &Limits { radius: 100, bad_pos: 32512, bad_neg: -384, bad_ceiling: 0 })
        .contact
        == Contact::None
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
    info_with_slopes(world, at, old, room, height, facing, limits, Slopes::default())
}

/// The caller's slope flags (coll+0x6C bits 1 and 2): a probe that meets a
/// big slope (GetHeight's type 2, a tilt over 2) above her reads as a wall
/// (-32767), and one below her as a pit (512) (0x8001B310..0x8001B37C, the
/// same for each of front, left and right).
#[derive(Clone, Copy, Default)]
pub(super) struct Slopes {
    pub walls: bool,
    pub pits: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn info_with_slopes(
    world: &WorldGeometry,
    at: IVec3,
    old: IVec3,
    room: usize,
    height: i32,
    facing: i32,
    limits: &Limits,
    slopes: Slopes,
) -> CollisionInfo {
    let top = at.y - height;
    // Rooms are resolved 160 above the body; each probe starts from the room
    // the previous one resolved to.
    let mut room = room;
    let mut probe = |x: i32, z: i32, flagged: bool| {
        let point = Vec3::new(units(x as f32), units((160 - top) as f32), units(z as f32));
        match world.floor_at(point, room) {
            Some(hit) => {
                room = hit.room;
                let mut floor = (-hit.height * 1024.0).round() as i32 - at.y;
                if flagged && (slopes.walls || slopes.pits) {
                    let [tx, tz] = world.slope_at(point, hit.room);
                    if tx.unsigned_abs() > 2 || tz.unsigned_abs() > 2 {
                        if slopes.walls && floor < 0 {
                            floor = -32767;
                        } else if slopes.pits && floor > 0 {
                            floor = 512;
                        }
                    }
                }
                Probe { floor, ceiling: (-hit.ceiling * 1024.0).round() as i32 - top }
            }
            None => Probe { floor: NO_HEIGHT, ceiling: NO_HEIGHT },
        }
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
    let mid = probe(at.x, at.z, false);
    let front = probe(at.x + front_at[0], at.z + front_at[1], true);
    let left = probe(at.x + left_at[0], at.z + left_at[1], true);
    let right = probe(at.x + right_at[0], at.z + right_at[1], true);
    let mut shift = IVec3::ZERO;
    let undo = old - at;
    let done = |contact, shift| CollisionInfo { mid, front, left, right, contact, shift };
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
