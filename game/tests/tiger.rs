//! Tiger 18 in Wall, recorded from the supplied PSX disc without memory pokes
//! (tools/record_tiger.py): its activation by Lara's floor trigger, AI slot,
//! mood, path target and movement, compared tick by tick.
//!
//! Lara is the tiger's input, not what is under test: before each tick the
//! port is given what the original's tiger control read at that point, all
//! from the previous tick's sample: Lara's pose, speed and health, the
//! tiger's mesh contacts from her collision pass, whether a shot hit it,
//! whether her trigger has activated it, and the control random state.
#![cfg(feature = "baked-assets")]

use bevy::prelude::*;
use engine::{
    animation::{AnimationGoal, ControllerAnimated, ObjectAnimation},
    objects::WorldObject,
    random::GameRandom,
    schedule::{InteractionSystems, WorldSystems},
    testing::{Alignment, Trajectory, compare, headless_app},
    triggers::TriggerSystems,
    world::{Level, WorldGeometry},
};
use game::{
    creature::{CreatureBody, Mood, spawn_components},
    interactions::{Enemy, InteractionsPlugin},
    player::{Motion, Player, SimulationEnabled},
    tiger::KIND,
};

/// Tiger state compared each tick, by the recording's column names.
const COLUMNS: [&str; 24] = [
    "tiger_x",
    "tiger_y",
    "tiger_z",
    "tiger_yaw",
    "tiger_roll",
    "tiger_animation",
    "tiger_frame",
    "tiger_state",
    "tiger_goal",
    "tiger_required",
    "tiger_speed",
    "tiger_room",
    "tiger_box",
    "tiger_health",
    "creature_head",
    "creature_max_turn",
    "creature_flags",
    "creature_mood",
    "creature_target_box",
    "creature_required_box",
    "creature_target_x",
    "creature_target_z",
    "seed",
    "tiger_touch",
];

#[derive(Resource)]
struct Replayed {
    tick: usize,
    starts: Vec<usize>,
    original: Trajectory,
    lara: Entity,
    tiger: Entity,
}

fn replay(
    enabled: Res<SimulationEnabled>,
    mut replayed: ResMut<Replayed>,
    mut random: ResMut<GameRandom>,
    mut players: Query<(&mut Player, &mut Motion)>,
    mut tigers: Query<(&mut WorldObject, &mut CreatureBody, &mut Enemy)>,
) {
    if !enabled.0 {
        return;
    }
    // What the tick about to run reads: the previous tick's end state.
    let r = &*replayed;
    let row = |name: &str| r.original.get(r.tick, name).unwrap();
    if let Ok((mut player, mut motion)) = players.get_mut(r.lara) {
        player.position = Vec3::new(row("lara_x") as f32, -row("lara_y") as f32, row("lara_z") as f32) / 1024.0;
        player.yaw = (row("lara_yaw") as f32 * std::f32::consts::TAU / 65536.0).rem_euclid(std::f32::consts::TAU);
        player.room = row("lara_room") as usize;
        player.health = row("lara_health") as i32;
        // Her signed item speed; the creatures only ask whether it is zero.
        motion.velocity = player.forward() * (row("lara_speed").abs() as f32 * 30.0 / 1024.0);
    }
    if let Ok((_, mut body, mut enemy)) = tigers.get_mut(r.tiger) {
        let status = row("tiger_status") as u32;
        body.touch_bits = row("tiger_touch") as u32;
        body.hit = status & 0x10 != 0;
        let _ = &mut enemy;
    }
    random.seed = row("seed") as u32;
    replayed.tick += 1;
}

