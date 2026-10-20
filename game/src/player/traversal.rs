//! Ledge jumps, hanging, pull-ups and landing recovery. Decisions stay in the game; authored clips,
//! bounds and end commands use the engine's existing animation machinery.
use super::*;
use collision::Contact;
use engine::animation::vertical_bounds;
use engine::world::Command;

/// The persistent goal requested by ledge collision. It is consumed by the
/// following animation pass, so a new pull-up request takes effect next tick.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct Traversal {
    pub goal: usize,
    /// Running jump latch: run-start clears it, and run frame4 enables it.
    pub run_jump_ready: bool,
    #[reflect(ignore)]
    pub(super) animation_stepped: bool,
    #[reflect(ignore)]
    /// A clip (and frame within it) the next animation pass starts at
    /// instead of advancing.
    pub(super) enter: Option<(usize, f32)>,
}

impl Default for Traversal {
    fn default() -> Self {
        Self { goal: 2, run_jump_ready: false, animation_stepped: false, enter: None }
    }
}

/// Forward-flight control requests fast fall9 once the fall speed read at
/// the start of the tick reaches132 (0x80038C4C..0x80038C60).
const FAST_FALL_SPEED: f32 = 132.0;

/// Facing quadrant, cardinal heading and its angular error. Grab accepts
/// +/-6370 binary-angle units (0x8003D3C0); vault uses the narrower 5460.
fn cardinal(yaw: f32) -> (usize, Vec3, i32) {
    let angle = (yaw * 65536.0 / std::f32::consts::TAU).round() as i32;
    let quadrant = ((angle + 8192) >> 14).rem_euclid(4) as usize;
    let delta = (angle - quadrant as i32 * 16384 + 32768).rem_euclid(65536) - 32768;
    (quadrant, [Vec3::Z, Vec3::X, -Vec3::Z, -Vec3::X][quadrant], delta)
}

/// Three cardinal front probes from GetCollisionInfo. Its across-axis
/// offset follows the unsnapped yaw; corners are +/-radius on that axis.
fn front(world: &WorldGeometry, position: Vec3, yaw: f32, room: usize) -> Option<[FloorHit; 3]> {
    let (quadrant, _, _) = cardinal(yaw);
    let r = 100;
    let (sin, cos) = yaw.sin_cos();
    let across = |trig: f32| ((trig * 16384.0).trunc() as i32 * r) >> 14;
    let offsets = match quadrant {
        0 => [[across(sin), r], [-r, r], [r, r]],
        1 => [[r, across(cos)], [r, r], [r, -r]],
        2 => [[across(sin), -r], [r, -r], [-r, -r]],
        _ => [[-r, across(cos)], [-r, -r], [-r, r]],
    };
    let base = (position * 1024.0).round() / 1024.0;
    let probe = |[x, z]: [i32; 2]| world.floor_at(base + Vec3::new(units(x as f32), 0.0, units(z as f32)), room);
    Some([probe(offsets[0])?, probe(offsets[1])?, probe(offsets[2])?])
}

fn apply_offset(player: &mut Player, offset: [i32; 3]) {
    let (sin, cos) = player.yaw.sin_cos();
    let [x, y, z] = offset.map(|n| n as f32);
    // Recorded traversal offsets follow a snapped cardinal heading. Round
    // float sine/cosine noise back to those exact world-unit axes.
    player.position += Vec3::new((cos * x + sin * z).round(), -y, (cos * z - sin * x).round()) / 1024.0;
}

/// Compression handler0x800390C8 accepts a forward jump when the floor256
/// ahead is no more than384 above the feet. GetHeightAhead0x8003DF60 samples
/// at the head (762 above the feet), following portals at that height.
fn jump_ahead_clear(world: &WorldGeometry, player: &Player) -> bool {
    let angle = (player.yaw * 65536.0 / std::f32::consts::TAU).round() as i32;
    let offset = Vec3::new(
        units(((engine::camera::chase::sin(angle) * 256) >> 14) as f32),
        PLAYER_HEIGHT,
        units(((engine::camera::chase::cos(angle) * 256) >> 14) as f32),
    );
    world
        .floor_at(player.position + offset, player.room)
        .is_some_and(|hit| hit.height <= player.position.y + units(384.0))
}

