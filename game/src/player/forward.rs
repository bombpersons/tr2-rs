//! Forward walking's room probes, authored step clips and collision stops.
//! PSX walk collision0x8003A62C; shared room deflection0x8003C544.
use super::backward::{grid_shift, ground_displacement, probes};
use super::collision::WalkMove;
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn step(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: Option<&WorldGeometry>,
    clips: Option<&PlayerAnimations>,
    pad: &ControllerState,
    collision: &mut CollisionFrame,
    dt: f32,
) -> bool {
    let (Some(world), Some(clips)) = (world, clips) else { return false };
    if !motion.grounded || !matches!(player.mode, PlayerMode::Walk | PlayerMode::WalkStopping) {
        return false;
    }
    let previous = animation.clone();
    let stopping = player.mode == PlayerMode::WalkStopping;
    let controls = GroundControls::read(pad);
    let request = if controls.direction.y > 0.0 {
        if controls.walk { clip::WALK_START } else { clip::RUN_START }
    } else {
        STANDING_CLIP
    };
    traversal.goal = if stopping && pad.held(Button::Square) {
        state::JUMP_PREPARE
    } else {
        clips.state(request).unwrap_or(state::STAND)
    };
    *animation = step_animation(animation, request, None, Some(clips), original_frames(dt));
    traversal.animation_stepped = true;
    if stopping {
        motion.velocity = Vec3::ZERO;
        // Collision changed state to standing before selecting a foot-stop
        // clip, whose authored state is still walking. Retain the override
        // until its natural link into11 (0x8003A6AC..0x8003A730).
        if !matches!(animation.animation, clip::WALK_STOP_RIGHT | clip::WALK_STOP_LEFT) {
            player.mode = PlayerMode::Idle;
        }
        return true;
    }
    let old = player.position;
    // The move angle was written before the last tick's side turn.
    let velocity = Quat::from_rotation_y(-motion.edge_turn as f32 * std::f32::consts::TAU / 65536.0) * motion.velocity;
    motion.edge_turn = 0;
    let proposed = old + ground_displacement(velocity, dt);
    let heading = player.yaw + motion.turn_rate as f32 * std::f32::consts::TAU / 65536.0;
    motion.movement_heading = Some(heading);
    if body_floor(Some(world), proposed, player.room).is_none() {
        return true;
    }
    let probes = probes(proposed, heading);
    let blocked = |position: Vec3| {
        let Some(hit) = body_floor(Some(world), position, player.room) else { return true };
        let relative = (old.y - hit.height) * 1024.0;
        let steep = world.slope_at(position, player.room).iter().any(|s| s.unsigned_abs() > 2);
        // +/-384 floor bounds, zero head margin, slopes as walls/pits and
        // deadly floors as pits (0x8003A668..0x8003A684).
        !(-384.0..=384.0).contains(&relative)
            || (steep && relative != 0.0)
            || hit.death
            || hit.ceiling < old.y + PLAYER_HEIGHT
    };
    // Preserve the existing vault controller's Action path until its walking
    // entry is independently recorded with the same room probes.
    if blocked(probes[0]) && pad.held(Button::Cross) {
        traversal.animation_stepped = false;
        *animation = previous;
        return false;
    }
    // Object pushes come between this move and the walk collision
    // (ItemPushLara 0x8001CA78 before 0x8003A62C): `collide`.
    collision.walk = Some(WalkMove { old, origin: collision.origin, previous });
    player.position = proposed;
    true
}