/// Lara's collision pass reads her pose after her own movement this tick,
/// which the recording's state after this tick gives.
fn replay_lara_pose(replayed: Res<Replayed>, mut players: Query<(&mut Player, &mut game::player::AnimationState)>) {
    let r = &*replayed;
    let Some(row) = r.original.get(r.tick, "lara_x").map(|_| |name: &str| r.original.get(r.tick, name).unwrap()) else {
        return;
    };
    if let Ok((mut player, mut animation)) = players.get_mut(r.lara) {
        player.position = Vec3::new(row("lara_x") as f32, -row("lara_y") as f32, row("lara_z") as f32) / 1024.0;
        player.yaw = (row("lara_yaw") as f32 * std::f32::consts::TAU / 65536.0).rem_euclid(std::f32::consts::TAU);
        player.room = row("lara_room") as usize;
        player.health = row("lara_health") as i32;
        let clip = row("lara_animation") as usize;
        *animation = game::player::AnimationState::at_frame(clip, (row("lara_frame") as usize - r.starts[clip]) as f32);
    }
}

/// Lara's phase of the tick, after the tiger has moved: her floor trigger
/// activates it and her shots take its hit points. The recording's state
/// after this tick says what they did. Only the tiger's own death writes
/// -16384, so that value is never replayed: the port must write it itself.
fn replay_lara_effects(replayed: Res<Replayed>, mut objects: Query<(&mut WorldObject, &mut Enemy)>) {
    let row = |name: &str| replayed.original.get(replayed.tick, name);
    if let (Some(status), Some(health), Ok((mut object, mut enemy))) =
        (row("tiger_status"), row("tiger_health"), objects.get_mut(replayed.tiger))
    {
        if status as u32 & 1 != 0 {
            object.active = true;
        }
        if health as i32 != -16384 {
            enemy.health = health as i32;
        }
    }
}

fn mood_code(mood: Mood) -> f64 {
    match mood {
        Mood::Bored => 0.0,
        Mood::Attack => 1.0,
        Mood::Escape => 2.0,
        Mood::Stalk => 3.0,
    }
}

/// `shots`: Lara fires in this route. Her gun draws from the same random
/// generator after the tiger, so the seed is compared only without shots.
fn compare_case(csv: &str, shots: bool) {
    compare_case_until(csv, shots, usize::MAX);
}

