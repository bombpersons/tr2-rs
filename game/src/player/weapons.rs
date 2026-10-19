//! Lara's guns: the hand state machine LaraGun (0x8003FC4C) and the pistol
//! draw (0x80047E60), holster (0x80047F1C), aim (0x800484B0) and fire
//! (0x8004874C, 0x80040E8C) handlers, ported per tick.
//!
//! Each arm plays a frame of the pistol animation object (level model 1):
//! frames 0-4 raise the gun to aim, 5-23 draw it from the holster and 24
//! onward are the recoil after a shot. Drawing and holstering step both arms;
//! aiming and firing step each arm on its own. Design numbers come from the
//! per-gun table at 0x80089434 (pistols at 0x80089464), baked into
//! `data/object_types.json` as [`GunTuning`].
//!
//! Angles are the original's 65536-per-turn integers and positions its
//! world units (y down), because target selection, lock boxes and the shot
//! ray compare them exactly.
//!
//! PSX evidence: docs/pistols-evidence.md, recordings
//! `tests/data/original/pistols_*.csv` (no pokes).
//! Not ported: guns other than pistols, flares (gun status 5 and the
//! flare branches of LaraGun), vehicles (0x8008C50A), wading depth limits,
//! smashable objects on the line of fire (0x80020618) and ricochets.
use bevy::prelude::*;
use engine::camera::chase::{cos, heading, isqrt, sin};
use engine::controller::{Button, ControllerState};
use engine::random::GameRandom;

use super::{Player, PlayerMode};

/// The gun Lara has selected (0x8008C4EC). The original numbers its guns
/// 1-8; this port has pistols only.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Gun {
    /// No gun: Draw does nothing (Home starts with 0 in 0x8008C4EC).
    #[default]
    Unarmed,
    Pistols,
}

/// What Lara's hands are doing (0x8008C4EA).
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GunStatus {
    /// 0: grabs, pickups, reaches and Draw are allowed.
    #[default]
    HandsFree,
    /// 1: a climb or pickup owns the hands until its clip's EmptyHands command.
    HandsBusy,
    /// 2: arm frames 5 to 22 while the guns come out.
    Drawing,
    /// 3: back to the holsters, frames 23 down to 5.
    Holstering,
    /// 4: guns out, aiming and firing.
    Ready,
}

/// One arm's gun: the pistol animation frame it shows, its aim and flash.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Default)]
pub struct Arm {
    /// Frame of the pistol animation object (0x8008C584 left, 0x8008C598 right).
    pub frame: i16,
    /// Aim relative to Lara, 65536 per turn (0x8008C58A/C, 0x8008C59E/A0).
    pub yaw: i16,
    pub pitch: i16,
    /// The target is inside this arm's aiming box (0x8008C588, 0x8008C59C).
    pub locked: bool,
    /// Ticks of muzzle flash left (0x8008C590, 0x8008C5A4).
    pub flash: i16,
}

/// Lara's guns, as LaraGun keeps them.
#[derive(Component, Reflect, Debug, Clone, Default)]
#[reflect(Component)]
pub struct WeaponState {
    pub status: GunStatus,
    /// The gun in Lara's hands or holsters (0x8008C4EC).
    pub gun: Gun,
    /// The gun the inventory asks for (0x8008C4EE); Draw re-requests `last`.
    pub requested: Gun,
    /// The gun Draw brings out (0x8008C4F0).
    pub last: Gun,
    pub left: Arm,
    pub right: Arm,
    /// The creature the guns follow (0x8008C568).
    pub target: Option<Entity>,
    /// Direction to the target from the guns, relative to Lara (0x8008C56C/E).
    pub target_angles: [i16; 2],
    /// Head and torso turn toward the aim, relative to Lara (0x8008C574/6,
    /// 0x8008C57A/C); the combat camera looks along their sum.
    pub head: [i16; 2],
    pub torso: [i16; 2],
    /// Monotonic count of arm shots, for hit tests, sound and flashes.
    pub shot_serial: u64,
    /// Which guns (left, right) are in Lara's hands rather than her
    /// holsters. The original swaps meshes in Lara's mesh table at these
    /// moments and keeps them there: both at draw frame 13 (0x80048324),
    /// each arm at its holster frame 12 (0x800483C8 left, 0x8004843C
    /// right). Dying frees the hands without holstering.
    pub in_hand: [bool; 2],
}

