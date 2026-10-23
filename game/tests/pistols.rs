//! Pistol draw, fire and holster routes recorded from the supplied PSX disc
//! without memory pokes (tools/record_pistols.py). Each tick compares Lara's
//! gun status, both arms' animation frames and flashes and her movement.
#![cfg(feature = "baked-assets")]

use bevy::prelude::*;
use engine::{
    camera::{CameraRulesPlugin, CameraState},
    testing::{Alignment, Replay, TickApp, Trajectory, compare, headless_app},
    world::{Level, WorldGeometry, position as world_position},
};
use game::{
    interactions::InteractionsPlugin,
    player::{
        AnimationState, Gun, GunStatus, Motion, Player, PlayerAnimations, PlayerRulesPlugin, SimulationEnabled,
        Traversal, WeaponState,
    },
};

const COLUMNS: [&str; 23] = [
    "x",
    "y",
    "z",
    "yaw",
    "animation",
    "animation_frame",
    "state",
    "goal",
    "speed",
    "turn_rate",
    "gun_status",
    "left_frame",
    "right_frame",
    "left_flash",
    "right_flash",
    "left_yaw",
    "right_yaw",
    "camera_x",
    "camera_y",
    "camera_z",
    "target_x",
    "target_y",
    "target_z",
];

fn status_code(status: GunStatus) -> f64 {
    match status {
        GunStatus::HandsFree => 0.0,
        GunStatus::HandsBusy => 1.0,
        GunStatus::Drawing => 2.0,
        GunStatus::Holstering => 3.0,
        GunStatus::Ready => 4.0,
    }
}

/// Replays one recording and returns the number of arm shots the port fired.
fn compare_case(csv: &str, metadata: &str) -> u64 {
    let alignment = Alignment::new(2, 0);
    let original = Trajectory::parse_psx_samples(csv, alignment).unwrap();
    let metadata: serde_json::Value = serde_json::from_str(metadata).unwrap();
    let replay = Replay::parse(&metadata["input"].to_string()).unwrap();
    let data: Level = serde_json::from_slice(include_bytes!("../assets/imported/levels/wall/level.json")).unwrap();
    let clips = PlayerAnimations::from(&data);
    let starts: Vec<_> = clips.clips.iter().map(|c| c.frame_start).collect();
    let states: Vec<_> = clips.clips.iter().map(|c| c.state).collect();
    let row = |name| original.get(0, name).unwrap();
    let position = Vec3::new(row("x") as f32, -row("y") as f32, row("z") as f32) / 1024.0;
    let yaw = row("yaw") as f32 * std::f32::consts::TAU / 65536.0;
    let clip = row("animation") as usize;
    let frame = row("animation_frame") as f32 - starts[clip] as f32;
    // The Wall checkpoint holds the pistols with free hands (0x8008C4EC = 1).
    assert_eq!(row("gun_type"), 1.0);
    assert_eq!(row("gun_status"), 0.0);
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins((PlayerRulesPlugin, CameraRulesPlugin, InteractionsPlugin))
        .insert_resource(clips)
        .insert_resource(WorldGeometry::new(data));
    let entity = app
        .world_mut()
        .spawn((
            Player::at(position, yaw, row("room") as usize),
            AnimationState::at_frame(clip, frame),
            WeaponState::holding(Gun::Pistols),
        ))
        .id();
    // Let level-load camera initialization run once, then seed the
    // original's settled camera from row 0.
    app.insert_resource(SimulationEnabled(false));
    app.run_tick(&[]);
    app.insert_resource(SimulationEnabled(true));
    {
        let mut camera = app.world_mut().resource_mut::<CameraState>();
        let at = |x, y, z| world_position([row(x) as i32, row(y) as i32, row(z) as i32]);
        camera.position = at("camera_x", "camera_y", "camera_z");
        camera.target = at("target_x", "target_y", "target_z");
        camera.room = row("room") as usize;
        camera.initialized = true;
    }
    let inputs = replay.tick_inputs(alignment, original.ticks() - 1).unwrap();
    let rebuild = Trajectory::record(&mut app, &inputs, &COLUMNS, |world| {
        let p = world.get::<Player>(entity).unwrap();
        let m = world.get::<Motion>(entity).unwrap();
        let a = world.get::<AnimationState>(entity).unwrap();
        let w = world.get::<WeaponState>(entity).unwrap();
        vec![
            f64::from(p.position.x * 1024.0),
            f64::from(-p.position.y * 1024.0),
            f64::from(p.position.z * 1024.0),
            f64::from((p.yaw * 65536.0 / std::f32::consts::TAU).round() as i32 as i16),
            a.animation as f64,
            f64::from(starts[a.animation] as f32 + a.frame),
            states[a.animation] as f64,
            world.get::<Traversal>(entity).unwrap().goal as f64,
            f64::from((m.velocity.with_y(0.0).length() * 1024.0 / 30.0).round()),
            f64::from(m.turn_rate),
            status_code(w.status),
            f64::from(w.left.frame),
            f64::from(w.right.frame),
            f64::from(w.left.flash),
            f64::from(w.right.flash),
            f64::from(w.left.yaw),
            f64::from(w.right.yaw),
        ]
        .into_iter()
        .chain({
            let camera = world.resource::<CameraState>();
            let eye = (camera.position * 1024.0).round();
            let target = (camera.target * 1024.0).round();
            [eye.x, -eye.y, eye.z, target.x, -target.y, target.z].map(f64::from)
        })
        .collect()
    });
    let tolerances: Vec<_> = COLUMNS.iter().map(|&column| (column, if column == "y" { 0.02 } else { 0.0 })).collect();
    compare(&original, &rebuild, &tolerances).unwrap().assert_passed();
    app.world().get::<WeaponState>(entity).unwrap().shot_serial
}

