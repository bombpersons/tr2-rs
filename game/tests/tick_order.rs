//! The phase contract is projected from native PSX call sites, not from the
//! engine schedule definition. Controller-specific behaviour has separate tests.
use bevy::prelude::*;
use engine::{
    schedule::{InteractionSystems, ItemSystems, SchedulePlugin, WorldSystems},
    testing::{TickApp, headless_app},
};

#[derive(Resource, Default)]
struct Calls(Vec<&'static str>);
fn mark(phase: &'static str) -> impl FnMut(ResMut<Calls>) {
    move |mut calls| calls.0.push(phase)
}

fn compare_recording(text: &str, tick_count: usize) {
    let recording: serde_json::Value = serde_json::from_str(text).unwrap();
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins(SchedulePlugin).init_resource::<Calls>().add_systems(
        FixedUpdate,
        (
            mark("items").in_set(ItemSystems::Control),
            mark("player_control").in_set(WorldSystems::Control),
            mark("player_animation").in_set(WorldSystems::Animation),
            mark("object_collision").in_set(InteractionSystems::Resolve),
            mark("weapons").in_set(InteractionSystems::Combat),
            mark("triggers").in_set(InteractionSystems::Triggers),
            mark("secondary_motion").in_set(WorldSystems::SecondaryMotion),
            mark("camera").in_set(WorldSystems::Camera),
            mark("sound").in_set(WorldSystems::Effects),
        ),
    );
    let ticks = recording["ticks"].as_array().unwrap();
    assert_eq!(ticks.len(), tick_count);
    for tick in ticks {
        let mut expected: Vec<&str> = tick["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|call| call["phase"].as_str().unwrap())
            // Active effects and separately ordered state collision/room update
            // are not yet represented by the port; do not imply they are.
            .filter(|phase| !["active_effects", "state_collision", "room_update"].contains(phase))
            .collect();
        expected.dedup(); // The native tick has multiple active items; this probe samples the phase once.
        app.run_tick(&[]);
        let actual = std::mem::take(&mut app.world_mut().resource_mut::<Calls>().0);
        assert_eq!(actual, expected, "native field {}", tick["field"]);
    }
}

#[test]
fn engine_phases_follow_both_recorded_native_ticks() {
    compare_recording(include_str!("data/original/tick_order.json"), 2);
}

#[test]
fn engine_phases_match_heldout_turn_recorded_after_schedule_change() {
    compare_recording(include_str!("data/original/tick_order_heldout.json"), 3);
}
