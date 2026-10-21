//! Pickups, switches, keyholes and puzzle holes: the original's collision
//! routines that start Lara's use clips, the switch's control, and the
//! status the switch, key and pickup triggers test.
//!
//! The collision routines run from Lara's object collision (0x8001C258)
//! for items within 4095 units on each axis. Each tests Lara's offset in
//! the item's frame against a bounds table in RAM (TestLaraPosition
//! 0x8001CE04), lines her up (AlignLaraPosition 0x8001D070) and starts
//! the clip. Evidence: `docs/interaction-evidence.md`.

use bevy::prelude::*;
use engine::{
    animation::{AnimationGoal, ObjectAnimation, step_animation_frame_from},
    camera::chase,
    controller::{Button, ControllerState},
    objects::{CODE_BITS, WorldObject},
    world::{Command, WorldGeometry},
};

use super::{InteractionProgress, Inventory, Keyhole, Pickup, Switch};
use crate::creature::ItemStatus;
use crate::player::{
    GunStatus, Motion, Player, PlayerAnimations, PlayerMode, SubmergedMotion, Traversal, WeaponState,
    contract::{clip, frame, state},
    original_units, using,
};

/// What the original keeps in an item for the use routines: its status
/// (`item+0x48` bits 1-2) and, for switches, its current animation state
/// (`item+0x0E`). The timer its control counts (`item+0x26`) is the
/// object's ([`WorldObject::timer`]).
#[derive(Component, Reflect, Debug, Clone, Default)]
#[reflect(Component)]
pub struct UsableItem {
    pub status: ItemStatus,
    pub state: usize,
}

/// A box in the item's frame (y down) and the angle window Lara's
/// rotation may differ from the item's by (pitch, yaw, roll).
#[derive(Debug, Clone, Copy)]
pub struct UseBounds {
    pub min: IVec3,
    pub max: IVec3,
    pub angle_min: [i16; 3],
    pub angle_max: [i16; 3],
}

/// Pickups on land and wading (0x800896D8). Lara's yaw stands in for the
/// item's, so only the pitch window matters.
pub const PICKUP_BOUNDS: UseBounds = UseBounds {
    min: IVec3::new(-256, -100, -256),
    max: IVec3::new(256, 100, 256),
    angle_min: [-1820, 0, 0],
    angle_max: [1820, 0, 0],
};
/// Where Lara stands for a pickup, in the item's frame (0x800896F0).
pub const PICKUP_STAND: IVec3 = IVec3::new(0, 0, -100);

/// Pickups underwater (0x80089720): a cube around the item, with every
/// angle within 8190 of the item's.
pub const UNDERWATER_PICKUP_BOUNDS: UseBounds =
    UseBounds { min: IVec3::splat(-512), max: IVec3::splat(512), angle_min: [-8190; 3], angle_max: [8190; 3] };
/// Where Lara swims to for an underwater pickup, in the tilted item's
/// frame (0x80089738): above it and behind.
pub const UNDERWATER_PICKUP_STAND: IVec3 = IVec3::new(0, -200, -350);
/// The pitch PickUpCollision gives an item underwater, nose down.
const UNDERWATER_PICKUP_PITCH: i16 = -4550;

/// Wall switches (0x80089744): level with the switch, in front of it.
pub const SWITCH_BOUNDS: UseBounds = UseBounds {
    min: IVec3::new(-220, 0, 292),
    max: IVec3::new(220, 0, 512),
    angle_min: [-1820, -5460, -1820],
    angle_max: [1820, 5460, 1820],
};

/// Underwater switches (0x80089780): a box around the switch, reaching
/// further in front than behind, with every angle within 14560.
pub const UNDERWATER_SWITCH_BOUNDS: UseBounds = UseBounds {
    min: IVec3::splat(-1024),
    max: IVec3::new(1024, 1024, 512),
    angle_min: [-14560; 3],
    angle_max: [14560; 3],
};
/// Where Lara swims to at an underwater switch, in its frame (0x80089798).
pub const UNDERWATER_SWITCH_STAND: IVec3 = IVec3::new(0, 0, 108);

