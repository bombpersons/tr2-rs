//! Wall's traps against the original, tick by tick, from one played run
//! (tools/record_wall_traps.py): crumbling tiles, the trapdoor, dart
//! shooters and darts, boulders, sliding and wall blades and the spiked
//! rollers.
//!
//! Lara is the traps' input, not what is under test. Before each tick the
//! port is given what the original's trap controls read then, from the
//! previous tick's row: Lara's pose, speeds, air flag and health, each
//! trap's touch bits from her collision pass, its trigger code and timer,
//! and the control random seed. After Lara's phase, a trap the original's
//! triggers put on the active list joins it. Everything a trap does
//! itself is compared exactly, and so is the damage it deals Lara.
//! There is one row per control tick: ticks the inventory stopped are not
//! there.
#![cfg(feature = "baked-assets")]

use bevy::prelude::*;
use engine::{
    animation::{AnimationGoal, ControllerAnimated, ObjectAnimation},
    objects::{Created, Initialized, WorldObject},
    random::GameRandom,
    schedule::{InteractionSystems, WorldSystems},
    testing::{TickApp, Trajectory, compare, headless_app},
    triggers::{ObjectActivated, TriggerSystems},
    world::{Level, WorldGeometry},
};
use game::{
    interactions::InteractionsPlugin,
    items::{ItemBody, ItemStatus},
    player::{Motion, Player, SimulationEnabled},
    traps::{ActiveTraps, Trap, darts::DART},
};

/// Each trap's fields compared, by the recording's column suffix.
const FIELDS: [&str; 14] = [
    "x",
    "y",
    "z",
    "pitch",
    "yaw",
    "state",
    "goal",
    "animation",
    "frame",
    "room",
    "speed",
    "fall_speed",
    "floor",
    "status",
];

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

/// The original's status word bits a trap's routines change: on the
/// active list (1), status (2-4) and gravity (8).
fn status_bits(trap: &Trap, body: &ItemBody) -> f64 {
    f64::from(u32::from(trap.listed) | status_code(body.status) << 1 | u32::from(body.gravity) << 3)
}

#[derive(Resource)]
struct Replayed {
    /// The row whose state the tick about to run reads.
    row: usize,
    original: Trajectory,
    lara: Entity,
    traps: Vec<(usize, Entity)>,
}

impl Replayed {
    fn get(&self, row: usize, name: &str) -> f64 {
        self.original.get(row, name).unwrap_or_else(|| panic!("no column {name}"))
    }
}

fn replay(
    enabled: Res<SimulationEnabled>,
    replayed: Res<Replayed>,
    mut random: ResMut<GameRandom>,
    mut players: Query<(&mut Player, &mut Motion)>,
    mut traps: Query<(&mut WorldObject, &mut ItemBody), Without<Created>>,
    mut darts: Query<&mut ItemBody, With<Created>>,
) {
    if !enabled.0 {
        return;
    }
    let r = &*replayed;
    let row = |name: &str| r.get(r.row, name);
    if let Ok((mut player, mut motion)) = players.get_mut(r.lara) {
        player.position = Vec3::new(row("lara_x") as f32, -row("lara_y") as f32, row("lara_z") as f32) / 1024.0;
        player.yaw = (row("lara_yaw") as f32 * std::f32::consts::TAU / 65536.0).rem_euclid(std::f32::consts::TAU);
        player.room = row("lara_room") as usize;
        player.health = row("lara_health") as i32;
        let per_second = 30.0 / 1024.0;
        motion.velocity = player.forward() * (row("lara_speed") as f32 * per_second)
            - Vec3::Y * (row("lara_fall_speed") as f32 * per_second);
        motion.grounded = row("lara_status") as u32 & 8 == 0;
    }
    for &(index, entity) in &r.traps {
        let Ok((mut object, mut body)) = traps.get_mut(entity) else { continue };
        body.touch_bits = row(&format!("i{index}_touch")) as u32;
        object.flags = row(&format!("i{index}_flags")) as u16;
        object.timer = row(&format!("i{index}_timer")) as i16;
        object.update_active();
    }
    // A dart the port created takes the touches of the original's dart in
    // the same place.
    for mut body in &mut darts {
        body.touch_bits = (106..136)
            .find(|slot| {
                let at = |axis: &str| r.original.get(r.row, &format!("d{slot}_{axis}"));
                at("kind") == Some(f64::from(DART))
                    && [at("x"), at("y"), at("z")] == body.position.map(|v| Some(f64::from(v)))
            })
            .map_or(0, |slot| r.get(r.row, &format!("d{slot}_touch")) as u32);
    }
    random.seed = row("seed") as u32;
}

/// Lara's triggers, after her phase: the traps the original's put on the
/// active list this tick.
fn replay_triggers(
    replayed: Res<Replayed>,
    traps: Query<(&WorldObject, &Trap)>,
    mut activated: MessageWriter<ObjectActivated>,
) {
    let r = &*replayed;
    for &(index, entity) in &r.traps {
        let Ok((object, trap)) = traps.get(entity) else { continue };
        if !trap.listed && r.get(r.row + 1, &format!("i{index}_status")) as u32 & 1 != 0 {
            activated.write(ObjectActivated { entity, index: object.index });
        }
    }
}

