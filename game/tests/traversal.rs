//! Traversal routes recorded from the supplied PSX disc without memory pokes.
#![cfg(feature = "baked-assets")]

use bevy::prelude::*;
use engine::{
    testing::{Alignment, Replay, Trajectory, compare, headless_app},
    world::{Level, WorldGeometry},
};
use game::player::{AnimationState, Motion, Player, PlayerAnimations, PlayerMode, PlayerRulesPlugin, Traversal};

fn compare_case(csv: &str, metadata: &str) {
    let alignment = Alignment::new(2, 0);
    let original = Trajectory::parse_psx_samples(csv, alignment).unwrap();
    let metadata: serde_json::Value = serde_json::from_str(metadata).unwrap();
    let replay = Replay::parse(&metadata["input"].to_string()).unwrap();
    let data: Level = serde_json::from_slice(&std::fs::read("assets/levels/assault/level.json").unwrap()).unwrap();
    let clips = PlayerAnimations::from(&data);
    let starts: Vec<_> = clips.clips.iter().map(|c| c.frame_start).collect();
    let states: Vec<_> = clips.clips.iter().map(|c| c.state).collect();
    let position = Vec3::new(
        original.get(0, "x").unwrap() as f32,
        -original.get(0, "y").unwrap() as f32,
        original.get(0, "z").unwrap() as f32,
    ) / 1024.0;
    let yaw = original.get(0, "yaw").unwrap() as f32 * std::f32::consts::TAU / 65536.0;
    let clip = original.get(0, "animation").unwrap() as usize;
    let frame = original.get(0, "animation_frame").unwrap() as f32 - starts[clip] as f32;
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins(PlayerRulesPlugin).insert_resource(clips).insert_resource(WorldGeometry::new(data));
    let entity = app
        .world_mut()
        .spawn((
            Player::at(position, yaw, original.get(0, "room").unwrap() as usize),
            AnimationState::at_frame(clip, frame),
        ))
        .id();
    let inputs = replay.tick_inputs(alignment, original.ticks() - 1).unwrap();
    let columns = [
        "x",
        "y",
        "z",
        "yaw",
        "animation",
        "animation_frame",
        "state",
        "room",
        "speed",
        "fall_speed",
        "health",
        "turn_rate",
        "goal",
    ];
    let rebuild = Trajectory::record(&mut app, &inputs, &columns, |world| {
        let p = world.get::<Player>(entity).unwrap();
        let m = world.get::<Motion>(entity).unwrap();
        let a = world.get::<AnimationState>(entity).unwrap();
        // Vault collision temporarily overrides the clip's state 2 with 19.
        let state = if matches!(p.mode, PlayerMode::Climbing { .. }) { 19 } else { states[a.animation] };
        vec![
            f64::from(p.position.x * 1024.0),
            f64::from(-p.position.y * 1024.0),
            f64::from(p.position.z * 1024.0),
            f64::from((p.yaw * 65536.0 / std::f32::consts::TAU).round() as i32 as i16),
            a.animation as f64,
            f64::from(starts[a.animation] as f32 + a.frame),
            state as f64,
            p.room as f64,
            f64::from((m.velocity.with_y(0.0).length() * 1024.0 / 30.0).round()),
            f64::from(-m.velocity.y * 1024.0 / 30.0),
            f64::from(p.health),
            f64::from(m.turn_rate),
            world.get::<Traversal>(entity).unwrap().goal as f64,
        ]
    });
    let tolerances: Vec<_> = columns.iter().map(|&column| (column, if column == "y" { 0.02 } else { 0.0 })).collect();
    compare(&original, &rebuild, &tolerances).unwrap().assert_passed();
}

#[test]
fn original_three_click_climb_matches_entry_offset_and_authored_end() {
    compare_case(
        include_str!("data/original/traversal_three_click.csv"),
        include_str!("data/original/traversal_three_click.json"),
    );
}

#[test]
fn heldout_three_click_climb_ignores_release_and_turn_until_authored_end() {
    compare_case(
        include_str!("data/original/traversal_three_click_heldout.csv"),
        include_str!("data/original/traversal_three_click_heldout.json"),
    );
}

#[test]
fn original_jump_up_grab_hang_and_pull_up_match_every_tick() {
    compare_case(
        include_str!("data/original/traversal_hang_climb.csv"),
        include_str!("data/original/traversal_hang_climb.json"),
    );
}

#[test]
fn original_release_from_hang_matches_drop_and_landing() {
    compare_case(
        include_str!("data/original/traversal_hang_drop.csv"),
        include_str!("data/original/traversal_hang_drop.json"),
    );
}

#[test]
fn heldout_early_pull_up_finishes_after_all_buttons_are_released() {
    compare_case(
        include_str!("data/original/traversal_hang_early_climb_heldout.csv"),
        include_str!("data/original/traversal_hang_early_climb_heldout.json"),
    );
}

#[test]
fn heldout_release_during_the_grab_pose_uses_the_current_animation_bounds() {
    compare_case(
        include_str!("data/original/traversal_hang_early_drop_heldout.csv"),
        include_str!("data/original/traversal_hang_early_drop_heldout.json"),
    );
}

#[test]
fn original_forward_jump_reaches_catches_and_climbs_the_ledge() {
    compare_case(
        include_str!("data/original/traversal_forward_climb.csv"),
        include_str!("data/original/traversal_forward_climb.json"),
    );
}

#[test]
fn original_forward_jump_lands_through_the_collision_animation_pass() {
    compare_case(
        include_str!("data/original/traversal_forward_landing.csv"),
        include_str!("data/original/traversal_forward_landing.json"),
    );
}

macro_rules! recorded_case {
    ($test:ident, $case:literal) => {
        #[test]
        fn $test() {
            compare_case(
                include_str!(concat!("data/original/traversal_", $case, ".csv")),
                include_str!(concat!("data/original/traversal_", $case, ".json")),
            );
        }
    };
}

recorded_case!(original_running_jump_uses_authored_launch_velocity_and_landing, "running_jump");
recorded_case!(early_running_jump_exposes_the_authored_other_foot_takeoff, "running_jump_heldout");
recorded_case!(heldout_forward_grab_can_be_released_before_the_hang_settles, "forward_drop_heldout");
recorded_case!(heldout_releasing_action_before_takeoff_refuses_the_ledge, "hang_no_action_heldout");
recorded_case!(heldout_forward_jump_can_resume_running_on_its_landing_tick, "landing_run_heldout");
recorded_case!(heldout_jump_request_waits_for_the_landing_cancel_range, "landing_cancel_heldout");
recorded_case!(original_standing_jump_matches_animation_and_recovery_as_well_as_position, "standing_jump");
recorded_case!(
    heldout_running_jump_validates_the_other_foot_after_dispatch_was_fixed,
    "running_jump_validation_heldout"
);