/// Keyholes (0x800897A4) and puzzle holes (0x800897C8).
pub const KEYHOLE_BOUNDS: UseBounds = UseBounds {
    min: IVec3::new(-200, 0, 312),
    max: IVec3::new(200, 0, 512),
    angle_min: [-1820, -5460, -1820],
    angle_max: [1820, 5460, 1820],
};
/// Where Lara stands at a keyhole (0x800897BC).
pub const KEYHOLE_STAND: IVec3 = IVec3::new(0, 0, 362);
/// Where Lara stands at a puzzle hole (0x800897E0), a little nearer.
pub const PUZZLE_STAND: IVec3 = IVec3::new(0, 0, 327);

/// An item's or Lara's rotation: pitch, yaw, roll (65536 per turn).
pub type Angles = [i16; 3];

/// phd_RotYXZ 0x80064338 applied to a unit matrix: rows scaled by 16384,
/// rounded after each turn as the original does, which skips a zero angle.
fn rotation([pitch, yaw, roll]: Angles) -> [[i32; 3]; 3] {
    let mut m = [[16384, 0, 0], [0, 16384, 0], [0, 0, 16384]];
    // Each turn mixes two columns of every row.
    let mut turn = |angle: i16, a: usize, b: usize, sign: i32| {
        if angle == 0 {
            return;
        }
        let (s, c) = (chase::sin(i32::from(angle)), chase::cos(i32::from(angle)));
        for row in &mut m {
            let (x, y) = (row[a], row[b]);
            row[a] = (c * x - sign * s * y) >> 14;
            row[b] = (c * y + sign * s * x) >> 14;
        }
    };
    turn(yaw, 0, 2, 1);
    turn(pitch, 1, 2, -1);
    turn(roll, 0, 1, -1);
    m
}

/// Lara's offset from an item, in the item's frame: TestLaraPosition's
/// transposed rotation (0x8001CE04).
fn local(item: IVec3, angles: Angles, lara: IVec3) -> IVec3 {
    let m = rotation(angles);
    let d = (lara - item).to_array();
    let column = |c: usize| (m[0][c] * d[0] + m[1][c] * d[1] + m[2][c] * d[2]) >> 14;
    IVec3::new(column(0), column(1), column(2))
}

/// A point given in an item's frame, in the world.
fn world_point(item: IVec3, angles: Angles, offset: IVec3) -> IVec3 {
    let m = rotation(angles);
    let o = offset.to_array();
    let row = |r: usize| (m[r][0] * o[0] + m[r][1] * o[1] + m[r][2] * o[2]) >> 14;
    item + IVec3::new(row(0), row(1), row(2))
}

/// TestLaraPosition 0x8001CE04 for an item turned only about y.
pub fn in_bounds(bounds: &UseBounds, item: IVec3, item_yaw: i16, lara: IVec3, lara_yaw: i16) -> bool {
    in_bounds_turned(bounds, item, [0, item_yaw, 0], lara, [0, lara_yaw, 0])
}

/// TestLaraPosition 0x8001CE04: each angle's difference first, then the box.
pub fn in_bounds_turned(
    bounds: &UseBounds,
    item: IVec3,
    item_angles: Angles,
    lara: IVec3,
    lara_angles: Angles,
) -> bool {
    for axis in 0..3 {
        let difference = lara_angles[axis].wrapping_sub(item_angles[axis]);
        if difference < bounds.angle_min[axis] || difference > bounds.angle_max[axis] {
            return false;
        }
    }
    let p = local(item, item_angles, lara);
    p.cmpge(bounds.min).all() && p.cmple(bounds.max).all()
}

/// phd_sqrt 0x80063E84: the integer square root, rounded down.
fn square_root(mut n: i32) -> i32 {
    let (mut bit, mut root) = (1 << 30, 0);
    while bit != 0 {
        if root + bit <= n {
            n -= root + bit;
            root = (root >> 1) | bit;
        } else {
            root >>= 1;
        }
        bit >>= 2;
    }
    root
}