/// Advance only the traversal states. The ordinary locomotion recordings
/// retain their established path. True means movement and animation were
/// both handled here, including on a transition back to standing.
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
    let mut mode = player.mode;
    // A back-flight handoff into fast fall ran back-jump control this tick.
    let fast_fall_control = matches!(mode, PlayerMode::FastFalling) && traversal.enter.is_none();
    if !matches!(
        mode,
        PlayerMode::JumpingUp { .. }
            | PlayerMode::JumpingForward
            | PlayerMode::FastFalling
            | PlayerMode::FatalLanding
            | PlayerMode::JumpImpact
            | PlayerMode::Hanging { .. }
            | PlayerMode::PullingUp
            | PlayerMode::Recovering
    ) {
        return false;
    }
    let (Some(world), Some(clips)) = (world, clips) else { return false };
    traversal.animation_stepped = true;
    let entering = traversal.enter.take();
    if entering.is_none() && matches!(mode, PlayerMode::JumpingForward) {
        match clips.state(animation.animation) {
            Some(15) if pad.held(Button::Up) && jump_ahead_clear(world, player) => traversal.goal = 3,
            Some(3) => {
                // Forward-flight control0x80038B88 resets reach/dive requests,
                // then (unless landing1/2/8 is pending) requests in turn,
                // each overriding the last: Action's reach11, Back or Roll's
                // twist68 (input&0x1002), Walk's dive52 and, at fall speed
                // 132, fast fall9 (0x80038C4C..0x80038C60). A twist request
                // otherwise persists. The current clip owns acceptance:
                // fall35 has no range for52 or68.
                if matches!(traversal.goal, 52 | 11) {
                    traversal.goal = 3;
                }
                if !matches!(traversal.goal, 1 | 2 | 8) {
                    if weapon.hands_free() && pad.held(Button::Cross) {
                        traversal.goal = 11;
                    }
                    if pad.held(Button::Down) || pad.held(Button::Circle) {
                        traversal.goal = 68;
                    }
                    if weapon.hands_free() && pad.held(Button::R1) {
                        traversal.goal = 52;
                    }
                    if -motion.velocity.y * 1024.0 / ORIGINAL_FPS >= FAST_FALL_SPEED {
                        traversal.goal = 9;
                    }
                }
            }
            _ => {}
        }
    }
    if matches!(mode, PlayerMode::Recovering) {
        // State2 handles input while the landing clip is still playing;
        // its authored ranges decide when that request can take effect.
        let controls = GroundControls::read(pad);
        traversal.goal = if pad.held(Button::Square) {
            15
        } else if controls.side < 0.0 {
            22
        } else if controls.side > 0.0 {
            21
        } else if controls.direction.y > 0.0 {
            if controls.walk { 0 } else { 1 }
        } else if controls.direction.y < 0.0 {
            if controls.walk { 16 } else { 5 }
        } else if controls.direction.x < 0.0 {
            7
        } else if controls.direction.x > 0.0 {
            6
        } else {
            2
        };
    }
    let mut a = ObjectAnimation { animation: animation.animation, frame: animation.frame };
    let mut goal = AnimationGoal { state: traversal.goal };
    // AnimateLara advances before collision; commands can move the actor
    // before that same tick's collision (0x80042E08..0x80043248).
    let cues = if let Some((clip, frame)) = entering {
        a.animation = clip;
        a.frame = frame;
        vec![]
    } else {
        advance_authored_animation(&world.data, &mut a, &mut goal, original_frames(dt))
    };
    // AnimateItem resets its goal on a natural state-changing link;
    // AnimateLara0x80042E08 preserves it. The controller owns any new goal.
    *animation = AnimationState::at_frame(a.animation, a.frame);
    for cue in cues {
        match cue.command {
            Command::SetPosition(offset) if cue.at_end => apply_offset(player, offset),
            Command::Effect { effect: 0, .. } => {
                // Effect0 turns yaw180 degrees (twist210's last frame).
                player.yaw = (player.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU);
            }
            Command::JumpVelocity([vertical, horizontal]) if cue.at_end => {
                let lift = match mode {
                    PlayerMode::JumpingUp { launch_speed } if launch_speed > 0.0 => launch_speed,
                    _ => units(-vertical as f32 * ORIGINAL_FPS),
                };
                motion.velocity = player.forward() * units(horizontal as f32 * ORIGINAL_FPS) + Vec3::Y * lift;
                motion.grounded = false;
            }
            _ => {}
        }
    }
    let state = clips.state(animation.animation).unwrap_or(2);
    if state == 25 && matches!(mode, PlayerMode::JumpingForward) {
        // Twist210 chains into211, whose back-jump handler0x8003B71C
        // owns this tick's move and collision.
        player.mode = PlayerMode::FallingBack;
        traversal.enter = Some((animation.animation, animation.frame));
        return super::backward::fall(player, motion, animation, traversal, world, clips, pad, dt);
    }
    if mode == PlayerMode::FatalLanding {
        // Fatal fast-fall collision leaves the fall clip with goal8; its
        // authored dispatch enters25, whose final frame holds the body.
        motion.velocity = Vec3::ZERO;
        return true;
    }
    if mode == PlayerMode::JumpImpact && animation.animation == STANDING_CLIP {
        // AnimateLara's natural end link restores the destination state
        // (0x80042FAC). Until then state14 overrides clip31's state2.
        mode = PlayerMode::Recovering;
        player.mode = mode;
    }
    if state == 9 && matches!(mode, PlayerMode::JumpingForward) {
        mode = PlayerMode::FastFalling;
        player.mode = mode;
    }
    if !motion.grounded {
        // Fast-fall control0x80038F6C brakes signed speed by95/100,
        // truncating toward zero. AnimateLara0x800431C8 then adds the
        // difference between consecutive authored integer speeds, even
        // after a clip link. Clip32->33 briefly reverses the rebound.
        let heading = motion.movement_heading.unwrap_or(player.yaw);
        let direction = Vec3::new(heading.sin(), 0.0, heading.cos());
        let mut speed = (motion.velocity.dot(direction) * 1024.0 / ORIGINAL_FPS).round() as i32;
        if fast_fall_control {
            speed = speed * 95 / 100;
        }
        if let Some(clip) = clips.clips.get(animation.animation) {
            let frame = animation.frame.floor() as i64;
            let velocity = |frame| (i64::from(clip.speed) + i64::from(clip.accel) * frame) >> 16;
            speed += (velocity(frame) - velocity(frame - 1)) as i32;
        }
        motion.velocity = (direction * units(speed as f32 * ORIGINAL_FPS)).with_y(motion.velocity.y);
    }
    if matches!(mode, PlayerMode::JumpingForward) && state == 28 {
        // Compression can finish into a vertical jump when no direction is
        // clear; the destination clip's command owns the launch velocity.
        mode = PlayerMode::JumpingUp { launch_speed: 0.0 };
        player.mode = mode;
    }
    if matches!(mode, PlayerMode::JumpingUp { .. }) && state == 2 && !motion.grounded {
        // A ceiling contact requests stop without cancelling ascent
        // (0x8003B864..0x8003B888). On the next authored dispatch to31,
        // standing collision settles the feet after one final airborne move.
        player.mode = PlayerMode::Recovering;
        return impact_motion(player, motion, animation, world, clips, dt);
    }
    match mode {
        PlayerMode::PullingUp => pull_up(player, motion, world, state),
        PlayerMode::Hanging { .. } if state == 19 => pull_up(player, motion, world, state),
        PlayerMode::Hanging { .. } => hang_collision(player, motion, animation, traversal, world, pad),
        PlayerMode::Recovering
            if state == 2 && motion.grounded && stop_collision(player, motion, animation, traversal, world) =>
        {
            true
        }
        PlayerMode::Recovering => recover(player, motion, animation, clips, state),
        PlayerMode::JumpImpact => impact_motion(player, motion, animation, world, clips, dt),
        _ => jump_motion(player, motion, animation, traversal, world, clips, weapon, pad, dt),
    }
}