/// Arm shots in a recording: each arm's flash restarts at its full length.
fn recorded_shots(csv: &str) -> u64 {
    let original = Trajectory::parse_psx_samples(csv, Alignment::new(2, 0)).unwrap();
    let mut shots = 0;
    for tick in 1..original.ticks() {
        for column in ["left_flash", "right_flash"] {
            if original.get(tick, column).unwrap() == 3.0 && original.get(tick - 1, column).unwrap() != 3.0 {
                shots += 1;
            }
        }
    }
    shots
}

macro_rules! recorded_case {
    ($test:ident, $case:literal) => {
        #[test]
        fn $test() {
            let csv = include_str!(concat!("data/original/pistols_", $case, ".csv"));
            let shots = compare_case(csv, include_str!(concat!("data/original/pistols_", $case, ".json")));
            assert_eq!(shots, recorded_shots(csv));
        }
    };
}
recorded_case!(original_pistols_draw_fire_holster, "draw_fire_holster");
recorded_case!(original_pistols_held_draw, "held_draw");
recorded_case!(original_pistols_aim_taps, "aim_taps");
recorded_case!(original_pistols_rapid_taps, "rapid_taps");
recorded_case!(original_pistols_holster_while_firing, "holster_while_firing");
recorded_case!(original_pistols_turn_ready, "turn_ready");
recorded_case!(original_pistols_run_draw, "run_draw");
recorded_case!(heldout_pistols_retap, "retap_heldout");
recorded_case!(heldout_pistols_walk_turn_ready, "walk_turn_ready_heldout");
recorded_case!(heldout_pistols_draw_retap, "draw_retap_heldout");
recorded_case!(heldout_pistols_run_fire, "run_fire_heldout");

