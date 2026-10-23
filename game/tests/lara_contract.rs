//! Lara's asset contract (`docs/lara-contract.md`) against the baked levels.
#![cfg(feature = "baked-assets")]
use std::path::PathBuf;

use engine::world::Level;
use game::player::contract::{self, Feature};

fn level(name: &str) -> Level {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/imported/levels").join(name).join("level.json");
    serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap()
}

#[test]
fn wall_meets_the_whole_contract() {
    let report = contract::check(&level("wall"));
    assert!(report.issues.is_empty(), "{report}");
    assert_eq!(report.provided(), Feature::ALL);
}

#[test]
fn every_playable_level_shares_the_contract() {
    // Each level carries its own copy of Lara's table; the controller's
    // numbers hold in all of them (cutscenes and the title have no Lara).
    for name in [
        "assault", "boat", "catacomb", "deck", "emprtomb", "floating", "house", "icecave", "keel", "living",
        "monastry", "opera", "platform", "rig", "skidoo", "unwater", "venice", "xian",
    ] {
        let report = contract::check(&level(name));
        assert!(
            report.meets(&[Feature::Ground, Feature::Jump, Feature::Climb, Feature::Skeleton]),
            "{name}:\n{report}"
        );
    }
}