/// The walk collision `step` held back, from where object pushes left Lara.
/// A push that could not clear put her back on the origin it last accepted,
/// which the probes start from.
#[allow(clippy::too_many_arguments)]
pub(super) fn collide(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: Option<&WorldGeometry>,
    clips: Option<&PlayerAnimations>,
    pad: &ControllerState,
    pending: WalkMove,
    origin: Vec3,
    dt: f32,
) {
    let (Some(world), Some(clips)) = (world, clips) else { return };
    let WalkMove { old, previous, .. } = pending;
    let old = if origin == pending.origin { old } else { origin.with_y(old.y) };
    let proposed = player.position;
    let Some(centre) = body_floor(Some(world), proposed, player.room) else { return };
    let heading = motion.movement_heading.unwrap_or(player.yaw);
    let probes = probes(proposed, heading);
    let blocked = |position: Vec3| {
        let Some(hit) = body_floor(Some(world), position, player.room) else { return true };
        let relative = (old.y - hit.height) * 1024.0;
        let steep = world.slope_at(position, player.room).iter().any(|s| s.unsigned_abs() > 2);
        !(-384.0..=384.0).contains(&relative)
            || (steep && relative != 0.0)
            || hit.death
            || hit.ceiling < old.y + PLAYER_HEIGHT
    };
    let axis = if heading.sin().abs() > heading.cos().abs() { 0 } else { 2 };
    if blocked(probes[0]) {
        player.position = old;
        player.position[axis] = proposed[axis] + grid_shift(probes[0][axis], proposed[axis]);
        motion.velocity = Vec3::ZERO;
        traversal.goal = state::STAND;
        let absolute = clips.clips[animation.animation].frame_start as f32 + animation.frame;
        let stop = if contract::frame::WALL_STOP_LEFT.contains(&absolute) {
            Some(clip::WALK_STOP_LEFT)
        } else if contract::frame::WALL_STOP_RIGHT.iter().any(|window| window.contains(&absolute)) {
            Some(clip::WALK_STOP_RIGHT)
        } else {
            None
        };
        if let Some(clip) = stop {
            animation.enter(clip, 0.0, STANDING_CLIP);
            player.mode = PlayerMode::WalkStopping;
        } else {
            player.mode = PlayerMode::Idle;
            collide_stop(&previous, clips, pad, dt, traversal, animation);
        }
    } else {
        // GetCollisionInfo's side contact (walls and pits, 384 either way):
        // LaraDeflectEdge shifts her out and turns her 910 away.
        let limits = collision::Limits { radius: 100, bad_pos: 384, bad_neg: -384, bad_ceiling: 0 };
        let slopes = collision::Slopes { walls: true, pits: true };
        let facing = (heading * 65536.0 / std::f32::consts::TAU).round() as i32 as i16 as i32;
        let at = collision::original(proposed);
        let info =
            collision::info_with_slopes(world, at, collision::original(old), player.room, 762, facing, &limits, slopes);
        let turn = match info.contact {
            collision::Contact::Left => 910,
            collision::Contact::Right => -910,
            _ => 0,
        };
        if turn != 0 {
            let shifted = collision::level(at + info.shift);
            player.position.x = shifted.x;
            player.position.z = shifted.z;
            player.yaw += turn as f32 * std::f32::consts::TAU / 65536.0;
            motion.edge_turn = turn;
        }
        let drop = ((old.y - centre.height) * 1024.0).round();
        let absolute = clips.clips[animation.animation].frame_start as f32 + animation.frame;
        // Foot ranges read after this tick's animation move, before snapping
        // to the centre floor (0x8003A740..0x8003A800).
        let step = if drop >= 129.0 {
            Some(if contract::frame::STEP_DOWN_LEFT.contains(&absolute) {
                clip::STEP_DOWN_LEFT
            } else {
                clip::STEP_DOWN_RIGHT
            })
        } else if (-384.0..-128.0).contains(&drop) {
            Some(if contract::frame::STEP_UP_RIGHT.contains(&absolute) {
                clip::STEP_UP_RIGHT
            } else {
                clip::STEP_UP_LEFT
            })
        } else {
            None
        };
        if let Some(clip) = step {
            animation.enter(clip, 0.0, clip::WALK_START);
        }
    }
    player.position.y = centre.height;
    player.room = world.floor_at(player.position, player.room).map_or(player.room, |hit| hit.room);
}