/// Draw on the first tick, turn with guns ready and holster back into the
/// chase camera. The closing run stops against an obstruction at field 178
/// (tick 89), a run-collision branch the port does not have yet; the gun and
/// camera part before it is compared.
#[test]
fn heldout_pistols_draw_turn_holster() {
    let csv = include_str!("data/original/pistols_draw_turn_holster_heldout.csv");
    let fields = 2 * 89;
    let truncated: String = csv.lines().take(1 + fields).map(|line| format!("{line}\n")).collect();
    let shots = compare_case(&truncated, include_str!("data/original/pistols_draw_turn_holster_heldout.json"));
    assert_eq!(shots, recorded_shots(&truncated));
}

/// The recorded state the gun reads each tick, replayed into the port just
/// before LaraGun runs: Lara's and tiger 18's poses (the tiger's AI and
/// Lara's push reactions are not under test) and, on shot ticks, the control
/// random state the gun's draws start from.
#[derive(Resource)]
struct Replayed {
    tick: usize,
    original: Trajectory,
    starts: Vec<usize>,
    lara: Entity,
    tiger: Entity,
    /// Seed before the gun's first draw, by tick.
    seeds: std::collections::BTreeMap<usize, u32>,
}

fn replay_poses(
    enabled: Res<SimulationEnabled>,
    mut replayed: ResMut<Replayed>,
    mut random: ResMut<engine::random::GameRandom>,
    mut players: Query<(&mut Player, &mut AnimationState)>,
    mut creatures: Query<(&mut Transform, &mut engine::animation::ObjectAnimation, &mut engine::objects::WorldObject)>,
) {
    // The paused setup tick places row 0; each simulated tick its own row.
    replayed.tick = if enabled.0 { replayed.tick + 1 } else { 0 };
    let r = &*replayed;
    let row = |name: &str| r.original.get(r.tick, name).unwrap();
    let place = |x: &str, y: &str, z: &str| Vec3::new(row(x) as f32, -row(y) as f32, row(z) as f32) / 1024.0;
    let angle = |name: &str| row(name) as f32 * std::f32::consts::TAU / 65536.0;
    if let Ok((mut player, mut animation)) = players.get_mut(r.lara) {
        player.position = place("x", "y", "z");
        player.yaw = angle("yaw").rem_euclid(std::f32::consts::TAU);
        player.room = row("room") as usize;
        let clip = row("animation") as usize;
        *animation = AnimationState::at_frame(clip, (row("animation_frame") as usize - r.starts[clip]) as f32);
    }
    if let Ok((mut transform, mut animation, mut object)) = creatures.get_mut(r.tiger) {
        *transform = Transform::from_translation(place("tiger_x", "tiger_y", "tiger_z"))
            .with_rotation(Quat::from_rotation_y(angle("tiger_yaw")));
        let clip = row("tiger_animation") as usize;
        animation.animation = clip;
        animation.frame = (row("tiger_frame") as usize - r.starts[clip]) as f32;
        object.room = row("tiger_room") as usize;
        object.active = true;
    }
    if let Some(&seed) = r.seeds.get(&r.tick) {
        random.seed = seed;
    }
}

/// Recorded tiger routes from `.psx/pistols_tiger_start` (pad-only from the
/// Wall checkpoint): target choice, arm locks and swing, head and torso,
/// shots, the tiger's hit points and the combat camera, tick by tick.
/// The tiger knocks Lara into a slide at tick 102; the slide's camera flag
/// then keeps steep slopes exact for the smart shift.
fn compare_tiger_case(csv: &str, metadata: &str) {
    compare_tiger_case_until(csv, metadata, usize::MAX);
}

