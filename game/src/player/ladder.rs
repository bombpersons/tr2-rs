//! Climbable walls: the climb flag TestTriggers keeps, the catch from a
//! vertical jump, hanging on a climbable wall and the climbing states
//! (56 to 61). All of it runs in the original's integer units (y down)
//! and in its order: the state's control, AnimateLara, then the state's
//! collision (docs/assault-course-evidence.md).
use super::*;
use collision::{Contact, Limits, NO_HEIGHT, level, original};
use engine::{
    animation::vertical_bounds,
    camera::chase::{cos, sin},
    world::Command,
};

/// Lara's collision radius (coll+0x30).
const RADIUS: i32 = 100;

/// Input bits the climbing handlers test: Left and step left, Right and
/// step right (0x404, 0x808 of the input word).
fn left(pad: &ControllerState) -> bool {
    pad.held(Button::Left)
}
fn right(pad: &ControllerState) -> bool {
    pad.held(Button::Right)
}

fn quadrant(yaw: i32) -> i32 {
    ((yaw + 0x2000) & 0xffff) >> 14
}

fn yaw(player: &Player) -> i32 {
    i32::from(binary_angle(player.yaw))
}

/// TestTriggers 0x8001EE84..0x8001EF5C, for Lara: the climb entry of her
/// sector's floor data holds one bit per wall direction, and she can climb
/// when the bit for the way she faces is set. Kept at the end of each tick
/// and read by the next tick's collision.
pub(super) fn climbable(world: &WorldGeometry, position: Vec3, room: usize, yaw: f32) -> bool {
    let q = quadrant(i32::from(binary_angle(yaw)));
    world.trigger_floor(position, room).and_then(|floor| floor.climb).is_some_and(|bits| bits >> q & 1 != 0)
}

/// Keep the climb flag at the end of the tick.
pub(super) fn update_climb(world: Option<Res<WorldGeometry>>, mut players: Query<(&Player, &mut Traversal)>) {
    let Some(world) = world else { return };
    for (player, mut traversal) in &mut players {
        traversal.climb = climbable(&world, player.position, player.room, player.yaw);
    }
}

/// GetFloor 0x8001E41C at (x, y, z), then GetHeight 0x8001E944 over the
/// sector found; `room` follows GetFloor.
fn height(world: &WorldGeometry, x: i32, y: i32, z: i32, room: &mut usize) -> i32 {
    if let Some(found) = world.floor_room(x, y, z, *room) {
        *room = found;
    }
    world.floor_height(x, y, z, *room).unwrap_or(NO_HEIGHT)
}

/// GetFloor at (x, y, z), then GetCeiling 0x8001F740 over the sector found.
fn ceiling(world: &WorldGeometry, x: i32, y: i32, z: i32, room: &mut usize) -> i32 {
    if let Some(found) = world.floor_room(x, y, z, *room) {
        *room = found;
    }
    world.ceiling_height(x, y, z, *room).unwrap_or(NO_HEIGHT)
}

/// The quadrant's probe point `front` ahead and `right` to the side of
/// `at`, and one step (2 units) on into the wall.
fn probe(at: IVec3, yaw: i32, front: i32, right: i32) -> (i32, i32, i32, i32) {
    match quadrant(yaw) {
        0 => (at.x + right, at.z + front, 0, 2),
        1 => (at.x + front, at.z - right, 2, 0),
        2 => (at.x - right, at.z - front, 0, -2),
        _ => (at.x - front, at.z + right, -2, 0),
    }
}

/// LaraTestClimb 0x8003F480: can Lara's hands (at `y`, her body `height`
/// below them) hold the wall at (x, z)? 1 yes, 0 no, -1 only hanging (the
/// wall stops above her feet). The shift is how far she must move up or
/// down to hold on.
#[allow(clippy::too_many_arguments)]
fn test_climb(
    world: &WorldGeometry,
    x: i32,
    y: i32,
    z: i32,
    step_x: i32,
    step_z: i32,
    height_below: i32,
    room: usize,
) -> (i32, i32) {
    let mut shift = 0;
    let mut found = room;
    let floor = self::height(world, x, y - 128, z, &mut found);
    if floor == NO_HEIGHT {
        return (0, 0);
    }
    let below = floor - 128 - (y + height_below);
    if below < -70 {
        return (0, 0);
    }
    if below < 0 {
        shift = below;
    }
    let above = ceiling(world, x, y - 128, z, &mut found.clone()) - y;
    if above > 70 {
        return (0, shift);
    }
    if above > 0 {
        if shift != 0 {
            return (0, shift);
        }
        shift = above;
    }
    let hang = i32::from(height_below + below > 899);
    let (x2, z2) = (x + step_x, z + step_z);
    let mut ahead_room = found;
    let mut ahead = self::height(world, x2, y, z2, &mut ahead_room);
    if ahead != NO_HEIGHT {
        ahead -= y;
    }
    let wall_top = if ahead < 71 {
        if ahead > 0 {
            if shift < 0 {
                return (0, shift);
            }
            shift = shift.max(ahead);
        }
        let feet = y + height_below;
        let mut low = room;
        let _ = self::height(world, x, feet, z, &mut low);
        let top = ceiling(world, x2, feet, z2, &mut low);
        if top == NO_HEIGHT {
            return (1, shift);
        }
        let top = top - y;
        if top <= ahead || top >= 512 {
            return (1, shift);
        }
        if top < 443 {
            return (-hang, shift);
        }
        top
    } else {
        let top = ceiling(world, x2, y, z2, &mut ahead_room) - y;
        if top >= 512 {
            return (1, shift);
        }
        if top < 443 {
            if top > 0 {
                return (-hang, shift);
            }
            if top < -69 || hang == 0 || shift > 0 {
                return (0, shift);
            }
            return (-1, shift.min(top));
        }
        top
    };
    if shift < 1 { (1, wall_top - 512) } else { (-hang, shift) }
}

