//! Venice's gunmen against the original, tick by tick, from a played run
//! (tools/record_wall_traps.py, cases `gunman`, `gunman_long` and
//! `gunman_rats`). See [`support::creature_replay`] for what is replayed;
//! the boxes doors left blocked come from the recording too.
#![cfg(feature = "baked-assets")]

mod support;

use support::creature_replay::CreatureReplay;

const VENICE: &[u8] = include_bytes!("../assets/levels/boat/level.json");
const SLOT_FIELDS: [&str; 8] =
    ["mood", "max_turn", "flags", "head", "target_x", "target_y", "target_z", "required_box"];

/// The first gunman (type 17) shoots at Lara by the canal until shot dead.
#[test]
fn the_first_gunman_matches_the_recording() {
    let csv = include_str!("data/original/venice_gunman.csv");
    let metadata = include_str!("data/original/venice_gunman.json");
    CreatureReplay::in_level(VENICE, csv, 17, &[3], &SLOT_FIELDS, |_| {})
        .with_blocked_boxes(metadata)
        .assert_matches(|_, _| {});
}

/// A gunman (type 18) fought at length: Lara boards the motorboat (row
/// 192), drives it under fire, dives off (646) and swims, takes a
/// medipack and shoots him from the bank. The boat (item 26) joined the
/// active list after him, so while it carries her it moves her before he
/// thinks (its control writes her position, `0x80014698`): the recording's
/// `seen_lara_*` columns give her as he saw her.
#[test]
fn the_long_fought_gunman_matches_the_recording() {
    let csv = include_str!("data/original/venice_gunman_long.csv");
    let metadata = include_str!("data/original/venice_gunman_long.json");
    CreatureReplay::in_level(VENICE, csv, 18, &[6], &SLOT_FIELDS, |_| {})
        .with_blocked_boxes(metadata)
        .assert_matches(|_, _| {});
}

/// A gunman (type 18) shot dead among the rats.
#[test]
fn the_gunman_among_the_rats_matches_the_recording() {
    let csv = include_str!("data/original/venice_gunman_rats.csv");
    let metadata = include_str!("data/original/venice_gunman_rats.json");
    CreatureReplay::in_level(VENICE, csv, 18, &[49], &SLOT_FIELDS, |_| {})
        .with_blocked_boxes(metadata)
        .assert_matches(|_, _| {});
}