/// MoveLaraPosition 0x8001D278 for anything but a flare: one tick of Lara
/// swimming to `offset` in the item's frame and turning to the item's
/// angles (Move3DPosTo3DPos 0x8001D4D4: 16 units, and 364 on each angle).
/// True once she is there.
fn move_lara(lara: &mut IVec3, lara_angles: &mut Angles, item: IVec3, item_angles: Angles, offset: IVec3) -> bool {
    let target = world_point(item, item_angles, offset);
    let d = target - *lara;
    let distance = square_root(d.length_squared());
    *lara = if distance > 16 { *lara + d * 16 / distance } else { target };
    for (angle, goal) in lara_angles.iter_mut().zip(item_angles) {
        let difference = goal.wrapping_sub(*angle);
        *angle = if difference > 364 {
            angle.wrapping_add(364)
        } else if difference < -364 {
            angle.wrapping_sub(364)
        } else {
            goal
        };
    }
    *lara == target && *lara_angles == item_angles
}

/// AlignLaraPosition 0x8001D070: face the item and stand at `offset` in its
/// frame, unless that spot's floor is more than a step from Lara's height
/// or its ceiling is too low. Returns Lara's new position.
pub fn align(world: Option<&WorldGeometry>, player: &mut Player, item: IVec3, item_yaw: i16, offset: IVec3) {
    player.yaw = engine::world::angle(item_yaw as u16);
    let target = world_point(item, [0, item_yaw, 0], offset);
    let lara = original_units(player.position);
    let Some(hit) = world.and_then(|w| w.floor_at(level(target), player.room)) else { return };
    let floor = original_units(Vec3::Y * hit.height).y;
    let ceiling = original_units(Vec3::Y * hit.ceiling).y;
    if (floor - lara.y).abs() <= 256 && (ceiling - lara.y).abs() > 761 {
        player.position = level(target);
    }
}

fn level(p: IVec3) -> Vec3 {
    engine::world::position(p.to_array())
}

/// An item's angles as its transform: the same turns as Lara's body.
fn item_rotation([pitch, yaw, roll]: Angles) -> Quat {
    let radians = |a: i16| crate::player::binary_radians(a);
    Quat::from_rotation_y(radians(yaw)) * Quat::from_rotation_x(-radians(pitch)) * Quat::from_rotation_z(-radians(roll))
}

fn binary_yaw(radians: f32) -> i16 {
    (radians * 65536.0 / std::f32::consts::TAU).round() as i32 as i16
}

/// Lara can start a use: alive, on the ground (status bit 8 clear),
/// standing (state 2), hands free.
fn ready(player: &Player, motion: &Motion, weapon: &WeaponState, lara_state: Option<usize>) -> bool {
    player.health > 0 && motion.grounded && weapon.hands_free() && lara_state == Some(state::STAND)
}