/// Standing collision settles the feet after the last airborne animation
/// move and clears gravity (0x8003AA20). Used by low-impact state14 through
/// 0x8003B2FC and by a vertical ceiling contact dispatching to standing.
fn impact_motion(
    player: &mut Player,
    motion: &mut Motion,
    animation: &AnimationState,
    world: &WorldGeometry,
    clips: &PlayerAnimations,
    dt: f32,
) -> bool {
    if motion.grounded {
        motion.velocity = player.forward() * clips.speed(animation).unwrap_or(0.0);
    } else {
        let fall_speed = -motion.velocity.y * 1024.0 / ORIGINAL_FPS;
        motion.velocity.y -= units(if fall_speed < 128.0 { 6.0 } else { 1.0 } * ORIGINAL_FPS) * original_frames(dt);
    }
    let heading = motion.movement_heading.unwrap_or(player.yaw);
    let direction = Vec3::new(heading.sin(), 0.0, heading.cos());
    let speed = (motion.velocity.dot(direction) * 1024.0 / ORIGINAL_FPS).round() as i32;
    let angle = (heading * 65536.0 / std::f32::consts::TAU).round() as i32;
    let ticks = original_frames(dt);
    player.position += Vec3::new(
        units(((engine::camera::chase::sin(angle) * speed) >> 14) as f32 * ticks),
        motion.velocity.y * dt,
        units(((engine::camera::chase::cos(angle) * speed) >> 14) as f32 * ticks),
    );
    if let Some(hit) = world.floor_at(player.position, player.room) {
        player.position.y = hit.height;
        player.room = hit.room;
    }
    motion.grounded = true;
    motion.velocity = player.forward() * units(speed as f32 * ORIGINAL_FPS);
    motion.movement_heading = Some(player.yaw);
    true
}

