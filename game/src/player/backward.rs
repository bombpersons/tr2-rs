//! Backward ground collision and hop-falls. PSX-only evidence and recordings:
//! docs/backward-collision-evidence.md. Distances below are original world units
//! at the boundary; gameplay positions remain Bevy y-up metres.
use super::*;
use collision::Contact;
use engine::world::Command;

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
    weapon: &WeaponState,
    pad: &ControllerState,
    dt: f32,
) -> bool {
    let (Some(world), Some(clips)) = (world, clips) else { return false };
    if player.mode == PlayerMode::FallingBack {
        return fall(player, motion, animation, traversal, world, clips, weapon, pad, dt);
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
pub(super) fn grid_shift(probe: f32, reference: f32) -> f32 {
    let probe = (probe * 1024.0).round() as i32;
    let reference = (reference * 1024.0).round() as i32;
    let delta = match probe.div_euclid(1024).cmp(&reference.div_euclid(1024)) {
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => -probe.rem_euclid(1024) - 1,
        std::cmp::Ordering::Less => 1025 - probe.rem_euclid(1024),
    };
    units(delta as f32)
}

pub(super) fn ground_displacement(velocity: Vec3, dt: f32) -> Vec3 {
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
pub(super) fn probes(position: Vec3, yaw: f32) -> [Vec3; 3] {
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

/// The flight heading relative to yaw: the back jump and hop-back fall
/// face behind, the right and left jumps to the side (0x8003B71C..0x8003B7B8).
fn flight_turn(state: Option<usize>) -> f32 {
    match state {
        Some(26) => std::f32::consts::FRAC_PI_2,
        Some(27) => -std::f32::consts::FRAC_PI_2,
        _ => std::f32::consts::PI,
    }
}

/// Back and side flight: the hop-back fall (state29), back jump (25) and
/// right/left jumps (26/27), whose handlers share collision0x8003C250 at
/// their own headings. Hands this tick to forward flight, reach or fast
/// fall when the clip enters state3, 11 or9; returns false then.
#[allow(clippy::too_many_arguments)]
pub(super) fn fall(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    clips: &PlayerAnimations,
    weapon: &WeaponState,
    pad: &ControllerState,
    dt: f32,
) -> bool {
    traversal.animation_stepped = true;
    let entering = traversal.enter.take();
    if entering.is_none() {
        // Each control requests fast fall9 at fall speed132. Otherwise the
        // back jump requests twist68 on Forward or Roll (input&0x1001)
        // unless goal1 or2 is pending, and the right and left jumps on the
        // opposite direction unless goal2 is; their clips have no twist
        // range (0x800395D4, 0x80039638, 0x8003969C). Hop-fall control
        // then requests reach11 on Action with free hands (0x80039720).
        let fall_speed = -motion.velocity.y * 1024.0 / ORIGINAL_FPS;
        let twist = match clips.state(animation.animation) {
            Some(25) => !matches!(traversal.goal, 1 | 2) && (pad.held(Button::Up) || pad.held(Button::Circle)),
            Some(26) => traversal.goal != 2 && pad.held(Button::Left),
            Some(27) => traversal.goal != 2 && pad.held(Button::Right),
            _ => false,
        };
        if fall_speed >= 132.0 {
            traversal.goal = 9;
        } else if twist {
            traversal.goal = 68;
        }
        if clips.state(animation.animation) == Some(29) && weapon.hands_free() && pad.held(Button::Cross) {
            traversal.goal = 11;
        }
    }
    let mut a = ObjectAnimation { animation: animation.animation, frame: animation.frame };
    let mut goal = AnimationGoal { state: traversal.goal };
    let cues = if let Some((clip, frame)) = entering {
        a = ObjectAnimation { animation: clip, frame };
        vec![]
    } else {
        advance_authored_animation(&world.data, &mut a, &mut goal, original_frames(dt))
    };
    *animation = AnimationState::at_frame(a.animation, a.frame);
    for cue in &cues {
        if matches!(cue.command, Command::Effect { effect: 0, .. }) {
            // Effect0 turns yaw180 degrees (twist212's last frame).
            player.yaw = (player.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU);
        }
    }
    let state = clips.state(a.animation);
    if matches!(state, Some(3 | 9 | 11)) {
        // Twist213 continues as forward flight, hop-fall reach94 as
        // forward reach, and fall83/84/85/36 as fast fall. Their handlers
        // own this tick's move and collision.
        player.mode = if state == Some(9) { PlayerMode::FastFalling } else { PlayerMode::JumpingForward };
        traversal.enter = Some((a.animation, a.frame));
        return false;
    }
    let facing = player.yaw + flight_turn(state);
    let back = Vec3::new(facing.sin(), 0.0, facing.cos());
    if motion.grounded && matches!(state, Some(25..=27)) {
        // Takeoffs74/80/78 move at their authored speed with no gravity;
        // the end command launches75/81/79 (slide_jump_back.csv: 99, then
        // 50/-94).
        motion.velocity = back * clips.speed(animation).unwrap_or(0.0);
    }
    for cue in cues {
        if let Command::JumpVelocity([vertical, horizontal]) = cue.command
            && cue.at_end
        {
            motion.velocity =
                back * units(horizontal as f32 * ORIGINAL_FPS) + Vec3::Y * units(-vertical as f32 * ORIGINAL_FPS);
            motion.grounded = false;
        }
    }
    if motion.grounded && state == Some(2) {
        player.mode = PlayerMode::Recovering;
        motion.velocity = Vec3::ZERO;
        // The landing clip's stop collision runs on this same tick.
        super::traversal::stop_collision(player, motion, animation, traversal, world);
        return true;
    }
    let old = player.position;
    let fall_speed = (-motion.velocity.y * 1024.0 / ORIGINAL_FPS).round();
    if !motion.grounded {
        motion.velocity.y -= units(if fall_speed < 128.0 { 6.0 } else { 1.0 } * ORIGINAL_FPS) * original_frames(dt);
    }
    // AnimateLara moves along the heading the previous collision left; the
    // velocity already points that way, so only its magnitude is needed.
    let speed = (motion.velocity.with_y(0.0).length() * 1024.0 / ORIGINAL_FPS).round();
    player.position += ground_displacement(motion.velocity, dt);
    // Hop-fall collision0x8003B9A8 faces backwards, and the jumps their own
    // way; all share forward-jump collision's limits and deflection
    // (0x8003B9C0..0x8003B9F4, 0x8003C250). It lands
    // without a second animation advance; the next tick accepts goal2.
    motion.movement_heading = Some(facing);
    motion.velocity =
        (Vec3::new(facing.sin(), 0.0, facing.cos()) * units(speed * ORIGINAL_FPS)).with_y(motion.velocity.y);
    let angle = (facing * 65536.0 / std::f32::consts::TAU).round() as i32 as i16 as i32;
    let limits = collision::Limits { radius: 100, bad_pos: -collision::NO_HEIGHT, bad_neg: -384, bad_ceiling: 192 };
    let info = collision::info(
        world,
        collision::original(player.position),
        collision::original(old),
        player.room,
        762,
        angle,
        &limits,
    );
    player.position += collision::level(info.shift);
    if matches!(info.contact, Contact::Front | Contact::TopFront) {
        super::traversal::rebound(player, motion, animation, traversal, info.mid.floor, facing, speed as i32);
    } else {
        super::traversal::deflect(player, motion, info.contact, angle, false);
    }
    if let Some(hit) = world.floor_at(player.position, player.room) {
        player.room = super::body_room(world, player.position, hit.room, 381.0);
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
    true
}
