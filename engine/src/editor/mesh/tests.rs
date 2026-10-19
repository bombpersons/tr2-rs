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

/// Two `box_level(4, 3)` rooms along x overlapping by two sectors, as
/// imported neighbours do: each one's border lies over the other's floor.
fn neighbours() -> Level {
    let mut level = box_level(4, 3);
    let mut b = level.rooms[0].clone();
    b.origin[0] = 2 * SECTOR;
    level.rooms.push(b);
    level
}

/// A's wall between its open sector (2, 1) and its border (3, 1).
fn shared_wall(level: &Level) -> usize {
    let r = &level.rooms[0];
    (0..r.quads.len())
        .find(|&i| wall_face(r, &r.quads[i][..4]).is_some_and(|w| (w.axis, w.at, w.along) == (0, 3 * SECTOR, 1)))
        .unwrap()
}

fn faces_in_plane(room: &Room, x: i32) -> Vec<Vec<[i32; 3]>> {
    let quads = room.quads.iter().map(|q| q[..4].to_vec());
    let triangles = room.triangles.iter().map(|t| t[..3].to_vec());
    quads
        .chain(triangles)
        .map(|c| c.iter().map(|&v| point(room, v)).collect::<Vec<_>>())
        .filter(|p| p.iter().all(|p| p[0] == x) && p.iter().all(|p| (SECTOR..=2 * SECTOR).contains(&p[2])))
        .collect()
}

/// Problems with how rooms join (the test rooms have no navigation boxes).
fn link_problems(level: &Level) -> Vec<String> {
    crate::editor::diagnose(level, None)
        .into_iter()
        .filter(|p| !matches!(p.check, crate::editor::Check::NavBoxMissing))
        .map(|p| p.detail)
        .collect()
}

/// The room collision finds at a level-file position, walking from `room`.
fn room_at(level: &Level, room: usize, p: [i32; 3]) -> Option<usize> {
    WorldGeometry::new(level.clone()).floor_at(position(p), room).map(|h| h.room)
}

#[test]
fn connecting_overlapping_rooms_opens_a_doorway_both_ways_and_disconnecting_closes_it() {
    let mut level = neighbours();
    let before = level.clone();
    // Collision cannot cross yet: A's border is solid.
    let doorway = [3584, -512, 1536];
    assert_eq!(room_at(&level, 0, doorway), None);
    let wall = shared_wall(&level);
    assert_eq!(connect(&mut level, 0, wall), Ok(1));
    let a = &level.rooms[0];
    assert_eq!(level.floor_info(&a.sectors[3 * 3 + 1]).portal, Some(1));
    assert_eq!((a.sectors[3 * 3 + 1].floor, a.sectors[3 * 3 + 1].ceiling), (0, -2048));
    assert_eq!(level.floor_info(&level.rooms[1].sectors[1]).portal, Some(0));
    assert!(faces_in_plane(a, 3 * SECTOR).is_empty(), "equal rooms need no wall in the opening");
    assert!(faces_in_plane(&level.rooms[1], SECTOR).is_empty());
    assert_eq!(level.rooms[0].portals.len(), 1);
    assert_eq!(level.rooms[0].portals[0].normal, [-1, 0, 0]);
    assert_eq!(level.rooms[1].portals[0].normal, [1, 0, 0]);
    assert_eq!(link_problems(&level), Vec::<String>::new());
    // Collision walks through the doorway, and back.
    assert_eq!(room_at(&level, 0, doorway), Some(1));
    assert_eq!(room_at(&level, 1, [2560, -512, 1536]), Some(0));
    for room in 0..2 {
        assert!(sector_mismatches(&level, room).is_empty());
        assert!(wall_mismatches(&level, room).is_empty());
    }
    crate::editor::validate_level(&level).unwrap();
    assert!(connect(&mut level, 0, 0).is_err(), "a floor is not a wall");
    // Disconnecting puts the wall and the solid border back.
    disconnect(&mut level, 0, 1).unwrap();
    assert!(level.rooms.iter().all(|r| r.portals.is_empty()));
    assert_eq!(level.rooms[0].sectors[3 * 3 + 1].floor, SOLID);
    assert_eq!(level.floor_info(&level.rooms[0].sectors[3 * 3 + 1]).portal, None);
    assert_eq!(faces_in_plane(&level.rooms[0], 3 * SECTOR).len(), 1);
    assert_eq!(faces_in_plane(&level.rooms[0], 3 * SECTOR), faces_in_plane(&before.rooms[0], 3 * SECTOR));
    assert_eq!(room_at(&level, 0, doorway), None);
    assert!(disconnect(&mut level, 0, 1).is_err(), "nothing left to disconnect");
}

