use super::*;
use crate::world::{Sector, Texture, Trigger, TriggerKind, WorldGeometry, position};
use bevy::math::Vec3;

/// A `width` x `depth` room: a ring of solid border sectors around open
/// floor at 0 and ceiling at -2048, with walls facing in, wound like
/// imported levels and sharing vertices between faces.
fn box_level(width: usize, depth: usize) -> Level {
    let mut room = Room { x_sectors: width, z_sectors: depth, top: -2048, ..Default::default() };
    let open = |x: usize, z: usize| x > 0 && z > 0 && x + 1 < width && z + 1 < depth;
    for x in 0..width {
        for z in 0..depth {
            let (floor, ceiling) = if open(x, z) { (0, -2048) } else { (SOLID, SOLID) };
            room.sectors.push(Sector { floor, ceiling, ..Default::default() });
        }
    }
    let mut added = BTreeMap::new();
    for x in 0..width {
        for z in 0..depth {
            if !open(x, z) {
                continue;
            }
            let (x0, z0) = (x as i32 * SECTOR, z as i32 * SECTOR);
            let square = |y| [[x0, y, z0], [x0 + SECTOR, y, z0], [x0 + SECTOR, y, z0 + SECTOR], [x0, y, z0 + SECTOR]];
            for (y, toward, texture) in [(0, [0, -1, 0], 0), (-2048, [0, 1, 0], 0)] {
                let c = square(y).map(|p| vertex_at(&mut room, p, 128, &mut added));
                let c = facing(&room, c, toward);
                room.quads.push([c[0], c[1], c[2], c[3], texture]);
            }
            let sides = [(-1, 0), (1, 0), (0, -1), (0, 1)];
            for (dx, dz) in sides {
                if open((x as i32 + dx) as usize, (z as i32 + dz) as usize) {
                    continue;
                }
                let foot = square(0);
                let edge = match (dx, dz) {
                    (-1, _) => [foot[0], foot[3]],
                    (1, _) => [foot[1], foot[2]],
                    (_, -1) => [foot[0], foot[1]],
                    _ => [foot[3], foot[2]],
                };
                let top = |p: [i32; 3]| [p[0], -2048, p[2]];
                let c =
                    [edge[0], edge[1], top(edge[1]), top(edge[0])].map(|p| vertex_at(&mut room, p, 128, &mut added));
                let c = facing(&room, c, [-dx, 0, -dz]);
                room.quads.push([c[0], c[1], c[2], c[3], 1]);
            }
        }
    }
    let texture = Texture { uv: [[0.0; 2]; 4], attribute: 0 };
    Level { format: 1, rooms: vec![room], textures: vec![texture.clone(), texture], ..Default::default() }
}

fn face_at(level: &Level, x: usize, z: usize, surface: Surface) -> usize {
    let room = &level.rooms[0];
    (0..room.quads.len())
        .find(|&i| sector_face(room, &room.quads[i]).is_some_and(|f| (f.x, f.z, f.surface) == (x, z, surface)))
        .unwrap()
}

fn sector(level: &Level, x: usize, z: usize) -> Sector {
    level.rooms[0].sectors[x * level.rooms[0].z_sectors + z]
}

/// Every floor and ceiling face matches its sector, and collision agrees.
fn consistent(level: &Level) {
    let mismatches = sector_mismatches(level, 0);
    assert!(mismatches.is_empty(), "{mismatches:?}");
    crate::editor::validate_level(level).unwrap();
}

#[test]
fn planes_and_corners_invert_each_other_and_match_collision() {
    for surface in [Surface::Floor, Surface::Ceiling] {
        for tx in -3..=3 {
            for tz in -3..=3 {
                let c = corners(surface, -1024, [tx, tz]);
                assert_eq!(plane(surface, c), Ok((-1024, [tx, tz])), "{surface:?} {c:?}");
            }
        }
    }
    // The floor and ceiling a sloped sector draws are where collision finds them.
    let mut level = box_level(3, 3);
    level.floor_infos.push(FloorInfo { floor_slope: [2, -1], ceiling_slope: [-1, 3], ..Default::default() });
    level.rooms[0].sectors[4] = Sector { floor: -512, ceiling: -2048, floor_info: Some(0), ..Default::default() };
    let world = WorldGeometry::new(level.clone());
    let floor = corners(Surface::Floor, -512, [2, -1]);
    let ceiling = corners(Surface::Ceiling, -2048, [-1, 3]);
    for (i, (dx, dz)) in [(0, 0), (1023, 0), (0, 1023), (1023, 1023)].into_iter().enumerate() {
        let hit = world.floor_at(position([1024 + dx, -100, 1024 + dz]), 0).unwrap();
        assert!(
            (-hit.height * 1024.0 - floor[i] as f32).abs() <= 1.0,
            "floor corner {i}: {} vs {}",
            -hit.height * 1024.0,
            floor[i]
        );
        assert!((-hit.ceiling * 1024.0 - ceiling[i] as f32).abs() <= 1.0, "ceiling corner {i}");
    }
    assert!(plane(Surface::Floor, [0, -256, 0, 0]).is_err(), "one raised corner is not a sector plane");
    assert!(plane(Surface::Floor, [0, -100, 0, -100]).is_err(), "slopes rise in whole steps");
}

