use bevy::prelude::*;
use engine::world::{Level, Room, WorldGeometry, format::Portal};

fn portal(room: usize, x: i32) -> Portal {
    Portal { room, normal: [0, 0, 1], vertices: vec![[x, 0, 0], [x, -1024, 0], [x, -1024, 1024], [x, 0, 1024]] }
}

#[test]
fn a_long_portal_cycle_cannot_hide_rooms_reachable_by_a_shorter_path() {
    let mut level = Level { rooms: vec![Room::default(); 11], ..default() };
    // The old stack visited 0 -> 2 -> ... -> 8 -> 1 first, exhausting
    // its depth budget at room 1 and never following the direct 0 -> 1.
    level.rooms[0].portals = vec![portal(1, 0), portal(2, 0)];
    for id in 2..8 {
        level.rooms[id].portals = vec![portal(id + 1, 0)];
    }
    level.rooms[8].portals = vec![portal(1, 0)];
    level.rooms[1].portals = vec![portal(0, 0), portal(9, 0)];
    level.rooms[9].portals = vec![portal(10, 0)];
    let mut world = WorldGeometry::new(level);
    assert!(world.visible_rooms(0, Vec3::ZERO).into_iter().all(|visible| visible));
    // File ordering must not change visibility.
    std::sync::Arc::make_mut(&mut world.data).rooms[0].portals.reverse();
    assert!(world.visible_rooms(0, Vec3::ZERO).into_iter().all(|visible| visible));
}

#[test]
fn visibility_still_limits_distance_and_depth_and_handles_invalid_roots() {
    let mut level = Level { rooms: vec![Room::default(); 11], ..default() };
    for id in 0..9 {
        level.rooms[id].portals = vec![portal(id + 1, 0)];
    }
    level.rooms[0].portals.push(portal(10, 30 * 1024));
    let world = WorldGeometry::new(level);
    assert_eq!(
        world.visible_rooms(0, Vec3::ZERO),
        [true, true, true, true, true, true, true, true, true, false, false]
    );
    assert!(world.visible_rooms(usize::MAX, Vec3::ZERO).into_iter().all(|visible| !visible));
}