#[test]
fn a_doorway_onto_a_higher_floor_keeps_a_riser_on_the_lower_side() {
    let mut level = neighbours();
    // Raise B's floor under A's border by one step.
    let r = &level.rooms[1];
    let floor = (0..r.quads.len())
        .find(|&i| sector_face(r, &r.quads[i]).is_some_and(|f| (f.x, f.z, f.surface) == (1, 1, Surface::Floor)))
        .unwrap();
    extrude(&mut level, 1, &[floor], -STEP).unwrap();
    let wall = shared_wall(&level);
    assert_eq!(connect(&mut level, 0, wall), Ok(1));
    let riser = faces_in_plane(&level.rooms[0], 3 * SECTOR);
    assert_eq!(riser.len(), 1);
    let ys: BTreeSet<_> = riser[0].iter().map(|p| p[1]).collect();
    assert_eq!(ys, BTreeSet::from([-STEP, 0]), "A sees a riser up to B's floor");
    assert!(faces_in_plane(&level.rooms[1], SECTOR).is_empty(), "B looks down into A without one");
    assert_eq!(level.rooms[0].sectors[3 * 3 + 1].floor, -STEP, "the doorway sector takes B's floor");
    assert_eq!(link_problems(&level), Vec::<String>::new());
    disconnect(&mut level, 1, 0).unwrap();
    assert!(link_problems(&level).is_empty());
}

#[test]
fn a_hole_opens_a_floor_into_the_room_below_and_closing_it_restores_both_faces() {
    let mut level = box_level(3, 3);
    let mut below = level.rooms[0].clone();
    for v in &mut below.vertices {
        v[1] += 2048;
    }
    for s in below.sectors.iter_mut().filter(|s| s.floor != SOLID) {
        (s.floor, s.ceiling) = (s.floor + 2048, s.ceiling + 2048);
    }
    (below.top, below.bottom) = (0, 2048);
    level.rooms.push(below);
    let before = level.clone();
    let floor = face_at(&level, 1, 1, Surface::Floor);
    assert_eq!(open_hole(&mut level, 0, floor), Ok(1));
    assert_eq!((sector(&level, 1, 1).below, level.rooms[1].sectors[4].above), (Some(1), Some(0)));
    assert!(!sector_faces(&level.rooms[0]).contains_key(&(1, 1, Surface::Floor)));
    assert!(!sector_faces(&level.rooms[1]).contains_key(&(1, 1, Surface::Ceiling)));
    assert_eq!((level.rooms[0].portals[0].normal, level.rooms[1].portals[0].normal), ([0, -1, 0], [0, 1, 0]));
    assert_eq!(level.rooms[1].portals[0].vertices[0][1], -1, "the lower side sits one unit short");
    assert_eq!(link_problems(&level), Vec::<String>::new());
    assert_eq!(room_at(&level, 0, [1536, 512, 1536]), Some(1));
    assert!(open_hole(&mut level, 0, floor).is_err(), "the face is gone");
    disconnect(&mut level, 0, 1).unwrap();
    for room in 0..2 {
        assert!(sector_mismatches(&level, room).is_empty());
        assert_eq!(sector_faces(&level.rooms[room]).len(), sector_faces(&before.rooms[room]).len());
    }
    assert_eq!(sector(&level, 1, 1).below, None);
    assert!(level.rooms.iter().all(|r| r.portals.is_empty()));
    // A floor over nothing at its height has no room to open into.
    let mut alone = box_level(3, 3);
    let floor = face_at(&alone, 1, 1, Surface::Floor);
    assert!(open_hole(&mut alone, 0, floor).unwrap_err().contains("no room below"));
}

