//! Shallow-water contact and the authored wading gait. PSX LaraControl
//! 0x800424F0, state65 0x80039D24, collision0x8003C028. Water contact is
//! independent of locomotion: standing, turning and backing remain possible.
use super::*;
use collision::{Contact, Limits, original};
use engine::{
    animation::advance,
    camera::chase::{cos, sin},
};

#[derive(Component, Reflect, Debug, Clone, Copy, Default)]
#[reflect(Component)]
pub struct Wading {
    pub active: bool,
}

#[derive(Reflect, serde::Deserialize, Debug, Clone)]
pub struct WadingTuning {
    /// LaraControl's feet-to-surface thresholds, original world units.
    pub leave_depth: i32,
    pub surface_depth: i32,
    /// State65's bank push/cap and the ground wrapper's recovery, binary angles.
    pub bank_push: i32,
    pub bank_limit: i32,
    pub bank_drag: i32,
    /// Mode4 and state65 ask for this chase elevation (0x80042790/39D40).
    pub camera_elevation: i32,
}

/// Mode4's transition before state control (0x8004278C). Retain the wading
/// gait until its authored dispatch accepts running after leaving the water.
#[allow(clippy::too_many_arguments)]
pub(super) fn transitions(
    player: &mut Player,
    motion: &mut Motion,
    water: &mut SubmergedMotion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    wading: &mut Wading,
    world: &WorldGeometry,
    clips: Option<&PlayerAnimations>,
    tuning: &WadingTuning,
) {
    if !wading.active {
        return;
    }
    motion.camera_elevation = Some(swimming::radians(tuning.camera_elevation));
    let below =
        world.water_surface(player.position, player.room).map(|h| ((h - player.position.y) * 1024.0).round() as i32);
    match below {
        None => wading.active = false,
        Some(depth) if depth < tuning.leave_depth => wading.active = false,
        Some(depth) if depth >= tuning.surface_depth => {
            let state = clips.and_then(|c| c.state(animation.animation));
            let (clip, goal) = match state {
                Some(16) => (140, 47),
                Some(21) => (144, 49),
                Some(22) => (143, 48),
                _ => (116, 34),
            };
            player.position.y += units((depth - 1) as f32);
            player.mode = PlayerMode::Surface;
            animation.enter(clip, 0.0, clip);
            traversal.goal = goal;
            water.speed = 0.0;
            water.bank = 0.0;
            water.dive_hold = 0;
            motion.pitch = 0.0;
            motion.grounded = false;
            player.room = body_room(world, player.position, player.room, 381.0);
            wading.active = false;
        }
        _ => {}
    }
}

/// Submerged shallow-water test0x80045BA4: stand up in water <=512 deep.
#[allow(clippy::too_many_arguments)]
pub(super) fn stand_up(
    player: &mut Player,
    motion: &mut Motion,
    water: &mut SubmergedMotion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    wading: &mut Wading,
    world: &WorldGeometry,
) {
    let (Some(surface), Some(floor)) =
        (world.water_surface(player.position, player.room), world.floor_at(player.position, player.room))
    else {
        return;
    };
    if ((surface - floor.height) * 1024.0).round() as i32 > 512 {
        return;
    }
    player.position.y = floor.height;
    player.mode = PlayerMode::PullingUp;
    animation.enter(192, 0.0, 192);
    traversal.goal = 2;
    wading.active = true;
    motion.grounded = true;
    motion.pitch = 0.0;
    motion.velocity = Vec3::ZERO;
    water.speed = 0.0;
    water.bank = 0.0;
}