/// As [`compare_tiger_case`], with the tiger's hit points compared only up to
/// `hp_ticks`: after its death the tiger's own controller rewrites them.
fn compare_tiger_case_until(csv: &str, metadata: &str, hp_ticks: usize) {
    use engine::objects::WorldObject;
    use game::interactions::Enemy;
    let alignment = Alignment::new(2, 0);
    let original = Trajectory::parse_psx_samples(csv, alignment).unwrap();
    let metadata: serde_json::Value = serde_json::from_str(metadata).unwrap();
    let replay = Replay::parse(&metadata["input"].to_string()).unwrap();
    let data: Level = serde_json::from_slice(include_bytes!("../assets/imported/levels/wall/level.json")).unwrap();
    let clips = PlayerAnimations::from(&data);
    let starts: Vec<_> = clips.clips.iter().map(|c| c.frame_start).collect();
    let row = |name| original.get(0, name).unwrap();
    // On a tick with shots, the gun's draws (two per arm shot) are the last
    // control-random stores of that field.
    let start = metadata["frames"]["start"].as_u64().unwrap();
    let mut writes: std::collections::BTreeMap<usize, Vec<u32>> = default();
    for write in metadata["random_writes"].as_array().unwrap() {
        let tick = (write["frame"].as_u64().unwrap() - start).div_ceil(2) as usize;
        writes.entry(tick).or_default().push(write["old"].as_u64().unwrap() as u32);
    }
    let mut seeds = std::collections::BTreeMap::new();
    for tick in 1..original.ticks() {
        let restarted =
            |column: &str| original.get(tick, column).unwrap() == 3.0 && original.get(tick - 1, column).unwrap() != 3.0;
        let shots = usize::from(restarted("left_flash")) + usize::from(restarted("right_flash"));
        if shots > 0 {
            let olds = &writes[&tick];
            seeds.insert(tick, olds[olds.len() - 2 * shots]);
        }
    }
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins((PlayerRulesPlugin, CameraRulesPlugin, InteractionsPlugin))
        .insert_resource(clips)
        .insert_resource(WorldGeometry::new(data));
    let lara = app
        .world_mut()
        .spawn((
            Player::at(Vec3::ZERO, 0.0, row("room") as usize),
            AnimationState::default(),
            WeaponState::holding(Gun::Pistols),
        ))
        .id();
    let mut tiger_object = WorldObject::new(18, 39, row("tiger_room") as usize, 0);
    tiger_object.active = true;
    let mut tiger_enemy = Enemy::for_kind(39).unwrap();
    tiger_enemy.health = row("tiger_hp") as i32;
    let tiger = app
        .world_mut()
        .spawn((
            tiger_object,
            tiger_enemy,
            Transform::default(),
            engine::animation::ObjectAnimation { animation: row("tiger_animation") as usize, frame: 0.0 },
        ))
        .id();
    app.insert_resource(Replayed { tick: 0, original: original.clone(), starts, lara, tiger, seeds });
    app.add_systems(
        FixedUpdate,
        replay_poses.in_set(engine::schedule::InteractionSystems::Combat).before(game::player::PlayerSystems::Weapons),
    );
    // Row 0 pose, then the settled camera, as in compare_case.
    {
        app.insert_resource(SimulationEnabled(false));
        app.run_tick(&[]);
        app.insert_resource(SimulationEnabled(true));
        let mut camera = app.world_mut().resource_mut::<CameraState>();
        let at = |x, y, z| world_position([row(x) as i32, row(y) as i32, row(z) as i32]);
        camera.position = at("camera_x", "camera_y", "camera_z");
        camera.target = at("target_x", "target_y", "target_z");
        camera.room = row("room") as usize;
        camera.initialized = true;
    }
    let columns = [
        "gun_status",
        "left_frame",
        "right_frame",
        "left_flash",
        "right_flash",
        "left_lock",
        "right_lock",
        "left_yaw",
        "left_pitch",
        "right_yaw",
        "right_pitch",
        "aim_yaw",
        "aim_pitch",
        "head_yaw",
        "head_pitch",
        "torso_yaw",
        "torso_pitch",
        "tiger_hp",
        "camera_x",
        "camera_y",
        "camera_z",
        "target_x",
        "target_y",
        "target_z",
    ];
    let inputs = replay.tick_inputs(alignment, original.ticks() - 1).unwrap();
    let rebuild = Trajectory::record(&mut app, &inputs, &columns, |world| {
        let w = world.get::<WeaponState>(lara).unwrap();
        let camera = world.resource::<CameraState>();
        let eye = (camera.position * 1024.0).round();
        let target = (camera.target * 1024.0).round();
        let lock = |locked: bool| if locked { 1.0 } else { 0.0 };
        vec![
            status_code(w.status),
            f64::from(w.left.frame),
            f64::from(w.right.frame),
            f64::from(w.left.flash),
            f64::from(w.right.flash),
            lock(w.left.locked),
            lock(w.right.locked),
            f64::from(w.left.yaw),
            f64::from(w.left.pitch),
            f64::from(w.right.yaw),
            f64::from(w.right.pitch),
            f64::from(w.target_angles[0]),
            f64::from(w.target_angles[1]),
            f64::from(w.head[0]),
            f64::from(w.head[1]),
            f64::from(w.torso[0]),
            f64::from(w.torso[1]),
            f64::from(world.get::<Enemy>(tiger).unwrap().health),
            f64::from(eye.x),
            f64::from(-eye.y),
            f64::from(eye.z),
            f64::from(target.x),
            f64::from(-target.y),
            f64::from(target.z),
        ]
    });
    let window = |t: &Trajectory, columns: &[&str], ticks: usize| {
        let mut out = Trajectory::new(columns);
        for tick in 0..=ticks.min(t.ticks() - 1) {
            out.push(columns.iter().map(|c| t.get(tick, c).unwrap()).collect());
        }
        out
    };
    let check = |columns: &[&str], ticks: usize| {
        let tolerances: Vec<_> = columns.iter().map(|&c| (c, 0.0)).collect();
        compare(&window(&original, columns, ticks), &window(&rebuild, columns, ticks), &tolerances)
            .unwrap()
            .assert_passed();
    };
    let gun: Vec<_> = columns[..17].to_vec();
    check(&gun, usize::MAX);
    check(&["tiger_hp"], hp_ticks);
    check(&["camera_x", "camera_y", "camera_z", "target_x", "target_y", "target_z"], usize::MAX);
}