impl WeaponState {
    /// Lara starts a level holding `gun` in its holsters.
    pub fn holding(gun: Gun) -> Self {
        Self { gun, requested: gun, last: gun, ..default() }
    }

    /// Grabs, reaches, swan dives and pickups test status 0 (for example
    /// 0x80038BEC, 0x8003D968, 0x80050474).
    pub fn hands_free(&self) -> bool {
        self.status == GunStatus::HandsFree
    }

    /// Guns out: combat triggers (TestTriggers 0x8001F0B0) and the turn
    /// handlers (0x80038DC4) test status 4.
    pub fn ready(&self) -> bool {
        self.status == GunStatus::Ready
    }

    /// LaraGun sets the combat camera type while drawing and while ready
    /// (0x8004001C, 0x800401B8), not while holstering.
    pub fn aiming_camera(&self) -> bool {
        matches!(self.status, GunStatus::Drawing | GunStatus::Ready)
    }

    /// Where the combat camera looks, relative to Lara's facing: at the
    /// target, or along head plus torso (0x800195A0..0x800195F8).
    pub fn camera_aim(&self) -> [i16; 2] {
        if self.target.is_some() {
            self.target_angles
        } else {
            [self.torso[0].wrapping_add(self.head[0]), self.torso[1].wrapping_add(self.head[1])]
        }
    }
}

/// One gun's entry in the table at 0x80089434 (48 bytes per gun). Angles
/// are 65536 per turn, distances world units, times ticks.
#[derive(Resource, Reflect, serde::Deserialize, Debug, Clone)]
#[reflect(Resource)]
pub struct GunTuning {
    /// +0x00: both arms lock onto targets inside this yaw/pitch box.
    pub lock_yaw: [i16; 2],
    pub lock_pitch: [i16; 2],
    /// +0x08 / +0x10: a locked arm keeps its target while inside its own box.
    pub left_yaw: [i16; 2],
    pub left_pitch: [i16; 2],
    pub right_yaw: [i16; 2],
    pub right_pitch: [i16; 2],
    /// +0x18: arm turn per tick towards the target.
    pub aim_speed: i16,
    /// +0x1A: shots spread up to half this either way, in yaw and pitch.
    pub spread: i16,
    /// +0x1C: muzzle height above Lara's origin.
    pub gun_height: i32,
    /// +0x20: hit points per hit.
    pub damage: i32,
    /// +0x24: targeting and shot range.
    pub range: i32,
    /// +0x28: recoil frames after a shot before the arm can fire again.
    pub recoil_frames: i16,
    /// +0x2A: muzzle flash ticks.
    pub flash_ticks: i16,
    /// +0x2C: shot sound.
    pub shot_sound: usize,
}

/// Sounds the gun code starts, for the game's audio.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GunSound(pub usize);

/// A shot that struck a creature (HitTarget 0x8004132C): `point` is where,
/// in world units (y down), for blood effects.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct GunHit {
    pub target: Entity,
    pub damage: i32,
    pub point: IVec3,
}

/// The draw sound at frame 13 (0x80047EB8) and the holster sound per arm at
/// frame 12 (0x80048044, 0x80048184).
pub const DRAW_SOUND: usize = 6;
pub const HOLSTER_SOUND: usize = 7;
/// Draw frame that puts the pistols in Lara's hands (0x80047E9C).
const DRAW_GRIP: i16 = 13;
/// Holster frame that puts them back on her thighs (0x80048030).
const HOLSTER_GRIP: i16 = 12;
/// Last draw frame: the next tick makes the guns ready (0x80047EC4).
const DRAW_END: i16 = 23;
/// Aim frames run 0..=4; recoil starts at 24 (0x80048890).
const AIM_FRAME: i16 = 4;
const RECOIL_START: i16 = 24;
/// The draw animation starts here when the frame is outside 4..=22.
const DRAW_START: i16 = 5;
/// A farther candidate still wins if its yaw is less than this much wider
/// than the best so far (0x80040B74: 0xAAA, 15 degrees).
const TARGET_YAW_SLACK: i32 = 0xaaa;