/// The use collision routines, for pickups and switches.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn collide(
    pad: Res<ControllerState>,
    world: Option<Res<WorldGeometry>>,
    clips: Option<Res<PlayerAnimations>>,
    mut inventory: ResMut<Inventory>,
    mut progress: ResMut<InteractionProgress>,
    mut players: Query<(
        &mut Player,
        &mut Motion,
        &mut engine::animation::AnimationState,
        &mut WeaponState,
        &mut Traversal,
        &mut SubmergedMotion,
    )>,
    mut objects: Query<(
        &mut WorldObject,
        &mut Transform,
        &mut UsableItem,
        Option<&Pickup>,
        Option<&Switch>,
        Option<&mut ObjectAnimation>,
        Option<&mut AnimationGoal>,
    )>,
) {
    let Ok((mut player, mut motion, mut animation, mut weapon, mut traversal, mut submerged)) = players.single_mut()
    else {
        return;
    };
    let action = pad.held(Button::Cross);
    let world = world.as_deref();
    let clips = clips.as_deref();
    for (mut object, mut transform, mut item, pickup, switch, item_animation, goal) in &mut objects {
        if item.status == ItemStatus::Invisible || player.health <= 0 {
            continue;
        }
        let at = original_units(transform.translation);
        let lara = original_units(player.position);
        if ((lara - at).abs().cmpgt(IVec3::splat(4095))).any() {
            continue;
        }
        let lara_state = clips.and_then(|c| c.state(animation.animation));
        let lara_yaw = binary_yaw(player.yaw);
        if let Some(pickup) = pickup {
            let mut take = |object: &mut WorldObject, item: &mut UsableItem| {
                item.status = ItemStatus::Invisible;
                object.consumed = true;
                inventory.add(object.kind);
                progress.pickups += 1;
                if pickup.secret.is_some() {
                    progress.secret_mask |= u32::from(inventory.secret_mask);
                    progress.secrets = progress.secret_mask.count_ones();
                }
                progress.message = pickup.message.clone();
            };
            if player.mode == PlayerMode::Swimming {
                // PickUpCollision 0x800502D4 underwater: the item is tilted
                // nose down and turned to Lara's heading for the test. Lara
                // swims to it a step a tick while Action is held, treading
                // and hands free; once there she picks it up from treading
                // and treads again after. Her hands are never marked busy.
                let item_angles = [UNDERWATER_PICKUP_PITCH, lara_yaw, 0];
                let mut lara_angles =
                    [crate::player::binary_angle(motion.pitch), lara_yaw, crate::player::binary_angle(submerged.bank)];
                if !in_bounds_turned(&UNDERWATER_PICKUP_BOUNDS, at, item_angles, lara, lara_angles) {
                    continue;
                }
                if lara_state == Some(state::PICKUP) {
                    if animation.animation == clip::UNDERWATER_PICKUP
                        && animation.frame == frame::UNDERWATER_PICKUP_TAKE
                    {
                        take(&mut object, &mut item);
                    }
                } else if action && lara_state == Some(state::TREAD) && weapon.hands_free() {
                    let mut at_lara = lara;
                    let there = move_lara(&mut at_lara, &mut lara_angles, at, item_angles, UNDERWATER_PICKUP_STAND);
                    player.position = level(at_lara);
                    motion.pitch = crate::player::binary_radians(lara_angles[0]);
                    player.yaw = crate::player::binary_radians(lara_angles[1]);
                    submerged.bank = crate::player::binary_radians(lara_angles[2]);
                    if there {
                        // Treading's range for 39 covers its whole clip.
                        animation.enter(clip::UNDERWATER_PICKUP, 0.0, clip::UNDERWATER_PICKUP);
                        traversal.goal = state::TREAD;
                    }
                } else {
                    continue;
                }
                // Only the routine's fall-through restores the item's angles.
                transform.rotation = item_rotation(item_angles);
                continue;
            }
            // On land and wading.
            if player.mode == PlayerMode::Surface {
                continue;
            }
            if !in_bounds(&PICKUP_BOUNDS, at, lara_yaw, lara, lara_yaw) {
                continue;
            }
            if lara_state == Some(state::PICKUP) {
                if animation.animation == clip::PICKUP && animation.frame == frame::PICKUP_TAKE {
                    take(&mut object, &mut item);
                }
            } else if action && ready(&player, &motion, &weapon, lara_state) {
                // The routine turns the item to Lara's heading (level, no
                // roll) for its tests and, once she starts, returns without
                // turning it back, so it keeps her heading.
                transform.rotation = Quat::from_rotation_y(player.yaw);
                align(world, &mut player, at, lara_yaw, PICKUP_STAND);
                using::start(&mut player, &mut motion, &mut animation, &mut weapon, clip::PICKUP);
            }
        } else if let Some(switch) = switch {
            let yaw = binary_yaw(transform.rotation.to_euler(EulerRot::YXZ).0);
            if switch.underwater {
                // SwitchCollision2 0x80050AC4: while Action is held, Lara
                // treading with free hands swims to the switch a step a tick;
                // once there she pulls it from treading and treads again.
                if !action
                    || item.status != ItemStatus::Inactive
                    || player.mode != PlayerMode::Swimming
                    || !weapon.hands_free()
                    || lara_state != Some(state::TREAD)
                {
                    continue;
                }
                let item_angles = [0, yaw, 0];
                let mut lara_angles =
                    [crate::player::binary_angle(motion.pitch), lara_yaw, crate::player::binary_angle(submerged.bank)];
                if !in_bounds_turned(&UNDERWATER_SWITCH_BOUNDS, at, item_angles, lara, lara_angles) || item.state >= 2 {
                    continue;
                }
                let mut at_lara = lara;
                let there = move_lara(&mut at_lara, &mut lara_angles, at, item_angles, UNDERWATER_SWITCH_STAND);
                player.position = level(at_lara);
                motion.pitch = crate::player::binary_radians(lara_angles[0]);
                player.yaw = crate::player::binary_radians(lara_angles[1]);
                submerged.bank = crate::player::binary_radians(lara_angles[2]);
                if !there {
                    continue;
                }
                submerged.speed = 0.0;
                // Treading's range for 40 covers its whole clip; the clip's
                // EmptyHands frees the hands as it links back to treading.
                animation.enter(clip::UNDERWATER_SWITCH, 0.0, clip::UNDERWATER_SWITCH);
                traversal.goal = state::TREAD;
                weapon.status = GunStatus::HandsBusy;
                item.status = ItemStatus::Active;
                if let (Some(mut a), Some(mut goal), Some(world)) = (item_animation, goal, world) {
                    goal.state = if item.state == 1 { 0 } else { 1 };
                    animate(world, &mut a, &mut goal, &mut item);
                }
                object.update_active();
                continue;
            }
            // SwitchCollision 0x800507E4.
            if !action || item.status != ItemStatus::Inactive || !ready(&player, &motion, &weapon, lara_state) {
                continue;
            }
            if !in_bounds(&SWITCH_BOUNDS, at, yaw, lara, lara_yaw) {
                continue;
            }
            let on = item.state == 1;
            if on && switch.one_way {
                continue;
            }
            player.yaw = engine::world::angle(yaw as u16);
            if let Some(distance) = switch.align {
                align(world, &mut player, at, yaw, IVec3::new(0, 0, distance));
            }
            // Lara's clip carries its state: 40 (switch off) or 41 (on).
            let (lara_clip, goal_state) = if on { (switch.clips[0], 0) } else { (switch.clips[1], 1) };
            using::start(&mut player, &mut motion, &mut animation, &mut weapon, lara_clip);
            item.status = ItemStatus::Active;
            if let (Some(mut a), Some(mut goal), Some(world)) = (item_animation, goal, world) {
                goal.state = goal_state;
                animate(world, &mut a, &mut goal, &mut item);
            }
            object.flags |= CODE_BITS;
            object.update_active();
        }
    }
}