/// LaraTestClimbPos 0x8003F764: [`test_climb`] `front` ahead of Lara and
/// `right` to her side, her hands `origin` from her position.
#[allow(clippy::too_many_arguments)]
fn test_climb_pos(
    world: &WorldGeometry,
    at: IVec3,
    yaw: i32,
    room: usize,
    front: i32,
    right: i32,
    origin: i32,
    height: i32,
) -> (i32, i32) {
    let (x, z, step_x, step_z) = probe(at, yaw, front, right);
    test_climb(world, x, at.y + origin, z, step_x, step_z, height, room)
}

/// LaraTestClimbUpPos 0x8003F994: can Lara climb a step (256) higher? 1
/// yes, 0 no, -1 the wall ends: she can climb off it at the ledge.
/// Returns that and the shift and the ledge's height.
fn test_climb_up_pos(
    world: &WorldGeometry,
    at: IVec3,
    yaw: i32,
    room: usize,
    front: i32,
    right: i32,
) -> (i32, i32, i32) {
    let y = at.y - 768;
    let (x, z, step_x, step_z) = probe(at, yaw, front, right);
    let mut shift = 0;
    let mut found = room;
    let above = ceiling(world, x, y, z, &mut found) + 256 - y;
    if above >= 71 {
        return (0, shift, 0);
    }
    if above > 0 {
        shift = above;
    }
    let (x2, z2) = (x + step_x, z + step_z);
    let ledge = self::height(world, x2, y, z2, &mut found);
    if ledge == NO_HEIGHT {
        return (1, shift, NO_HEIGHT);
    }
    let ledge = ledge - y;
    if ledge < 129 {
        if ledge > 0 && shift < ledge {
            shift = ledge;
        }
        let hands = at.y - 256;
        let mut low = room;
        let _ = self::height(world, x, hands, z, &mut low);
        let top = ceiling(world, x2, hands, z2, &mut low) - y;
        if ledge < top {
            return (i32::from(top >= 512), shift, ledge);
        }
        (1, shift, ledge)
    } else {
        let top = ceiling(world, x2, y, z2, &mut found) - y;
        if top < 512 {
            if ledge - top < 763 {
                return (0, shift, ledge);
            }
            return (-1, ledge, ledge);
        }
        (1, shift, ledge)
    }
}

/// 0x8003CC74: from a hang, can Lara put her feet on the climbable wall?
/// Both hands' sides must hold (700 above her, 512 tall); she moves by
/// their shift.
fn hang_to_wall(world: &WorldGeometry, at: &mut IVec3, yaw: i32, room: usize) -> bool {
    let (right, right_shift) = test_climb_pos(world, *at, yaw, room, RADIUS, RADIUS + 120, -700, 512);
    if right != 1 {
        return false;
    }
    let (left, left_shift) = test_climb_pos(world, *at, yaw, room, RADIUS, -(RADIUS + 80), -700, 512);
    if left != 1 {
        return false;
    }
    let shift = match (right_shift, left_shift) {
        (0, 0) => return true,
        (0, left) => left,
        (right, 0) => right,
        (right, left) if (right < 0) != (left < 0) => return false,
        // 0x8003CD00..0x8003CD34: the left's shift wins when it goes further.
        (right, left) if (right < 0 && left < right) || (right > 0 && right < left) => left,
        (right, _) => right,
    };
    at.y += shift;
    true
}

/// LaraTestHangOnClimbWall 0x8003CAD8: with the climb flag set and Lara not
/// rising, she steps onto the wall by the collision's shift along her
/// facing axis, then her hands (her clip's top bound) must hold it at her
/// two sides and the middle; the middle's shift moves her unless it holds
/// outright. A failed hold keeps the step: the caller's ShiftItem then
/// adds the shift again.
#[allow(clippy::too_many_arguments)]
fn hang_on_wall(
    world: &WorldGeometry,
    at: &mut IVec3,
    yaw: i32,
    room: usize,
    shift: IVec3,
    fall_speed: i32,
    animation: &AnimationState,
    climb: bool,
) -> bool {
    if !climb || fall_speed < 0 {
        return false;
    }
    match quadrant(yaw) {
        0 | 2 => at.z += shift.z,
        _ => at.x += shift.x,
    }
    let (top, bottom) = vertical_bounds(&world.data, animation).unwrap_or((0, 0));
    let hold = |at: IVec3, right: i32| test_climb_pos(world, at, yaw, room, RADIUS, right, top, bottom - top);
    if hold(*at, RADIUS).0 == 0 || hold(*at, -RADIUS).0 == 0 {
        return false;
    }
    let (middle, shift) = hold(*at, 0);
    if middle == 0 {
        return false;
    }
    if middle != 1 {
        at.y += shift;
    }
    true
}

