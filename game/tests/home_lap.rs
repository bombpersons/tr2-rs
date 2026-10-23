//! A lap of the assault course at Lara's Home against the original, tick
//! by tick (tools/record_wall_traps.py, case `lap`, from the handoff
//! `.psx/handoffs/home_lap`): vaults, jumps and grabs, the ladders and the
//! shimmy, the slides, the swan dive into the pool, wading and climbing
//! out, and the zip-line at the end.
//!
//! Lara starts standing where the original's Lara first stood still, with
//! the course's objects placed as the level places them, and from then on
//! runs on her own rules, given the input word the original read each
//! tick. Her position, clip, frame and room are compared exactly. In runs
//! that record Winston, he starts where the save has him and is compared
//! too, with the control random and the idle search replayed.
#![cfg(feature = "baked-assets")]

use bevy::prelude::*;
use engine::{
    animation::ObjectAnimation,
    controller::Button,
    objects::{ObjectTypes, SpawnContext, WorldObject},
    random::GameRandom,
    testing::{TickApp, Trajectory, compare, headless_app},
    world::{BoxFlags, Level, NO_BOX, WorldGeometry, angle, position},
};
use game::{
    creature::{self, IdleSearch, Mood},
    interactions::InteractionsPlugin,
    player::{AnimationState, Player, PlayerAnimations, PlayerRulesPlugin},
    winston,
};

mod support;
use support::winston::{Pose, place};

const HOME: &[u8] = include_bytes!("../assets/imported/levels/assault/level.json");
const LAP: &str = include_str!("data/original/home_lap.csv");
/// The first row Lara stands still: she ran into the course, stopped
/// against the first block and stands before vaulting it.
const START: usize = 191;
/// A second lap, held out from the port's work on the first (case `lap2`,
/// `.psx/handoffs/home_lap2`): from the house, down a ladder to its foot,
/// jumps back off the ladders and up onto them. Lara stands still in the
/// house from its first row.
const HELD_OUT: &str = include_str!("data/original/home_lap2.csv");
/// Ladder edges and walks brushing walls at the side (case `ladders`,
/// `.psx/handoffs/home_ladders`), from the house.
const LADDERS: &str = include_str!("data/original/home_ladders.csv");
/// Runs and walks into the lattice wall with Up+Action, and walks along
/// walls and windows (case `walls`, `.psx/handoffs/home_walls`), from the
/// house.
const WALLS: &str = include_str!("data/original/home_walls.csv");
const LARA: [&str; 7] = ["x", "y", "z", "yaw", "animation", "frame", "room"];
/// Winston's columns, in runs that record him (item 1 from Lara's at Home).
const WINSTON: [&str; 7] = ["i1_x", "i1_y", "i1_z", "i1_yaw", "i1_animation", "i1_frame", "i1_room"];

/// The original's input word (0x8008AAD4) as pad buttons, for the bits the
/// lap holds. 0x100000 (set with Action as it is first pressed) is not
/// read by anything ported.
fn buttons(input: u32) -> Vec<Button> {
    [
        (0x1, Button::Up),
        (0x2, Button::Down),
        (0x4, Button::Left),
        (0x8, Button::Right),
        (0x10, Button::Square),
        (0x20, Button::Triangle),
        (0x40, Button::Cross),
        (0x80, Button::R1),
        (0x200, Button::L1),
        (0x1000, Button::Circle),
    ]
    .into_iter()
    .filter(|&(bit, _)| input & bit != 0)
    .map(|(_, button)| button)
    // The pad's R2 with Left or Right reaches the handlers as the step
    // bits 0x400 and 0x800 (0x80069634..0x800696F0).
    .chain(
        [(0x400, Button::Left), (0x800, Button::Right)]
            .into_iter()
            .filter(|&(bit, _)| input & bit != 0)
            .flat_map(|(_, side)| [Button::R2, side]),
    )
    .collect()
}