/// Lara's held buttons as LaraGun reads them: Draw (input bit 0x20) and
/// Action (0x40) are tested held, not pressed, so holding Draw toggles
/// between drawing and holstering (pistols_held_draw). Look (0x200) stops
/// the holster from turning her head.
#[derive(Debug, Clone, Copy, Default)]
pub struct GunInput {
    pub draw: bool,
    pub action: bool,
    pub look: bool,
}

impl GunInput {
    pub fn read(pad: &ControllerState) -> Self {
        Self { draw: pad.held(Button::Triangle), action: pad.held(Button::Cross), look: pad.held(Button::L1) }
    }
}

/// A creature the guns can aim at, as the targeting code sees it. Positions
/// are world units with y down.
#[derive(Debug, Clone)]
pub struct GunTarget {
    pub entity: Entity,
    /// The creature's origin, for the range test.
    pub position: IVec3,
    /// Where the guns aim (0x80040C40): the centre of its current collision
    /// box, a third of the way down from the top.
    pub aim_point: IVec3,
    pub room: usize,
    pub alive: bool,
    /// Its mesh spheres (centre, radius), for the shot ray.
    pub spheres: Vec<(IVec3, i32)>,
}

/// Where Lara is and what she can see, for one tick of LaraGun.
pub struct GunWorld<'a> {
    /// Lara's origin (world units, y down), room and facing.
    pub position: IVec3,
    pub room: usize,
    pub yaw: i16,
    /// Creatures in creature-slot order (0x8008C1C0).
    pub targets: &'a [GunTarget],
    /// Line of sight from a point in one room to a point in another (0x8001FB54).
    pub line_of_sight: &'a dyn Fn(IVec3, usize, IVec3, usize) -> bool,
}

/// What one tick of LaraGun did.
#[derive(Debug, Default)]
pub struct GunTick {
    pub shots: u32,
    pub hits: Vec<GunHit>,
}

/// Run LaraGun for one tick. `alive` is health above zero; `can_draw` says
/// whether guns may come out where Lara is (on land; water status 0).
#[allow(clippy::too_many_arguments)]
pub fn update(
    weapon: &mut WeaponState,
    input: GunInput,
    alive: bool,
    can_draw: bool,
    tuning: &GunTuning,
    world: &GunWorld,
    random: &mut GameRandom,
    sounds: &mut impl FnMut(usize),
) -> GunTick {
    if !input.look {
        reset_look(weapon);
    }
    weapon.left.flash = (weapon.left.flash - 1).max(0);
    weapon.right.flash = (weapon.right.flash - 1).max(0);
    if !alive {
        weapon.status = GunStatus::HandsFree;
        return GunTick::default();
    }
    match weapon.status {
        GunStatus::HandsFree => {
            if input.draw {
                weapon.requested = weapon.last;
            }
            if weapon.requested != weapon.gun || input.draw {
                if can_draw {
                    weapon.gun = weapon.requested;
                    start_draw(weapon);
                } else {
                    // 0x8003FF4C: remember the request but leave the guns away.
                    weapon.last = weapon.requested;
                    weapon.gun = weapon.requested;
                }
            }
        }
        GunStatus::Ready if input.draw || weapon.requested != weapon.gun || !can_draw => {
            weapon.status = GunStatus::Holstering;
        }
        _ => {}
    }
    match weapon.status {
        GunStatus::Drawing => {
            if weapon.gun != Gun::Unarmed {
                weapon.last = weapon.gun;
            }
            match weapon.gun {
                Gun::Pistols => draw_pistols(weapon, sounds),
                // No gun to draw: the status switch falls back to free hands
                // in the same tick (0x80040078).
                Gun::Unarmed => weapon.status = GunStatus::HandsFree,
            }
            GunTick::default()
        }
        GunStatus::Holstering => {
            holster_pistols(weapon, input, sounds);
            GunTick::default()
        }
        GunStatus::Ready => pistol_handler(weapon, input, tuning, world, random, sounds),
        _ => GunTick::default(),
    }
}

