//! Wall's zip-line against the original, tick by tick: Lara takes hold of
//! it in the cave before the T-Rex valley, rides it down into room 39
//! until it stops at the far wall, drops off and lands
//! (tools/record_wall_traps.py, case `zipline`).
//!
//! Unlike the other traps (`tests/traps.rs`), Lara is under test here:
//! she starts standing where the original's Lara stood the tick before
//! she took hold, and from then on runs on her own rules, given the pad
//! input the original read each tick. Her position, state, clip, frame
//! and room are compared exactly with the line's, from the grab, through
//! the ride and the drop, to her landing.
#![cfg(feature = "baked-assets")]

use bevy::prelude::*;
use engine::{
    animation::{AnimationGoal, ControllerAnimated, ObjectAnimation},
    controller::Button,
    objects::{Initialized, WorldObject},
    testing::{TickApp, Trajectory, compare, headless_app},
    world::{Level, WorldGeometry},
};
use game::{
    interactions::InteractionsPlugin,
    items::{ItemBody, ItemStatus},
    player::{AnimationState, Gun, Player, PlayerAnimations, PlayerRulesPlugin, WeaponState},
    traps::{ActiveTraps, Trap},
};

const ZIP_LINE: usize = 93;
/// The last row before Lara takes hold: she stands behind the handle
/// with Action held.
const START: usize = 89;

/// The original's input word (0x8008AAD4) as pad buttons, for the bits
/// these runs hold (docs/slide-jump-evidence.md, docs/pistols-evidence.md).
/// 0x200 (L1, look) and 0x200000 are not read by anything ported.
fn buttons(input: u32) -> Vec<Button> {
    [
        (0x1, Button::Up),
        (0x2, Button::Down),
        (0x10, Button::Square),
        (0x20, Button::Triangle),
        (0x40, Button::Cross),
        (0x80, Button::R1),
        (0x1000, Button::Circle),
    ]
    .into_iter()
    .filter(|&(bit, _)| input & bit != 0)
    .map(|(_, button)| button)
    .collect()
}

fn status_code(status: ItemStatus) -> u32 {
    match status {
        ItemStatus::Inactive => 0,
        ItemStatus::Active => 1,
        ItemStatus::Deactivated => 2,
        ItemStatus::Invisible => 3,
    }
}

fn status_of(code: u32) -> ItemStatus {
    match (code >> 1) & 3 {
        0 => ItemStatus::Inactive,
        1 => ItemStatus::Active,
        2 => ItemStatus::Deactivated,
        _ => ItemStatus::Invisible,
    }
}

const LARA: [&str; 7] = ["x", "y", "z", "state", "animation", "frame", "room"];
const LINE: [&str; 11] =
    ["x", "y", "z", "state", "goal", "animation", "frame", "room", "fall_speed", "flags", "status"];

#[test]
fn the_zip_line_ride_matches_the_recording() {
    compare_ride(include_str!("data/original/traps_zipline.csv"), None);
}

/// Held out: recorded after the ride was ported. Action is let go part of
/// the way down, while the ride clip's range for the drop is shut: Lara
/// hangs on, falling and put back on the line each tick, until it opens.
/// It ends 25 ticks after she lands: then the valley's tiger (item 22,
/// not spawned here) reaches her and pushes her.
#[test]
fn letting_go_of_the_zip_line_matches_the_recording() {
    compare_ride(include_str!("data/original/traps_zipline_release.csv"), Some(450));
}

