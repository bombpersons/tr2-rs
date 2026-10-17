//! Trigger fixtures are independently authored; asset checks use the real disc extraction.
use bevy::prelude::*;
use engine::{
    controller::Button,
    testing::{TickApp, headless_app},
    world::{
        Level, Room, Sector, WorldGeometry,
        format::{Action, FloorInfo, Trigger, TriggerKind},
    },
};
use game::{
    interactions::*,
    player::{Player, SimulationEnabled, WeaponState},
};
fn app() -> App {
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins(InteractionsPlugin);
    app
}
/// A 2x2-sector room whose first sector has `trigger`.
fn geometry(trigger: Trigger) -> WorldGeometry {
    let mut sectors = vec![Sector { floor: 0, ceiling: -4096, ..Default::default() }; 4];
    sectors[0].floor_info = Some(0);
    let room = Room { top: -4096, bottom: 0, x_sectors: 2, z_sectors: 2, sectors, ..Default::default() };
    let info = FloorInfo { triggers: vec![trigger], ..Default::default() };
    WorldGeometry::new(Level { name: "fixture".into(), rooms: vec![room], floor_infos: vec![info], ..Default::default() })
}
/// A floor trigger with the full code mask that activates placement `index`.
fn activate(index: u16) -> Trigger {
    let action = Action { action: "activate".into(), argument: index, camera: None };
    Trigger { id: 1, kind: TriggerKind::Floor, timer: 0, once: false, mask: 31, actions: vec![action] }
}
#[test]
fn authored_pad_activates_only_its_original_object_index() {
    let mut app = app();
    app.insert_resource(geometry(activate(9)));
    app.world_mut().spawn(Player::at(Vec3::new(0.5, 0.0, 0.5), 0.0, 0));
    let yes = app.world_mut().spawn(WorldObject::new(9, 106, 0, 0)).id();
    let no = app.world_mut().spawn(WorldObject::new(10, 106, 0, 0)).id();
    app.run_tick(&[]);
    assert!(app.world().get::<WorldObject>(yes).unwrap().active);
    assert!(!app.world().get::<WorldObject>(no).unwrap().active);
}
#[test]
fn collecting_same_medipack_twice_is_impossible() {
    let mut app = app();
    app.world_mut().spawn(Player::default());
    let pickup = app.world_mut().spawn(WorldObject::new(5, 149, 0, 0)).id();
    app.run_ticks(3, &[Button::Cross]);
    assert!(app.world().get::<WorldObject>(pickup).unwrap().consumed);
    assert_eq!(app.world().resource::<Inventory>().small_medipacks, 1);
    assert_eq!(app.world().resource::<InteractionProgress>().pickups, 1);
}
#[test]
fn an_unrelated_room_cannot_supply_a_key() {
    let mut app = app();
    app.world_mut().spawn(Player::default());
    app.world_mut().spawn(WorldObject::new(5, 193, 1, 0));
    app.run_tick(&[Button::Cross]);
    assert_eq!(app.world().resource::<Inventory>().items.get(&193), None);
}
#[test]
fn healing_consumes_the_correct_size_and_never_revives() {
    let mut inventory = Inventory::default();
    inventory.add(149);
    inventory.add(150);
    let mut p = Player::default();
    p.health = 100;
    assert!(inventory.use_medipack(false, &mut p));
    assert_eq!(p.health, 600);
    assert!(inventory.use_medipack(true, &mut p));
    assert_eq!(p.health, 1000);
    inventory.add(150);
    p.damage(1000);
    assert!(!inventory.use_medipack(true, &mut p));
    assert_eq!(inventory.large_medipacks, 1);
}
#[test]
fn each_new_pistol_shot_can_hit_once_and_kill_is_counted_once() {
    let mut app = app();
    let p = app.world_mut().spawn(Player::default()).id();
    let mut object = WorldObject::new(4, 39, 0, 0);
    object.active = true;
    let enemy = app.world_mut().spawn((object, Transform::from_xyz(0.0, 0.0, 2.0))).id();
    app.run_tick(&[]);
    app.world_mut().get_mut::<Enemy>(enemy).unwrap().health = 1;
    app.world_mut().get_mut::<WeaponState>(p).unwrap().shot_serial = 1;
    app.run_tick(&[]);
    assert_eq!(app.world().get::<Enemy>(enemy).unwrap().health, 0);
    app.run_ticks(10, &[]);
    assert_eq!(app.world().resource::<InteractionProgress>().kills, 1);
}
#[test]
fn pause_blocks_pickups_and_trigger_activation() {
    let mut app = app();
    app.insert_resource(SimulationEnabled(false));
    app.world_mut().spawn(Player::default());
    let e = app.world_mut().spawn(WorldObject::new(1, 190, 0, 0)).id();
    app.run_tick(&[Button::Cross]);
    assert!(!app.world().get::<WorldObject>(e).unwrap().consumed);
    assert_eq!(app.world().resource::<InteractionProgress>().secrets, 0);
}
#[cfg(feature = "baked-assets")]
#[test]
fn real_wall_start_camera_operand_is_never_an_object_activation() {
    let data: Level = serde_json::from_slice(include_bytes!("../assets/levels/wall/level.json")).unwrap();
    let room = &data.rooms[35];
    let triggers = &data.floor_info(&room.sectors[2 * room.z_sectors + 1]).triggers;
    assert_eq!(triggers.len(), 1);
    let actions: Vec<_> = triggers[0].actions.iter().map(|a| (a.action.as_str(), a.argument)).collect();
    assert_eq!(actions, [("camera", 2)]);
    // The source chain contains a camera setting operand. It must not become
    // an accidental object activation (the real camera operand has value0x8100).
}

#[test]
fn initially_open_object_closes_when_its_authored_reverse_trigger_fires() {
    let mut app = app();
    app.insert_resource(geometry(activate(4)));
    app.world_mut().spawn(Player::at(Vec3::new(0.5, 0.0, 0.5), 0.0, 0));
    let object = WorldObject::new(4, 106, 0, CODE_BITS);
    assert!(object.active);
    let entity = app.world_mut().spawn(object).id();
    app.run_tick(&[]);
    assert!(!app.world().get::<WorldObject>(entity).unwrap().active);
}