/// One AnimateItem step for a switch; its clip's kill command at the end
/// deactivates it (command 4, 0x8001DF08), which the switch trigger waits for.
fn animate(world: &WorldGeometry, a: &mut ObjectAnimation, goal: &mut AnimationGoal, item: &mut UsableItem) {
    let step = step_animation_frame_from(&world.data, a, goal, &mut item.state);
    if step.cues.iter().any(|c| c.at_end && matches!(c.command, Command::Kill)) {
        item.status = ItemStatus::Deactivated;
    }
}

/// SwitchControl 0x800516B8, for active switches: once its trigger stops
/// holding it (a timed switch running out) it asks to go back to state 1.
pub(super) fn control(
    world: Option<Res<WorldGeometry>>,
    mut switches: Query<(&mut WorldObject, &mut UsableItem, &mut ObjectAnimation, &mut AnimationGoal), With<Switch>>,
) {
    let Some(world) = world else { return };
    for (mut object, mut item, mut a, mut goal) in &mut switches {
        if item.status != ItemStatus::Active && item.status != ItemStatus::Deactivated {
            continue;
        }
        object.flags |= CODE_BITS;
        if !object.trigger_active() {
            goal.state = 1;
            object.timer = 0;
        }
        animate(&world, &mut a, &mut goal, &mut item);
    }
}