/// ResetLook 0x800385EC, which Lara's control runs at the start of every
/// tick unless Look is held (0x80038220): head pitch and yaw ease an eighth
/// of the way back to straight ahead (truncated toward zero), snapping to
/// zero within 363, and the torso copies the head. The gun code later in
/// the tick overwrites both while a target is locked. Not ported: the skip
/// for camera type 2 and the look-flag test at 0x8008C510.
fn reset_look(weapon: &mut WeaponState) {
    let ease = |value: i16| if (-363..=363).contains(&value) { 0 } else { value - value / 8 };
    weapon.head = weapon.head.map(ease);
    weapon.torso = weapon.head;
}

/// 0x8003FE34..0x8003FE48: InitialiseNewWeapon, then status 2 from frame 0.
fn start_draw(weapon: &mut WeaponState) {
    weapon.left = Arm::default();
    weapon.right = Arm::default();
    weapon.target = None;
    weapon.status = GunStatus::Drawing;
}

/// DrawPistols 0x80047E60: both arms share the left arm's frame. A frame
/// outside 4..=22 restarts at 5; the guns reach Lara's hands at 13 with the
/// draw sound, and the tick that would show 23 makes them ready instead.
fn draw_pistols(weapon: &mut WeaponState, sounds: &mut impl FnMut(usize)) {
    let old = weapon.left.frame;
    let mut frame = if (4..DRAW_END).contains(&old) { old + 1 } else { DRAW_START };
    if (4..DRAW_END).contains(&old) {
        if frame == DRAW_GRIP {
            sounds(DRAW_SOUND);
            weapon.in_hand = [true, true];
        } else if frame == DRAW_END {
            // ReadyPistols 0x800482A0: aim and target cleared, frames back to 0.
            weapon.status = GunStatus::Ready;
            weapon.left = Arm { flash: weapon.left.flash, ..default() };
            weapon.right = Arm { flash: weapon.right.flash, ..default() };
            weapon.target = None;
            frame = 0;
        }
    }
    weapon.left.frame = frame;
    weapon.right.frame = frame;
}

/// UndrawPistols 0x80047F1C, per arm: recoil drops to the aim pose, aim
/// frames ease the arm's rotation home by 1/frame of it per tick, the rest
/// pose jumps to 23 and the holster frames count down to 5. When both arms
/// rest on 5 the hands are free again. Unless Look is held, head and torso
/// follow a quarter of the arms' summed aim (0x80048200).
fn holster_pistols(weapon: &mut WeaponState, input: GunInput, sounds: &mut impl FnMut(usize)) {
    for (side, arm) in [&mut weapon.left, &mut weapon.right].into_iter().enumerate() {
        let old = arm.frame;
        arm.frame = if old >= RECOIL_START {
            AIM_FRAME
        } else if (1..=AIM_FRAME).contains(&old) {
            // Integer division truncates toward zero, as `div` does.
            arm.pitch -= arm.pitch / old;
            arm.yaw -= arm.yaw / old;
            old - 1
        } else if old == 0 {
            arm.yaw = 0;
            arm.pitch = 0;
            DRAW_END
        } else if (DRAW_START + 1..=DRAW_END).contains(&old) {
            if old - 1 == HOLSTER_GRIP {
                sounds(HOLSTER_SOUND);
                weapon.in_hand[side] = false;
            }
            old - 1
        } else {
            old
        };
    }
    if weapon.left.frame == DRAW_START && weapon.right.frame == DRAW_START {
        weapon.status = GunStatus::HandsFree;
        weapon.left = Arm { flash: weapon.left.flash, yaw: weapon.left.yaw, pitch: weapon.left.pitch, ..default() };
        weapon.right = Arm { flash: weapon.right.flash, yaw: weapon.right.yaw, pitch: weapon.right.pitch, ..default() };
        weapon.target = None;
    }
    if !input.look {
        let quarter = |a: i16, b: i16| ((i32::from(a) + i32::from(b)) / 4) as i16;
        weapon.head = [quarter(weapon.left.yaw, weapon.right.yaw), quarter(weapon.left.pitch, weapon.right.pitch)];
        weapon.torso = weapon.head;
    }
}

