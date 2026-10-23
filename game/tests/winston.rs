//! Local follow trajectories from ordinary PSX Home replays. Lara follows
//! recorded positions so this isolates the item controller from avatar rules;
//! each item tick reads the preceding avatar pose, as ControlPhase does.
#![cfg(feature = "baked-assets")]
use bevy::prelude::*;
use engine::{
    animation::{AnimationGoal, ObjectAnimation},
    testing::{TickApp, headless_app},
    world::{Level, WorldGeometry, position},
};
use game::{
    creature::{Creature, ItemBody},
    player::Player,
    winston::WinstonRulesPlugin,
};

mod support;
use support::winston::{Pose, place};

fn compare_case(csv: &str) {
    let data: Level = serde_json::from_slice(include_bytes!("../assets/imported/levels/assault/level.json")).unwrap();
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins(WinstonRulesPlugin);
    let rows: Vec<Vec<i32>> = csv.lines().skip(1).map(|l| l.split(',').map(|v| v.parse().unwrap()).collect()).collect();
    let headers: Vec<_> = csv.lines().next().unwrap().split(',').collect();
    let at = |row: &[i32], key: &str| row[headers.iter().position(|&s| s == key).unwrap()];
    let pos = |row: &[i32], prefix: &str| {
        position([at(row, &format!("{prefix}_x")), at(row, &format!("{prefix}_y")), at(row, &format!("{prefix}_z"))])
    };
    let first = &rows[0];
    let world = WorldGeometry::new(data);
    let actor = place(&mut app, &world, &pose(first, &at), None);
    let player = app.world_mut().spawn(Player::at(pos(first, "lara"), 0.0, at(first, "lara_room") as usize)).id();
    app.insert_resource(world);
    let mut previous = first;
    for row in rows.iter().skip(2).step_by(2) {
        let mut avatar = app.world_mut().get_mut::<Player>(player).unwrap();
        avatar.position = pos(previous, "lara");
        avatar.room = at(previous, "lara_room") as usize;
        app.run_tick(&[]);
        let body = app.world().get::<ItemBody>(actor).unwrap();
        let a = app.world().get::<ObjectAnimation>(actor).unwrap();
        let goal = app.world().get::<AnimationGoal>(actor).unwrap();
        let creature = app.world().get::<Creature>(actor).unwrap();
        let clip = &app.world().resource::<WorldGeometry>().data.animations[a.animation];
        let actual = [
            body.position[0],
            body.position[1],
            body.position[2],
            i32::from(body.yaw),
            a.animation as i32,
            clip.frame_start as i32 + a.frame as i32,
            clip.state as i32,
            goal.state as i32,
            i32::from(body.speed),
            body.room as i32,
        ];
        let expected = ["x", "y", "z", "yaw", "animation", "animation_frame", "state", "goal", "speed", "room"]
            .map(|k| at(row, &format!("actor_{k}")));
        assert_eq!(actual, expected, "native field {}", at(row, "run_frame"));
        assert_eq!(
            (creature.target[0], creature.target[2]),
            (at(row, "target_x"), at(row, "target_z")),
            "navigation target at field{}",
            at(row, "run_frame")
        );
        previous = row;
    }
}

/// Winston's recorded item fields in `row`.
fn pose(row: &[i32], at: &dyn Fn(&[i32], &str) -> i32) -> Pose {
    let field = |key: &str| at(row, &format!("actor_{key}"));
    Pose {
        position: [field("x"), field("y"), field("z")],
        yaw: field("yaw") as i16,
        room: field("room") as usize,
        state: field("state") as usize,
        goal: field("goal") as usize,
        animation: field("animation") as usize,
        frame: field("animation_frame") as usize,
        speed: field("speed") as i16,
        touch_bits: 0,
    }
}

#[test]
fn original_local_run_follow_start_and_complete_stop() {
    compare_case(include_str!("data/original/winston_local_run.csv"));
}

#[test]
fn heldout_short_run_settles_after_authored_deceleration() {
    compare_case(include_str!("data/original/winston_heldout_short.csv"));
}

#[test]
fn heldout_diagonal_follow_uses_previous_avatar_pose() {
    compare_case(include_str!("data/original/winston_heldout_turn.csv"));
}

#[test]
fn heldout_walking_departure_starts_and_stops_the_attendant() {
    compare_case(include_str!("data/original/winston_heldout_walk.csv"));
}

#[test]
fn original_neighbor_box_target_keeps_the_corridor_coordinate_then_rejoins_lara() {
    compare_case(include_str!("data/original/winston_nav_run.csv"));
}

#[test]
fn heldout_neighbor_box_short_run_rejoins_lara_before_completing_its_stop() {
    compare_case(include_str!("data/original/winston_nav_short_heldout.csv"));
}

#[test]
fn heldout_neighbor_box_walking_departure_uses_the_same_corridor() {
    compare_case(include_str!("data/original/winston_nav_walk_heldout.csv"));
}

#[test]
fn heldout_neighbor_box_turn_retains_a_corridor_target_before_rejoining() {
    compare_case(include_str!("data/original/winston_nav_turn_heldout.csv"));
}
