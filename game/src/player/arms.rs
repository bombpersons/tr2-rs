//! How Lara's guns are drawn: arm poses from the pistol animation object,
//! the hand/holster mesh swaps, muzzle flashes and the gunfire light.
//! Presentation only: it reads [`WeaponState`] after the tick and never
//! changes it.
//!
//! DrawLara 0x80025BDC draws the body from Lara's own clip, turning the
//! torso (joint 7) and head (joint 14) by the gun code's torso and head
//! angles after their keys (0x800261C0, 0x80026240). While a gun is
//! drawing, holstering or ready, it then draws each arm through the gun's
//! case of the table at 0x800101F0 (pistols: 0x80026654):
//!
//! - The upper arm hangs at its offset from the turned torso but takes its
//!   orientation from Lara's own item rotation (the matrix two levels up the
//!   stack, copied at 0x8002667C), so the hips' and torso's turns do not
//!   reach it. It turns by the arm's aim (RotYXZ of 0x8008C59E/A0/A2 right,
//!   0x8008C58A/C/E left), then by the pistol object's key for that joint.
//! - Forearm and hand follow with the pistol object's keys (0x80026754..).
//! - The key is the arm's frame counted across the pistol object's clips
//!   (base 0x8008C594/0x8008C580 plus (frame - clip start) * frame size).
//!
//! The meshes come from Lara's mesh table (0x8008C52C), which the gun code
//! rewrites rather than DrawLara choosing: a drawn gun puts the pistol
//! object's hand mesh in the hand and Lara's own (empty) thigh on the leg;
//! a holstered one the other way round (0x80048324, 0x800483C8, 0x8004843C;
//! the start of Wall copies the pistol thighs at 0x80043B9C). Home starts
//! without pistols and keeps Lara's own thighs (`.psx/home_start`).
//!
//! PSX evidence: docs/pistols-evidence.md ("Presentation").
use bevy::prelude::*;
use engine::animation::{JointPose, PoseOverrides, model_frame_key};
use engine::camera::chase::{cos, sin};
use engine::lighting::{DynamicLight, DynamicLights};
use engine::psx::{PsxBlend, PsxMaterial};
use engine::render::{ItemLighting, JointAttachment, JointMeshes, LevelSystems, static_lit_mesh};
use engine::world::{Level, WorldGeometry, position};

use super::weapons::{Gun, GunStatus, WeaponState};
use super::{Player, original_units};

/// The pistol animation object: 0x800414D8 maps pistols to object 1. Its
/// clips 218-221 hold the arm frames (0-4 aim, 5-23 draw, 24+ recoil) and
/// its meshes the drawn and holstered guns.
pub const PISTOL_MODEL: u16 = 1;
/// The gun flash object DrawGunFlash 0x80028454 draws (object 240).
pub const FLASH_MODEL: u16 = 240;

use super::contract::joint;
const TORSO: usize = joint::TORSO;
const HEAD: usize = joint::HEAD;
/// Upper arm, forearm and hand, left then right (the order of
/// [`WeaponState::in_hand`]): DrawLara draws 8-10 for the right arm and
/// 11-13 for the left.
const ARMS: [[usize; 3]; 2] = [
    [joint::LEFT_UPPER_ARM, joint::LEFT_UPPER_ARM + 1, joint::LEFT_HAND],
    [joint::RIGHT_UPPER_ARM, joint::RIGHT_UPPER_ARM + 1, joint::RIGHT_HAND],
];
/// Thigh joints that carry the holsters, left then right.
const THIGHS: [usize; 2] = [joint::LEFT_THIGH, joint::RIGHT_THIGH];

/// DrawGunFlash's default branch, used by the pistols: from the hand,
/// translate (0, 185, 40), pitch -16380, roll a random draw * 2.
const FLASH_OFFSET: [i32; 3] = [0, 185, 40];
const FLASH_PITCH: i16 = -16380;
/// S_CalculateStaticLight(5120) for the flash mesh (0x80028680): its
/// shades draw as grey 73 in the GPU capture of a flash frame.
const FLASH_LIGHT: i32 = 5120;