/// PistolHandler 0x800484B0: keep or find a target, swing both arms toward
/// it, turn head and torso, then step each arm's aim and recoil frames.
fn pistol_handler(
    weapon: &mut WeaponState,
    input: GunInput,
    tuning: &GunTuning,
    world: &GunWorld,
    random: &mut GameRandom,
    sounds: &mut impl FnMut(usize),
) -> GunTick {
    // Holding Action keeps the current target; otherwise one is chosen afresh
    // every tick.
    if input.action {
        update_lock(weapon, tuning, world);
    } else {
        weapon.target = None;
    }
    if weapon.target.is_none() {
        find_target(weapon, tuning, world);
    }
    let angles = weapon.target_angles;
    aim_arm(&mut weapon.left, angles, tuning.aim_speed);
    aim_arm(&mut weapon.right, angles, tuning.aim_speed);
    turn_head(weapon);
    animate_pistols(weapon, input, tuning, world, random, sounds)
}

/// The guns' muzzle position: Lara's origin raised by the gun height.
fn muzzle(world: &GunWorld, tuning: &GunTuning) -> IVec3 {
    world.position - IVec3::Y * tuning.gun_height
}

/// phd_GetVectorAngles 0x80065210: the heading and elevation of a vector
/// (positive elevation is up, since y points down).
pub fn vector_angles(delta: IVec3) -> [i16; 2] {
    let yaw = heading(delta.x, delta.z) as i16;
    let mut d = delta;
    while d.x != d.x as i16 as i32 || d.y != d.y as i16 as i32 || d.z != d.z as i16 as i32 {
        d >>= 2;
    }
    let horizontal = isqrt(d.x * d.x + d.z * d.z);
    let pitch = heading(d.y, horizontal) as i16;
    let pitch = if (d.y > 0 && pitch > 0) || (d.y < 0 && pitch < 0) { pitch.wrapping_neg() } else { pitch };
    [yaw, pitch]
}

fn inside(angles: [i16; 2], yaw: [i16; 2], pitch: [i16; 2]) -> bool {
    (yaw[0]..=yaw[1]).contains(&angles[0]) && (pitch[0]..=pitch[1]).contains(&angles[1])
}

/// LaraGetNewTarget/update 0x800406B4: aim at the current target. With line
/// of sight, the lock box locks both arms; outside it, each arm keeps its
/// lock only while the target stays inside that arm's own box.
fn update_lock(weapon: &mut WeaponState, tuning: &GunTuning, world: &GunWorld) {
    let Some(target) = weapon.target.and_then(|entity| world.targets.iter().find(|t| t.entity == entity)) else {
        weapon.target = None;
        weapon.left.locked = false;
        weapon.right.locked = false;
        weapon.target_angles = [0, 0];
        return;
    };
    let from = muzzle(world, tuning);
    let [yaw, pitch] = vector_angles(target.aim_point - from);
    let angles = [yaw.wrapping_sub(world.yaw), pitch];
    if !(world.line_of_sight)(from, world.room, target.aim_point, target.room) {
        weapon.left.locked = false;
        weapon.right.locked = false;
    } else if inside(angles, tuning.lock_yaw, tuning.lock_pitch) {
        weapon.left.locked = true;
        weapon.right.locked = true;
    } else {
        if weapon.left.locked && !inside(angles, tuning.left_yaw, tuning.left_pitch) {
            weapon.left.locked = false;
        }
        if weapon.right.locked && !inside(angles, tuning.right_yaw, tuning.right_pitch) {
            weapon.right.locked = false;
        }
    }
    weapon.target_angles = angles;
}