fn pull_up(player: &mut Player, motion: &mut Motion, world: &WorldGeometry, state: usize) -> bool {
    motion.velocity = Vec3::ZERO;
    player.mode = PlayerMode::PullingUp;
    // Clip97 raises by its authored735, then102 moves forward200.
    // Only after102 chains into standing does collision settle the feet.
    if state == 2 {
        player.mode = PlayerMode::Idle;
        motion.grounded = true;
        if let Some(hit) = world.floor_at(player.position, player.room) {
            player.room = hit.room;
            player.position.y = hit.height;
        }
    }
    true
}

fn hang_collision(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    pad: &ControllerState,
) -> bool {
    let PlayerMode::Hanging { ledge } = player.mode else { return false };
    // Reach catch leaves gravity and speed2 active for its first hang
    // pass. That pass restores the previous pose, clears gravity/fall,
    // and retains speed2 until the next animation pass writes speed0
    // (forward_climb fields82..86, HangTest0x8003CDE4).
    let settling = !motion.grounded;
    motion.velocity = if settling { motion.velocity.with_y(0.0) } else { Vec3::ZERO };
    motion.grounded = true;
    if !pad.held(Button::Cross) || player.health <= 0 {
        // Hang release (0x8003CF80..0x8003D0EC): switch to the
        // vertical-jump loop at+9, lower by its bottom bound and seed
        // speed2/fall1. Gravity begins on the following tick.
        *animation = AnimationState::at_frame(28, 9.0);
        let (_, bottom) = vertical_bounds(&world.data, animation).unwrap_or((0, 0));
        player.position.y -= units(bottom as f32);
        motion.velocity = player.forward() * units(2.0 * ORIGINAL_FPS) - Vec3::Y * units(ORIGINAL_FPS);
        motion.grounded = false;
        player.mode = PlayerMode::JumpingUp { launch_speed: 0.0 };
        traversal.goal = 28;
    } else {
        let (top, _) = vertical_bounds(&world.data, animation).unwrap_or((-726, 0));
        if !settling {
            player.position.y = ledge.y + units(top as f32);
        }
        // Hang collision asks to climb only with front floor
        // [-849,-651] and clearance at all three probes (0x8003AFCC).
        // Static-obstruction branches remain unported. Goal persists
        // until authored dispatch accepts it.
        if pad.held(Button::Up)
            // HangTest probes after nudging two units toward the wall,
            // then collision restores the contact position (0x8003CE50).
            && let Some(hits) = front(world, player.position + player.forward() * units(2.0), player.yaw, player.room)
        {
            let relative = ((player.position.y - hits[0].height) * 1024.0).round() as i32;
            if (-849..=-651).contains(&relative) && hits.iter().all(|h| h.ceiling - h.height >= PLAYER_HEIGHT) {
                traversal.goal = 19;
            }
        }
    }
    true
}

/// LaraDeflectEdgeJump0x8003C66C: every frontal impact quarters speed and
/// reverses the movement heading. More than512 above the floor it also
/// enters fast-fall32 at+1; below129 it enters low impact31 with state/goal
/// 14, overriding that clip's declared state2 until an authored end link.
/// Between them the current clip and goal survive. Thresholds are tested in
/// the original's integral world units; `clearance` is the floor's depth
/// below the feet. Fall speed at or below0 becomes1.
pub(super) fn rebound(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    clearance: i32,
    collision_heading: f32,
    speed: i32,
) {
    if clearance > 512 {
        *animation = AnimationState::at_frame(32, 1.0);
        traversal.goal = 9;
        player.mode = PlayerMode::FastFalling;
    } else if clearance < 129 {
        *animation = AnimationState::at_frame(31, 0.0);
        traversal.goal = 14;
        player.mode = PlayerMode::JumpImpact;
    }
    let heading = collision_heading + std::f32::consts::PI;
    motion.movement_heading = Some(heading);
    motion.velocity = (Vec3::new(heading.sin(), 0.0, heading.cos()) * units((speed / 4) as f32 * ORIGINAL_FPS))
        .with_y(motion.velocity.y.min(-units(ORIGINAL_FPS)));
}

