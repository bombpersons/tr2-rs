//! Wall's three crows against the original, tick by tick, from the played
//! run (tools/record_wall_traps.py, case `crows`): roused by Lara's floor
//! trigger, they fly at her, glide, dive and peck; one is shot down. See
//! [`support::creature_replay`] for what is replayed.
#![cfg(feature = "baked-assets")]

mod support;

use game::crow::{BirdInitialised, KIND};
use support::creature_replay::CreatureReplay;

/// The crows, in the order their trigger activates them.
const CROWS: [usize; 3] = [13, 12, 2];

const SLOT_FIELDS: [&str; 7] = ["mood", "max_turn", "flags", "target_x", "target_y", "target_z", "required_box"];

#[test]
fn the_crows_match_the_recording() {
    // The birds' own set-up (their random turn and resting clip) ran
    // before the recording starts.
    CreatureReplay::new(include_str!("data/original/traps_crows.csv"), KIND, &CROWS, &SLOT_FIELDS, |entity| {
        entity.insert(BirdInitialised);
    })
    .assert_matches(|_, _| {});
}
