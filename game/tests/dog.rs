//! Venice's dogs against the original, tick by tick, from a played run
//! (tools/record_wall_traps.py, cases `dogs` and `dog_shot`). See
//! [`support::creature_replay`] for what is replayed; the boxes doors left
//! blocked come from the recording too.
#![cfg(feature = "baked-assets")]

mod support;

use game::dog::KIND;
use support::creature_replay::CreatureReplay;

const VENICE: &[u8] = include_bytes!("../assets/levels/boat/level.json");
const SLOT_FIELDS: [&str; 8] =
    ["mood", "max_turn", "flags", "head", "target_x", "target_y", "target_z", "required_box"];

/// The first two dogs wake by the canal, run at Lara, leap and bite, and are
/// shot dead; the second wakes while a gunman is about.
#[test]
fn the_first_dogs_match_the_recording() {
    let csv = include_str!("data/original/venice_dogs.csv");
    let metadata = include_str!("data/original/venice_dogs.json");
    CreatureReplay::in_level(VENICE, csv, KIND, &[1, 11], &SLOT_FIELDS, |_| {})
        .with_blocked_boxes(metadata)
        .assert_matches(|_, _| {});
}

/// A dog that bites Lara twice and is shot dead.
#[test]
fn the_shot_dog_matches_the_recording() {
    let csv = include_str!("data/original/venice_dog_shot.csv");
    let metadata = include_str!("data/original/venice_dog_shot.json");
    CreatureReplay::in_level(VENICE, csv, KIND, &[32], &SLOT_FIELDS, |_| {})
        .with_blocked_boxes(metadata)
        .assert_matches(|_, _| {});
}