/// The dart slots' live darts: how many, and their summed positions.
fn original_darts(original: &Trajectory, row: usize) -> [f64; 4] {
    let mut darts = [0.0; 4];
    for slot in 106..136 {
        let get = |name: &str| original.get(row, &format!("d{slot}_{name}"));
        let (Some(kind), Some(status)) = (get("kind"), get("status")) else { continue };
        if kind as u16 == DART && status as u32 & 1 != 0 {
            darts[0] += 1.0;
            for (sum, axis) in darts[1..].iter_mut().zip(["x", "y", "z"]) {
                *sum += get(axis).unwrap();
            }
        }
    }
    darts
}

/// Replay one recorded case and compare the traps it names.
fn compare_case(csv: &str, items: &[usize]) {
    let original = Trajectory::parse_csv(csv).unwrap();
    let data: Level = serde_json::from_slice(include_bytes!("../assets/imported/levels/wall/level.json")).unwrap();
    let world = WorldGeometry::new(data);
    let starts: Vec<usize> = world.data.animations.iter().map(|c| c.frame_start).collect();
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins(InteractionsPlugin).insert_resource(world.clone());
    let row0 = |name: &str| original.get(0, name).unwrap_or_else(|| panic!("no column {name}"));
    let lara = app.world_mut().spawn((Player::at(Vec3::ZERO, 0.0, row0("lara_room") as usize), Motion::default())).id();
    let mut traps = Vec::new();
    for &index in items {
        let placement = &world.data.placements[index];
        let field = |name: &str| row0(&format!("i{index}_{name}"));
        let status = field("status") as u32;
        let clip = field("animation") as usize;
        let body = ItemBody {
            position: [field("x") as i32, field("y") as i32, field("z") as i32],
            pitch: field("pitch") as i16,
            yaw: field("yaw") as i16,
            roll: field("roll") as i16,
            room: field("room") as usize,
            speed: field("speed") as i16,
            fall_speed: field("fall_speed") as i16,
            floor: field("floor") as i32,
            current_state: field("state") as usize,
            required_state: field("required") as usize,
            gravity: status & 8 != 0,
            status: status_of(status),
            collidable: status & 0x20 != 0,
            ..default()
        };
        let mut object = WorldObject::from_placement(index, placement);
        object.room = body.room;
        let entity = app
            .world_mut()
            .spawn((
                object,
                Trap { listed: status & 1 != 0, start: placement.position, start_room: placement.room },
                body,
                ObjectAnimation { animation: clip, frame: (field("frame") as usize - starts[clip]) as f32 },
                AnimationGoal { state: field("goal") as usize },
                ControllerAnimated,
                Transform::default(),
                // Built here, so the type registry must not build it again.
                Initialized,
            ))
            .id();
        if status & 1 != 0 {
            app.world_mut().resource_mut::<ActiveTraps>().0.push(entity);
        }
        traps.push((index, entity));
    }
    app.insert_resource(Replayed { row: 0, original: original.clone(), lara, traps: traps.clone() });
    app.add_systems(
        FixedUpdate,
        (
            replay.in_set(WorldSystems::Prepare),
            replay_triggers.in_set(InteractionSystems::Triggers).before(TriggerSystems::Dispatch),
        ),
    );
    app.insert_resource(SimulationEnabled(true));

    let mut columns =
        vec!["lara_damage".to_string(), "darts".into(), "dart_x".into(), "dart_y".into(), "dart_z".into()];
    for &index in items {
        columns.extend(FIELDS.iter().map(|f| format!("i{index}_{f}")));
    }
    let names: Vec<&str> = columns.iter().map(String::as_str).collect();
    let capture = |app: &mut App, health_before: f64| {
        let world = app.world_mut();
        let mut row = vec![health_before - f64::from(world.get::<Player>(lara).unwrap().health)];
        let mut darts = [0.0; 4];
        let mut query = world.query_filtered::<(&WorldObject, &Trap, &ItemBody), With<Created>>();
        for (object, trap, body) in query.iter(world) {
            if object.kind == DART && trap.listed {
                darts[0] += 1.0;
                for axis in 0..3 {
                    darts[axis + 1] += f64::from(body.position[axis]);
                }
            }
        }
        row.extend(darts);
        for &(_, entity) in &traps {
            let body = world.get::<ItemBody>(entity).unwrap();
            let trap = world.get::<Trap>(entity).unwrap();
            let animation = world.get::<ObjectAnimation>(entity).unwrap();
            let goal = world.get::<AnimationGoal>(entity).unwrap();
            row.extend([
                f64::from(body.position[0]),
                f64::from(body.position[1]),
                f64::from(body.position[2]),
                f64::from(body.pitch),
                f64::from(body.yaw),
                body.current_state as f64,
                goal.state as f64,
                animation.animation as f64,
                (starts[animation.animation] + animation.frame as usize) as f64,
                body.room as f64,
                f64::from(body.speed),
                f64::from(body.fall_speed),
                f64::from(body.floor),
                status_bits(trap, body),
            ]);
        }
        row
    };
    let mut expected = Trajectory::new(&names);
    let mut rebuild = Trajectory::new(&names);
    let expected_row = |row: usize| {
        let health = |r: usize| original.get(r, "lara_health").unwrap();
        let damage = if row == 0 { 0.0 } else { (health(row - 1) - health(row)).max(0.0) };
        let mut values = vec![damage];
        values.extend(original_darts(&original, row));
        for &index in items {
            for field in FIELDS {
                let value = original.get(row, &format!("i{index}_{field}")).unwrap();
                values.push(if field == "status" { f64::from(value as u32 & 0xF) } else { value });
            }
        }
        values
    };
    expected.push(expected_row(0));
    rebuild.push(expected_row(0));
    for row in 1..original.ticks() {
        app.world_mut().resource_mut::<Replayed>().row = row - 1;
        let health_before = original.get(row - 1, "lara_health").unwrap();
        app.run_tick(&[]);
        expected.push(expected_row(row));
        rebuild.push(capture(&mut app, health_before));
    }
    if let Ok(path) = std::env::var("TRAPS_DUMP") {
        std::fs::write(format!("{path}.port.csv"), rebuild.to_csv()).unwrap();
        std::fs::write(format!("{path}.original.csv"), expected.to_csv()).unwrap();
    }
    let tolerances: Vec<_> = names.iter().map(|&c| (c, 0.0)).collect();
    compare(&expected, &rebuild, &tolerances).unwrap().assert_passed();
}