/// LaraCheckForLetGo 0x8003F394: on the wall Lara has no fall speed; let
/// go of Action (or die) and she drops (clip 34, speed 2, fall 1).
fn let_go(
    player: &mut Player,
    motion: &mut Motion,
    weapon: &mut WeaponState,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    pad: &ControllerState,
) -> bool {
    motion.velocity = Vec3::ZERO;
    motion.grounded = true;
    if pad.held(Button::Cross) && player.health >= 1 {
        return false;
    }
    traversal.goal = state::FORWARD_JUMP;
    animation.enter(clip::FALL, 0.0, clip::FALL);
    let heading = motion.movement_heading.unwrap_or(player.yaw);
    motion.velocity =
        Vec3::new(heading.sin(), 0.0, heading.cos()) * units(2.0 * ORIGINAL_FPS) - Vec3::Y * units(ORIGINAL_FPS);
    motion.grounded = false;
    player.mode = PlayerMode::JumpingForward;
    weapon.status = GunStatus::HandsFree;
    true
}

/// AnimateLara 0x80042E08 for one frame: the clip advances toward the goal,
/// end commands move Lara, and she moves at the clip's speed along the
/// movement heading.
fn animate(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &Traversal,
    world: &WorldGeometry,
    dt: f32,
) {
    let mut a = ObjectAnimation { animation: animation.animation, frame: animation.frame };
    let mut goal = AnimationGoal { state: traversal.goal };
    let cues = advance_authored_animation(&world.data, &mut a, &mut goal, original_frames(dt));
    *animation = AnimationState::at_frame(a.animation, a.frame);
    for cue in cues {
        if let Command::SetPosition(offset) = cue.command
            && cue.at_end
        {
            traversal::apply_offset(player, offset);
        }
    }
    let Some(clip) = world.data.animations.get(animation.animation) else { return };
    let heading = motion.movement_heading.unwrap_or(player.yaw);
    let mut at = original(player.position);
    let speed = if motion.grounded {
        ((i64::from(clip.speed) + i64::from(clip.accel) * animation.frame.floor() as i64) >> 16) as i32
    } else {
        // With gravity on (a reach's catch), speed and fall carry on for
        // this one pass: fall gains 6 below 128.
        let mut fall = (-motion.velocity.y * 1024.0 / ORIGINAL_FPS).round() as i32;
        fall += if fall < 128 { 6 } else { 1 };
        at.y += fall;
        motion.velocity.y = -units(fall as f32 * ORIGINAL_FPS);
        (motion.velocity.with_y(0.0).length() * 1024.0 / ORIGINAL_FPS).round() as i32
    };
    let heading = i32::from(binary_angle(heading));
    at.x += (sin(heading) * speed) >> 14;
    at.z += (cos(heading) * speed) >> 14;
    player.position = level(at);
}

