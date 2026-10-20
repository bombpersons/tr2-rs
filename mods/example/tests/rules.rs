//! The mod's rules, headless: the game's rules plugins plus the mod's,
//! spawning the mod's own level the way the renderer does.
use bevy::prelude::*;
use engine::{
    animation::ObjectAnimation,
    controller::Button,
    objects::{ObjectDiagnostics, ObjectTypes, SpawnContext, WorldObject},
    schedule::Avatar,
    testing::{TickApp, headless_app},
    world::{Level, WorldGeometry, position},
};
use example_mod::{Chimes, LEVEL, ModPlugin, Spin, Wanderer};
use game::interactions::{InteractionsPlugin, Pickup};

fn app() -> App {
    engine::assets::add_root(example_mod::GAME_ASSETS);
    let path = engine::config::asset_path(&format!("levels/{LEVEL}/level.json"));
    let level: Level = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let world = WorldGeometry::new(level);
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins((game::player::PlayerRulesPlugin, InteractionsPlugin, ModPlugin)).insert_resource(world.clone());
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

#[test]
fn the_mods_level_spawns_every_type() {
    let mut app = app();
    let wanderers = app.world_mut().query::<&Wanderer>().iter(app.world()).count();
    let pickups = app.world_mut().query::<&Pickup>().iter(app.world()).count();
    assert_eq!((wanderers, pickups), (1, 1));
    assert!(app.world().resource::<ObjectDiagnostics>().unregistered_types.is_empty());
}

#[test]
fn the_replaced_pickup_keeps_the_games_hook_and_spins() {
    let mut app = app();
    let spinning = app.world_mut().query_filtered::<(), (With<Pickup>, With<Spin>)>().iter(app.world()).count();
    assert_eq!(spinning, 1);
}

#[test]
fn the_wanderer_walks_onto_the_trigger_and_chimes() {
    let mut app = app();
    let start = app.world_mut().query_filtered::<&Avatar, With<Wanderer>>().single(app.world()).unwrap().position;
    // Facing +z from sector (2, 2): turn an eighth right, toward the
    // trigger's sector (3, 3), and walk onto it.
    for _ in 0..9 {
        app.run_tick(&[Button::Right]);
    }
    let mut chimed = false;
    for _ in 0..40 {
        app.run_tick(&[Button::Up]);
        chimed |= app.world().resource::<Chimes>().0 > 0;
    }
    let end = app.world_mut().query_filtered::<&Avatar, With<Wanderer>>().single(app.world()).unwrap().position;
    assert!(end.distance(start) > 1.0, "the wanderer should walk: {start} -> {end}");
    assert!(chimed, "the floor trigger should fire the mod's chime action");
}
