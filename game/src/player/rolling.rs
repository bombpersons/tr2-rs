//! Authored grounded roll, including its turn effect and room collision.
use super::backward::{grid_shift, probes};
use super::*;
use engine::animation::{AnimationGoal, ObjectAnimation, advance_authored_animation};
use engine::world::Command;

#[allow(clippy::too_many_arguments)]
pub(super) fn step(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: Option<&WorldGeometry>,
    clips: Option<&PlayerAnimations>,
    dt: f32,
) -> bool {
    let PlayerMode::Rolling { entering } = player.mode else { return false };
    let (Some(world), Some(clips)) = (world, clips) else { return false };
    let mut authored = if entering {
        // Standing control0x800389F0..0x80038A1C enters146 at+2;
        // this same tick's animation step advances it to+3.
        traversal.goal = 2;
        ObjectAnimation { animation: 146, frame: 2.0 }
    } else {
        ObjectAnimation { animation: animation.animation, frame: animation.frame }
    };
    let mut goal = AnimationGoal { state: traversal.goal };
    let cues = advance_authored_animation(&world.data, &mut authored, &mut goal, original_frames(dt));
    *animation = AnimationState::at_frame(authored.animation, authored.frame);
    traversal.animation_stepped = true;
    let heading = motion.movement_heading.unwrap_or(player.yaw);
    for cue in cues {
        if matches!(cue.command, Command::Effect { effect: 0, .. }) {
            // Authored effect0 at147/frame3903 turns yaw180 degrees.
            // Movement still uses the preceding collision heading this tick.
            player.yaw = (player.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU);
            motion.pitch = -motion.pitch; // Same effect0x8002A03C..0x8002A050.
        }
    }
    let state = clips.state(animation.animation).unwrap_or(2);
    let speed = clips.speed(animation).unwrap_or(0.0);
    let speed_units = (speed * 1024.0 / ORIGINAL_FPS).round() as i32;
    let angle = (heading * 65536.0 / std::f32::consts::TAU).round() as i32;
    let proposed = player.position
        + Vec3::new(
            units(((engine::camera::chase::sin(angle) * speed_units) >> 14) as f32),
            0.0,
            units(((engine::camera::chase::cos(angle) * speed_units) >> 14) as f32),
        ) * original_frames(dt);
    let collision_heading = player.yaw
        + motion.turn_rate as f32 * std::f32::consts::TAU / 65536.0
        + if state == 23 { std::f32::consts::PI } else { 0.0 };
    // Roll first-half collision0x8003BC50 follows yaw; second-half
    // 0x8003BD14 follows yaw+180, continuing the same physical direction.
    motion.movement_heading = Some(collision_heading);
    motion.velocity = Vec3::new(collision_heading.sin(), 0.0, collision_heading.cos()) * speed;
    if state == 2 {
        player.mode = PlayerMode::Recovering;
        motion.velocity = Vec3::ZERO;
        return true;
    }
    player.mode = PlayerMode::Rolling { entering: false };
    let old = player.position;
    let Some(centre) = world.floor_at(proposed, player.room) else { return true };
    let probes = probes(proposed, collision_heading);
    let blocked = |point: Vec3| {
        let Some(hit) = world.floor_at(point, player.room) else { return true };
        // Roll permits drops, but rejects steps above384 and any front head
        // obstruction (0x8003BC8C..0x8003BCA8 and0x8003BD58..0x8003BD74).
        hit.height > old.y + units(384.0) || hit.ceiling < old.y + PLAYER_HEIGHT
    };
    player.position = proposed;
    let axis = if collision_heading.sin().abs() > collision_heading.cos().abs() { 0 } else { 2 };
    if blocked(probes[0]) {
        // ShiftItem corrects the collision without changing authored speed,
        // clip, state or goal. The wall cannot cancel a committed roll.
        player.position = old;
        player.position[axis] = proposed[axis] + grid_shift(probes[0][axis], proposed[axis]);
    } else {
        for probe in &probes[1..] {
            if blocked(*probe) {
                let side = 2 - axis;
                player.position[side] += grid_shift(probe[side], probes[0][side]);
                break;
            }
        }
    }
    let drop = (old.y - centre.height) * 1024.0;
    if (state == 45 && drop > 384.0) || (state == 23 && drop > 200.0) {
        // The two collision handlers enter their existing forward/backward
        // fall controllers, seeding gravity without adding it on this tick.
        let (clip, mode, goal) =
            if state == 45 { (34, PlayerMode::JumpingForward, 3) } else { (93, PlayerMode::FallingBack, 29) };
        animation.enter(clip, 0.0, clip);
        player.mode = mode;
        traversal.goal = goal;
        motion.grounded = false;
        return true;
    }
    player.position.y = centre.height;
    player.room = world.floor_at(player.position, player.room).map_or(player.room, |hit| hit.room);
    true
}