/// The vertical jump's catch of a climbable wall (0x8003D3C0 with
/// LaraTestEdgeCatch 0x8003D2AC returning -1): the jump's collision met a
/// wall ahead with Action held, hands free and room overhead, and Lara's
/// hands crossed a click line (a multiple of 256) this tick. She hangs
/// (clip 96 from its 12th frame) with her hands on that line.
#[allow(clippy::too_many_arguments)]
pub(super) fn catch(
    player: &mut Player,
    motion: &mut Motion,
    weapon: &WeaponState,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    pad: &ControllerState,
    old: Vec3,
    proposed: Vec3,
    reach: bool,
) -> Catch {
    let speed = (motion.velocity.with_y(0.0).dot(player.forward()) * 1024.0 / ORIGINAL_FPS).round() as i32;
    let fall = (-motion.velocity.y * 1024.0 / ORIGINAL_FPS).round() as i32;
    let yaw = yaw(player);
    let mut at = original(proposed);
    // Jump-up collision 0x8003B7B8 probes 870 tall, behind her when she
    // moves back; reach collision 0x8003B18C is the body's 762, with no
    // step down.
    let (facing, height, bad_neg) =
        if reach { (yaw, 762, 0) } else { (if speed < 0 { yaw + 0x8000 } else { yaw }, 870, -384) };
    let limits = Limits { radius: RADIUS, bad_pos: 0x7f00, bad_neg, bad_ceiling: 192 };
    let info = collision::info(world, at, original(old), player.room, height, facing as i16 as i32, &limits);
    let missed = Catch::Missed(IVec3::ZERO);
    if info.contact != Contact::Front || !pad.held(Button::Cross) || !weapon.hands_free() || info.mid.ceiling > -384 {
        return missed;
    }
    // A reach needs 200 below her.
    if reach && info.mid.floor < 200 {
        return missed;
    }
    let (top, _) = vertical_bounds(&world.data, animation).unwrap_or((0, 0));
    let gap = info.front.floor - top;
    if (gap >= 0 || gap + fall >= 0) && (gap < 1 || gap + fall < 1) {
        // A ledge: the edge catch, not this one.
        return missed;
    }
    let hands = at.y + top;
    if hands >> 8 == (hands + fall) >> 8 {
        return missed;
    }
    let line = (if fall > 0 { hands + fall } else { hands }) & !0xff;
    let before = at;
    if !hang_on_wall(world, &mut at, yaw, player.room, info.shift, fall, animation, traversal.climb) {
        return Catch::Missed(at - before);
    }
    let snapped = match (yaw & 0xffff) as u16 {
        a if a.wrapping_add(0x18e2) < 0x31c5 => 0,
        a if a.wrapping_add(0xd8e2) < 0x31c5 => 0x4000,
        a if a.wrapping_add(0x671c) >= 0xce39 => 0x8000,
        a if a.wrapping_add(0x58e2) <= 0x31c4 => 0xc000,
        _ => return Catch::Missed(at - before),
    };
    traversal.goal = state::HANG;
    if reach {
        // Reach catch 0x8003D5BC: swinging in (clip 150) where the floor 256
        // ahead is below her hands and the ceiling over 400 above
        // (0x8003D808), else clip 96; she keeps speed 2 and fall 1.
        let swing = swing_in(world, at, snapped, player.room);
        *animation = AnimationState::at_frame(if swing { clip::HANG_SWING } else { clip::HANG }, 0.0);
        let (top, _) = vertical_bounds(&world.data, animation).unwrap_or((0, 0));
        at.y = line - top;
        // Only her yaw snaps: her move angle stays the reach's, along
        // which the next pass moves that speed 2 (home_lap2).
        let heading = motion.movement_heading.unwrap_or(player.yaw);
        player.yaw = collision::angle_radians(snapped);
        let along = Vec3::new(heading.sin(), 0.0, heading.cos());
        motion.velocity = along * units(2.0 * ORIGINAL_FPS) - Vec3::Y * units(ORIGINAL_FPS);
        motion.grounded = false;
    } else {
        *animation = AnimationState::at_frame(clip::HANG, 12.0);
        let (top, _) = vertical_bounds(&world.data, animation).unwrap_or((0, 0));
        at.y = line - top;
        at.x += info.shift.x;
        at.z += info.shift.z;
        player.yaw = collision::angle_radians(snapped);
        motion.velocity = Vec3::ZERO;
        motion.grounded = true;
        motion.movement_heading = Some(player.yaw);
    }
    player.position = level(at);
    motion.turn_rate = 0;
    // The catch also makes the hands busy (gun status 1); `step` marks it.
    player.mode = PlayerMode::Ladder;
    Catch::Hung
}

/// What a jump's climbable-wall catch did.
pub(super) enum Catch {
    Hung,
    /// No catch; LaraTestHangOnClimbWall may have left her stepped onto the
    /// wall by this much (original units).
    Missed(IVec3),
}

/// TestLaraVault 0x8003D924's climbable-wall branch, for a collision that
/// met a wall ahead with Action held and hands free, Lara facing within
/// 5460 of a wall's axis. Where the front is no vault (not 384 to 896
/// above her) and no ledge to jump to (not 896 to 1920, sides even), and
/// the climb flag is set: wading, or with the wall rising past her reach,
/// she climbs onto it from where she stands (stand clip 11, goal 56)
/// when her hands can hold it (0x8003CC74). Returns whether she did.
#[allow(clippy::too_many_arguments)]
pub(super) fn vault_onto_wall(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    pad: &ControllerState,
    hands_free: bool,
    info: &collision::CollisionInfo,
    at: IVec3,
    wading: bool,
    dt: f32,
) -> bool {
    if info.contact != Contact::Front || !pad.held(Button::Cross) || !hands_free || !traversal.climb {
        return false;
    }
    let yaw = yaw(player);
    let snapped = match (yaw & 0xffff) as u16 {
        a if a.wrapping_add(0x1554) < 0x2aa9 => 0,
        a if a.wrapping_add(0xd554) < 0x2aa9 => 0x4000,
        a if a.wrapping_add(0x6aaa) > 0xd554 => 0x8000,
        a if a.wrapping_add(0x5554) < 0x2aa9 => 0xc000,
        _ => return false,
    };
    let front = info.front.floor;
    let uneven = (info.left.floor - info.right.floor).abs() > 0x3b;
    if (-640..=-384).contains(&front) || (-896..=-640).contains(&front) {
        return false;
    }
    if !uneven && (-1920..=-896).contains(&front) {
        return false;
    }
    let reachable =
        front > -1920 || wading || info.left.floor > -1920 || info.right.floor > -2048 || info.mid.ceiling > -1158;
    if !reachable {
        // The wall rises past her reach: she jumps up to it (stand clip
        // 11 toward goal 28, launched at 116), then shifts off the wall.
        player.position = level(at);
        *animation = AnimationState::at_frame(clip::STAND, 0.0);
        traversal.goal = state::JUMP_UP;
        animate(player, motion, animation, traversal, world, dt);
        player.yaw = collision::angle_radians(snapped);
        player.position = level(original(player.position) + info.shift);
        player.mode = PlayerMode::JumpingUp { launch_speed: units(116.0) * ORIGINAL_FPS };
        motion.velocity = Vec3::ZERO;
        return true;
    }
    if (front > -1025 && info.front.ceiling < 506) || info.mid.ceiling > -518 {
        return false;
    }
    let mut at = at + info.shift;
    if !hang_to_wall(world, &mut at, yaw, player.room) {
        return false;
    }
    player.position = level(at);
    *animation = AnimationState::at_frame(clip::STAND, 0.0);
    traversal.goal = state::CLIMB_STANCE;
    animate(player, motion, animation, traversal, world, dt);
    player.yaw = collision::angle_radians(snapped);
    player.mode = PlayerMode::Ladder;
    true
}