/// The gunfire light PistolHandler adds while either flash is lit
/// (0x800486A8..0x8004872C): at Lara's feet, sin/cos(yaw) >> 4 (1024 units)
/// ahead, intensity 2^12, reach 2^11.
const GUNFIRE_AHEAD_SHIFT: i32 = 4;
const GUNFIRE_INTENSITY: u32 = 12;
const GUNFIRE_FALLOFF: u32 = 11;

/// The joint overrides and mesh swaps that draw `weapon` on Lara's
/// skeleton (15 joints).
pub fn gun_look(level: &Level, weapon: &WeaponState) -> (PoseOverrides, JointMeshes) {
    let joints = level.model(super::LARA).map_or(joint::COUNT, |m| m.joints.len());
    let mut pose = vec![JointPose::default(); joints];
    let mut meshes = vec![None; joints];
    // [x, y, z] turns: the gun code's angles are [yaw, pitch].
    let turn = |[yaw, pitch]: [i16; 2]| [pitch, yaw, 0];
    if let Some(torso) = pose.get_mut(TORSO) {
        torso.after = turn(weapon.torso);
    }
    if let Some(head) = pose.get_mut(HEAD) {
        head.after = turn(weapon.head);
    }
    let Some(pistols) = level.model(PISTOL_MODEL).filter(|_| weapon.gun == Gun::Pistols) else {
        return (PoseOverrides(pose), JointMeshes(meshes));
    };
    if matches!(weapon.status, GunStatus::Drawing | GunStatus::Holstering | GunStatus::Ready) {
        for (arm, joints) in [weapon.left, weapon.right].into_iter().zip(ARMS) {
            let keys = model_frame_key(level, pistols, arm.frame.max(0) as usize);
            for (i, &joint) in joints.iter().enumerate() {
                let Some(p) = pose.get_mut(joint) else { continue };
                p.keys = keys;
                if i == 0 {
                    p.from_root = true;
                    p.before = turn([arm.yaw, arm.pitch]);
                }
            }
        }
    }
    for side in 0..2 {
        let joint = if weapon.in_hand[side] { ARMS[side][2] } else { THIGHS[side] };
        if let (Some(slot), Some(&mesh)) = (meshes.get_mut(joint), pistols.meshes.get(joint)) {
            *slot = Some(mesh);
        }
    }
    (PoseOverrides(pose), JointMeshes(meshes))
}

/// Whether the flash on one side is drawn: DrawLara's gun case draws it
/// while that arm's flash counter is non-zero (0x80026A08, 0x80026C90).
pub fn flash_visible(weapon: &WeaponState, side: usize) -> bool {
    let arm = if side == 0 { weapon.left } else { weapon.right };
    arm.flash != 0
        && weapon.gun == Gun::Pistols
        && matches!(weapon.status, GunStatus::Drawing | GunStatus::Holstering | GunStatus::Ready)
}

/// The gunfire light for this tick, if PistolHandler adds one: only while
/// the guns are ready and either flash is lit.
pub fn gunfire_light(weapon: &WeaponState, feet: IVec3, yaw: i16) -> Option<DynamicLight> {
    (weapon.status == GunStatus::Ready
        && weapon.gun == Gun::Pistols
        && (weapon.left.flash != 0 || weapon.right.flash != 0))
        .then(|| {
            let yaw = i32::from(yaw);
            DynamicLight {
                position: feet + IVec3::new(sin(yaw) >> GUNFIRE_AHEAD_SHIFT, 0, cos(yaw) >> GUNFIRE_AHEAD_SHIFT),
                intensity: GUNFIRE_INTENSITY,
                falloff: GUNFIRE_FALLOFF,
            }
        })
}

/// One muzzle flash drawn at a hand.
#[derive(Component)]
pub struct GunFlash {
    side: usize,
    /// The flash counter and shot count the roll was drawn for.
    rolled_for: (i16, u64),
}

/// Lara's flashes have been spawned.
#[derive(Component)]
struct GunFlashes;

pub struct ArmsPlugin;

impl Plugin for ArmsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DynamicLights>()
            .init_resource::<FlashRandom>()
            .add_systems(Update, (spawn_flashes, dress_guns, gunfire).chain().before(LevelSystems::Pose));
    }
}

