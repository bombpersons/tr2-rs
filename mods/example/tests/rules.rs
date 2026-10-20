//! The mod's rules, headless: the game's rules plugins plus the mod's,
//! spawning the mod's own level the way the renderer does. Its character is
//! the game's player type, driven by Lara's controller through the mod's
//! hand-made clips.
use bevy::prelude::*;
use engine::{
    animation::{AnimationLibrary, ObjectAnimation},
    controller::Button,
    objects::{ObjectDiagnostics, ObjectTypes, SpawnContext, WorldObject},
    testing::{TickApp, headless_app},
    world::{Level, WorldGeometry, position},
};
use example_mod::{CHARACTER_FEATURES, Chimes, LEVEL, ModPlugin, Spin};
use game::interactions::{InteractionsPlugin, Pickup};
use game::player::contract::{Feature, clip, state};
use game::player::{AnimationState, Player, PlayerMode};

fn level() -> Level {
    engine::assets::add_root(example_mod::GAME_ASSETS);
    let path = engine::config::asset_path(&format!("levels/{LEVEL}/level.json"));
    serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap()
}

fn app() -> App {
    let level = level();
    let clips = AnimationLibrary::from(&level);
    let world = WorldGeometry::new(level);
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins((game::player::PlayerRulesPlugin, InteractionsPlugin, ModPlugin))
        .insert_resource(world.clone())
        .insert_resource(clips);
    let root = app.world_mut().spawn_empty().id();
    for (index, placement) in world.data.placements.iter().enumerate() {
        let hook = app.world().resource::<ObjectTypes>().get(placement.kind).and_then(|t| t.spawn_hook().cloned());
        let mut commands = app.world_mut().commands();
        let mut entity = commands.spawn(Transform::from_translation(position(placement.position)));
        match hook {
            Some(hook) => hook(
                &mut entity,
                &SpawnContext { index, placement, world: &world, render_root: root, meshes: &[], lit_material: None },
            ),
            None => {
                entity.insert((
                    WorldObject::from_placement(index, placement),
                    ObjectAnimation { animation: 0, frame: 0.0 },
                ));
            }
        }
    }
    app.world_mut().flush();
    app.run_tick(&[]);
    app
}

fn character(app: &mut App) -> (Player, AnimationState) {
    let (player, animation) =
        app.world_mut().query::<(&Player, &AnimationState)>().single(app.world()).expect("one character");
    (player.clone(), animation.clone())
}

fn clip_state(app: &App, animation: &AnimationState) -> Option<usize> {
    app.world().resource::<AnimationLibrary>().state(animation.animation)
}

#[test]
fn the_character_meets_the_ground_part_of_laras_contract() {
    let report = example_mod::character_contract(&level());
    assert!(report.meets(CHARACTER_FEATURES), "{report}");
    // The rest is filled with standing clips, and reported as such.
    assert!(!report.meets(&[Feature::Jump]));
}

#[test]
fn the_mods_level_spawns_every_type() {
    let mut app = app();
    let players = app.world_mut().query::<&Player>().iter(app.world()).count();
    let pickups = app.world_mut().query::<&Pickup>().iter(app.world()).count();
    assert_eq!((players, pickups), (1, 1));
    assert!(app.world().resource::<ObjectDiagnostics>().unregistered_types.is_empty());
}

#[test]
fn the_replaced_pickup_keeps_the_games_hook_and_spins() {
    let mut app = app();
    let spinning = app.world_mut().query_filtered::<(), (With<Pickup>, With<Spin>)>().iter(app.world()).count();
    assert_eq!(spinning, 1);
}

#[test]
fn the_character_settles_standing() {
    let mut app = app();
    for _ in 0..30 {
        app.run_tick(&[]);
    }
    let (player, animation) = character(&mut app);
    assert_eq!(player.mode, PlayerMode::Idle);
    assert_eq!(clip_state(&app, &animation), Some(state::STAND), "clip {}", animation.animation);
}

#[test]
fn the_character_runs_onto_the_trigger_chimes_and_stops() {
    let mut app = app();
    let (start, _) = character(&mut app);
    let mut chimed = false;
    let mut clips = vec![];
    for _ in 0..60 {
        app.run_tick(&[Button::Up]);
        chimed |= app.world().resource::<Chimes>().0 > 0;
        clips.push(character(&mut app).1.animation);
    }
    let (running, _) = character(&mut app);
    assert!(clips.contains(&clip::RUN_START) && clips.contains(&clip::RUN), "run start into the run loop: {clips:?}");
    assert_eq!(running.mode, PlayerMode::Run);
    assert!(
        running.position.z - start.position.z > 1.0,
        "the character should run: {} -> {}",
        start.position,
        running.position
    );
    assert!(chimed, "the floor trigger should fire the mod's chime action");
    // Release: the run stops through its authored range and stands.
    for _ in 0..30 {
        app.run_tick(&[]);
    }
    let (stopped, animation) = character(&mut app);
    assert_eq!(stopped.mode, PlayerMode::Idle);
    assert_eq!(clip_state(&app, &animation), Some(state::STAND));
}

#[test]
fn the_character_walks_and_turns() {
    let mut app = app();
    let (start, _) = character(&mut app);
    let mut walked = vec![];
    for _ in 0..40 {
        app.run_tick(&[Button::Up, Button::R1]);
        walked.push(character(&mut app).1.animation);
    }
    let (walking, _) = character(&mut app);
    assert_eq!(walking.mode, PlayerMode::Walk);
    assert!(walked.contains(&clip::WALK_START) && walked.contains(&clip::WALK), "{walked:?}");
    let distance = walking.position.z - start.position.z;
    assert!(distance > 0.3, "the character should walk: {distance}");
    for _ in 0..30 {
        app.run_tick(&[]);
    }
    // Turning on the spot, through the turn clip.
    let (before, _) = character(&mut app);
    let mut turned = vec![];
    for _ in 0..20 {
        app.run_tick(&[Button::Right]);
        turned.push(character(&mut app).1.animation);
    }
    let (after, _) = character(&mut app);
    assert!(turned.contains(&clip::TURN_RIGHT), "{turned:?}");
    assert!(after.yaw != before.yaw, "the character should turn");
    assert!(after.position.distance(before.position) < 0.01, "turning stays on the spot");
}

#[test]
fn unsupported_moves_do_not_panic() {
    // Jump, roll, hop back and sidestep ask for clips the character only has
    // standing fillers for.
    for buttons in [&[Button::Square][..], &[Button::Circle], &[Button::Down], &[Button::R2, Button::Left]] {
        let mut app = app();
        for _ in 0..60 {
            app.run_tick(buttons);
        }
        for _ in 0..60 {
            app.run_tick(&[]);
        }
        let (player, _) = character(&mut app);
        assert!(player.health > 0, "{buttons:?}");
    }
}