/// The floor vertices on the line `x` across the room.
fn floor_line(level: &Level, x: i32) -> BTreeSet<usize> {
    let r = &level.rooms[0];
    (0..r.vertices.len()).filter(|&v| r.vertices[v][0] == x && r.vertices[v][1] == 0).collect()
}

#[test]
fn moving_floor_vertices_rederives_heights_and_slopes_of_every_touched_sector() {
    let mut level = box_level(5, 4);
    consistent(&level);
    // Faces share corners, so raising one line of them tilts the sectors on both sides.
    let line = floor_line(&level, 2048);
    move_vertices(&mut level, 0, &line, [0, -256, 0]).unwrap();
    consistent(&level);
    for z in 1..3 {
        assert_eq!(level.floor_info(&sector(&level, 1, z)).floor_slope, [1, 0]);
        assert_eq!(level.floor_info(&sector(&level, 2, z)).floor_slope, [-1, 0]);
        assert_eq!(sector(&level, 1, z).floor, -256, "a sloped floor's height is its highest corner");
        assert_eq!(sector(&level, 3, z).floor, 0, "sectors not touched stay");
    }
    move_vertices(&mut level, 0, &line, [0, 256, 0]).unwrap();
    consistent(&level);
    assert_eq!(sector(&level, 1, 1).floor_info, None, "flat again, the sector needs no floor data");
    // Steps that are not whole, a lone corner, or leaving the grid are refused.
    assert!(move_vertices(&mut level.clone(), 0, &line, [0, -100, 0]).is_err());
    let r = &level.rooms[0];
    let centre = (0..r.vertices.len()).find(|&v| r.vertices[v][..3] == [2048, 0, 2048]).unwrap();
    let error = move_vertices(&mut level.clone(), 0, &BTreeSet::from([centre]), [0, -256, 0]).unwrap_err();
    assert!(error.contains("evenly"), "{error}");
    let error = move_vertices(&mut level.clone(), 0, &line, [0, -4096, 0]).unwrap_err();
    assert!(error.contains("above the ceiling"), "{error}");
    assert!(move_vertices(&mut level.clone(), 0, &line, [8192, 0, 0]).is_err());
}

#[test]
fn shared_floor_data_is_copied_before_a_slope_changes() {
    let mut level = box_level(4, 4);
    let trigger = Trigger { id: 0, kind: TriggerKind::Floor, timer: 0, once: false, mask: 31, actions: vec![] };
    level.floor_infos.push(FloorInfo { triggers: vec![trigger], ..Default::default() });
    for index in [5, 6, 9] {
        level.rooms[0].sectors[index].floor_info = Some(0);
    }
    let line = floor_line(&level, 1024);
    move_vertices(&mut level, 0, &line, [0, 256, 0]).unwrap();
    consistent(&level);
    let info = level.floor_info(&sector(&level, 1, 1)).clone();
    assert_eq!(info.floor_slope, [1, 0]);
    assert_eq!(info.triggers.len(), 1, "the sector keeps its triggers");
    assert_eq!(sector(&level, 1, 2).floor_info, sector(&level, 1, 1).floor_info, "equal floor data is shared");
    assert_eq!(level.floor_infos.len(), 2);
    assert_eq!(level.floor_infos[0].floor_slope, [0, 0], "the shared original is unchanged");
    assert_eq!(sector(&level, 2, 1).floor_info, Some(0));
}

#[test]
fn extruding_a_floor_builds_steps_and_extruding_back_removes_them() {
    let mut level = box_level(5, 5);
    let original = level.clone();
    let quads = vec![face_at(&level, 2, 2, Surface::Floor), face_at(&level, 2, 3, Surface::Floor)];
    let region = extrude(&mut level, 0, &quads, -512).unwrap();
    consistent(&level);
    assert_eq!(sector(&level, 2, 2).floor, -512);
    assert_eq!(sector(&level, 2, 3).floor, -512);
    assert_eq!(sector(&level, 1, 2).floor, 0, "neighbours stay where they were");
    // Steps toward open neighbours (four sides of two sectors, less the shared
    // edge and the edge against the wall); none hide inside the wall.
    assert_eq!(level.rooms[0].quads.len(), original.rooms[0].quads.len() + 5);
    for i in original.rooms[0].quads.len()..level.rooms[0].quads.len() {
        let w = wall_face(&level.rooms[0], &level.rooms[0].quads[i][..4]).unwrap();
        // Each step is seen from the lower side: away from the raised region.
        let line = w.at / SECTOR;
        let seen = if w.toward > 0 { line } else { line - 1 };
        let (x, z) = if w.axis == 0 { (seen, w.along) } else { (w.along, seen) };
        assert_eq!(sector(&level, x as usize, z as usize).floor, 0, "step {i} faces the lower floor");
    }
    extrude(&mut level, 0, &region, 512).unwrap();
    consistent(&level);
    assert_eq!(level.rooms[0].quads.len(), original.rooms[0].quads.len(), "opposite steps cancel");
    assert_eq!(sector(&level, 2, 2).floor, 0);
    let mixed = [face_at(&level, 2, 2, Surface::Floor), face_at(&level, 2, 2, Surface::Ceiling)];
    assert!(extrude(&mut level.clone(), 0, &mixed, 256).is_err());
}