/// [`compare_case`] up to tick `ticks`.
fn compare_case_until(csv: &str, shots: bool, ticks: usize) {
    // Tick results show in odd fields: row 1 is the state before tick 0.
    let alignment = Alignment::new(2, 1);
    let mut original = Trajectory::parse_psx_samples(csv, alignment).unwrap();
    if original.ticks() > ticks.saturating_add(1) {
        let mut window = Trajectory::new(&original.columns().iter().map(String::as_str).collect::<Vec<_>>());
        for tick in 0..=ticks {
            window.push(original.columns().iter().map(|c| original.get(tick, c).unwrap()).collect());
        }
        original = window;
    }
    let data: Level = serde_json::from_slice(include_bytes!("../assets/levels/wall/level.json")).unwrap();
    let world = WorldGeometry::new(data);
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins(InteractionsPlugin).insert_resource(world.clone());
    let row = |name: &str| original.get(0, name).unwrap();
    let lara = app.world_mut().spawn((Player::at(Vec3::ZERO, 0.0, row("lara_room") as usize), Motion::default())).id();
    let object = WorldObject::from_placement(18, &world.data.placements[18]);
    assert_eq!(object.kind, KIND);
    let setup = Enemy::setup_for_kind(KIND).unwrap();
    let (creature, mut body) = spawn_components(&world, &object, setup);
    // The item's box field is not set until its AI slot is: start from the
    // original's leftover value so the columns compare from tick 0.
    body.box_number = row("tiger_box") as i16;
    body.current_state = row("tiger_state") as usize;
    let clip = row("tiger_animation") as usize;
    let start = world.data.animations[clip].frame_start;
    let tiger = app
        .world_mut()
        .spawn((
            object,
            Enemy::for_kind(KIND).unwrap(),
            creature,
            body,
            ControllerAnimated,
            AnimationGoal { state: row("tiger_goal") as usize },
            ObjectAnimation { animation: clip, frame: (row("tiger_frame") as usize - start) as f32 },
            Transform::default(),
            // Built here, so the type registry must not build it again.
            engine::objects::Initialized,
        ))
        .id();
    let starts: Vec<usize> = world.data.animations.iter().map(|c| c.frame_start).collect();
    app.insert_resource(Replayed { tick: 0, starts: starts.clone(), original: original.clone(), lara, tiger });
    app.add_systems(
        FixedUpdate,
        (
            replay.in_set(WorldSystems::Prepare),
            replay_lara_pose.in_set(InteractionSystems::Use),
            replay_lara_effects.in_set(InteractionSystems::Triggers).before(TriggerSystems::Dispatch),
        ),
    );
    app.insert_resource(SimulationEnabled(true));
    let ticks = original.ticks() - 1;
    let inputs: Vec<Vec<engine::controller::Button>> = vec![vec![]; ticks];
    let mut last_boxes = (0.0, 0.0);
    let rebuild = Trajectory::record(&mut app, &inputs, &COLUMNS, |world| {
        let body = world.get::<CreatureBody>(tiger).unwrap();
        let creature = world.get::<game::creature::Creature>(tiger).unwrap();
        let animation = world.get::<ObjectAnimation>(tiger).unwrap();
        let goal = world.get::<AnimationGoal>(tiger).unwrap();
        let enemy = world.get::<Enemy>(tiger).unwrap();
        // A released slot keeps its last values in the original's memory.
        if let Some(search) = creature.search.as_ref() {
            last_boxes = (f64::from(search.target_box), f64::from(search.required_box));
        }
        vec![
            f64::from(body.position[0]),
            f64::from(body.position[1]),
            f64::from(body.position[2]),
            f64::from(body.yaw),
            f64::from(body.roll),
            animation.animation as f64,
            (starts[animation.animation] + animation.frame as usize) as f64,
            body.current_state as f64,
            goal.state as f64,
            body.required_state as f64,
            f64::from(body.speed),
            body.room as f64,
            f64::from(body.box_number),
            f64::from(enemy.health),
            f64::from(creature.head),
            f64::from(creature.max_turn),
            f64::from(creature.flags),
            mood_code(creature.mood),
            last_boxes.0,
            last_boxes.1,
            f64::from(creature.target[0]),
            f64::from(creature.target[2]),
            f64::from(world.resource::<GameRandom>().seed),
            f64::from(body.touch_bits),
        ]
    });
    if let Ok(path) = std::env::var("TIGER_DUMP") {
        std::fs::write(path, rebuild.to_csv()).unwrap();
    }
    // The port's own contacts (tiger_touch) are dumped but not compared: the
    // original tests spheres before pushing Lara, and the recording only has
    // her pushed position. docs/tiger-evidence.md gives their agreement.
    let tolerances: Vec<_> =
        COLUMNS.iter().filter(|&&c| c != "tiger_touch" && !(shots && c == "seed")).map(|&c| (c, 0.0)).collect();
    compare(&original, &rebuild, &tolerances).unwrap().assert_passed();
}

#[test]
fn original_tiger_stand() {
    compare_case(include_str!("data/original/tiger_stand.csv"), false);
}

#[test]
fn original_tiger_flee() {
    compare_case(include_str!("data/original/tiger_flee.csv"), false);
}

#[test]
fn original_tiger_shoot() {
    compare_case(include_str!("data/original/tiger_shoot.csv"), true);
}

// Held out: recorded after the creature routines and the tiger were written.

/// The tiger bites until Lara dies at tick 471. The original then shows no
/// new frame for several fields and catches up four logic ticks at once,
/// which a tick-per-row replay cannot follow, so the route ends there.
#[test]
fn heldout_tiger_long_stand() {
    compare_case_until(include_str!("data/original/tiger_long_stand_heldout.csv"), false, 471);
}

#[test]
fn heldout_tiger_walk() {
    compare_case(include_str!("data/original/tiger_walk_heldout.csv"), false);
}

#[test]
fn heldout_tiger_circle() {
    compare_case(include_str!("data/original/tiger_circle_heldout.csv"), false);
}

#[test]
fn heldout_tiger_run_back() {
    compare_case(include_str!("data/original/tiger_run_back_heldout.csv"), false);
}
