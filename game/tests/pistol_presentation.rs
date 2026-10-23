//! Which pistol frames and meshes Lara is drawn with (game::player::arms),
//! stepped through the ported gun rules on Wall's level data. The frame and
//! mesh choices follow DrawLara 0x80025BDC and the mesh-table writes at
//! 0x80048324 / 0x800483C8 / 0x8004843C (docs/pistols-evidence.md).
#![cfg(feature = "baked-assets")]

use bevy::prelude::*;
use engine::animation::{model_pose, model_pose_with};
use engine::lighting::DynamicLight;
use engine::random::GameRandom;
use engine::world::Level;
use game::interactions::pistol_tuning;
use game::player::arms::{PISTOL_MODEL, flash_visible, gun_look, gunfire_light};
use game::player::weapons::{self, GunInput, GunWorld};
use game::player::{Gun, GunStatus, WeaponState};

fn wall() -> Level {
    serde_json::from_slice(include_bytes!("../assets/imported/levels/wall/level.json")).unwrap()
}

/// One LaraGun tick on open ground with nothing to aim at.
fn tick(weapon: &mut WeaponState, input: GunInput) {
    let los = |_: IVec3, _: usize, _: IVec3, _: usize| true;
    let world = GunWorld { position: IVec3::ZERO, room: 0, yaw: 0, targets: &[], line_of_sight: &los };
    weapons::update(weapon, input, true, true, &pistol_tuning(), &world, &mut GameRandom::default(), &mut |_| {});
}

const DRAW: GunInput = GunInput { draw: true, action: false, look: false };
const FIRE: GunInput = GunInput { draw: false, action: true, look: false };
const NONE: GunInput = GunInput { draw: false, action: false, look: false };

/// (left thigh, right thigh, right hand, left hand) mesh overrides.
fn swaps(level: &Level, weapon: &WeaponState) -> [Option<usize>; 4] {
    let (_, meshes) = gun_look(level, weapon);
    [1, 4, 10, 13].map(|joint| meshes.0[joint])
}

#[test]
fn guns_move_from_thighs_to_hands_at_draw_frame_13() {
    let level = wall();
    let pistols = level.model(PISTOL_MODEL).unwrap();
    // Wall's pistol meshes: holstered thighs 15/16, guns in hand 17/18.
    let (thigh_l, thigh_r, hand_r, hand_l) =
        (pistols.meshes[1], pistols.meshes[4], pistols.meshes[10], pistols.meshes[13]);
    let mut weapon = WeaponState::holding(Gun::Pistols);
    // Holstered at the level start (0x80043B9C copies the pistol thighs).
    assert_eq!(swaps(&level, &weapon), [Some(thigh_l), Some(thigh_r), None, None]);
    let (pose, _) = gun_look(&level, &weapon);
    assert!(pose.0[8].keys.is_none(), "free hands draw the arms from Lara's own clip");

    tick(&mut weapon, DRAW);
    while weapon.left.frame < 12 {
        tick(&mut weapon, NONE);
    }
    assert_eq!(weapon.status, GunStatus::Drawing);
    assert_eq!(swaps(&level, &weapon), [Some(thigh_l), Some(thigh_r), None, None], "frame 12 still holstered");
    let (pose, _) = gun_look(&level, &weapon);
    // Frame 12 is key 7 of clip 219 (frames 5..=12); every arm joint uses it,
    // and the upper arms turn from Lara's root, not the torso.
    for joint in [8, 9, 10, 11, 12, 13] {
        assert_eq!(pose.0[joint].keys, Some((219, 7)), "joint {joint}");
    }
    assert!(pose.0[8].from_root && pose.0[11].from_root && !pose.0[9].from_root);

    tick(&mut weapon, NONE);
    assert_eq!(weapon.left.frame, 13);
    assert_eq!(swaps(&level, &weapon), [None, None, Some(hand_r), Some(hand_l)]);
    assert_eq!(gun_look(&level, &weapon).0.0[8].keys, Some((220, 0)));

    // Ready: frame 0 is the first aim key; holding Action raises to 4 and fires.
    while weapon.status != GunStatus::Ready {
        tick(&mut weapon, NONE);
    }
    assert_eq!(gun_look(&level, &weapon).0.0[10].keys, Some((218, 0)));
    for _ in 0..4 {
        tick(&mut weapon, FIRE);
    }
    assert_eq!(gun_look(&level, &weapon).0.0[13].keys, Some((218, 4)));
    assert!(!flash_visible(&weapon, 0) && gunfire_light(&weapon, IVec3::ZERO, 0).is_none());
    tick(&mut weapon, FIRE);
    assert_eq!((weapon.left.frame, weapon.right.frame), (24, 24));
    assert_eq!(gun_look(&level, &weapon).0.0[8].keys, Some((221, 0)), "recoil starts clip 221");
    assert!(flash_visible(&weapon, 0) && flash_visible(&weapon, 1));
    // The gunfire light: 1024 units ahead of Lara's feet (yaw 0 faces +z).
    assert_eq!(
        gunfire_light(&weapon, IVec3::new(100, 200, 300), 0),
        Some(DynamicLight { position: IVec3::new(100, 200, 1324), intensity: 12, falloff: 11 })
    );
    tick(&mut weapon, FIRE);
    tick(&mut weapon, FIRE);
    assert_eq!(gun_look(&level, &weapon).0.0[8].keys, Some((221, 2)));
}