/// Wading ground handlers. These keep their requested goal until the level's
/// authored ranges accept it; neither releasing Up nor re-holding it cuts a stride.
#[allow(clippy::too_many_arguments)]
pub(super) fn control(
    player: &mut Player,
    motion: &mut Motion,
    water: &mut SubmergedMotion,
    animation: &AnimationState,
    traversal: &mut Traversal,
    wading: &Wading,
    pad: &ControllerState,
    clips: Option<&PlayerAnimations>,
    tuning: &SwimmingTuning,
) -> bool {
    let Some(state) = clips.and_then(|c| c.state(animation.animation)) else {
        return false;
    };
    if !(wading.active || player.mode == PlayerMode::Wade)
        || !matches!(state, 0 | 1 | 2 | 6 | 7 | 16 | 20 | 21 | 22 | 55 | 65)
    {
        return false;
    }
    if player.health <= 0 || matches!(player.mode, PlayerMode::Climbing { .. } | PlayerMode::Dead) {
        return false;
    }
    if state == 2 && pad.held(Button::Square) {
        return false;
    }
    let controls = GroundControls::read(pad);
    let forward = controls.direction.y;
    let turn = if controls.direction.x < -0.1 {
        -1
    } else if controls.direction.x > 0.1 {
        1
    } else {
        0
    };
    let walking = controls.walk;
    let gait = if wading.active {
        65
    } else if walking {
        0
    } else {
        1
    };
    let push = |motion: &mut Motion, cap: i32| {
        if turn != 0 {
            motion.turn_rate = (motion.turn_rate + turn * TURN_PUSH).clamp(-cap, cap);
        }
    };
    match state {
        65 => {
            motion.camera_elevation = Some(swimming::radians(tuning.wading.camera_elevation));
            if player.health > 0 {
                push(motion, 1456);
                let bank = (swimming::angle(water.bank) + turn * tuning.wading.bank_push)
                    .clamp(-tuning.wading.bank_limit, tuning.wading.bank_limit);
                water.bank = swimming::radians(bank);
            }
            traversal.goal = if player.health > 0 && forward > 0.0 { if wading.active { 65 } else { 1 } } else { 2 };
        }
        0 | 1 => {
            push(motion, if state == 0 { 728 } else { 1456 });
            traversal.goal = if forward > 0.0 { gait } else { 2 };
        }
        2 => {
            traversal.goal = if controls.side < 0.0 {
                22
            } else if controls.side > 0.0 {
                21
            } else if turn < 0 {
                7
            } else if turn > 0 {
                6
            } else {
                2
            };
            if forward > 0.0 {
                traversal.goal = gait;
                // Stop in mode4 delegates ordinary Up to walk control;
                // Up+Walk instead delegates to state65 (0x80038ABC).
                push(motion, if walking { 1456 } else { 728 });
                if walking {
                    water.bank = swimming::radians(
                        (swimming::angle(water.bank) + turn * tuning.wading.bank_push)
                            .clamp(-tuning.wading.bank_limit, tuning.wading.bank_limit),
                    );
                }
            } else if forward < 0.0 {
                traversal.goal = 16;
                push(motion, 728);
            }
        }
        6 | 7 | 20 => {
            let sign = if state == 7 || (state == 20 && motion.turn_rate < 0) { -1 } else { 1 };
            if state == 20 {
                motion.turn_rate = if motion.turn_rate < 0 { -1456 } else { 1456 };
            } else {
                motion.turn_rate += sign * TURN_PUSH;
                if walking && !motion.guns_ready {
                    motion.turn_rate = motion.turn_rate.clamp(-728, 728);
                } else if motion.guns_ready || motion.turn_rate.abs() > 728 {
                    traversal.goal = 20;
                }
            }
            if state != 20 && forward > 0.0 {
                traversal.goal = gait;
            } else if turn != sign {
                traversal.goal = 2;
            }
        }
        16 => {
            push(motion, 728);
            if forward >= 0.0 || (!wading.active && !walking) {
                traversal.goal = 2;
            }
        }
        21 | 22 => {
            push(motion, 728);
            if (state == 21 && controls.side <= 0.0) || (state == 22 && controls.side >= 0.0) {
                traversal.goal = 2;
            }
        }
        _ => {}
    }
    motion.turn_rate = swimming::decay(motion.turn_rate, TURN_DECAY);
    water.bank = swimming::radians(swimming::decay(swimming::angle(water.bank), tuning.wading.bank_drag));
    water.air_ticks = tuning.max_air_ticks;
    motion.oxygen_seconds = water.air_ticks as f32 / ORIGINAL_FPS;
    true
}