/// Replay one recorded ride from [`START`] to `last` (default: its end)
/// and compare it.
fn compare_ride(csv: &str, last: Option<usize>) {
    let original = Trajectory::parse_csv(csv).unwrap();
    let data: Level = serde_json::from_slice(include_bytes!("../assets/levels/wall/level.json")).unwrap();
    let clips = PlayerAnimations::from(&data);
    let world = WorldGeometry::new(data);
    let starts: Vec<usize> = world.data.animations.iter().map(|c| c.frame_start).collect();
    let get = |row: usize, name: &str| original.get(row, name).unwrap_or_else(|| panic!("no column {name}"));

    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins((PlayerRulesPlugin, InteractionsPlugin))
        .insert_resource(PlayerAnimations::from(&*world.data))
        .insert_resource(world.clone());

    let line = |name: &str| get(START, &format!("i{ZIP_LINE}_{name}"));
    let status = line("status") as u32;
    let clip = line("animation") as usize;
    let placement = &world.data.placements[ZIP_LINE];
    let mut object = WorldObject::from_placement(ZIP_LINE, placement);
    object.room = line("room") as usize;
    object.flags = line("flags") as u16;
    let zip = app
        .world_mut()
        .spawn((
            object,
            Trap { listed: status & 1 != 0, start: placement.position, start_room: placement.room },
            ItemBody {
                position: [line("x") as i32, line("y") as i32, line("z") as i32],
                yaw: line("yaw") as i16,
                room: line("room") as usize,
                fall_speed: line("fall_speed") as i16,
                floor: line("floor") as i32,
                current_state: line("state") as usize,
                status: status_of(status),
                collidable: status & 0x20 != 0,
                ..default()
            },
            ObjectAnimation { animation: clip, frame: (line("frame") as usize - starts[clip]) as f32 },
            AnimationGoal { state: line("goal") as usize },
            ControllerAnimated,
            Transform::default(),
            Initialized,
        ))
        .id();
    assert!(app.world().resource::<ActiveTraps>().0.is_empty());

    let lara = |name: &str| get(START, &format!("lara_{name}"));
    let at = engine::world::position([lara("x") as i32, lara("y") as i32, lara("z") as i32]);
    let yaw = engine::world::angle(lara("yaw") as i32 as u16);
    let clip = lara("animation") as usize;
    let player = app
        .world_mut()
        .spawn((
            Player::at(at, yaw, lara("room") as usize),
            AnimationState::at_frame(clip, (lara("frame") as usize - starts[clip]) as f32),
            WeaponState::holding(Gun::Pistols),
        ))
        .id();

    let mut names: Vec<String> = LARA.iter().map(|f| format!("lara_{f}")).collect();
    names.extend(LINE.iter().map(|f| format!("i{ZIP_LINE}_{f}")));
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let expected_row = |row: usize| {
        let mut values: Vec<f64> = LARA.iter().map(|f| get(row, &format!("lara_{f}"))).collect();
        for field in LINE {
            let value = get(row, &format!("i{ZIP_LINE}_{field}"));
            values.push(if field == "status" { f64::from(value as u32 & 0xF) } else { value });
        }
        values
    };
    let capture = |app: &mut App| {
        let world = app.world_mut();
        let p = world.get::<Player>(player).unwrap();
        let pose = world.get::<AnimationState>(player).unwrap();
        let feet = (p.position * 1024.0).round();
        let mut values = vec![
            f64::from(feet.x),
            f64::from(-feet.y),
            f64::from(feet.z),
            clips.state(pose.animation).unwrap() as f64,
            pose.animation as f64,
            (starts[pose.animation] + pose.frame as usize) as f64,
            p.room as f64,
        ];
        let body = world.get::<ItemBody>(zip).unwrap();
        let object = world.get::<WorldObject>(zip).unwrap();
        let trap = world.get::<Trap>(zip).unwrap();
        let animation = world.get::<ObjectAnimation>(zip).unwrap();
        let goal = world.get::<AnimationGoal>(zip).unwrap();
        values.extend([
            f64::from(body.position[0]),
            f64::from(body.position[1]),
            f64::from(body.position[2]),
            body.current_state as f64,
            goal.state as f64,
            animation.animation as f64,
            (starts[animation.animation] + animation.frame as usize) as f64,
            body.room as f64,
            f64::from(body.fall_speed),
            f64::from(object.flags),
            f64::from(u32::from(trap.listed) | status_code(body.status) << 1 | u32::from(body.gravity) << 3),
        ]);
        values
    };

    let mut expected = Trajectory::new(&names);
    let mut rebuild = Trajectory::new(&names);
    expected.push(expected_row(START));
    rebuild.push(capture(&mut app));
    for row in START + 1..=last.unwrap_or(original.ticks() - 1) {
        app.run_tick(&buttons(get(row, "input") as u32));
        expected.push(expected_row(row));
        rebuild.push(capture(&mut app));
    }
    if let Ok(path) = std::env::var("ZIPLINE_DUMP") {
        std::fs::write(format!("{path}.port.csv"), rebuild.to_csv()).unwrap();
        std::fs::write(format!("{path}.original.csv"), expected.to_csv()).unwrap();
    }
    let tolerances: Vec<_> = names.iter().map(|&c| (c, 0.0)).collect();
    compare(&expected, &rebuild, &tolerances).unwrap().assert_passed();
}
