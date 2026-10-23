//! A Venice club man against the original, tick by tick, from a played run
//! (tools/record_wall_traps.py, case `club_man`). See
//! [`support::creature_replay`] for what is replayed; the boxes doors left
//! blocked come from the recording too.
#![cfg(feature = "baked-assets")]

mod support;

use game::club_man::KIND;
use support::creature_replay::CreatureReplay;

const VENICE: &[u8] = include_bytes!("../assets/imported/levels/boat/level.json");
const SLOT_FIELDS: [&str; 9] =
    ["mood", "max_turn", "flags", "head", "neck", "target_x", "target_y", "target_z", "required_box"];

/// A club man wakes, comes at Lara, hits her once and is shot dead.
#[test]
fn the_club_man_matches_the_recording() {
    let csv = include_str!("data/original/venice_club_man.csv");
    let metadata = include_str!("data/original/venice_club_man.json");
    CreatureReplay::in_level(VENICE, csv, KIND, &[10], &SLOT_FIELDS, |_| {})
        .with_blocked_boxes(metadata)
        .assert_matches(|_, _| {});
}