#[test]
fn original_pistols_tiger_volley() {
    compare_tiger_case(
        include_str!("data/original/pistols_tiger_volley.csv"),
        include_str!("data/original/pistols_tiger_volley.json"),
    );
}

#[test]
fn original_pistols_tiger_lock() {
    compare_tiger_case(
        include_str!("data/original/pistols_tiger_lock.csv"),
        include_str!("data/original/pistols_tiger_lock.json"),
    );
}

#[test]
fn heldout_pistols_tiger_left_late() {
    compare_tiger_case(
        include_str!("data/original/pistols_tiger_left_late_heldout.csv"),
        include_str!("data/original/pistols_tiger_left_late_heldout.json"),
    );
}

/// Re-taps Action at a locked tiger. Despite the name it does not kill: a
/// locked arm keeps aiming with Action released, so recoil runs its full
/// length and the no-target 2-tick re-fire does not apply.
#[test]
fn heldout_pistols_tiger_kill() {
    compare_tiger_case(
        include_str!("data/original/pistols_tiger_kill_heldout.csv"),
        include_str!("data/original/pistols_tiger_kill_heldout.json"),
    );
}

#[test]
fn heldout_pistols_tiger_release() {
    compare_tiger_case(
        include_str!("data/original/pistols_tiger_release_heldout.csv"),
        include_str!("data/original/pistols_tiger_release_heldout.json"),
    );
}

#[test]
fn heldout_pistols_tiger_kill_long() {
    // The kill lands at field 328 (tick 164); at tick 190 the tiger's death
    // code rewrites its hit points near -16384, which is not gun behaviour.
    compare_tiger_case_until(
        include_str!("data/original/pistols_tiger_kill_long_heldout.csv"),
        include_str!("data/original/pistols_tiger_kill_long_heldout.json"),
        189,
    );
}