/// The non-frontal airborne deflections. Forward jump0x8003C5F4 and reach
/// 0x8003C7F4 share them: a side contact turns910 away from the wall
/// (tables at0x80010994 and0x80010A14). A ceiling contact (and, for reach,
/// a front-ceiling contact) stops ascent. Forward jump's frontal rebound is
/// handled by its caller; reach keeps only the shift for a front contact.
pub(super) fn deflect(player: &mut Player, motion: &mut Motion, contact: Contact, facing: i32, reach: bool) {
    const SIDE_TURN: f32 = 910.0 * std::f32::consts::TAU / 65536.0;
    let stop_ascent = |motion: &mut Motion, fall_speed: f32| {
        // Fall speed is y-down; only a nonpositive speed is replaced.
        if motion.velocity.y >= 0.0 {
            motion.velocity.y = -units(fall_speed * ORIGINAL_FPS);
        }
    };
    match contact {
        Contact::Left => player.yaw += SIDE_TURN,
        Contact::Right => player.yaw -= SIDE_TURN,
        Contact::Top => stop_ascent(motion, 1.0),
        Contact::TopFront if reach => stop_ascent(motion, 1.0),
        Contact::Clamp => {
            // Wedged between floor and ceiling: step back100 along the
            // collision facing, lose horizontal speed and start falling at16
            // (0x8003C760..0x8003C7D8). Not yet exercised by a recording.
            use engine::camera::chase::{cos, sin};
            player.position.x -= units(((sin(facing) * 100) >> 14) as f32);
            player.position.z -= units(((cos(facing) * 100) >> 14) as f32);
            motion.velocity = Vec3::ZERO.with_y(motion.velocity.y);
            stop_ascent(motion, 16.0);
        }
        _ => {}
    }
}

/// Stop collision0x8003AA20 after the clip's move: a body probe (limits
/// +/-384, ceiling0), then the ledge-fall test0x8003A500, then
/// TestLaraSlide0x8003DD30, else ShiftItem and settling onto the floor.
/// True when Lara left standing for flight or a slide.
///
/// Ledge fall: a centre floor more than384 below enters clip34/state3 with
/// zero fall speed and the current speed. Slide: on a floor tilted3 or
/// more the shift applies and the slide starts with no speed. The downhill
/// heading is cardinal, X winning a tie; Lara slides forward (70, state24)
/// when facing within90 degrees of it, otherwise backward (104, state32).
/// Not modelled: the ceiling/clamp restore0x8003C4C4 and the slopes-as-
/// walls/pits probe flags (collision.rs).
pub(super) fn stop_collision(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
) -> bool {
    let yaw = (player.yaw * 65536.0 / std::f32::consts::TAU).round() as i32;
    let limits = collision::Limits { radius: 100, bad_pos: 384, bad_neg: -384, bad_ceiling: 0 };
    let at = collision::original(player.position);
    let info = collision::info(world, at, at, player.room, 762, yaw as i16 as i32, &limits);
    if info.mid.floor > 384 {
        player.mode = PlayerMode::JumpingForward;
        animation.enter(34, 0.0, 34);
        traversal.goal = 3;
        motion.grounded = false;
        motion.velocity.y = 0.0;
        return true;
    }
    player.position += collision::level(info.shift);
    let [tx, tz] = world.slope_at(player.position - collision::level(info.shift), player.room).map(i32::from);
    if tx.abs() < 3 && tz.abs() < 3 {
        player.position.y -= units(info.mid.floor as f32);
        return false;
    }
    let downhill = if tz.abs() > tx.abs() {
        if tz > 0 { -32768 } else { 0 }
    } else if tx > 0 {
        -16384
    } else {
        16384
    };
    let forward = (downhill - yaw + 16384) & 0xffff <= 0x8000;
    let heading = collision::angle_radians(downhill);
    let (clip, state, facing) = if forward { (70, 24, downhill) } else { (104, 32, downhill - 32768) };
    animation.enter(clip, 0.0, clip);
    traversal.goal = state;
    player.yaw = collision::angle_radians(facing);
    player.mode = PlayerMode::Sliding { direction: Vec3::new(heading.sin(), 0.0, heading.cos()), backwards: !forward };
    motion.movement_heading = Some(heading);
    motion.velocity = Vec3::ZERO;
    true
}