#[test]
fn lowering_a_floor_beside_a_wall_fills_the_gap_below_it() {
    let mut level = box_level(4, 4);
    let before = level.rooms[0].quads.len();
    let floor = face_at(&level, 1, 1, Surface::Floor);
    extrude(&mut level, 0, &[floor], 512).unwrap();
    consistent(&level);
    assert_eq!(sector(&level, 1, 1).floor, 512);
    // Two sides face open neighbours, two face walls: all four need a face.
    assert_eq!(level.rooms[0].quads.len(), before + 4);
    let ceiling = face_at(&level, 2, 2, Surface::Ceiling);
    extrude(&mut level, 0, &[ceiling], 1024).unwrap();
    consistent(&level);
    assert_eq!(sector(&level, 2, 2).ceiling, -1024);
}

#[test]
fn pushing_a_wall_opens_the_sector_behind_it_and_grows_the_grid() {
    let mut level = box_level(4, 4);
    let wall = (0..level.rooms[0].quads.len())
        .find(|&i| {
            let w = wall_face(&level.rooms[0], &level.rooms[0].quads[i][..4]);
            w.is_some_and(|w| w.axis == 0 && w.at == 1024 && w.along == 1)
        })
        .unwrap();
    let absolute: Vec<_> = level.rooms[0].quads[wall][..4]
        .iter()
        .map(|&v| level.rooms[0].vertices[v][0] + level.rooms[0].origin[0])
        .collect();
    let moved = push_wall(&mut level, 0, wall).unwrap();
    consistent(&level);
    let room = &level.rooms[0];
    assert_eq!((room.x_sectors, room.origin[0]), (5, -1024), "the border sector became floor; a new border was added");
    assert_eq!(moved, [wall]);
    // The wall moved one sector out in world space.
    for (k, &v) in room.quads[wall][..4].iter().enumerate() {
        assert_eq!(room.vertices[v][0] + room.origin[0], absolute[k] - 1024);
    }
    let opened = sector(&level, 1, 1);
    assert_eq!((opened.floor, opened.ceiling), (0, -2048));
    assert_eq!(sector(&level, 0, 1).floor, SOLID);
    // Behind the moved wall is solid; the new space joins the old one.
    let world = WorldGeometry::new(level.clone());
    let hit = world.floor_at(Vec3::new(0.5, 0.1, 1.5), 0).unwrap();
    assert_eq!(hit.height, 0.0);
    assert!(world.floor_at(Vec3::new(-0.5, 0.1, 1.5), 0).is_none());
    // Pushing it again opens another sector beyond.
    push_wall(&mut level, 0, wall).unwrap();
    consistent(&level);
    assert_eq!(level.rooms[0].x_sectors, 6);
}

#[test]
fn walls_may_not_stand_in_the_open_between_sectors() {
    let mut level = box_level(4, 4);
    let r = &mut level.rooms[0];
    let mut added = BTreeMap::new();
    let c = [[2048, 0, 1024], [2048, 0, 2048], [2048, -1024, 2048], [2048, -1024, 1024]]
        .map(|p| vertex_at(r, p, 128, &mut added));
    r.quads.push([c[0], c[1], c[2], c[3], 1]);
    let face = Face::Quad(r.quads.len() - 1);
    assert!(check_walls(&level, 0, [face]).is_err());
    // A step between floors at different heights is fine.
    level.rooms[0].sectors[2 * 4 + 1].floor = -1024;
    assert!(check_walls(&level, 0, [face]).is_ok());
}

#[test]
fn painting_sets_face_textures() {
    let mut level = box_level(3, 3);
    paint(&mut level, 0, &[Face::Quad(0), Face::Quad(1)], 1).unwrap();
    assert_eq!((level.rooms[0].quads[0][4], level.rooms[0].quads[1][4]), (1, 1));
    assert!(paint(&mut level, 0, &[Face::Quad(0)], 2).is_err());
}

#[test]
fn rotating_a_texture_cycles_corners_and_keeps_the_sector() {
    let mut level = box_level(3, 3);
    let quad = face_at(&level, 1, 1, Surface::Floor);
    let before = level.rooms[0].quads[quad];
    let face = sector_face(&level.rooms[0], &before);
    rotate_texture(&mut level, 0, &[Face::Quad(quad)]).unwrap();
    let after = level.rooms[0].quads[quad];
    assert_eq!(after, [before[1], before[2], before[3], before[0], before[4]]);
    assert_eq!(sector_face(&level.rooms[0], &after), face);
    for _ in 0..3 {
        rotate_texture(&mut level, 0, &[Face::Quad(quad)]).unwrap();
    }
    assert_eq!(level.rooms[0].quads[quad], before, "four quarter turns are a whole turn");
    consistent(&level);
}
