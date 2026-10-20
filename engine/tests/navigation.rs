//! Hand-authored overlapping boxes exercise the generic corridor query.
use bevy::prelude::*;
use engine::world::{CorridorLimits, Level, NavBox, Room, Sector, WorldGeometry};

fn level() -> Level {
    let sectors =
        (0..4).flat_map(|x| (0..4).map(move |_| Sector { nav_box: Some(usize::from(x == 0)), ..default() })).collect();
    Level {
        rooms: vec![Room { x_sectors: 4, z_sectors: 4, sectors, ..default() }],
        boxes: vec![
            NavBox { x_min: 0, x_max: 4, z_min: 0, z_max: 4, floor: 0, overlap: 0 },
            NavBox { x_min: 0, x_max: 2, z_min: 0, z_max: 4, floor: 0, overlap: 1 },
        ],
        overlaps: vec![0x8001, 0x8000],
        ..default()
    }
}

#[test]
fn adjacent_corridor_keeps_the_perpendicular_coordinate_with_clearance() {
    let world = WorldGeometry::new(level());
    let origin = Vec3::new(2.5, 0.0, 0.25);
    let goal = Vec3::new(0.5, 0.0, 3.0);
    assert_eq!(world.adjacent_path_target(origin, 0, goal, 0, 512), Some(Vec3::new(0.5, 0.0, 0.5)));
    assert_eq!(world.adjacent_path_target(origin.with_z(2.25), 0, goal, 0, 512), Some(Vec3::new(0.5, 0.0, 2.25)));
    // Geometric containment resumes direct following before the sector's
    // navigation id changes to the destination box.
    assert_eq!(world.adjacent_path_target(origin.with_x(1.5), 0, goal, 0, 512), Some(goal));
}

#[test]
fn missing_links_blocked_destinations_and_height_changes_are_explicitly_unsupported() {
    let origin = Vec3::new(2.5, 0.0, 2.0);
    let goal = Vec3::new(0.5, 0.0, 3.0);
    for variant in 0..3 {
        let mut data = level();
        match variant {
            0 => data.overlaps[0] = 0x8000,
            1 => data.boxes[1].overlap |= 0x4000,
            _ => data.boxes[1].floor = 256,
        }
        assert_eq!(WorldGeometry::new(data).adjacent_path_target(origin, 0, goal, 0, 512), None);
    }
}

#[test]
fn caller_limits_accept_boundary_steps_and_drops_but_refuse_the_next_unit() {
    let origin = Vec3::new(2.5, 0.0, 0.25);
    let goal = Vec3::new(0.5, -0.25, 3.0);
    let limits = CorridorLimits { margin: 512, max_step: 256, max_drop: 512, zone: None };
    for (floor, allowed) in [(-257, false), (-256, true), (0, true), (512, true), (513, false)] {
        let mut data = level();
        data.boxes[1].floor = floor;
        let target = WorldGeometry::new(data).adjacent_path_target_with_limits(origin, 0, goal, 0, limits);
        assert_eq!(target, allowed.then_some(Vec3::new(0.5, -0.25, 0.5)), "floor{floor}");
    }
}

#[test]
fn caller_selected_zones_refuse_disconnected_or_missing_movement_classes() {
    let origin = Vec3::new(2.5, 0.0, 2.0);
    let goal = Vec3::new(0.5, -0.25, 3.0);
    for (zones, allowed) in
        [(vec![vec![3, 3]], true), (vec![vec![3, 4]], false), (vec![vec![3]], false), (vec![], false)]
    {
        let mut data = level();
        data.boxes[1].floor = 256;
        data.zones = zones;
        let limits = CorridorLimits { margin: 512, max_step: 256, max_drop: 512, zone: Some(0) };
        let target = WorldGeometry::new(data).adjacent_path_target_with_limits(origin, 0, goal, 0, limits);
        assert_eq!(target.is_some(), allowed);
    }
}