/// Home's objects as the renderer spawns them, without Lara.
fn spawn_objects(app: &mut App, world: &WorldGeometry) {
    let root = app.world_mut().spawn_empty().id();
    for (index, placement) in world.data.placements.iter().enumerate() {
        if placement.kind == game::player::LARA {
            continue;
        }
        let hook = app.world().resource::<ObjectTypes>().get(placement.kind).and_then(|t| t.spawn_hook().cloned());
        let first = world.data.model(placement.kind).map_or(0, |m| m.animations[0]);
        let mut commands = app.world_mut().commands();
        let mut entity = commands.spawn(
            Transform::from_translation(position(placement.position))
                .with_rotation(Quat::from_rotation_y(angle(placement.rotation))),
        );
        match hook {
            Some(hook) => hook(
                &mut entity,
                &SpawnContext { index, placement, world, render_root: root, meshes: &[], lit_material: None },
            ),
            None => {
                entity.insert((
                    WorldObject::from_placement(index, placement),
                    ObjectAnimation { animation: first, frame: 0.0 },
                ));
            }
        }
    }
    app.world_mut().flush();
}

/// Winston where the run's first row has him: the save the runs start from
/// is mid-game, with him active and holding AI slot 0, not at his
/// level-start placement. His slot is rebuilt from the recorded boxes and
/// its search run to the end.
fn seed_winston(app: &mut App, get: &dyn Fn(&str) -> f64) -> Entity {
    let world = app.world().resource::<WorldGeometry>().clone();
    let mut objects = app.world_mut().query::<(Entity, &WorldObject)>();
    let placed = objects.iter(app.world()).find(|(_, o)| o.kind == winston::KIND).map(|(e, _)| e);
    let field = |name: &str| get(&format!("i1_{name}"));
    let pose = Pose {
        position: [field("x") as i32, field("y") as i32, field("z") as i32],
        yaw: field("yaw") as i16,
        room: field("room") as usize,
        state: field("state") as usize,
        goal: field("goal") as usize,
        animation: field("animation") as usize,
        frame: field("frame") as usize,
        speed: field("speed") as i16,
        touch_bits: field("touch") as u32,
    };
    let entity = place(app, &world, &pose, placed);
    let mut creature = app.world_mut().get_mut::<creature::Creature>(entity).unwrap();
    creature.mood = [Mood::Bored, Mood::Attack, Mood::Escape, Mood::Stalk][get("s0_mood") as usize];
    creature.max_turn = get("s0_max_turn") as i16;
    let search = creature.search.as_mut().unwrap();
    let (required, target_box) = (get("s0_required_box") as i16, get("s0_target_box") as i16);
    search.required_box = if required == NO_BOX { target_box } else { required };
    while search.update(&world.data, &BoxFlags::new(&world.data), false, 1) {}
    search.required_box = required;
    search.target = [get("s0_target_x") as i32, get("s0_target_y") as i32, get("s0_target_z") as i32];
    entity
}