/// 0x8003D808: is there a drop 256 ahead (at Lara's height) with the ceiling
/// over 400 above?
fn swing_in(world: &WorldGeometry, at: IVec3, facing: i32, room: usize) -> bool {
    let (x, z) = match facing {
        0 => (at.x, at.z + 256),
        0x4000 => (at.x + 256, at.z),
        0x8000 => (at.x, at.z - 256),
        _ => (at.x - 256, at.z),
    };
    let mut found = room;
    let floor = self::height(world, x, at.y, z, &mut found);
    let top = ceiling(world, x, at.y, z, &mut found.clone());
    floor != NO_HEIGHT && floor - at.y > 0 && top - at.y < -400
}

/// One tick on a climbable wall (or hanging from one): the state's control,
/// AnimateLara, then the state's collision.
#[allow(clippy::too_many_arguments)]
pub(super) fn step(
    player: &mut Player,
    motion: &mut Motion,
    weapon: &mut WeaponState,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    clips: &PlayerAnimations,
    pad: &ControllerState,
    dt: f32,
) -> bool {
    if player.mode != PlayerMode::Ladder {
        return false;
    }
    traversal.animation_stepped = true;
    if weapon.status == GunStatus::HandsFree {
        weapon.status = GunStatus::HandsBusy;
    }
    let old = original(player.position);
    let old_pose = animation.clone();
    let yaw = yaw(player);
    // Control.
    match clips.state(animation.animation) {
        Some(state::HANG) => {
            // 0x80038FD8: Left or Right shimmies.
            motion.camera_angle = 0;
            motion.camera_elevation = Some(swimming::radians(-10920));
            if left(pad) {
                traversal.goal = state::HANG_LEFT;
            } else if right(pad) {
                traversal.goal = state::HANG_RIGHT;
            }
        }
        Some(state::HANG_LEFT) => {
            // 0x80039770: let go of Left and she stops.
            motion.camera_angle = 0;
            motion.camera_elevation = Some(swimming::radians(-10920));
            if !left(pad) {
                traversal.goal = state::HANG;
            }
        }
        Some(state::HANG_RIGHT) => {
            // 0x800397BC.
            motion.camera_angle = 0;
            motion.camera_elevation = Some(swimming::radians(-10920));
            if !right(pad) {
                traversal.goal = state::HANG;
            }
        }
        Some(state::CLIMB_STANCE) => {
            // 0x8003E9F0.
            motion.camera_elevation = Some(swimming::radians(-3640));
            if left(pad) {
                traversal.goal = state::CLIMB_LEFT;
            } else if right(pad) {
                traversal.goal = state::CLIMB_RIGHT;
            } else if pad.held(Button::Square) {
                traversal.goal = state::JUMP_BACK;
                weapon.status = GunStatus::HandsFree;
                motion.movement_heading = Some(player.yaw + std::f32::consts::PI);
            }
        }
        Some(state::CLIMB_LEFT) => {
            // 0x8003E950.
            motion.camera_angle = -5460;
            motion.camera_elevation = Some(swimming::radians(-2730));
            if !left(pad) {
                traversal.goal = state::CLIMB_STANCE;
            }
        }
        Some(state::CLIMB_RIGHT) => {
            // 0x8003E9A0.
            motion.camera_angle = 5460;
            motion.camera_elevation = Some(swimming::radians(-2730));
            if !right(pad) {
                traversal.goal = state::CLIMB_STANCE;
            }
        }
        Some(state::CLIMBING) => motion.camera_elevation = Some(swimming::radians(5460)),
        Some(state::CLIMB_DOWN) => motion.camera_elevation = Some(swimming::radians(-8190)),
        _ => {}
    }
    animate(player, motion, animation, traversal, world, dt);
    // Collision.
    let state = clips.state(animation.animation).unwrap_or(state::STAND);
    let mut at = original(player.position);
    match state {
        state::HANG => hang(player, motion, weapon, animation, traversal, world, pad, old),
        state::HANG_LEFT | state::HANG_RIGHT => {
            // Shimmy collision 0x8003BA74 / 0x8003BAC8: the hang test, the
            // move sideways along the wall.
            let side = if state == state::HANG_RIGHT { 0x4000 } else { -0x4000 };
            hang_test(player, motion, weapon, animation, traversal, world, pad, old);
            if player.mode == PlayerMode::Ladder {
                motion.movement_heading = Some(collision::angle_radians(yaw + side));
            }
        }
        state::CLIMB_STANCE => {
            if !let_go(player, motion, weapon, animation, traversal, pad) {
                stance(&mut at, yaw, player.room, animation, traversal, world, pad);
                player.position = level(at);
            }
        }
        state::CLIMBING => {
            if !let_go(player, motion, weapon, animation, traversal, pad) {
                climb_up(player, motion, animation, traversal, world, pad, dt);
            }
        }
        state::CLIMB_DOWN => {
            if !let_go(player, motion, weapon, animation, traversal, pad) {
                climb_down(player, motion, animation, traversal, world, pad, dt);
            }
        }
        state::CLIMB_LEFT | state::CLIMB_RIGHT => {
            if !let_go(player, motion, weapon, animation, traversal, pad) {
                // 0x8003EB40 / 0x8003EBC8.
                let to_right = state == state::CLIMB_RIGHT;
                motion.movement_heading = Some(collision::angle_radians(yaw + if to_right { 0x4000 } else { -0x4000 }));
                let side = if to_right { RADIUS + 120 } else { -(RADIUS + 80) };
                let (result, shift) = test_climb_pos(world, at, yaw, player.room, RADIUS, side, -512, 512);
                climb_sideways(
                    player,
                    motion,
                    animation,
                    traversal,
                    world,
                    pad,
                    old,
                    &old_pose,
                    clips.state(old_pose.animation),
                    result,
                    shift,
                    dt,
                );
            }
        }
        state::PULL_UP => {
            // Climbing off the top: the ordinary pull-up finishes it.
            player.mode = PlayerMode::PullingUp;
        }
        state::JUMP_BACK => {
            // Push-off clip182 holds her on the wall without gravity until
            // its end command launches clip183 (speed50, fall-94), as the
            // ground take-offs do.
            player.mode = PlayerMode::FallingBack;
            motion.velocity = Vec3::ZERO;
            motion.grounded = true;
        }
        _ => {}
    }
    if player.mode == PlayerMode::Ladder {
        let at = original(player.position);
        player.room = world.floor_room(at.x, at.y - 381, at.z, player.room).unwrap_or(player.room);
    }
    true
}