#[test]
fn the_trapdoor_matches_the_recording() {
    compare_case(include_str!("data/original/traps_trapdoor.csv"), &[14]);
}

#[test]
fn the_dart_corridor_matches_the_recording() {
    compare_case(include_str!("data/original/traps_darts.csv"), &(31..45).collect::<Vec<_>>());
}

#[test]
fn the_first_crumbling_tiles_match_the_recording() {
    compare_case(include_str!("data/original/traps_tiles_entrance.csv"), &[46, 47]);
}

#[test]
fn the_boulders_match_the_recording() {
    compare_case(include_str!("data/original/traps_boulders.csv"), &[73, 74]);
}

#[test]
fn the_first_sliding_blades_match_the_recording() {
    compare_case(include_str!("data/original/traps_sliding_blades.csv"), &[61, 62]);
}

#[test]
fn the_wall_blades_and_their_tiles_match_the_recording() {
    compare_case(include_str!("data/original/traps_wall_blades.csv"), &[49, 50, 55, 64, 65, 66, 67, 51, 56, 57, 53]);
}

#[test]
fn the_tiles_and_sliding_blades_beyond_match_the_recording() {
    compare_case(include_str!("data/original/traps_tiles_and_blades.csv"), &[82, 83, 84, 75, 76, 77, 78, 79, 80, 81]);
}

#[test]
fn the_spiked_rollers_match_the_recording() {
    compare_case(include_str!("data/original/traps_spindles.csv"), &[90, 91, 88]);
}

/// The trapdoor's trigger is not under Lara's feet: it is on the floor of
/// the pool two rooms below (31 over 3 over 33), which GetHeight reaches by
/// following the rooms below whatever the height. With the game's own
/// triggers, Lara standing where the original's stood the tick it opened
/// sets it off, and she drops through.
#[test]
fn standing_over_the_trapdoor_opens_it_and_drops_lara() {
    let data: Level = serde_json::from_slice(include_bytes!("../assets/imported/levels/wall/level.json")).unwrap();
    let world = WorldGeometry::new(data);
    let placement = world.data.placements[14].clone();
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins((InteractionsPlugin, game::player::PlayerRulesPlugin)).insert_resource(world);
    app.insert_resource(SimulationEnabled(true));
    // traps_trapdoor.csv, tick 47: the trigger lists the trapdoor.
    let start = Vec3::new(53523.0, 3072.0, 34831.0) / 1024.0;
    let lara = app.world_mut().spawn(Player::at(start, 0.0, 31)).id();
    let trapdoor = app.world_mut().spawn(WorldObject::from_placement(14, &placement)).id();
    let mut opened = None;
    for tick in 0..60 {
        app.run_tick(&[]);
        if opened.is_none() && app.world().get::<ItemBody>(trapdoor).is_some_and(|b| b.current_state == 1) {
            opened = Some(tick);
        }
    }
    assert!(opened.is_some(), "the trapdoor never opened");
    assert!(app.world().get::<Trap>(trapdoor).unwrap().listed);
    let player = app.world().get::<Player>(lara).unwrap();
    assert!(player.position.y < start.y - 1.0, "Lara stayed on the trapdoor at {}", player.position);
}