/// KeyHoleCollision 0x80050F80 and PuzzleHoleCollision 0x800512CC, with the
/// matching item chosen straight from the inventory. The original opens the
/// inventory ring when Action is pressed without one (0x80030F30) and uses
/// the item chosen there on the next tick; the ring is not ported.
#[allow(clippy::type_complexity)]
pub(super) fn use_keys(
    pad: Res<ControllerState>,
    world: Option<Res<WorldGeometry>>,
    clips: Option<Res<PlayerAnimations>>,
    mut inventory: ResMut<Inventory>,
    mut progress: ResMut<InteractionProgress>,
    mut players: Query<(&mut Player, &mut Motion, &mut engine::animation::AnimationState, &mut WeaponState)>,
    mut holes: Query<(&mut WorldObject, &Transform, &mut UsableItem, &Keyhole)>,
) {
    let Ok((mut player, mut motion, mut animation, mut weapon)) = players.single_mut() else { return };
    let clips = clips.as_deref();
    let lara_state = clips.and_then(|c| c.state(animation.animation));
    for (mut object, transform, mut item, keyhole) in &mut holes {
        let at = original_units(transform.translation);
        let yaw = binary_yaw(transform.rotation.to_euler(EulerRot::YXZ).0);
        let lara = original_units(player.position);
        if ((lara - at).abs().cmpgt(IVec3::splat(4095))).any() {
            continue;
        }
        if keyhole.puzzle && lara_state == Some(state::USE_PUZZLE) {
            // The piece goes in on clip 134's frame 80 (0x800512CC): the
            // hole becomes its filled type, four ids on (182 -> 186).
            if animation.animation == clip::USE_PUZZLE
                && animation.frame == frame::PUZZLE_PLACE
                && in_bounds(&KEYHOLE_BOUNDS, at, yaw, lara, binary_yaw(player.yaw))
            {
                object.kind += 4;
            }
            continue;
        }
        if !pad.held(Button::Cross) || !ready(&player, &motion, &weapon, lara_state) {
            continue;
        }
        if !in_bounds(&KEYHOLE_BOUNDS, at, yaw, lara, binary_yaw(player.yaw)) {
            continue;
        }
        if item.status != ItemStatus::Inactive || !inventory.consume(keyhole.key) {
            progress.message = "A matching item is required".into();
            continue;
        }
        align(world.as_deref(), &mut player, at, yaw, if keyhole.puzzle { PUZZLE_STAND } else { KEYHOLE_STAND });
        let clip = if keyhole.puzzle { clip::USE_PUZZLE } else { clip::USE_KEY };
        using::start(&mut player, &mut motion, &mut animation, &mut weapon, clip);
        item.status = ItemStatus::Active;
    }
}

/// Which use objects let their triggers fire this tick: a switch whose
/// clip has run its kill command (SwitchTrigger 0x8005172C), a used
/// keyhole once Lara's hands are free again (KeyTrigger 0x80051874), a
/// collected pickup (PickupTrigger 0x800518E4).
#[allow(clippy::type_complexity)]
pub(super) fn ready_sources(
    mut triggers: ResMut<engine::triggers::TriggerState>,
    players: Query<&WeaponState, With<Player>>,
    items: Query<(&WorldObject, &UsableItem, Has<Switch>, Has<Keyhole>, Has<Pickup>)>,
) {
    let hands_busy = players.iter().any(|w| w.status == crate::player::weapons::GunStatus::HandsBusy);
    for (object, item, switch, keyhole, pickup) in &items {
        let ready = (switch && item.status == ItemStatus::Deactivated)
            || (keyhole && item.status == ItemStatus::Active && !hands_busy)
            || (pickup && item.status == ItemStatus::Invisible);
        if ready {
            triggers.mark_ready(object.index);
        }
    }
}

/// What the trigger tests do to their object when they pass: a switch
/// back in state 0 with a timer stays active for that long (one tick for a
/// timer of 1, else seconds of 30 ticks), any other switch goes inactive;
/// keyholes and pickups are spent.
pub(super) fn sources_fired(
    mut fired: MessageReader<engine::triggers::TriggerSourceFired>,
    mut items: Query<(&mut WorldObject, &mut UsableItem, Has<Switch>)>,
) {
    for source in fired.read() {
        for (mut object, mut item, switch) in &mut items {
            if object.index != source.index {
                continue;
            }
            if switch {
                if item.state == 0 && source.timer > 0 {
                    object.timer = if source.timer == 1 { 1 } else { i16::from(source.timer) * 30 };
                    item.status = ItemStatus::Active;
                } else {
                    item.status = ItemStatus::Inactive;
                }
            } else {
                item.status = ItemStatus::Deactivated;
            }
        }
    }
}
