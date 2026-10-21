//! What the original keeps in every item and the routine that animates
//! one: the fields controllers read and write (position, angles, speeds,
//! state, status) and AnimateItem `0x8001DD98`. Creatures, traps and other
//! moving objects share them.
//!
//! Positions, angles and speeds are the original's integers (world units,
//! y down, 65536 to a turn); `Transform` follows them.
use bevy::prelude::*;
use engine::{
    animation::{AnimationCue, AnimationGoal, ObjectAnimation, step_animation_frame_from},
    camera::chase::{cos, sin},
    world::{Command, Level},
};

/// GetHeight's answer inside a wall or over no floor.
pub const NO_HEIGHT: i32 = -32512;

/// Item status bits 1-2 (`item+0x48`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
pub enum ItemStatus {
    /// Placed, not yet triggered.
    #[default]
    Inactive,
    Active,
    /// Its animation ran a kill command; the AI slot is released.
    Deactivated,
    /// Taken out of the world: a collected pickup.
    Invisible,
}

/// The original item fields a creature's code reads and writes.
#[derive(Component, Reflect, Debug, Clone, Default)]
#[reflect(Component)]
pub struct ItemBody {
    /// World units, y down.
    pub position: [i32; 3],
    pub pitch: i16,
    pub yaw: i16,
    pub roll: i16,
    pub room: usize,
    /// Navigation box under the creature, refreshed by [`ai_info`].
    pub box_number: i16,
    pub speed: i16,
    pub fall_speed: i16,
    /// Floor height under it, from the last movement.
    pub floor: i32,
    /// The state it is in (`item+0x0E`). Usually its clip's, but a
    /// controller may set it apart (a death it is about to play).
    pub current_state: usize,
    /// A state the controller wants played once (`item+0x12`), or 0.
    pub required_state: usize,
    pub gravity: bool,
    /// Shot since its last animation step (status bit 0x10).
    pub hit: bool,
    /// Which of its meshes touched Lara in her last collision pass.
    pub touch_bits: u32,
    pub status: ItemStatus,
    /// Lara collides with it (status bit 0x20).
    pub collidable: bool,
    /// Taken out of the level (KillItem `0x8003749C`): no longer drawn.
    pub removed: bool,
    /// When it last joined its room's item list (see [`RoomOrder`]).
    pub room_entry: u64,
}

/// The order items joined their rooms' item lists. The original keeps a
/// list per room; an item is pushed onto its head when placed (in placement
/// order) and when it changes room (ItemNewRoom). CreatureCollided walks a
/// room's list from the head and stops at itself, so a creature only walks
/// into those that joined the room after it did.
#[derive(Resource, Debug, Default)]
pub struct RoomOrder {
    next: u64,
}

impl RoomOrder {
    /// Move `body` into `room`, at the head of its list if it changed room.
    pub fn enter(&mut self, body: &mut ItemBody, room: usize) {
        if body.room != room {
            body.room = room;
            body.room_entry = self.next;
            self.next += 1;
        }
    }

    /// Placements take entries up to their index; later moves come after.
    pub(crate) fn after_placements(&mut self, count: usize) {
        self.next = self.next.max(count as u64);
    }
}

/// AnimateItem `0x8001DD98`: one step of the item's clip, its end-of-clip
/// commands, then its speed (from the clip, or gravity while falling) moves
/// it along its heading. Returns the step's cues for sound and effects.
pub fn animate_item(
    level: &Level,
    body: &mut ItemBody,
    animation: &mut ObjectAnimation,
    goal: &mut AnimationGoal,
) -> Vec<AnimationCue> {
    body.touch_bits = 0;
    body.hit = false;
    let step = step_animation_frame_from(level, animation, goal, &mut body.current_state);
    if (step.changed || step.ended) && body.required_state == body.current_state {
        body.required_state = 0;
    }
    for cue in step.cues.iter().filter(|c| c.at_end) {
        match cue.command {
            Command::SetPosition([dx, dy, dz]) => {
                let yaw = i32::from(body.yaw);
                body.position[0] += (cos(yaw) * dx + sin(yaw) * dz) >> 14;
                body.position[1] += dy;
                body.position[2] += (cos(yaw) * dz - sin(yaw) * dx) >> 14;
            }
            Command::JumpVelocity([vertical, horizontal]) => {
                body.fall_speed = vertical as i16;
                body.speed = horizontal as i16;
                body.gravity = true;
            }
            Command::Kill => body.status = ItemStatus::Deactivated,
            _ => {}
        }
    }
    if let Some(clip) = level.animations.get(animation.animation) {
        if body.gravity {
            body.fall_speed += if body.fall_speed < 128 { 6 } else { 1 };
            body.position[1] += i32::from(body.fall_speed);
        } else {
            let frame = animation.frame.floor() as i32;
            body.speed = (clip.speed.wrapping_add(clip.accel.wrapping_mul(frame)) >> 16) as i16;
        }
    }
    let yaw = i32::from(body.yaw);
    body.position[0] += (sin(yaw) * i32::from(body.speed)) >> 14;
    body.position[2] += (cos(yaw) * i32::from(body.speed)) >> 14;
    step.cues
}
