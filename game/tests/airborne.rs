//! Airborne collision routes recorded from the supplied PSX disc. Metadata
//! distinguishes ordinary routes from controlled initial-position fixtures.
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
    let data: Level = serde_json::from_slice(
        &std::fs::read(format!("assets/levels/{}/level.json", metadata["level"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
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
        // Collision can override the clip's authored state until its end link.
        let state = match p.mode {
            PlayerMode::Climbing { .. } => 19,
            PlayerMode::JumpImpact => 14,
            PlayerMode::HitWall { .. } => 12,
            _ => states[a.animation],
        };
        vec![
            f64::from(p.position.x * 1024.0),
            f64::from(-p.position.y * 1024.0),
            f64::from(p.position.z * 1024.0),
            f64::from((p.yaw * 65536.0 / std::f32::consts::TAU).round() as i32 as i16),
            a.animation as f64,
            f64::from(starts[a.animation] as f32 + a.frame),
            state as f64,
            p.room as f64,
            f64::from({
                // Ground collision can refresh the backward probe heading
                // after the entry move. Only state9 has signed rebound speed.
                let speed = if p.mode == PlayerMode::FastFalling {
                    let heading = m.movement_heading.unwrap_or(p.yaw);
                    m.velocity.dot(Vec3::new(heading.sin(), 0.0, heading.cos()))
                } else {
                    m.velocity.with_y(0.0).length()
                };
                (speed * 1024.0 / 30.0).round()
            }),
            f64::from(-m.velocity.y * 1024.0 / 30.0),
            f64::from(p.health),
            f64::from(m.turn_rate),
            world.get::<Traversal>(entity).unwrap().goal as f64,
        ]
    });
    let tolerances: Vec<_> = columns.iter().map(|&column| (column, if column == "y" { 0.02 } else { 0.0 })).collect();
    compare(&original, &rebuild, &tolerances).unwrap().assert_passed();
}

macro_rules! recorded_case {
    ($test:ident, $case:literal) => {
        #[test]
        fn $test() {
            compare_case(
                include_str!(concat!("data/original/airborne_", $case, ".csv")),
                include_str!(concat!("data/original/airborne_", $case, ".json")),
            );
        }
    };
}

#[test]
fn original_action_before_low_ceiling_contact_waits_for_the_authored_reach_range() {
    compare_case(include_str!("data/original/ceiling_reach.csv"), include_str!("data/original/ceiling_reach.json"));
}

#[test]
fn original_reach_ceiling_contact_keeps_its_pose_and_forward_speed() {
    compare_case(
        include_str!("data/original/ceiling_reach_late.csv"),
        include_str!("data/original/ceiling_reach_late.json"),
    );
}

#[test]
fn calibration_reach_ceiling_release_exposes_the_authored_run_stop() {
    compare_case(
        include_str!("data/original/ceiling_reach_late_release_heldout.csv"),
        include_str!("data/original/ceiling_reach_late_release_heldout.json"),
    );
}

#[test]
fn heldout_delayed_reach_ceiling_contact_ignores_jump_during_recovery() {
    compare_case(
        include_str!("data/original/ceiling_reach_late_delay_heldout.csv"),
        include_str!("data/original/ceiling_reach_late_delay_heldout.json"),
    );
}

#[test]
fn heldout_reach_ceiling_release_validates_landing_run_and_authored_stop() {
    compare_case(
        include_str!("data/original/ceiling_reach_late_validation_heldout.csv"),
        include_str!("data/original/ceiling_reach_late_validation_heldout.json"),
    );
}

#[test]
fn original_vertical_jump_under_a_low_ceiling_requests_its_authored_landing() {
    compare_case(
        include_str!("data/original/ceiling_vertical.csv"),
        include_str!("data/original/ceiling_vertical.json"),
    );
}

#[test]
fn heldout_delayed_vertical_jump_with_action_uses_ceiling_recovery_without_grabbing() {
    compare_case(
        include_str!("data/original/ceiling_vertical_delay_heldout.csv"),
        include_str!("data/original/ceiling_vertical_delay_heldout.json"),
    );
}

#[test]
fn heldout_held_vertical_jump_repeats_through_authored_ceiling_recovery() {
    compare_case(
        include_str!("data/original/ceiling_vertical_hold_heldout.csv"),
        include_str!("data/original/ceiling_vertical_hold_heldout.json"),
    );
}

#[test]
fn original_forward_jump_under_a_low_ceiling_keeps_its_heading_and_horizontal_speed() {
    compare_case(include_str!("data/original/ceiling_forward.csv"), include_str!("data/original/ceiling_forward.json"));
}

#[test]
fn heldout_delayed_forward_jump_under_a_low_ceiling_preserves_ordinary_landing() {
    compare_case(
        include_str!("data/original/ceiling_forward_delay_heldout.csv"),
        include_str!("data/original/ceiling_forward_delay_heldout.json"),
    );
}

#[test]
fn heldout_early_release_and_recovery_jump_under_a_low_ceiling() {
    compare_case(
        include_str!("data/original/ceiling_forward_release_heldout.csv"),
        include_str!("data/original/ceiling_forward_release_heldout.json"),
    );
}

recorded_case!(original_forward_jump_rebounds_into_fast_fall_and_heavy_landing, "forward_wall");
recorded_case!(original_forward_ledge_fall_lands_into_the_authored_running_clip, "forward_ledge_run");
recorded_case!(heldout_delayed_ledge_fall_lands_running_then_hits_a_wall, "forward_ledge_delay_heldout");
recorded_case!(
    heldout_released_ledge_fall_ignores_airborne_jump_and_recovers_before_running,
    "forward_ledge_release_validation_heldout"
);
recorded_case!(original_reach_keeps_speed_against_the_wall_and_uses_ordinary_landing, "reach_wall");
recorded_case!(original_turning_jump_uses_the_previous_collision_heading_before_rebounding, "forward_steer");
recorded_case!(heldout_delayed_rebound_ignores_jump_during_heavy_landing, "forward_delay_heldout");
recorded_case!(heldout_released_reach_lands_before_resuming_backward_walk, "reach_release_heldout");
recorded_case!(original_middle_wall_rebounds_without_changing_the_jump_clip, "middle_wall");
recorded_case!(original_wall_blocks_forward_takeoff_and_uses_the_vertical_jump, "blocked_takeoff");
recorded_case!(original_running_jump_hits_a_low_wall, "running_low_wall");
recorded_case!(heldout_middle_impact_ignores_another_jump_while_airborne, "middle_delay_heldout");
recorded_case!(calibration_low_impact_exposes_fast_turn_goal_before_its_dispatch_window, "low_cancel_heldout");
recorded_case!(heldout_low_impact_validates_recovery_and_authored_left_fast_turn, "low_cancel_validation_heldout");
recorded_case!(original_run_start_wall_stop_retries_a_held_jump, "run_start_retry_probe");
recorded_case!(heldout_releasing_jump_before_run_start_wall_contact_keeps_lara_grounded, "run_start_release_heldout");
recorded_case!(heldout_early_held_jump_retries_after_run_start_wall_contact, "run_start_hold_heldout");
recorded_case!(heldout_run_release_uses_the_left_authored_braking_clip, "run_stop_left_heldout");
recorded_case!(heldout_run_release_uses_the_right_authored_braking_clip, "run_stop_right_heldout");
recorded_case!(calibration_reholding_run_exposes_the_committed_braking_clip, "run_stop_rehold_heldout");
recorded_case!(heldout_reholding_run_waits_for_the_committed_braking_clip, "run_stop_rehold_validation_heldout");
recorded_case!(heldout_restarting_run_after_a_late_braking_request, "run_stop_restart_heldout");