/// 0x80040948: pick the creature to aim at. Candidates must be alive, within
/// range on every axis and by distance, visible, and inside the lock box
/// measured from the torso's aim. The nearest wins unless its yaw is more
/// than 15 degrees wider than the best so far.
fn find_target(weapon: &mut WeaponState, tuning: &GunTuning, world: &GunWorld) {
    let from = muzzle(world, tuning);
    let range = tuning.range;
    let (mut best, mut best_distance, mut best_yaw) = (None, i32::MAX, i32::from(i16::MAX));
    for target in world.targets.iter().filter(|t| t.alive) {
        let d = target.position - from;
        if d.x.abs() > range || d.y.abs() > range || d.z.abs() > range {
            continue;
        }
        let distance = d.x * d.x + d.y * d.y + d.z * d.z;
        if distance >= range * range || !(world.line_of_sight)(from, world.room, target.aim_point, target.room) {
            continue;
        }
        let [yaw, pitch] = vector_angles(target.aim_point - from);
        let angles = [yaw.wrapping_sub(world.yaw).wrapping_sub(weapon.torso[0]), pitch.wrapping_sub(weapon.torso[1])];
        if !inside(angles, tuning.lock_yaw, tuning.lock_pitch) {
            continue;
        }
        let wide = i32::from(angles[0]).abs();
        if wide < best_yaw + TARGET_YAW_SLACK && distance < best_distance {
            best = Some(target.entity);
            best_distance = distance;
            best_yaw = wide;
        }
    }
    weapon.target = best;
    update_lock(weapon, tuning, world);
}

/// AimWeapon 0x80040D7C: a locked arm turns toward the target angles, an
/// unlocked one toward straight ahead, at most `speed` per axis per tick.
fn aim_arm(arm: &mut Arm, target: [i16; 2], speed: i16) {
    let goal = if arm.locked { target } else { [0, 0] };
    let step = |current: i16, goal: i16| {
        let (low, high) = (i32::from(goal) - i32::from(speed), i32::from(goal) + i32::from(speed));
        if i32::from(current) < low {
            current.wrapping_add(speed)
        } else if i32::from(current) > high {
            current.wrapping_sub(speed)
        } else {
            goal
        }
    };
    arm.yaw = step(arm.yaw, goal[0]);
    arm.pitch = step(arm.pitch, goal[1]);
}

/// 0x80048548..0x80048698: with both arms locked, head and torso turn a
/// quarter of the arms' summed aim; with one, half of that arm's.
fn turn_head(weapon: &mut WeaponState) {
    let (l, r) = (weapon.left, weapon.right);
    let aim = match (l.locked, r.locked) {
        (true, true) => {
            [((i32::from(l.yaw) + i32::from(r.yaw)) / 4) as i16, ((i32::from(l.pitch) + i32::from(r.pitch)) / 4) as i16]
        }
        (true, false) => [l.yaw / 2, l.pitch / 2],
        (false, true) => [r.yaw / 2, r.pitch / 2],
        (false, false) => return,
    };
    weapon.head = aim;
    weapon.torso = aim;
}

/// AnimatePistols 0x8004874C. An arm raises through the aim frames while it
/// is locked, or while Action is held with nothing targeted; otherwise it
/// lowers, and recoil drops straight to the aim pose. At frame 4 with Action
/// held it fires and starts its recoil, which returns to 4 after
/// `recoil_frames`. The right arm steps first and plays the shot sound; the
/// left plays it only if the right did not fire.
fn animate_pistols(
    weapon: &mut WeaponState,
    input: GunInput,
    tuning: &GunTuning,
    world: &GunWorld,
    random: &mut GameRandom,
    sounds: &mut impl FnMut(usize),
) -> GunTick {
    let mut tick = GunTick::default();
    let target = weapon.target.and_then(|entity| world.targets.iter().find(|t| t.entity == entity));
    let mut sounded = false;
    for arm in [&mut weapon.right, &mut weapon.left] {
        let aiming = arm.locked || (input.action && target.is_none());
        let old = arm.frame;
        arm.frame = if !aiming {
            if old >= RECOIL_START {
                AIM_FRAME
            } else if (1..=AIM_FRAME).contains(&old) {
                old - 1
            } else {
                old
            }
        } else if old < AIM_FRAME {
            old + 1
        } else if old == AIM_FRAME {
            if input.action {
                // FireWeapon 0x80040E8C: pistol ammo is refilled to 1000
                // before each shot, so it always fires.
                let aim = [arm.yaw.wrapping_add(world.yaw), arm.pitch];
                if let Some(hit) = fire(aim, muzzle(world, tuning), target, tuning, random) {
                    tick.hits.push(hit);
                }
                arm.flash = tuning.flash_ticks;
                tick.shots += 1;
                if !sounded {
                    sounds(tuning.shot_sound);
                    sounded = true;
                }
                RECOIL_START
            } else {
                old
            }
        } else if old >= RECOIL_START {
            if old + 1 == RECOIL_START + tuning.recoil_frames { AIM_FRAME } else { old + 1 }
        } else {
            old
        };
    }
    weapon.shot_serial += u64::from(tick.shots);
    tick
}