#[test]
fn several_faces_join_in_one_edit_even_as_faces_come_and_go() {
    let mut level = box_level(4, 4);
    let mut b = level.rooms[0].clone();
    b.origin[0] = 2 * SECTOR;
    level.rooms.push(b);
    let r = &level.rooms[0];
    let walls: Vec<usize> = (0..r.quads.len())
        .filter(|&i| wall_face(r, &r.quads[i][..4]).is_some_and(|w| (w.axis, w.at) == (0, 3 * SECTOR)))
        .collect();
    assert_eq!(walls.len(), 2);
    assert_eq!(connect_faces(&mut level, 0, &walls), Ok(BTreeSet::from([1])));
    let doorways = level.rooms[0].sectors.iter().filter(|s| level.floor_info(s).portal == Some(1)).count();
    assert_eq!(doorways, 2);
    assert_eq!(level.rooms[0].portals.len(), 1, "one portal spans the whole opening");
    let v = &level.rooms[0].portals[0].vertices;
    let zs: BTreeSet<_> = v.iter().map(|p| p[2]).collect();
    assert_eq!(zs, BTreeSet::from([SECTOR, 3 * SECTOR]));
    assert_eq!(link_problems(&level), Vec::<String>::new());
}

fn spec(x: [i32; 2], z: [i32; 2]) -> NewRoom {
    NewRoom { x, z, floor: 0, ceiling: -2048, textures: [0, 0, 1], shade: 128, ambient: Default::default() }
}

#[test]
fn a_new_room_is_built_like_an_imported_one_and_connects_to_its_neighbour() {
    let mut level = box_level(4, 3);
    // Open floor beside room 0's, sharing an edge: the grids overlap by two sectors.
    assert_eq!(new_room(&mut level, &spec([3, 5], [1, 2])), Ok(1));
    let built = &level.rooms[1];
    let expected = &neighbours().rooms[1];
    assert_eq!((built.origin, built.x_sectors, built.z_sectors), (expected.origin, 4, 3));
    assert_eq!(built.vertices, expected.vertices);
    assert_eq!(built.quads, expected.quads);
    assert!(sector_mismatches(&level, 1).is_empty() && wall_mismatches(&level, 1).is_empty());
    crate::editor::validate_level(&level).unwrap();
    assert_eq!(link_problems(&level), Vec::<String>::new());
    let wall = shared_wall(&level);
    assert_eq!(connect(&mut level, 0, wall), Ok(1));
    assert_eq!(room_at(&level, 0, [3584, -512, 1536]), Some(1));
    assert_eq!(link_problems(&level), Vec::<String>::new());
    for bad in [
        NewRoom { x: [3, 3], ..spec([3, 5], [1, 2]) },
        NewRoom { floor: -2048, ceiling: 0, ..spec([3, 5], [1, 2]) },
        NewRoom { floor: 100, ..spec([3, 5], [1, 2]) },
        NewRoom { textures: [0, 0, 9], ..spec([3, 5], [1, 2]) },
    ] {
        assert!(new_room(&mut level, &bad).is_err(), "{bad:?}");
    }
    assert_eq!(typical_textures(&level, 1), [0, 0, 1]);
    assert_eq!(typical_shade(&level, 1), 128);
}

#[test]
fn connecting_cuts_a_room_wide_wall_down_to_the_doorway() {
    let text = include_str!("../../../assets/levels/sandbox/level.json");
    let mut level: Level = serde_json::from_str(text).unwrap();
    let wide = |room: &Room| {
        let in_plane = |q: &&[usize; 5]| q[..4].iter().all(|&v| room.vertices[v][0] == 5 * SECTOR);
        let span = |q: &[usize; 5]| {
            let zs: Vec<i32> = q[..4].iter().map(|&v| room.vertices[v][2]).collect();
            zs.iter().max().unwrap() - zs.iter().min().unwrap()
        };
        room.quads.iter().filter(in_plane).filter(|q| span(q) > SECTOR).count()
    };
    assert_eq!(wide(&level.rooms[0]), 1, "the sandbox's +x wall is one face across the room");
    let room = new_room(&mut level, &spec([5, 7], [3, 4])).unwrap();
    let r = &level.rooms[room];
    let wall = (0..r.quads.len())
        .find(|&i| wall_face(r, &r.quads[i][..4]).is_some_and(|w| (w.axis, w.at) == (0, SECTOR)))
        .unwrap();
    assert_eq!(connect(&mut level, room, wall), Ok(0));
    // What is left of the wall covers the rest of the edge, in sector pieces.
    let s = &level.rooms[0];
    let spans: BTreeSet<_> = s
        .quads
        .iter()
        .map(|q| q[..4].iter().map(|&v| point(s, v)).collect::<Vec<_>>())
        .filter(|p| p.iter().all(|p| p[0] == 5 * SECTOR))
        .map(|p| (p.iter().map(|p| p[2]).min().unwrap(), p.iter().map(|p| p[2]).max().unwrap()))
        .collect();
    assert!(!spans.contains(&(3 * SECTOR, 4 * SECTOR)), "the doorway is open: {spans:?}");
    assert!(spans.contains(&(SECTOR, 2 * SECTOR)) && spans.contains(&(4 * SECTOR, 5 * SECTOR)), "{spans:?}");
    assert_eq!(wide(&level.rooms[0]), 0);
    assert_eq!(room_at(&level, 0, [4608, -512, 3584]), Some(0));
    assert_eq!(room_at(&level, 0, [5632, -512, 3584]), Some(room));
    assert!(wall_mismatches(&level, 0).is_empty());
    assert_eq!(link_problems(&level), Vec::<String>::new());
}