/// HangTest 0x8003CDE4 on a climbable wall: the probe two units into the
/// wall, letting go, and holding on where the wall still holds her (else
/// back where she was). Returns the probe while she still hangs.
#[allow(clippy::too_many_arguments)]
fn hang_test(
    player: &mut Player,
    motion: &mut Motion,
    weapon: &mut WeaponState,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    pad: &ControllerState,
    old: IVec3,
) -> Option<collision::CollisionInfo> {
    let yaw = yaw(player);
    let room = player.room;
    motion.velocity = Vec3::ZERO;
    motion.grounded = true;
    motion.movement_heading = Some(player.yaw);
    let mut at = original(player.position);
    // Nudge two units into the wall, then probe.
    match quadrant(yaw) {
        0 => at.z += 2,
        1 => at.x += 2,
        2 => at.z -= 2,
        _ => at.x -= 2,
    }
    let limits = Limits { radius: RADIUS, bad_pos: 0x7f00, bad_neg: -384, bad_ceiling: 0 };
    let info = collision::info(world, at, old, room, 762, yaw, &limits);
    let hang_end = clip_start(world, clip::HANG) + 21;
    let at_end = animation.animation == clip::HANG && absolute(world, animation) == hang_end;
    if !traversal.climb {
        // Off the climbable wall: hang from its edge as from any ledge.
        player.position = level(at);
        let (top, _) = vertical_bounds(&world.data, animation).unwrap_or((-726, 0));
        player.mode = PlayerMode::Hanging { ledge: player.position - Vec3::Y * units(top as f32) };
        return None;
    }
    if !pad.held(Button::Cross) || player.health < 1 {
        // Let go: drop from the hands (0x8003CF2C..0x8003CFD8), along the
        // wall from where her hips are (0x8003CF40..0x8003CF80).
        let hips = hips(world, animation, yaw);
        if quadrant(yaw) % 2 == 0 {
            at.x += hips.x;
        } else {
            at.z += hips.z;
        }
        player.position = level(at + IVec3::Y * 256);
        traversal.goal = state::FORWARD_JUMP;
        animation.enter(clip::FALL, 0.0, clip::FALL);
        motion.velocity = player.forward() * units(2.0 * ORIGINAL_FPS) - Vec3::Y * units(ORIGINAL_FPS);
        motion.grounded = false;
        player.mode = PlayerMode::JumpingForward;
        weapon.status = GunStatus::HandsFree;
        return None;
    }
    // HangTest's climbable branch (0x8003D0F8..0x8003D18C).
    if hang_on_wall(world, &mut at, yaw, room, info.shift, 0, animation, true) {
        if at_end && hang_to_wall(world, &mut at, yaw, room) {
            traversal.goal = state::CLIMB_STANCE;
        }
    } else {
        at = old;
        traversal.goal = state::HANG;
        *animation = AnimationState::at_frame(clip::HANG, 21.0);
    }
    player.position = level(at);
    Some(info)
}

