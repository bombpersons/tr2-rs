//! Backward ground collision and hop-falls. PSX-only evidence and recordings:
//! docs/backward-collision-evidence.md. Distances below are original world units
//! at the boundary; gameplay positions remain Bevy y-up metres.
use super::*;
use engine::animation::{AnimationGoal, ObjectAnimation, advance_authored_animation};

/// Walk-back accepts +/-384 heights; hop-back has no downward probe limit
/// and falls when its centre is over200 above the floor (0x8003B414/0x8003AC48).
#[allow(clippy::too_many_arguments)]
pub(super) fn step(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: Option<&WorldGeometry>,
    clips: Option<&PlayerAnimations>,
    pad: &ControllerState,
    dt: f32,
) -> bool {
    let (Some(world), Some(clips)) = (world, clips) else { return false };
    if player.mode == PlayerMode::FallingBack {
        fall(player, motion, animation, traversal, world, clips, dt);
        return true;
    }
    if !motion.grounded || !matches!(player.mode, PlayerMode::Back | PlayerMode::HopBack) {
        return false;
    }
    let request = mode_clip(player, motion, pad);
    let previous = animation.clone();
    let prior = clips.state(animation.animation);
    let next = step_animation(animation, request, None, Some(clips), original_frames(dt));
    let state = clips.state(next.animation);
    traversal.goal = if prior == Some(5) { 2 } else { clips.state(request).unwrap_or(2) };
    *animation = next;
    traversal.animation_stepped = true;
    if !matches!(state, Some(5 | 16)) {
        // An entry request may still be waiting for an authored window;
        // keep standing until the clip actually enters backward movement.
        traversal.goal = state.unwrap_or(2);
        player.mode = PlayerMode::Idle;
        motion.velocity = Vec3::ZERO;
        return true;
    }
    let old = player.position;
    let proposed = old + ground_displacement(motion.velocity, dt);
    // Turning occurs after animation motion and before collision in the
    // original. apply_turn_rate commits that yaw later in this schedule.
    let heading = player.yaw + motion.turn_rate as f32 * std::f32::consts::TAU / 65536.0 + std::f32::consts::PI;
    motion.movement_heading = Some(heading);
    let Some(centre) = world.floor_at(proposed, player.room) else { return true };
    let probes = probes(proposed, heading);
    let down_limit = if state == Some(16) { 384.0 } else { 32512.0 };
    let blocked = |position: Vec3| {
        let Some(hit) = world.floor_at(position, player.room) else { return true };
        let relative = (old.y - hit.height) * 1024.0;
        let steep = world.slope_at(position, player.room).iter().any(|s| s.unsigned_abs() > 2);
        // GetCollisionInfo0x8001B2E0: uphill steep slopes are walls;
        // downhill steep slopes read as512 with slopes-as-pits enabled.
        let relative = if steep && relative < 0.0 {
            -32767.0
        } else if steep && relative > 0.0 {
            512.0
        } else {
            relative
        };
        !(-384.0..=down_limit).contains(&relative) || hit.ceiling < old.y + PLAYER_HEIGHT
    };
    let axis = if heading.sin().abs() > heading.cos().abs() { 0 } else { 2 };
    player.position = proposed;
    if blocked(probes[0]) {
        // A frontal hit restores the transverse coordinate, shifts to radius
        // +1 from the grid edge, and stops immediately (0x8003C544/A56C).
        player.position = old;
        player.position[axis] = proposed[axis] + grid_shift(probes[0][axis], proposed[axis]);
        player.mode = PlayerMode::Idle;
        motion.velocity = Vec3::ZERO;
        traversal.goal = 2;
        if prior == Some(2) {
            // LaraColStop0x8003A56C restores a preceding standing/turn
            // pose and advances it. Repeated walk requests at an edge
            // therefore keep playing idle instead of restarting clip11.
            *animation = step_animation(&previous, STANDING_CLIP, None, Some(clips), original_frames(dt));
        } else {
            animation.enter(STANDING_CLIP, 0.0, STANDING_CLIP);
        }
    } else {
        // Corner contact shifts sideways and turns away by910 binary-angle
        // units (0x8003C580/C5A4). Static meshes are a separate pending pass.
        for (index, sign) in [(1, 1.0), (2, -1.0)] {
            if blocked(probes[index]) {
                let side = 2 - axis;
                player.position[side] += grid_shift(probes[index][side], probes[0][side]);
                player.yaw += sign * 910.0 * std::f32::consts::TAU / 65536.0;
                break;
            }
        }
    }
    let drop = ((old.y - centre.height) * 1024.0).round();
    if state == Some(5) && drop > 200.0 {
        // Collision only seeds gravity here; its first +6 is next tick.
        animation.enter(93, 0.0, 93);
        traversal.goal = 29;
        player.mode = PlayerMode::FallingBack;
        motion.grounded = false;
        motion.velocity.y = 0.0;
        return true;
    }
    if state == Some(16) && (129.0..384.0).contains(&drop) {
        // Foot-dependent descending step: source absolute frames964..993
        // select62, all others61 (0x8003B4D8..0x8003B538).
        let absolute = clips.clips[animation.animation].frame_start as f32 + animation.frame;
        let clip = if (964.0..994.0).contains(&absolute) { 62 } else { 61 };
        animation.enter(clip, 0.0, 40);
    }
    player.position.y = centre.height;
    player.room = world.floor_at(player.position, player.room).map_or(player.room, |h| h.room);
    true
}