/// Sector values with floor data resolved, for comparing rooms rebuilt
/// with different floor-data indices.
type SectorValues = (i32, i32, Option<usize>, Option<usize>, FloorInfo);

fn sector_values(level: &Level, room: usize) -> Vec<SectorValues> {
    level.rooms[room]
        .sectors
        .iter()
        .map(|s| (s.floor, s.ceiling, s.below, s.above, level.floor_info(s).clone()))
        .collect()
}

fn joins_cleanly(level: &Level) {
    assert_eq!(link_problems(level), Vec::<String>::new());
    for room in 0..level.rooms.len() {
        assert!(sector_mismatches(level, room).is_empty(), "room {room}: {:?}", sector_mismatches(level, room));
        assert!(wall_mismatches(level, room).is_empty(), "room {room}: {:?}", wall_mismatches(level, room));
    }
    crate::editor::validate_level(level).unwrap();
}

#[test]
fn splitting_along_a_line_makes_two_joined_rooms_and_merging_restores_one() {
    let mut level = box_level(5, 3);
    let original = sector_values(&level, 0);
    level.placements.push(crate::world::Placement {
        kind: 1,
        room: 0,
        position: [3584, 0, 1536],
        ..Default::default()
    });
    assert_eq!(split_along(&mut level, 0, 0, 2), Ok(1));
    let (a, b) = (&level.rooms[0], &level.rooms[1]);
    assert_eq!((a.x_sectors, b.x_sectors, b.origin[0]), (3, 4, SECTOR));
    assert_eq!(level.floor_info(&a.sectors[2 * 3 + 1]).portal, Some(1), "A's new border is a doorway");
    assert_eq!(level.floor_info(&b.sectors[1]).portal, Some(0));
    assert!(a.vertices.iter().all(|v| v[0] <= 2 * SECTOR), "A keeps only its side's faces");
    assert_eq!((a.portals[0].room, a.portals[0].normal), (1, [-1, 0, 0]));
    assert_eq!(level.placements[0].room, 1, "objects follow their side");
    joins_cleanly(&level);
    // Each part's doorway sector leads into the other.
    assert_eq!(room_at(&level, 0, [2560, -512, 1536]), Some(1));
    assert_eq!(room_at(&level, 1, [1536, -512, 1536]), Some(0));
    assert_eq!(merge_rooms(&mut level, 0, 1), Ok(0));
    assert_eq!(level.rooms.len(), 1);
    assert_eq!(sector_values(&level, 0), original);
    assert!(level.rooms[0].portals.is_empty());
    assert_eq!(level.placements[0].room, 0);
    joins_cleanly(&level);
}

#[test]
fn splitting_a_joined_room_hands_its_doorways_to_the_part_they_open_into() {
    let mut level = neighbours();
    let wall = shared_wall(&level);
    connect(&mut level, 0, wall).unwrap();
    // Room 1 spans x 2048..6144; its open floor is x 3072..5120.
    assert_eq!(split_along(&mut level, 1, 0, 2), Ok(2));
    // Room 0's doorway lands in the part over x 3072..4096: room 1.
    assert_eq!(room_at(&level, 0, [3584, -512, 1536]), Some(1));
    assert_eq!(room_at(&level, 1, [4608, -512, 1536]), Some(2));
    assert!(level.rooms[0].portals.iter().all(|p| p.room == 1));
    joins_cleanly(&level);
    assert!(split_along(&mut level, 1, 0, 1).is_err(), "too near the edge");
    assert!(merge_rooms(&mut level, 0, 2).is_err(), "rooms 0 and 2 are not joined");
}