/// Replay a recorded lap from row `start` to `last` and compare Lara.
fn compare_lap(lap: &str, start: usize, last: usize) {
    let original = Trajectory::parse_csv(lap).unwrap();
    let data: Level = serde_json::from_slice(HOME).unwrap();
    let world = WorldGeometry::new(data);
    let starts: Vec<usize> = world.data.animations.iter().map(|c| c.frame_start).collect();
    let get = |row: usize, name: &str| original.get(row, name).unwrap_or_else(|| panic!("no column {name}"));

    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins((PlayerRulesPlugin, InteractionsPlugin))
        .insert_resource(PlayerAnimations::from(&*world.data))
        .insert_resource(world.clone());
    spawn_objects(&mut app, &world);
    let winston = original.get(start, "i1_x").is_some().then(|| seed_winston(&mut app, &|name| get(start, name)));
    if winston.is_some() {
        // Winston draws from the control random as he picks boxes to wander to.
        app.insert_resource(GameRandom::new(get(start, "seed") as u32));
    }
    // The original searches a box at a time while it waits for the next
    // field, as far as the wait allows: the recording's visits are made
    // between ticks, from where its round-robin cursor stood.
    let idle = original.get(start, "idle_steps").is_some();
    if idle {
        let cursor = (get(start, "search_cursor") + get(start, "idle_steps")) as usize % 5;
        app.insert_resource(IdleSearch { steps: None, cursor });
    }

    let lara = |name: &str| get(start, &format!("lara_{name}"));
    let at = position([lara("x") as i32, lara("y") as i32, lara("z") as i32]);
    let yaw = angle(lara("yaw") as i32 as u16);
    let clip = lara("animation") as usize;
    let player = app
        .world_mut()
        .spawn((
            Player::at(at, yaw, lara("room") as usize),
            AnimationState::at_frame(clip, (lara("frame") as usize - starts[clip]) as f32),
        ))
        .id();

    let mut names: Vec<String> = LARA.iter().map(|f| format!("lara_{f}")).collect();
    if winston.is_some() {
        names.extend(WINSTON.iter().map(|f| f.to_string()));
        names.push("seed".into());
    }
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let expected_row = |row: usize| names.iter().map(|&f| get(row, f)).collect::<Vec<_>>();
    let capture = |app: &mut App| {
        let world = app.world();
        let p = world.get::<Player>(player).unwrap();
        let pose = world.get::<AnimationState>(player).unwrap();
        let feet = (p.position * 1024.0).round();
        let mut row = vec![
            f64::from(feet.x),
            f64::from(-feet.y),
            f64::from(feet.z),
            f64::from((p.yaw * 65536.0 / std::f32::consts::TAU).round() as i32 as i16),
            pose.animation as f64,
            (starts[pose.animation] + pose.frame as usize) as f64,
            p.room as f64,
        ];
        if let Some(winston) = winston {
            let body = world.get::<creature::ItemBody>(winston).unwrap();
            let a = world.get::<ObjectAnimation>(winston).unwrap();
            let [x, y, z] = body.position;
            row.extend([
                f64::from(x),
                f64::from(y),
                f64::from(z),
                f64::from(body.yaw),
                a.animation as f64,
                (starts[a.animation] + a.frame as usize) as f64,
                body.room as f64,
                f64::from(world.resource::<GameRandom>().seed),
            ]);
        }
        row
    };

    let mut expected = Trajectory::new(&names);
    let mut rebuild = Trajectory::new(&names);
    expected.push(expected_row(start));
    rebuild.push(capture(&mut app));
    for row in start + 1..=last {
        if idle {
            app.world_mut().resource_mut::<IdleSearch>().steps = Some(get(row, "idle_steps") as usize);
        }
        app.run_tick(&buttons(get(row, "input") as u32));
        expected.push(expected_row(row));
        rebuild.push(capture(&mut app));
    }
    if let Ok(path) = std::env::var("HOME_LAP_DUMP") {
        std::fs::write(format!("{path}.port.csv"), rebuild.to_csv()).unwrap();
        std::fs::write(format!("{path}.original.csv"), expected.to_csv()).unwrap();
    }
    let tolerances: Vec<_> = names.iter().map(|&c| (c, 0.0)).collect();
    compare(&expected, &rebuild, &tolerances).unwrap().assert_passed();
}

#[test]
fn the_assault_course_lap_matches_the_recording() {
    compare_lap(LAP, START, Trajectory::parse_csv(LAP).unwrap().ticks() - 1);
}

#[test]
fn a_held_out_lap_matches_the_recording() {
    compare_lap(HELD_OUT, 0, Trajectory::parse_csv(HELD_OUT).unwrap().ticks() - 1);
}

#[test]
fn ladder_edges_and_wall_brushes_match_the_recording() {
    compare_lap(LADDERS, 0, Trajectory::parse_csv(LADDERS).unwrap().ticks() - 1);
}

/// Includes the windows (type 72) pushing Lara back before her walk
/// collision probes, beside a sill.
#[test]
fn lattice_jumps_and_wall_walks_match_the_recording() {
    compare_lap(WALLS, 0, Trajectory::parse_csv(WALLS).unwrap().ticks() - 1);
}
