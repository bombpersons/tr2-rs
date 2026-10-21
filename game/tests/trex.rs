//! Wall's two T-Rexes against the original, tick by tick, from a played
//! run (tools/record_wall_traps.py, cases `trex_shot` and `trex_kill`). See
//! [`support::creature_replay`] for what is replayed.
#![cfg(feature = "baked-assets")]

mod support;

use engine::animation::AnimationState;
use game::{
    player::{Player, PlayerMode},
    trex::KIND,
};
use support::creature_replay::CreatureReplay;

const SLOT_FIELDS: [&str; 9] =
    ["mood", "max_turn", "flags", "head", "neck", "target_x", "target_y", "target_z", "required_box"];

/// The far T-Rex wakes, runs at Lara, stops to roar and walks, and is shot
/// dead.
#[test]
fn the_shot_t_rex_matches_the_recording() {
    CreatureReplay::new(include_str!("data/original/traps_trex_shot.csv"), KIND, &[104], &SLOT_FIELDS, |_| {})
        .assert_matches(|_, _| {});
}

/// The hidden T-Rex appears, chases and bites Lara, and takes her in its
/// jaws: both go into their kill clips, she at its position and facing.
#[test]
fn the_t_rex_that_kills_lara_matches_the_recording() {
    let csv = include_str!("data/original/traps_trex_kill.csv");
    let mut killed = None;
    CreatureReplay::new(csv, KIND, &[99], &SLOT_FIELDS, |_| {}).assert_matches(|replay, row| {
        let world = replay.app.world();
        let player = world.get::<Player>(replay.lara).unwrap();
        let PlayerMode::Killed { animation, set_camera } = player.mode else { return };
        if killed.is_some() {
            return;
        }
        killed = Some(row);
        let get = |name: &str| replay.original.get(row, name).unwrap();
        // Her clip and frame as CreatureKill leaves them; her own
        // animation's advance this tick takes her one frame on.
        let pose = world.get::<AnimationState>(replay.lara).unwrap();
        assert_eq!(animation, get("lara_animation") as usize);
        assert_eq!(pose.animation, animation);
        assert_eq!(replay.starts[animation] + pose.frame as usize + 1, get("lara_frame") as usize);
        assert!(set_camera);
        let position = (player.position * 1024.0).round();
        assert_eq!(
            [position.x, -position.y, position.z],
            [get("lara_x"), get("lara_y"), get("lara_z")].map(|v| v as f32)
        );
        let yaw = (player.yaw * 65536.0 / std::f32::consts::TAU).round() as i32 as i16;
        assert_eq!(yaw, get("lara_yaw") as i16);
        assert_eq!(player.room, get("lara_room") as usize);
        assert!(player.health <= 0);
    });
    assert_eq!(killed, Some(1942), "the T-Rex should take Lara on the recorded tick");
}