#[test]
fn each_gun_returns_to_its_holster_at_its_own_frame_12() {
    let level = wall();
    let pistols = level.model(PISTOL_MODEL).unwrap();
    // Holstering with the left arm at 13 and the right still recoiling: the
    // right drops to its aim frame first, so the left holsters first.
    let mut weapon = WeaponState::holding(Gun::Pistols);
    weapon.status = GunStatus::Holstering;
    weapon.in_hand = [true, true];
    weapon.left.frame = 13;
    weapon.right.frame = 24;
    tick(&mut weapon, NONE);
    assert_eq!((weapon.left.frame, weapon.right.frame), (12, 4));
    assert_eq!(swaps(&level, &weapon), [Some(pistols.meshes[1]), None, Some(pistols.meshes[10]), None]);
    // Dying frees the hands but leaves the guns where they are.
    let mut dead = weapon.clone();
    let los = |_: IVec3, _: usize, _: IVec3, _: usize| true;
    let world = GunWorld { position: IVec3::ZERO, room: 0, yaw: 0, targets: &[], line_of_sight: &los };
    weapons::update(&mut dead, NONE, false, true, &pistol_tuning(), &world, &mut GameRandom::default(), &mut |_| {});
    assert_eq!(dead.status, GunStatus::HandsFree);
    assert_eq!(swaps(&level, &dead), swaps(&level, &weapon));
}

#[test]
fn without_pistols_lara_keeps_her_own_meshes_and_arms() {
    let level = wall();
    let weapon = WeaponState::holding(Gun::Unarmed);
    let (pose, meshes) = gun_look(&level, &weapon);
    assert!(meshes.0.iter().all(Option::is_none));
    assert!(pose.0.iter().all(|p| p.keys.is_none() && !p.from_root));
}

#[test]
fn torso_turn_moves_the_gun_arms_but_does_not_turn_them() {
    let level = wall();
    let lara = level.model(0).unwrap();
    let mut weapon = WeaponState::holding(Gun::Pistols);
    weapon.status = GunStatus::Ready;
    let straight = gun_look(&level, &weapon).0;
    weapon.torso = [8192, 0];
    let turned = gun_look(&level, &weapon).0;
    let clip = lara.animations[0] + 103;
    let a = model_pose_with(&level, lara, clip, 0.0, &straight.0);
    let b = model_pose_with(&level, lara, clip, 0.0, &turned.0);
    // The torso (7) and head (14) turn; the upper arm (8) keeps its
    // orientation from Lara's root but hangs from the turned shoulder.
    assert!(a[7].rotation.angle_between(b[7].rotation) > 0.7);
    assert!(a[8].rotation.abs_diff_eq(b[8].rotation, 1e-6));
    assert!(a[8].translation.distance(b[8].translation) > 0.03);
    // With no overrides the pose is the clip's.
    assert_eq!(model_pose(&level, lara, clip, 0.0), model_pose_with(&level, lara, clip, 0.0, &[]));
}