/// True completes this tick; false hands the new authored motion to ordinary collision.
fn recover(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    clips: &PlayerAnimations,
    state: usize,
) -> bool {
    motion.velocity = Vec3::ZERO;
    let handoff = match state {
        0 => Some((PlayerMode::Walk, 20)),
        1 => Some((PlayerMode::Run, 6)),
        5 => Some((PlayerMode::HopBack, 88)),
        16 => Some((PlayerMode::Back, 40)),
        21 => Some((PlayerMode::StepRight { stopping: false }, 67)),
        22 => Some((PlayerMode::StepLeft { stopping: false }, 65)),
        _ => None,
    };
    if let Some((next, request)) = handoff {
        player.mode = next;
        motion.locomotion_frame = 0.0;
        motion.velocity = player.forward() * clips.speed(animation).unwrap_or(0.0);
        animation.acknowledge(request);
        // The ordinary collision system consumes this newly authored
        // movement; the animation pass knows it was already advanced.
        return false;
    } else if state == 15 {
        player.mode = PlayerMode::JumpingForward;
    } else if state == 6 || state == 7 {
        player.mode = PlayerMode::Idle;
        motion.turning = if state == 6 { 1 } else { -1 };
        motion.turn_frames = 0.0;
        animation.acknowledge(if state == 6 { 12 } else { 13 });
    } else if animation.animation == STANDING_CLIP || animation.animation == 103 {
        player.mode = PlayerMode::Idle;
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn jump_motion(
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
    let mode = player.mode;
    let state = clips.state(animation.animation).unwrap_or(2);
    // Preparation26 is motionless. Its end jump command launches27; the
    // authored data owns the preparation duration and takeoff frame.
    if motion.grounded && state == 2 {
        player.mode = PlayerMode::Recovering;
        motion.velocity = Vec3::ZERO;
        return true;
    }
    if motion.grounded && !matches!(mode, PlayerMode::JumpingForward) {
        return true;
    }
    if motion.grounded {
        motion.velocity = player.forward() * clips.speed(animation).unwrap_or(0.0);
    }
    let old = player.position;
    let fall_speed = -motion.velocity.y * 1024.0 / ORIGINAL_FPS;
    if !motion.grounded {
        motion.velocity.y -= units(if fall_speed < 128.0 { 6.0 } else { 1.0 } * ORIGINAL_FPS) * original_frames(dt);
    }
    // AnimateLara0x80043248 uses the same verified sine table as the chase
    // camera, with an arithmetic shift for each horizontal coordinate. This
    // changes oblique trajectories by visible whole units over many ticks.
    let heading = motion.movement_heading.unwrap_or(player.yaw);
    let direction = Vec3::new(heading.sin(), 0.0, heading.cos());
    let angle = (heading * 65536.0 / std::f32::consts::TAU).round() as i32;
    let speed = (motion.velocity.dot(direction) * 1024.0 / ORIGINAL_FPS).round() as i32;
    let ticks = original_frames(dt);
    let displacement = Vec3::new(
        units(((engine::camera::chase::sin(angle) * speed) >> 14) as f32 * ticks),
        motion.velocity.y * dt,
        units(((engine::camera::chase::cos(angle) * speed) >> 14) as f32 * ticks),
    );
    let proposed = old + displacement;
    // Forward collision0x8003AB00 refreshes its probe heading after
    // animation motion. Fast-fall0x8003AEEC keeps the rebound heading.
    let collision_yaw = player.yaw + motion.turn_rate as f32 * std::f32::consts::TAU / 65536.0;
    let vertical_jump = matches!(mode, PlayerMode::JumpingUp { .. });
    let collision_heading = if matches!(mode, PlayerMode::FastFalling) {
        heading
    } else if (matches!(mode, PlayerMode::JumpingForward) || vertical_jump) && speed < 0 {
        collision_yaw + std::f32::consts::PI
    } else {
        collision_yaw
    };
    // Jump-up keeps its signed animation movement along yaw, while probing
    // behind Lara for negative speed (0x8003B7D8..0x8003B820).
    let movement_heading = if vertical_jump { collision_yaw } else { collision_heading };
    motion.movement_heading = Some(movement_heading);
    let direction = Vec3::new(movement_heading.sin(), 0.0, movement_heading.cos());
    motion.velocity = (direction * units(speed as f32 * ORIGINAL_FPS)).with_y(motion.velocity.y);
    let (quadrant, forward, delta) = cardinal(player.yaw);
    let hits = front(world, proposed, collision_heading, player.room);
    let forward_jump = matches!(mode, PlayerMode::JumpingForward) && state == 3;
    let reach = matches!(mode, PlayerMode::JumpingForward) && state == 11;
    // Jump-up collision uses870 rather than the ordinary762-unit body
    // probe (0x8003B828). Jump/reach handlers allow192 at the front ceiling
    // (reach0x8003B1C4..0x8003B1D0).
    let height = if vertical_jump { units(870.0) } else { PLAYER_HEIGHT };
    let centre = world.floor_at(proposed, player.room);
    // GetCollisionInfo0x8001B768 sets a ceiling contact at the centre
    // without treating the front as a wall: airborne bad_ceiling is192
    // (0x8003AB40). Deflection0x8003C748 then stops ascent, retaining
    // horizontal speed, heading, clip and goal. See ceiling-evidence.md.
    let ceiling_contact =
        (forward_jump || reach || vertical_jump) && centre.is_some_and(|hit| proposed.y + height >= hit.ceiling);
    // Forward-jump0x8003AB30..0x8003AB44 and reach0x8003B1B4..0x8003B1D0
    // classify the whole body with GetCollisionInfo: any drop, 192 of
    // front-ceiling allowance, and floors up to384 above the feet for a
    // jump but none for reach (bad_neg0: a landing reach meets its front
    // floor as a wall). The shift (0x8001C09C) precedes either deflection.
    let facing = (collision_heading * 65536.0 / std::f32::consts::TAU).round() as i32 as i16 as i32;
    let deflection = (forward_jump || reach).then(|| {
        let limits = collision::Limits {
            radius: 100,
            bad_pos: -collision::NO_HEIGHT,
            bad_neg: if reach { 0 } else { -384 },
            bad_ceiling: 192,
        };
        collision::info(
            world,
            collision::original(proposed),
            collision::original(old),
            player.room,
            762,
            facing,
            &limits,
        )
    });
    let mut blocked = false;
    if let Some(info) = &deflection {
        let shift = collision::level(info.shift);
        player.position.x = proposed.x + shift.x;
        player.position.z = proposed.z + shift.z;
        blocked = matches!(info.contact, Contact::Front | Contact::TopFront);
    }
    if let Some(hits) = hits {
        // A descending sweep can cross its floor without turning that floor
        // into a horizontal wall. This is the same upper-endpoint rule used
        // by ordinary locomotion and the retained standing-jump recording.
        let floor_test = if vertical_jump {
            // Jump-up allows the front floor up to384 above the feet
            // (0x8003B7E4). Once the rising feet clear that margin, the
            // retained2-unit wall speed can move over the ledge.
            proposed.y + units(384.0)
        } else if motion.grounded {
            old.y + STEP_HEIGHT
        } else {
            old.y.max(proposed.y)
        };
        if deflection.is_none() {
            let ceiling_margin = if vertical_jump { units(192.0) } else { 0.0 };
            blocked = hits[0].height > floor_test || hits[0].ceiling + ceiling_margin < proposed.y + height;
            if blocked {
                let (_, collision_forward, _) = cardinal(collision_heading);
                let axis = if collision_forward.x != 0.0 { 0 } else { 2 };
                player.position[axis] = wall_contact(old[axis], proposed[axis]);
            } else {
                player.position.x = proposed.x;
                player.position.z = proposed.z;
            }
        }
        // Edge catch0x8003D2AC compares the interpolated top bound with
        // the front floor and this tick's fall speed. It catches on either
        // direction of crossing, rather than an arbitrary height tolerance.
        let (top, _) = vertical_bounds(&world.data, animation).unwrap_or((-870, 0));
        let gap = ((proposed.y - hits[0].height) * 1024.0).round() as i32 - top;
        let fall = (-motion.velocity.y * 1024.0 / ORIGINAL_FPS).round() as i32;
        let crosses = (gap <= 0 && gap + fall >= 0) || (gap >= 0 && gap + fall <= 0);
        let even = ((hits[1].height - hits[2].height) * 1024.0).abs() < 60.0;
        let ceiling = world.floor_at(proposed, player.room).is_some_and(|h| h.ceiling >= proposed.y + units(384.0));
        let reaching = matches!(mode, PlayerMode::JumpingForward);
        let can_grab = if reaching {
            state == 11 && world.floor_at(proposed, player.room).is_some_and(|h| proposed.y - h.height >= units(200.0))
        } else {
            matches!(mode, PlayerMode::JumpingUp { .. }) && !motion.grounded
        };
        if blocked
            && can_grab
            && weapon.hands_free()
            && pad.held(Button::Cross)
            && delta.abs() <= 6370
            && even
            && ceiling
            && crosses
        {
            // Vertical-jump grab0x8003D3C0 starts96 at+12. Reach grab
            // 0x8003D5BC starts96 at0, retaining speed2/fall1 for that tick.
            *animation = AnimationState::at_frame(96, if reaching { 0.0 } else { 12.0 });
            let (top, _) = vertical_bounds(&world.data, animation).unwrap();
            let ledge = Vec3::new(player.position.x, hits[0].height, player.position.z);
            player.position.y = ledge.y + units(top as f32);
            player.yaw = quadrant as f32 * std::f32::consts::FRAC_PI_2;
            player.mode = PlayerMode::Hanging { ledge };
            motion.velocity =
                if reaching { forward * units(2.0 * ORIGINAL_FPS) - Vec3::Y * units(ORIGINAL_FPS) } else { Vec3::ZERO };
            motion.grounded = !reaching;
            motion.turn_rate = 0;
            traversal.goal = 10;
            return true;
        }
    }
    player.position.y = proposed.y;
    if let Some(info) = &deflection {
        player.position.y += collision::level(info.shift).y;
        deflect(player, motion, info.contact, facing, reach);
    } else if ceiling_contact && !blocked {
        player.position.y = centre.unwrap().ceiling - height;
        if vertical_jump {
            // Type8 requests standing but keeps signed speed and fall speed
            // (0x8003B884..0x8003B8B4); clip28's dispatch owns recovery.
            traversal.goal = 2;
        } else {
            // Reach type8 shares the stop-ascent rule, retaining its clip,
            // goal and horizontal speed (0x8003C870..0x8003C884).
            motion.velocity.y = motion.velocity.y.min(-units(ORIGINAL_FPS));
        }
    }
    if blocked
        && matches!(mode, PlayerMode::JumpingForward)
        && state == 3
        && let Some(floor) = world.floor_at(proposed, player.room)
    {
        // Reach uses0x8003C7F4 and deliberately keeps its speed/pose.
        let clearance = ((proposed.y - floor.height) * 1024.0).round() as i32;
        rebound(player, motion, animation, traversal, clearance, collision_heading, speed);
    }
    if vertical_jump {
        let speed = if blocked || ceiling_contact {
            // Any contact retains signed speed+/-2; zero selects-2
            // (0x8003B89C..0x8003B8B4).
            if speed > 0 { 2 } else { -2 }
        } else if -motion.velocity.y * 1024.0 / ORIGINAL_FPS < -70.0 {
            // Only early, unobstructed ascent accepts directional input.
            // Collision changes speed after this tick's displacement:
            // Up adds1 below5, otherwise Down subtracts2 at or above-4
            // (0x8003B8B8..0x8003B92C).
            if pad.held(Button::Up) && speed < 5 {
                speed + 1
            } else if pad.held(Button::Down) && speed >= -4 {
                speed - 2
            } else {
                speed
            }
        } else {
            speed
        };
        motion.velocity = (player.forward() * units(speed as f32 * ORIGINAL_FPS)).with_y(motion.velocity.y);
    }
    if let Some(hit) = world.floor_at(player.position, player.room) {
        // Water entry is LaraControl's depth test at the start of a tick.
        player.room = super::body_room(world, player.position, hit.room, 381.0);
        if !motion.grounded && motion.velocity.y <= 0.0 && player.position.y <= hit.height {
            landing_damage(player, motion.velocity);
            player.position.y = hit.height;
            motion.velocity.y = 0.0;
            motion.grounded = true;
            motion.air_animation = 0;
            if player.health <= 0 {
                if matches!(mode, PlayerMode::FastFalling) {
                    //0x8003AF54..0x8003AF60 requests death without replacing
                    // the current fast-fall clip on the contact tick.
                    traversal.goal = 8;
                    player.mode = PlayerMode::FatalLanding;
                } else {
                    motion.velocity = Vec3::ZERO;
                }
                return true;
            }
            traversal.goal = if state == 3 && pad.held(Button::Up) && !pad.held(Button::R1) { 1 } else { 2 };
            if matches!(mode, PlayerMode::FastFalling) {
                // Fast-fall collision0x8003AF64 forces heavy landing24
                // on this tick, retaining horizontal speed until control
                // runs next tick. Its40 authored frames own recovery.
                *animation = AnimationState::at_frame(24, 0.0);
                player.mode = PlayerMode::Recovering;
            }
            // The landing collision sets a goal; the next authored step
            // dispatches to31, which holds control through its landing pose.
            if matches!(mode, PlayerMode::JumpingForward) && state == 3 {
                // Forward-jump collision0x8003ABEC calls AnimateLara again
                // on landing. Vertical-jump collision deliberately waits for
                // the next tick. forward_landing.csv distinguishes them.
                let mut a = ObjectAnimation { animation: animation.animation, frame: animation.frame };
                let mut goal = AnimationGoal { state: traversal.goal };
                advance_authored_animation(&world.data, &mut a, &mut goal, ticks);
                *animation = AnimationState::at_frame(a.animation, a.frame);
                traversal.goal = goal.state;
                if player.mode == PlayerMode::JumpImpact {
                    // A pre-launch low impact can settle on this same tick.
                    // AnimateLara writes grounded clip speed, but its state14
                    // override survives until an authored dispatch/end link.
                    motion.velocity = player.forward() * clips.speed(animation).unwrap_or(0.0);
                } else if clips.state(a.animation) == Some(1) {
                    player.mode = PlayerMode::Run;
                    animation.acknowledge(6);
                    let speed = clips.speed(animation).unwrap_or(0.0);
                    motion.velocity = player.forward() * speed;
                    let speed = (speed * 1024.0 / ORIGINAL_FPS).round() as i32;
                    player.position.x += units(((engine::camera::chase::sin(angle) * speed) >> 14) as f32 * ticks);
                    player.position.z += units(((engine::camera::chase::cos(angle) * speed) >> 14) as f32 * ticks);
                } else {
                    player.mode = PlayerMode::Recovering;
                    motion.velocity = Vec3::ZERO;
                }
            }
        }
    }
    if motion.grounded && state == 2 {
        player.mode = PlayerMode::Recovering;
        motion.velocity = Vec3::ZERO;
    }
    true
}
