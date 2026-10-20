//! Forward walking's room probes, authored step clips and collision stops.
//! PSX walk collision0x8003A62C; shared room deflection0x8003C544.
use super::backward::{grid_shift, ground_displacement, probes};
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
    let proposed = old + ground_displacement(motion.velocity, dt);
    let heading = player.yaw + motion.turn_rate as f32 * std::f32::consts::TAU / 65536.0;
    motion.movement_heading = Some(heading);
    let Some(centre) = world.floor_at(proposed, player.room) else { return true };
    let probes = probes(proposed, heading);
    let blocked = |position: Vec3| {
        let Some(hit) = world.floor_at(position, player.room) else { return true };
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
    let axis = if heading.sin().abs() > heading.cos().abs() { 0 } else { 2 };
    player.position = proposed;
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
            if matches!(
                clips.state(previous.animation),
                Some(state::STAND | state::TURN_RIGHT | state::TURN_LEFT | state::FAST_TURN)
            ) {
                *animation = step_animation(&previous, STANDING_CLIP, None, Some(clips), original_frames(dt));
            } else {
                animation.enter(STANDING_CLIP, 0.0, STANDING_CLIP);
            }
        }
    } else {
        for (index, sign) in [(1, 1.0), (2, -1.0)] {
            if blocked(probes[index]) {
                let side = 2 - axis;
                player.position[side] += grid_shift(probes[index][side], probes[0][side]);
                player.yaw += sign * 910.0 * std::f32::consts::TAU / 65536.0;
                break;
            }
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
    true
}