/// One shot's random spread: (r - 0x4000) * spread / 65536, truncated
/// toward zero (0x80040FF0..0x80041010).
fn spread(random: &mut GameRandom, spread: i16) -> i16 {
    ((random.draw() - 0x4000) * i32::from(spread) / 65536) as i16
}

/// FireWeapon 0x80040E8C for a gun with ammo: jitter the aim (pitch first,
/// then yaw), then cast a ray from the muzzle against the target's spheres
/// in the gun's own frame (0x80064F04 builds it). The nearest sphere the
/// ray enters is hit. A miss has no gameplay effect here.
fn fire(
    aim: [i16; 2],
    muzzle: IVec3,
    target: Option<&GunTarget>,
    tuning: &GunTuning,
    random: &mut GameRandom,
) -> Option<GunHit> {
    let pitch = i32::from(aim[1].wrapping_add(spread(random, tuning.spread)));
    let yaw = i32::from(aim[0].wrapping_add(spread(random, tuning.spread)));
    let target = target?;
    let (sx, cx, sy, cy) = (sin(pitch), cos(pitch), sin(yaw), cos(yaw));
    // Rows of the gun frame with no roll: right, down, forward (scale 16384).
    let right = IVec3::new(cy, 0, -sy);
    let down = IVec3::new((sx * sy) >> 14, cx, (sx * cy) >> 14);
    let forward = IVec3::new((cx * sy) >> 14, -sx, (cx * cy) >> 14);
    let nearest = target
        .spheres
        .iter()
        .filter_map(|&(centre, radius)| {
            let d = centre - muzzle;
            let (x, y, z) = (right.dot(d) >> 14, down.dot(d) >> 14, forward.dot(d) >> 14);
            (x.abs() < radius && y.abs() < radius && radius < z && x * x + y * y <= radius * radius)
                .then_some(z - radius)
        })
        .min()?;
    Some(GunHit { target: target.entity, damage: tuning.damage, point: muzzle + ((forward * nearest) >> 14) })
}

/// Guns may come out on land; underwater and surface swimming refuse
/// (water status 0 only; wading depth 0x8008C514 is not ported).
pub fn can_draw(player: &Player) -> bool {
    !matches!(player.mode, PlayerMode::Swimming | PlayerMode::Surface)
}

/// Levels that start without guns. The per-level start record
/// (0x800DE250 + 44 * level, read by 0x80043BE4) gives Home no pistols:
/// 0x8008C4EC is 0 in `.psx/home_start` and 1 in `.psx/wall_game`.
pub const UNARMED_LEVELS: &[&str] = &["assault"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_angles_point_up_for_targets_above() {
        assert_eq!(vector_angles(IVec3::new(0, 0, 1000)), [0, 0]);
        // 45 degrees is 8192; the arctangent table rounds it to 8193.
        let [yaw, pitch] = vector_angles(IVec3::new(1000, -1000, 0));
        assert_eq!(yaw, 16384);
        assert!((8192..=8193).contains(&pitch));
        assert!((-8193..=-8192).contains(&vector_angles(IVec3::new(0, 1000, 1000))[1]));
    }
}
