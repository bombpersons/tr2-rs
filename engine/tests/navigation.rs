//! Hand-authored overlapping boxes exercise the generic corridor query.
use bevy::prelude::*;
use engine::world::{Level, NavBox, Room, Sector, WorldGeometry};

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