/// Hang collision 0x8003AFCC: HangTest, then from the hang's last frame Up
/// pulls up onto a ledge ahead or climbs the wall, Down lets her down a step.
#[allow(clippy::too_many_arguments)]
fn hang(
    player: &mut Player,
    motion: &mut Motion,
    weapon: &mut WeaponState,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    pad: &ControllerState,
    old: IVec3,
) {
    let Some(info) = hang_test(player, motion, weapon, animation, traversal, world, pad, old) else { return };
    if traversal.goal != state::HANG {
        return;
    }
    let hang_end = clip_start(world, clip::HANG) + 21;
    let at_end = animation.animation == clip::HANG && absolute(world, animation) == hang_end;
    if pad.held(Button::Up) {
        let [front, left, right] = [info.front, info.left, info.right];
        if (-849..=-651).contains(&front.floor)
            && front.floor >= front.ceiling
            && left.floor >= left.ceiling
            && right.floor >= right.ceiling
        {
            traversal.goal = if pad.held(Button::R1) { state::HANDSTAND } else { state::PULL_UP };
        } else if at_end && info.mid.ceiling < -255 {
            *animation = AnimationState::at_frame(clip::HANG_CLIMB_UP, 0.0);
        }
    } else if pad.held(Button::Down) && at_end {
        *animation = AnimationState::at_frame(clip::HANG_CLIMB_DOWN, 0.0);
    }
}

fn clip_start(world: &WorldGeometry, clip: usize) -> usize {
    world.data.animations[clip].frame_start
}

/// GetJointAbsPosition 0x8005BE3C for joint 0 with no offset, less Lara's
/// position: her hips, the root offset of the stored pose nearest the clip's
/// time, turned by her (snapped) yaw.
fn hips(world: &WorldGeometry, animation: &AnimationState, yaw: i32) -> IVec3 {
    let clip = &world.data.animations[animation.animation];
    let rate = clip.frame_rate.max(1);
    let offset = animation.frame.floor() as usize;
    let index = (offset / rate + usize::from(offset % rate > rate / 2)).min(clip.poses.len().saturating_sub(1));
    let Some(pose) = clip.poses.get(index) else { return IVec3::ZERO };
    let [x, y, z] = pose.root.map(i32::from);
    match quadrant(yaw) {
        0 => IVec3::new(x, y, z),
        1 => IVec3::new(z, y, -x),
        2 => IVec3::new(-x, y, -z),
        _ => IVec3::new(-z, y, x),
    }
}

fn absolute(world: &WorldGeometry, animation: &AnimationState) -> usize {
    clip_start(world, animation.animation) + animation.frame.floor() as usize
}

/// Stance collision 0x8003EC4C: Up climbs a step if both sides can (or
/// climbs off the top at a ledge they agree on), Down steps down if both
/// sides hold lower, or hangs when only the hands would.
fn stance(
    at: &mut IVec3,
    yaw: i32,
    room: usize,
    animation: &AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    pad: &ControllerState,
) {
    if animation.animation != clip::CLIMB_STANCE {
        return;
    }
    if pad.held(Button::Up) {
        if traversal.goal == state::PULL_UP {
            return;
        }
        traversal.goal = state::CLIMB_STANCE;
        let (right, right_shift, right_ledge) = test_climb_up_pos(world, *at, yaw, room, RADIUS, RADIUS + 120);
        let (left, left_shift, left_ledge) = test_climb_up_pos(world, *at, yaw, room, RADIUS, -(RADIUS + 80));
        if right == 0 || left == 0 {
            return;
        }
        if right < 0 || left < 0 {
            if (left_ledge - right_ledge).abs() > 120 {
                return;
            }
            traversal.goal = state::PULL_UP;
            at.y += -256 + (right_ledge + left_ledge) / 2;
            return;
        }
        // The right's shift wins when it goes further the same way.
        let shift = match (right_shift, left_shift) {
            (0, left) => left,
            (right, 0) => right,
            (right, left) if (right < 0) != (left < 0) => return,
            (right, left) if (right < 0 && right < left) || (right > 0 && left < right) => right,
            (_, left) => left,
        };
        traversal.goal = state::CLIMBING;
        at.y += shift;
    } else if pad.held(Button::Down) {
        if traversal.goal == state::HANG {
            return;
        }
        traversal.goal = state::CLIMB_STANCE;
        at.y += 256;
        let (right, right_shift) = test_climb_pos(world, *at, yaw, room, RADIUS, RADIUS + 120, -512, 512);
        let (left, mut left_shift) = test_climb_pos(world, *at, yaw, room, RADIUS, -(RADIUS + 80), -512, 512);
        at.y -= 256;
        if right == 0 || left == 0 {
            return;
        }
        if right_shift != 0 && left_shift != 0 {
            if (right_shift < 0) != (left_shift < 0) {
                return;
            }
            if (right_shift < 0 && right_shift < left_shift) || (right_shift > 0 && left_shift < right_shift) {
                left_shift = right_shift;
            }
        }
        if right != 1 || left != 1 {
            traversal.goal = state::HANG;
            return;
        }
        traversal.goal = state::CLIMB_DOWN;
        at.y += left_shift;
    }
}

