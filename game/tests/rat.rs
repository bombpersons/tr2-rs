//! Venice's rats against the original, tick by tick, from a played run
//! (tools/record_wall_traps.py, case `rats`). See
//! [`support::creature_replay`] for what is replayed; the boxes doors left
//! blocked come from the recording too.
#![cfg(feature = "baked-assets")]

mod support;

use game::rat::KIND;
use support::creature_replay::CreatureReplay;

const VENICE: &[u8] = include_bytes!("../assets/levels/boat/level.json");
const SLOT_FIELDS: [&str; 8] =
    ["mood", "max_turn", "flags", "head", "target_x", "target_y", "target_z", "required_box"];

/// Three rats wake: two wander and are left alive, the third bites Lara and
/// is shot dead. One trigger wakes 37 before 36 (the order the original's
/// active list runs them).
#[test]
fn the_rats_match_the_recording() {
    let csv = include_str!("data/original/venice_rats.csv");
    let metadata = include_str!("data/original/venice_rats.json");
    CreatureReplay::in_level(VENICE, csv, KIND, &[37, 36, 45], &SLOT_FIELDS, |_| {})
        .with_blocked_boxes(metadata)
        .assert_matches(|_, _| {});
}