/// The draw-side random generator for the flash roll (0x8006E54C). Its
/// sequence is not compared with the original.
#[derive(Resource)]
struct FlashRandom(u32);

impl Default for FlashRandom {
    fn default() -> Self {
        Self(0xd371f947)
    }
}

impl FlashRandom {
    fn roll(&mut self) -> i16 {
        self.0 = self.0.wrapping_mul(1103515245).wrapping_add(12345);
        // (rand << 17) >> 16: twice a 15-bit draw, as a signed angle.
        ((((self.0 >> 10) & 0x7fff) << 1) as u16) as i16
    }
}

fn flash_offset(roll: i16) -> Transform {
    Transform::from_translation(position(FLASH_OFFSET)).with_rotation(engine::animation::joint_turn([
        FLASH_PITCH,
        0,
        roll,
    ]))
}

#[allow(clippy::type_complexity)]
fn spawn_flashes(
    mut commands: Commands,
    world: Option<Res<WorldGeometry>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PsxMaterial>>,
    players: Query<(Entity, &ItemLighting), (With<WeaponState>, Without<GunFlashes>)>,
) {
    let Some(world) = world else { return };
    for (lara, lighting) in &players {
        commands.entity(lara).insert(GunFlashes);
        let data = &world.data;
        let Some(mesh) = data.model(FLASH_MODEL).and_then(|m| data.meshes.get(*m.meshes.first()?)) else { continue };
        let mesh = meshes.add(static_lit_mesh(mesh, &data.textures, FLASH_LIGHT));
        let texture = materials.get(&lighting.0).and_then(|m| m.texture.clone());
        // Semi-transparent quads, mode 0 (B/2 + F/2) in the flash capture.
        let material =
            materials.add(PsxMaterial { texture, lit: false, blend: PsxBlend::Half, double_sided: true, ..default() });
        for (side, arm) in ARMS.iter().enumerate() {
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                flash_offset(0),
                Visibility::Hidden,
                JointAttachment { joint: arm[2], offset: flash_offset(0) },
                GunFlash { side, rolled_for: (0, 0) },
                ChildOf(lara),
                Name::new(format!("GunFlash{side}")),
            ));
        }
    }
}

#[allow(clippy::type_complexity)]
fn dress_guns(
    mut commands: Commands,
    world: Option<Res<WorldGeometry>>,
    mut random: ResMut<FlashRandom>,
    mut players: Query<(Entity, &WeaponState, Option<&mut PoseOverrides>, Option<&mut JointMeshes>)>,
    mut flashes: Query<(&mut GunFlash, &ChildOf, &mut JointAttachment, &mut Visibility)>,
) {
    let Some(world) = world else { return };
    for (lara, weapon, pose, meshes) in &mut players {
        let (new_pose, new_meshes) = gun_look(&world.data, weapon);
        match pose {
            Some(mut pose) => {
                pose.set_if_neq(new_pose);
            }
            None => {
                commands.entity(lara).insert(new_pose);
            }
        }
        match meshes {
            Some(mut meshes) => {
                meshes.set_if_neq(new_meshes);
            }
            None => {
                commands.entity(lara).insert(new_meshes);
            }
        }
    }
    for (mut flash, parent, mut attachment, mut visibility) in &mut flashes {
        let Ok((_, weapon, _, _)) = players.get(parent.parent()) else { continue };
        let shown = flash_visible(weapon, flash.side);
        visibility.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
        let arm = if flash.side == 0 { weapon.left } else { weapon.right };
        // A fresh roll every tick the flash is drawn.
        let key = (arm.flash, weapon.shot_serial);
        if shown && flash.rolled_for != key {
            flash.rolled_for = key;
            attachment.offset = flash_offset(random.roll());
        }
    }
}

fn gunfire(mut lights: ResMut<DynamicLights>, players: Query<(&Player, &WeaponState)>) {
    let yaw = |p: &Player| (p.yaw * 65536.0 / std::f32::consts::TAU).round() as i32 as i16;
    let new = DynamicLights(
        players.iter().filter_map(|(p, w)| gunfire_light(w, original_units(p.position), yaw(p))).collect(),
    );
    lights.set_if_neq(new);
}
