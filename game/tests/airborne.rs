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
    let mut actor = Player::at(position, yaw, original.get(0, "room").unwrap() as usize);
    actor.health = original.get(0, "health").unwrap() as i32;
    let falling_fixture = metadata["initial_mode"] == "fast_falling";
    if falling_fixture {
        actor.mode = PlayerMode::FastFalling;
    } else if metadata["initial_mode"] == "fatal_landing" {
        actor.mode = PlayerMode::FatalLanding;
    }
    let entity = app.world_mut().spawn((actor, AnimationState::at_frame(clip, frame))).id();
    if falling_fixture {
        let speed = original.get(0, "speed").unwrap() as f32 * 30.0 / 1024.0;
        let fall = original.get(0, "fall_speed").unwrap() as f32 * 30.0 / 1024.0;
        let mut motion = app.world_mut().get_mut::<Motion>(entity).unwrap();
        motion.grounded = false;
        motion.velocity = (Vec3::new(yaw.sin(), 0.0, yaw.cos()) * speed).with_y(-fall);
        motion.movement_heading = Some(yaw);
        app.world_mut().get_mut::<Traversal>(entity).unwrap().goal = original.get(0, "goal").unwrap() as usize;
    }
    if metadata["initial_mode"] == "fatal_landing" {
        app.world_mut().get_mut::<Traversal>(entity).unwrap().goal = original.get(0, "goal").unwrap() as usize;
    }
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
            PlayerMode::WalkStopping => 2,
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
                let speed = if matches!(p.mode, PlayerMode::JumpingUp { .. }) {
                    m.velocity.dot(p.forward())
                } else if p.mode == PlayerMode::FastFalling {
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
recorded_case!(original_fast_fall_at_140_lands_without_damage, "fall_damage_140");
recorded_case!(original_fast_fall_at_141_loses_five_health, "fall_damage_141");
recorded_case!(original_fast_fall_at_153_loses_862_health, "fall_damage_153");
recorded_case!(original_fast_fall_above_154_uses_the_authored_fatal_landing, "fall_damage_155_impact");
recorded_case!(
    original_fatal_landing_plays_and_holds_the_authored_body_pose_after_the_native_wait,
    "fall_damage_fatal_pose"
);
recorded_case!(heldout_fast_fall_at_142_ignores_jump_during_heavy_recovery, "fall_damage_142_heldout");
recorded_case!(heldout_fast_fall_at_147_recovers_then_walks_with_reduced_health, "fall_damage_147_heldout");
recorded_case!(heldout_fast_fall_at_154_reaches_zero_health_on_contact, "fall_damage_154_heldout");
recorded_case!(heldout_moving_fatal_fast_fall_retains_contact_speed_and_displacement, "fall_damage_155_moving_heldout");
recorded_case!(heldout_fatal_ground_pose_ignores_movement_and_jump_input, "fall_damage_fatal_pose_heldout");
recorded_case!(original_walk_wall_retains_standing_before_a_blocked_start, "walk_wall");
recorded_case!(original_walk_ledge_uses_the_foot_stop_with_standing_control, "walk_ledge");
recorded_case!(original_walk_steps_up_through_authored_foot_clips, "walk_step_up");
recorded_case!(original_walk_steps_down_through_authored_foot_clips, "walk_step_down");
recorded_case!(original_roll_on_an_oblique_runway_uses_authored_motion_and_turn, "roll_runway");
recorded_case!(original_first_half_roll_departure_uses_forward_fall_and_landing, "roll_first_half_fall");
recorded_case!(heldout_first_half_roll_departure_recovers_before_walking, "roll_first_half_walk_heldout");
recorded_case!(
    heldout_first_half_roll_departure_ignores_jump_then_recovers_into_running,
    "roll_first_half_run_heldout"
);
recorded_case!(original_roll_steps_up_without_cancelling_the_authored_second_half, "roll_step_up");
recorded_case!(original_roll_steps_down_then_completes_authored_recovery, "roll_step_down");
recorded_case!(original_roll_wall_keeps_its_speed_and_pose_while_position_is_blocked, "roll_wall");
recorded_case!(heldout_delayed_roll_ignores_turn_input_until_recovery, "roll_runway_delay_heldout");
recorded_case!(heldout_held_roll_repeats_from_the_authored_standing_recovery, "roll_wall_hold_heldout");
recorded_case!(heldout_roll_recovery_hands_a_walk_request_to_its_authored_dispatch, "roll_recovery_walk_heldout");
recorded_case!(heldout_roll_near_a_ledge_finishes_before_departing, "roll_ledge_heldout");
recorded_case!(heldout_roll_departure_uses_backward_fall_and_landing, "roll_ledge_fall_heldout");
recorded_case!(heldout_circle_during_walking_does_not_force_a_roll, "roll_during_walk_heldout");
recorded_case!(
    heldout_walk_release_midway_through_a_descending_step_finishes_the_step,
    "walk_step_down_mid_release_heldout"
);
recorded_case!(heldout_walk_uses_the_other_descending_step_foot, "walk_step_down_other_heldout");
recorded_case!(heldout_walk_uses_the_other_ascending_step_foot, "walk_step_up_other_heldout");
recorded_case!(heldout_walk_release_after_a_descending_step_waits_for_authored_stop, "walk_step_down_release_heldout");
recorded_case!(heldout_walk_ledge_stop_ignores_early_jump_then_refuses_a_restart, "walk_ledge_jump_request_heldout");
recorded_case!(original_vertical_jump_accelerates_forward_only_during_early_ascent, "vertical_steer_forward");
recorded_case!(original_vertical_jump_accelerates_backward_with_signed_speed, "vertical_steer_back_open");
recorded_case!(heldout_vertical_jump_reverses_early_then_ignores_late_steering, "vertical_steer_reverse_heldout");
recorded_case!(heldout_vertical_jump_ignores_steering_after_early_ascent, "vertical_steer_late_heldout");
recorded_case!(original_vertical_jump_waits_for_wall_clearance_before_accelerating, "vertical_steer_wall");
recorded_case!(heldout_vertical_jump_clears_the_wall_and_completes_landing, "vertical_clearance_heldout");
recorded_case!(heldout_delayed_vertical_jump_ignores_jump_during_descent_and_lands, "vertical_clearance_delay_heldout");
recorded_case!(
    calibration_vertical_jump_clears_the_front_floor_before_the_feet_reach_it,
    "braking_wall_validation_heldout"
);
recorded_case!(original_forward_ledge_fall_lands_into_the_authored_running_clip, "forward_ledge_run");
recorded_case!(calibration_released_ledge_fall_exposes_braking_wall_state_override, "forward_ledge_release_heldout");
recorded_case!(calibration_walk_after_ledge_fall_exposes_flight_goal_and_authored_stop, "forward_ledge_walk_heldout");
recorded_case!(heldout_walk_release_waits_for_the_left_authored_stop, "walk_stop_left_heldout");
recorded_case!(heldout_walk_release_waits_for_the_right_authored_stop, "walk_stop_right_heldout");
recorded_case!(heldout_walk_rehold_waits_for_the_committed_stop_before_restarting, "walk_stop_rehold_heldout");
recorded_case!(
    heldout_delayed_walk_request_in_flight_lands_before_walking_and_stopping,
    "forward_ledge_walk_validation_heldout"
);
recorded_case!(
    heldout_braking_wall_contact_retains_its_state_until_the_standing_end_link,
    "braking_wall_release_validation_heldout"
);
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

macro_rules! corner_case {
    ($test:ident, $case:literal) => {
        #[test]
        fn $test() {
            compare_case(
                include_str!(concat!("data/original/corner_", $case, ".csv")),
                include_str!(concat!("data/original/corner_", $case, ".json")),
            );
        }
    };
}

corner_case!(original_forward_jump_side_contact_under_a_low_ceiling_turns_away_from_the_wall, "low_left");
corner_case!(original_forward_jump_side_contact_in_the_open_hall_turns_and_keeps_rising, "open_left");
corner_case!(original_reach_side_contact_turns_without_slowing, "open_reach");
corner_case!(heldout_forward_jump_right_contact_turns_into_a_low_frontal_impact, "open_right_heldout");
corner_case!(heldout_delayed_right_contact_under_a_low_ceiling, "low_right_delay_heldout");
corner_case!(heldout_reach_side_contact_release_landing_and_run, "open_reach_release_heldout");
corner_case!(heldout_right_reach_contact_landing_run_and_release, "open_right_reach_validation_heldout");

macro_rules! fall_case {
    ($test:ident, $case:literal) => {
        #[test]
        fn $test() {
            compare_case(
                include_str!(concat!("data/original/fall_", $case, ".csv")),
                include_str!(concat!("data/original/fall_", $case, ".json")),
            );
        }
    };
}

fall_case!(original_long_jump_into_a_pit_enters_fast_fall_and_heavy_landing, "pit_jump");
fall_case!(original_run_onto_a_steep_ramp_slides_off_its_edge, "ramp_run");
fall_case!(original_backward_hop_onto_a_steep_ramp_slides_off_backwards, "ramp_hop_back");
fall_case!(heldout_angled_pit_jump_fast_falls_into_a_heavy_landing, "pit_angled_heldout");
fall_case!(heldout_delayed_ramp_run_slides_off_and_restarts, "ramp_run_late_heldout");
fall_case!(heldout_backward_hop_from_a_new_start_slides_off_backwards, "ramp_back_near_heldout");

macro_rules! slide_jump_case {
    ($test:ident, $case:literal) => {
        #[test]
        fn $test() {
            compare_case(
                include_str!(concat!("data/original/slide_jump_", $case, ".csv")),
                include_str!(concat!("data/original/slide_jump_", $case, ".json")),
            );
        }
    };
}

slide_jump_case!(original_jump_from_a_forward_slide_takes_the_running_jump, "forward");
slide_jump_case!(original_jump_from_a_back_slide_takes_the_back_jump, "back");
slide_jump_case!(heldout_late_jump_from_a_forward_slide, "forward_late_heldout");
slide_jump_case!(heldout_back_with_jump_keeps_the_forward_slide, "forward_away_heldout");
slide_jump_case!(heldout_jump_held_into_a_back_slide_jumps_at_once, "back_held_heldout");
slide_jump_case!(heldout_forward_with_jump_keeps_the_back_slide, "back_away_heldout");
slide_jump_case!(original_back_during_a_standing_jump_twists_into_a_back_jump, "twist_pit");
slide_jump_case!(original_forward_during_a_slide_back_jump_twists_into_forward_flight, "twist_slide_back");
slide_jump_case!(heldout_roll_during_a_standing_jump_twists, "twist_roll_heldout");
slide_jump_case!(heldout_back_after_the_twist_window_keeps_flying, "twist_late_heldout");
slide_jump_case!(heldout_roll_during_a_slide_back_jump_twists, "back_twist_roll_heldout");
slide_jump_case!(heldout_hop_back_into_a_pit_fast_falls, "hop_pit_heldout");

macro_rules! side_jump_case {
    ($test:ident, $case:literal) => {
        #[test]
        fn $test() {
            compare_case(
                include_str!(concat!("data/original/side_jump_", $case, ".csv")),
                include_str!(concat!("data/original/side_jump_", $case, ".json")),
            );
        }
    };
}

side_jump_case!(original_left_jump_off_the_block_fast_falls_into_the_pit, "left_pit");
side_jump_case!(original_right_jump_across_the_block_lands_on_a_slope, "right_block");
side_jump_case!(original_right_jump_toward_a_wall_jumps_straight_up, "right_wall");
side_jump_case!(original_back_during_a_running_jump_twists_into_a_back_jump, "run_twist");
side_jump_case!(original_action_during_a_hop_fall_reaches_and_grabs, "hop_reach");
side_jump_case!(heldout_right_jump_off_the_block_fast_falls_into_the_pit, "right_pit_heldout");
side_jump_case!(heldout_left_jump_toward_a_wall_jumps_straight_up, "left_wall_heldout");
side_jump_case!(heldout_left_and_right_take_the_left_jump, "left_over_right_heldout");
side_jump_case!(heldout_forward_and_right_take_the_forward_jump, "forward_over_right_heldout");
side_jump_case!(heldout_blocked_right_and_back_take_the_back_jump, "wall_then_back_heldout");
side_jump_case!(heldout_standing_back_jump_fast_falls_into_the_pit, "standing_back_heldout");
side_jump_case!(heldout_opposite_direction_during_a_side_jump_does_not_twist, "side_twist_heldout");
side_jump_case!(heldout_roll_during_a_running_jump_twists, "run_twist_roll_heldout");
side_jump_case!(heldout_turning_running_jump_jumps_forward, "run_turn_jump_heldout");
side_jump_case!(heldout_late_hop_fall_reach_misses_and_fast_falls, "hop_reach_late_heldout");
side_jump_case!(heldout_walking_ignores_jump, "walk_jump_heldout");