/// Climb-up collision 0x8003EF70, on the step clip (161) at its foot
/// frames (0, 28/29, 57): carry on while Up holds and both sides can,
/// climb off the top where the wall ends, else stop.
fn climb_up(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    pad: &ControllerState,
    dt: f32,
) {
    if animation.animation != clip::CLIMB_UP {
        return;
    }
    let frame = animation.frame.floor() as i32;
    let step = match frame {
        0 => 0,
        28 | 29 => -256,
        57 => -512,
        _ => return,
    };
    let yaw = yaw(player);
    let mut at = original(player.position);
    at.y += -256 + step;
    let (right, _, right_ledge) = test_climb_up_pos(world, at, yaw, player.room, RADIUS, RADIUS + 120);
    let (left, _, left_ledge) = test_climb_up_pos(world, at, yaw, player.room, RADIUS, -(RADIUS + 80));
    at.y += 256;
    player.position = level(at);
    if right == 0 || left == 0 || !pad.held(Button::Up) {
        traversal.goal = state::CLIMB_STANCE;
        if step != 0 {
            animate(player, motion, animation, traversal, world, dt);
        }
    } else if right < 0 || left < 0 {
        traversal.goal = state::CLIMB_STANCE;
        animate(player, motion, animation, traversal, world, dt);
        if (left_ledge - right_ledge).abs() < 121 {
            traversal.goal = state::PULL_UP;
            let mut at = original(player.position);
            at.y += -256 + (right_ledge + left_ledge) / 2;
            player.position = level(at);
        }
    } else {
        traversal.goal = state::CLIMBING;
        let mut at = original(player.position);
        at.y -= step;
        player.position = level(at);
    }
}

/// Climb-down collision 0x8003F154, on the step clip (168) at its foot
/// frames: carry on while Down holds and both sides hold lower, hang where
/// only the hands would, else stop.
fn climb_down(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    pad: &ControllerState,
    dt: f32,
) {
    if animation.animation != clip::CLIMB_DOWN {
        return;
    }
    let step = match animation.frame.floor() as i32 {
        0 => 0,
        28 | 29 => 256,
        57 => 512,
        _ => return,
    };
    let yaw = yaw(player);
    let mut at = original(player.position);
    at.y += 256 + step;
    let (right, right_shift) = test_climb_pos(world, at, yaw, player.room, RADIUS, RADIUS + 120, -512, 512);
    let (left, left_shift) = test_climb_pos(world, at, yaw, player.room, RADIUS, -(RADIUS + 80), -512, 512);
    at.y -= 256;
    player.position = level(at);
    let mut stop = |player: &mut Player, animation: &mut AnimationState, traversal: &mut Traversal| {
        traversal.goal = state::CLIMB_STANCE;
        animate(player, motion, animation, traversal, world, dt);
    };
    if right == 0 || left == 0 || !pad.held(Button::Down) {
        traversal.goal = state::CLIMB_STANCE;
        if step != 0 {
            stop(player, animation, traversal);
        }
        return;
    }
    if right_shift != 0 && left_shift != 0 && (right_shift < 0) != (left_shift < 0) {
        stop(player, animation, traversal);
        return;
    }
    if right != -1 && left != -1 {
        traversal.goal = state::CLIMB_DOWN;
        let mut at = original(player.position);
        at.y -= step;
        player.position = level(at);
        return;
    }
    // Only the hands hold: hang (0x8003F334..0x8003F370).
    *animation = AnimationState::at_frame(clip::CLIMB_STANCE, 0.0);
    traversal.goal = state::HANG;
    animate(player, motion, animation, traversal, world, dt);
}

/// LaraDoClimbLeftRight 0x8003F864: a side step goes on (or stops) where
/// the wall holds, Lara goes back where it doesn't, and hangs where only
/// her hands would hold.
#[allow(clippy::too_many_arguments)]
fn climb_sideways(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    pad: &ControllerState,
    old: IVec3,
    old_pose: &AnimationState,
    old_state: Option<usize>,
    result: i32,
    shift: i32,
    dt: f32,
) {
    let mut at = original(player.position);
    match result {
        1 => {
            traversal.goal = if left(pad) {
                state::CLIMB_LEFT
            } else if right(pad) {
                state::CLIMB_RIGHT
            } else {
                state::CLIMB_STANCE
            };
            at.y += shift;
            player.position = level(at);
        }
        0 => {
            at.x = old.x;
            at.z = old.z;
            player.position = level(at);
            traversal.goal = state::CLIMB_STANCE;
            if old_state == Some(state::CLIMB_STANCE) {
                *animation = old_pose.clone();
                animate(player, motion, animation, traversal, world, dt);
            } else {
                *animation = AnimationState::at_frame(clip::CLIMB_STANCE, 0.0);
            }
        }
        _ => {
            traversal.goal = state::HANG;
            for _ in 0..64 {
                animate(player, motion, animation, traversal, world, dt);
                if world.data.animations[animation.animation].state == state::HANG {
                    break;
                }
            }
            let mut at = original(player.position);
            at.x = old.x;
            at.z = old.z;
            player.position = level(at);
        }
    }
}