/// GetGridShift0x8001B9EC: push a probe into the reference sector with a
/// one-unit margin. A probe that has not crossed a grid edge needs no shift.
fn grid_shift(probe: f32, reference: f32) -> f32 {
    let probe = (probe * 1024.0).round() as i32;
    let reference = (reference * 1024.0).round() as i32;
    let delta = match probe.div_euclid(1024).cmp(&reference.div_euclid(1024)) {
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => -probe.rem_euclid(1024) - 1,
        std::cmp::Ordering::Less => 1025 - probe.rem_euclid(1024),
    };
    units(delta as f32)
}

fn ground_displacement(velocity: Vec3, dt: f32) -> Vec3 {
    let mut displacement = velocity * dt;
    for axis in [0, 2] {
        let n = displacement[axis] * 1024.0;
        let stable = if (n - n.round()).abs() < 0.0001 { n.round() } else { n };
        displacement[axis] = units(stable.floor());
    }
    displacement
}

/// GetCollisionInfo's front/left/right cardinal probes, radius100; the front
/// probe's transverse offset follows the original sine/cosine table.
fn probes(position: Vec3, yaw: f32) -> [Vec3; 3] {
    let angle = (yaw * 65536.0 / std::f32::consts::TAU).round() as i32;
    let quadrant = ((angle + 8192) >> 14).rem_euclid(4);
    let across = |trig| (trig * 100) >> 14;
    let offsets = match quadrant {
        0 => [[across(engine::camera::chase::sin(angle)), 100], [-100, 100], [100, 100]],
        1 => [[100, across(engine::camera::chase::cos(angle))], [100, 100], [100, -100]],
        2 => [[across(engine::camera::chase::sin(angle)), -100], [100, -100], [-100, -100]],
        _ => [[-100, across(engine::camera::chase::cos(angle))], [-100, -100], [-100, 100]],
    };
    offsets.map(|[x, z]| position + Vec3::new(units(x as f32), 0.0, units(z as f32)))
}

#[allow(clippy::too_many_arguments)]
fn fall(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    clips: &PlayerAnimations,
    dt: f32,
) {
    traversal.animation_stepped = true;
    let mut a = ObjectAnimation { animation: animation.animation, frame: animation.frame };
    let mut goal = AnimationGoal { state: traversal.goal };
    advance_authored_animation(&world.data, &mut a, &mut goal, original_frames(dt));
    *animation = AnimationState::at_frame(a.animation, a.frame);
    if motion.grounded && clips.state(a.animation) == Some(2) {
        player.mode = PlayerMode::Recovering;
        motion.velocity = Vec3::ZERO;
        return;
    }
    let old = player.position;
    let fall_speed = (-motion.velocity.y * 1024.0 / ORIGINAL_FPS).round();
    motion.velocity.y -= units(if fall_speed < 128.0 { 6.0 } else { 1.0 } * ORIGINAL_FPS) * original_frames(dt);
    player.position += ground_displacement(motion.velocity, dt);
    // Hop-fall collision0x8003B9A8 keeps the backward heading and lands
    // without a second animation advance. The next tick accepts goal2.
    if let Some(hit) = world.floor_at(player.position, player.room) {
        player.room = hit.room;
        if hit.water {
            // Preserve the existing provisional water handoff. Entry and
            // swimming behavior are the next separate evidence milestone.
            player.mode = PlayerMode::Swimming;
            motion.grounded = false;
            return;
        }
        if player.position.y <= hit.height && motion.velocity.y < 0.0 {
            landing_damage(player, motion.velocity);
            player.position.y = hit.height;
            motion.grounded = true;
            motion.velocity.y = 0.0;
            traversal.goal = if player.health > 0 { 2 } else { 8 };
            if player.health == 0 {
                motion.velocity = Vec3::ZERO;
            }
        }
    } else {
        player.position = old;
    }
}