/// Animate, move along the previous collision heading, and resolve the new
/// heading against room probes. State65 lowers by at most50 units per tick.
#[allow(clippy::too_many_arguments)]
pub(super) fn step(
    player: &mut Player,
    motion: &mut Motion,
    water: &mut SubmergedMotion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    wading: &Wading,
    world: Option<&WorldGeometry>,
    clips: Option<&PlayerAnimations>,
    dt: f32,
) -> bool {
    let (Some(world), Some(clips)) = (world, clips) else {
        return false;
    };
    if matches!(player.mode, PlayerMode::Climbing { .. } | PlayerMode::Dead) {
        return false;
    }
    let prior = clips.state(animation.animation);
    if !(wading.active || player.mode == PlayerMode::Wade)
        || !matches!(prior, Some(0 | 1 | 2 | 6 | 7 | 16 | 20 | 21 | 22 | 55 | 65))
    {
        return false;
    }
    let previous = animation.clone();
    if prior != Some(traversal.goal)
        && let Some((clip, frame)) = clips.dispatch(animation, traversal.goal)
    {
        animation.enter(clip, frame, clip);
    } else {
        advance(animation, Some(clips), original_frames(dt));
    }
    traversal.animation_stepped = true;
    let state = clips.state(animation.animation).unwrap_or(2);
    let speed = clips.speed(animation).unwrap_or(0.0);
    let heading = swimming::angle(motion.movement_heading.unwrap_or(player.yaw));
    let n = (speed * 1024.0 / ORIGINAL_FPS).round() as i32;
    let old = original(player.position);
    let mut at = old + IVec3::new((sin(heading) * n) >> 14, 0, (cos(heading) * n) >> 14);
    let yaw = swimming::angle(player.yaw) + motion.turn_rate;
    let facing = (yaw
        + match state {
            16 => 32768,
            21 => 16384,
            22 => -16384,
            _ => 0,
        }) as i16 as i32;
    motion.movement_heading = Some(swimming::radians(facing));
    // State55's collision0x8003BFF0/3C208 only queries the body. It
    // neither shifts nor snaps to the floor during the stand-up clip.
    if state == 55 {
        player.position = collision::level(at);
        player.room = body_room(world, player.position, player.room, 381.0);
        motion.velocity = Vec3::ZERO;
        motion.grounded = true;
        player.mode = PlayerMode::PullingUp;
        return true;
    }
    let limits = Limits {
        radius: 100,
        bad_pos: if state == 65 || (wading.active && matches!(state, 16 | 21 | 22)) { 32512 } else { 384 },
        bad_neg: if matches!(state, 21 | 22) { -128 } else { -384 },
        bad_ceiling: 0,
    };
    let info = collision::info(world, at, old, player.room, 762, facing, &limits);
    at += info.shift;
    motion.velocity = Vec3::new(swimming::radians(heading).sin(), 0.0, swimming::radians(heading).cos()) * speed;
    match info.contact {
        Contact::Front | Contact::TopFront if state == 65 => {
            water.bank = 0.0;
            motion.velocity = Vec3::ZERO;
            traversal.goal = 2;
            if matches!(prior, Some(2 | 6 | 7 | 20)) {
                *animation = step_animation(&previous, STANDING_CLIP, None, Some(clips), original_frames(dt));
            } else {
                animation.enter(STANDING_CLIP, 0.0, STANDING_CLIP);
            }
        }
        Contact::Left if state == 65 => player.yaw += swimming::radians(910),
        Contact::Right if state == 65 => player.yaw -= swimming::radians(910),
        Contact::Clamp | Contact::Top | Contact::TopFront => {
            at = old;
            motion.velocity = Vec3::ZERO;
        }
        _ => {}
    }
    if info.mid.floor != collision::NO_HEIGHT {
        at.y += if state == 65 { info.mid.floor.min(50) } else { info.mid.floor };
    }
    player.position = collision::level(at);
    player.room = body_room(world, player.position, player.room, 381.0);
    motion.grounded = true;
    player.mode = match clips.state(animation.animation) {
        Some(65) => PlayerMode::Wade,
        Some(0) => PlayerMode::Walk,
        Some(1) => PlayerMode::Run,
        Some(16) => PlayerMode::Back,
        Some(21) => PlayerMode::StepRight { stopping: traversal.goal == 2 },
        Some(22) => PlayerMode::StepLeft { stopping: traversal.goal == 2 },
        Some(55) => PlayerMode::PullingUp,
        _ => PlayerMode::Idle,
    };
    true
}