#[test]
fn splitting_at_a_height_stacks_two_rooms_and_merging_restores_one() {
    let mut level = box_level(4, 3);
    let original = sector_values(&level, 0);
    level.placements.push(crate::world::Placement {
        kind: 1,
        room: 0,
        position: [1536, -1536, 1536],
        ..Default::default()
    });
    assert!(split_height(&mut level, 0, -1000).is_err(), "whole steps only");
    assert!(split_height(&mut level, 0, 0).is_err(), "inside the room only");
    assert_eq!(split_height(&mut level, 0, -1024), Ok(1));
    let (low, high) = (&level.rooms[0], &level.rooms[1]);
    assert_eq!((low.sectors[4].floor, low.sectors[4].ceiling, low.sectors[4].above), (0, -1024, Some(1)));
    assert_eq!((high.sectors[4].floor, high.sectors[4].ceiling, high.sectors[4].below), (-1024, -2048, Some(0)));
    assert!(high.vertices.iter().all(|v| v[1] <= -1024) && low.vertices.iter().all(|v| v[1] >= -1024));
    assert_eq!(level.placements[0].room, 1);
    joins_cleanly(&level);
    assert_eq!(room_at(&level, 0, [1536, -1536, 1536]), Some(1));
    assert_eq!(room_at(&level, 1, [1536, -512, 1536]), Some(0));
    assert_eq!(merge_rooms(&mut level, 1, 0), Ok(0));
    assert_eq!(sector_values(&level, 0), original);
    assert_eq!(level.placements[0].room, 0);
    joins_cleanly(&level);
}

#[test]
fn closing_part_of_a_hole_brings_back_both_faces_and_shrinks_the_portals() {
    // A 2 x 1 room over another, its floor open into it over both sectors.
    let mut level = box_level(4, 3);
    let mut below = level.rooms[0].clone();
    for v in &mut below.vertices {
        v[1] += 2048;
    }
    for s in below.sectors.iter_mut().filter(|s| s.floor != SOLID) {
        (s.floor, s.ceiling) = (s.floor + 2048, s.ceiling + 2048);
    }
    (below.top, below.bottom) = (0, 2048);
    level.rooms.push(below);
    let floors = [face_at(&level, 1, 1, Surface::Floor), face_at(&level, 2, 1, Surface::Floor)];
    assert_eq!(open_holes(&mut level, 0, &floors), Ok(BTreeSet::from([1])));
    let face = |level: &Level, room: usize, x: usize, surface: Surface| {
        let r = &level.rooms[room];
        (0..r.quads.len())
            .find(|&i| sector_face(r, &r.quads[i]).is_some_and(|f| (f.x, f.z, f.surface) == (x, 1, surface)))
    };
    // Seen from above through the hole: the lower room's floor at (1, 1).
    let lower_floor = face(&level, 1, 1, Surface::Floor).unwrap();
    let wall =
        (0..level.rooms[1].quads.len()).find(|&i| sector_face(&level.rooms[1], &level.rooms[1].quads[i]).is_none());
    assert!(close_holes(&mut level, 1, &[wall.unwrap()]).is_err(), "a wall is beside no hole");
    assert_eq!(close_holes(&mut level, 1, &[lower_floor]), Ok(BTreeSet::from([0])));
    let (upper, lower) = (&level.rooms[0], &level.rooms[1]);
    assert_eq!((upper.sectors[3 + 1].below, lower.sectors[3 + 1].above), (None, None));
    assert!(face(&level, 0, 1, Surface::Floor).is_some() && face(&level, 1, 1, Surface::Ceiling).is_some());
    assert_eq!(upper.sectors[2 * 3 + 1].below, Some(1), "the other sector is still open");
    let xs: BTreeSet<i32> = upper.portals[0].vertices.iter().map(|v| v[0]).collect();
    assert_eq!(xs, BTreeSet::from([2 * SECTOR, 3 * SECTOR]), "the portal covers only the hole left");
    assert_eq!(room_at(&level, 0, [1536, -512, 1536]).and(room_at(&level, 0, [1536, 512, 1536])), Some(0));
    assert_eq!(room_at(&level, 0, [2560, 512, 1536]), Some(1));
    joins_cleanly(&level);
    // Seen from below: the upper room's ceiling over the last hole.
    let upper_ceiling = face(&level, 0, 2, Surface::Ceiling).unwrap();
    assert_eq!(close_holes(&mut level, 0, &[upper_ceiling]), Ok(BTreeSet::from([1])));
    assert!(level.rooms.iter().all(|r| r.portals.is_empty()));
    joins_cleanly(&level);
    assert!(disconnect(&mut level, 0, 1).is_err(), "nothing joins them any more");
}
