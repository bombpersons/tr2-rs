//! Rebuilding navigation boxes, their overlaps and zones where edits left
//! them missing or stale, keeping every other box exactly as authored.
//!
//! What the imported levels show (all 25, 17,047 boxes):
//!
//! - A box is a rectangle of world sectors, and every sector naming a box
//!   lies inside it. A flat box takes its sectors' floor (6,725 of 6,736);
//!   sloped boxes sit lower than the slope's top by 1 to 7 steps, with no
//!   single rule (1,402 match the slope's lowest corner, 2,096 its steepest
//!   axis, 3,593 neither). New sloped boxes take the lowest corner, an
//!   approximation, since rebuilt boxes group sectors of one floor and slope.
//! - Overlap lists are symmetric and only join boxes that share an edge or
//!   overlap, at any height difference; neighbours behind walls are left out.
//!   Rebuilt lists join a box to the boxes reached by walking from each of its
//!   edge sectors to the next one (through doorways, down to the floor below).
//! - Zones 0 to 3 are connected areas under step limits of 1, 2, 3 and 4
//!   steps (256 units each): across overlaps one step apart, zone 0 agrees
//!   13,657 times to 170 and splits two steps apart. Zone 4 (flying) mostly
//!   ignores height; rebuilt boxes join it over any overlap. Zones 5 to 9
//!   repeat 0 to 4 for the flipped level. New boxes join the zones of the
//!   boxes they reach, merging zones they newly connect, or start new ones.
//! - Sectors over a hole name a box too, usually (64%) the box of the floor
//!   beneath, which rebuilt ones take.
//!
//! Rebuilt boxes lose the authored blockable and blocked bits (doors set
//! over them); nothing in the editor knows which boxes a door covers.
use super::{Check, diagnose, locate_room};
use crate::world::{Level, NavBox, Sector, format::SOLID};
use std::collections::{BTreeMap, BTreeSet};

/// Overlap list entries index boxes in 15 bits and end with this bit.
const END: u16 = 0x8000;
/// A box's overlap field indexes the overlap list in 14 bits.
const INDEX: i32 = 0x3fff;
/// Step limits of zones 0 to 4 (none for flying).
const ZONE_STEPS: [Option<i32>; 5] = [Some(256), Some(512), Some(768), Some(1024), None];

/// What a rebuild changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NavRebuild {
    /// Boxes built (reusing the indices of the boxes they replace first).
    pub built: usize,
    /// Old boxes no sector needed any more, and removed.
    pub removed: usize,
    /// Sectors given a box.
    pub sectors: usize,
}

type Cell = (usize, usize, usize);
/// A room's rebuilt sectors: their floor and floor slope.
type Floors = BTreeMap<(usize, usize), (i32, [i8; 2])>;

fn sector(level: &Level, (room, x, z): Cell) -> &Sector {
    let r = &level.rooms[room];
    &r.sectors[x * r.z_sectors + z]
}

fn sector_mut(level: &mut Level, (room, x, z): Cell) -> &mut Sector {
    let r = &mut level.rooms[room];
    &mut r.sectors[x * r.z_sectors + z]
}

/// The sector at a world sector `(gx, gz)` reached from `room`, walking
/// through doorways and down holes to the floor, as collision does.
fn reach(level: &Level, room: usize, gx: i32, gz: i32, y: i32) -> Option<Cell> {
    let p = [gx * 1024 + 512, y, gz * 1024 + 512];
    let mut room = locate_room(level, room, p)?;
    for _ in 0..64 {
        let r = &level.rooms[room];
        let (x, z) = ((gx - r.origin[0].div_euclid(1024)) as usize, (gz - r.origin[2].div_euclid(1024)) as usize);
        let s = r.sectors.get(x * r.z_sectors + z)?;
        match s.below {
            Some(below) if below != room => room = below,
            _ => return (s.floor > SOLID).then_some((room, x, z)),
        }
    }
    None
}

fn world_cell(level: &Level, (room, x, z): Cell) -> (i32, i32) {
    let r = &level.rooms[room];
    (r.origin[0].div_euclid(1024) + x as i32, r.origin[2].div_euclid(1024) + z as i32)
}

/// Every box's overlap list.
fn overlap_lists(level: &Level) -> Vec<Vec<usize>> {
    level
        .boxes
        .iter()
        .map(|b| {
            let mut list = vec![];
            let mut i = (b.overlap & INDEX) as usize;
            while let Some(&entry) = level.overlaps.get(i) {
                list.push((entry & !END) as usize);
                if entry & END != 0 {
                    break;
                }
                i += 1;
            }
            list
        })
        .collect()
}

/// Write changed lists after the existing ones, so unchanged boxes keep
/// their entries; compact everything when the 14-bit index would overflow.
/// An empty list is a box listing only itself, which no query follows.
fn store_lists(level: &mut Level, lists: &[Vec<usize>], changed: &BTreeSet<usize>) {
    let encode = |list: &[usize], own: usize| -> Vec<u16> {
        let list = if list.is_empty() { vec![own] } else { list.to_vec() };
        let last = list.len() - 1;
        list.iter().enumerate().map(|(i, &b)| b as u16 | if i == last { END } else { 0 }).collect()
    };
    let extra: usize = changed.iter().map(|&b| lists[b].len().max(1)).sum();
    let boxes: Vec<usize> = if level.overlaps.len() + extra > INDEX as usize {
        level.overlaps.clear();
        (0..lists.len()).collect()
    } else {
        changed.iter().copied().collect()
    };
    for b in boxes {
        let at = level.overlaps.len() as i32;
        level.overlaps.extend(encode(&lists[b], b));
        let flags = level.boxes[b].overlap & !INDEX;
        level.boxes[b].overlap = flags | at;
    }
}

/// Rebuild navigation where it is missing or stale. `opened` is the level
/// as opened, for finding boxes whose floor changed since.
pub fn rebuild_navigation(level: &mut Level, opened: Option<&Level>) -> Result<NavRebuild, String> {
    let problems = diagnose(level, opened);
    let mut dirty_boxes = BTreeSet::new();
    let mut cells: BTreeSet<Cell> = BTreeSet::new();
    for p in &problems {
        let Some((x, z)) = p.sector else { continue };
        let cell = (p.room, x, z);
        match p.check {
            Check::NavBoxMissing => {
                cells.insert(cell);
            }
            Check::NavBoxOutside | Check::NavBoxStale => {
                dirty_boxes.extend(sector(level, cell).nav_box);
            }
            _ => (),
        }
    }
    if cells.is_empty() && dirty_boxes.is_empty() {
        return Ok(NavRebuild::default());
    }
    // Every sector of a dirty box is rebuilt with it.
    for (room, r) in level.rooms.iter().enumerate() {
        for x in 0..r.x_sectors {
            for z in 0..r.z_sectors {
                if r.sectors[x * r.z_sectors + z].nav_box.is_some_and(|b| dirty_boxes.contains(&b)) {
                    cells.insert((room, x, z));
                }
            }
        }
    }
    let mut holes = vec![];
    let mut floors: BTreeMap<usize, Floors> = BTreeMap::new();
    for &cell in &cells {
        let s = *sector(level, cell);
        sector_mut(level, cell).nav_box = None;
        if s.floor <= SOLID || level.floor_info(&s).portal.is_some() {
            continue;
        }
        if s.below.is_some() {
            holes.push(cell);
        } else {
            floors.entry(cell.0).or_default().insert((cell.1, cell.2), (s.floor, level.floor_info(&s).floor_slope));
        }
    }
    // Rectangles of one floor and slope in each room: grow along z, then x.
    let mut built: Vec<(NavBox, Vec<Cell>)> = vec![];
    for (&room, grid) in &floors {
        let mut taken = BTreeSet::new();
        for (&(x, z), &key) in grid {
            if taken.contains(&(x, z)) {
                continue;
            }
            let free = |x: usize, z: usize, taken: &BTreeSet<(usize, usize)>| {
                grid.get(&(x, z)) == Some(&key) && !taken.contains(&(x, z))
            };
            let mut z1 = z + 1;
            while free(x, z1, &taken) {
                z1 += 1;
            }
            let mut x1 = x + 1;
            while (z..z1).all(|zz| free(x1, zz, &taken)) {
                x1 += 1;
            }
            let mut members = vec![];
            for xx in x..x1 {
                for zz in z..z1 {
                    taken.insert((xx, zz));
                    members.push((room, xx, zz));
                }
            }
            let r = &level.rooms[room];
            let (ox, oz) = (r.origin[0].div_euclid(1024), r.origin[2].div_euclid(1024));
            let (floor, slope) = key;
            let low = floor + (i32::from(slope[0].unsigned_abs()) + i32::from(slope[1].unsigned_abs())) * 256;
            built.push((
                NavBox {
                    x_min: ox + x as i32,
                    x_max: ox + x1 as i32,
                    z_min: oz + z as i32,
                    z_max: oz + z1 as i32,
                    floor: low,
                    overlap: 0,
                },
                members,
            ));
        }
    }
    // Indices: reuse the replaced boxes', then append; drop any left over.
    let mut lists = overlap_lists(level);
    let reuse: Vec<usize> = dirty_boxes.iter().copied().collect();
    let mut indices = vec![];
    for (i, (b, _)) in built.iter().enumerate() {
        let index = match reuse.get(i) {
            Some(&old) => {
                level.boxes[old] = *b;
                old
            }
            None => {
                level.boxes.push(*b);
                lists.push(vec![]);
                for zone in &mut level.zones {
                    zone.push(0);
                }
                level.boxes.len() - 1
            }
        };
        indices.push(index);
    }
    let fresh: BTreeSet<usize> = indices.iter().copied().collect();
    for list in &mut lists {
        // A reused index names a different box now: forget the old one everywhere.
        list.retain(|b| !dirty_boxes.contains(b));
    }
    for (&index, (_, members)) in indices.iter().zip(&built) {
        lists[index].clear();
        for &cell in members {
            sector_mut(level, cell).nav_box = Some(index);
        }
    }
    // Holes take the box of the floor beneath.
    for &cell in &holes {
        let (gx, gz) = world_cell(level, cell);
        let y = sector(level, cell).floor;
        if let Some(bottom) = reach(level, cell.0, gx, gz, y) {
            sector_mut(level, cell).nav_box = sector(level, bottom).nav_box;
        }
    }
    // Overlaps: the boxes reached by stepping off each edge sector.
    let mut changed: BTreeSet<usize> = fresh.clone();
    for (&index, (b, members)) in indices.iter().zip(&built) {
        for &cell in members {
            let (gx, gz) = world_cell(level, cell);
            let y = sector(level, cell).floor - 1;
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (nx, nz) = (gx + dx, gz + dz);
                if (b.x_min..b.x_max).contains(&nx) && (b.z_min..b.z_max).contains(&nz) {
                    continue;
                }
                let Some(other) = reach(level, cell.0, nx, nz, y).and_then(|c| sector(level, c).nav_box) else {
                    continue;
                };
                if other == index {
                    continue;
                }
                for (from, to) in [(index, other), (other, index)] {
                    if !lists[from].contains(&to) {
                        lists[from].push(to);
                        changed.insert(from);
                    }
                }
            }
        }
    }
    for &b in &fresh {
        lists[b].sort_unstable();
    }
    // Lists that dropped a replaced box changed too.
    let before = overlap_lists(level);
    for (b, list) in lists.iter().enumerate() {
        if before.get(b) != Some(list) {
            changed.insert(b);
        }
    }
    zones(level, &lists, &fresh);
    let removed = reuse.len().saturating_sub(built.len());
    let gone: Vec<usize> = reuse.iter().skip(built.len()).copied().collect();
    store_lists(level, &lists, &changed);
    remove_boxes(level, &gone);
    Ok(NavRebuild { built: built.len(), removed, sectors: cells.len() })
}

/// Give `fresh` boxes zones: the zone of an existing box they reach within
/// each zone's step limit (merging zones they newly join), or a new one.
fn zones(level: &mut Level, lists: &[Vec<usize>], fresh: &BTreeSet<usize>) {
    let count = level.boxes.len();
    if level.zones.is_empty() {
        level.zones = vec![vec![0; count]; 10];
    }
    for (k, step) in ZONE_STEPS.iter().enumerate() {
        let joined = |a: usize, b: usize| step.is_none_or(|s| (level.boxes[a].floor - level.boxes[b].floor).abs() <= s);
        // Connected groups of fresh boxes, and the existing boxes they reach.
        let mut seen = BTreeSet::new();
        let mut updates: Vec<(Vec<usize>, BTreeSet<usize>)> = vec![];
        for &start in fresh {
            if !seen.insert(start) {
                continue;
            }
            let (mut group, mut touched, mut stack) = (vec![], BTreeSet::new(), vec![start]);
            while let Some(b) = stack.pop() {
                group.push(b);
                for &n in &lists[b] {
                    if n >= count || !joined(b, n) {
                        continue;
                    }
                    if fresh.contains(&n) {
                        if seen.insert(n) {
                            stack.push(n);
                        }
                    } else {
                        touched.insert(n);
                    }
                }
            }
            updates.push((group, touched));
        }
        for flipped in [k, k + 5] {
            let Some(zone) = level.zones.get_mut(flipped) else { continue };
            for (group, reached) in &updates {
                let touched: BTreeSet<i16> = reached.iter().filter_map(|&b| zone.get(b).copied()).collect();
                let id = match touched.first() {
                    Some(&first) => {
                        for z in zone.iter_mut() {
                            if touched.contains(z) {
                                *z = first;
                            }
                        }
                        first
                    }
                    None => zone.iter().copied().max().unwrap_or(0) + 1,
                };
                for &b in group {
                    zone[b] = id;
                }
            }
        }
    }
}

/// Remove boxes nothing names, renumbering the rest everywhere they appear.
fn remove_boxes(level: &mut Level, gone: &[usize]) {
    if gone.is_empty() {
        return;
    }
    let gone: BTreeSet<usize> = gone.iter().copied().collect();
    let mut map = vec![None; level.boxes.len()];
    let mut next = 0;
    for (b, slot) in map.iter_mut().enumerate() {
        if !gone.contains(&b) {
            *slot = Some(next);
            next += 1;
        }
    }
    let lists = overlap_lists(level);
    let mut kept_boxes = vec![];
    let mut kept_lists = vec![];
    for (b, nav) in level.boxes.iter().enumerate() {
        if map[b].is_some() {
            kept_boxes.push(*nav);
            kept_lists.push(lists[b].iter().filter_map(|&n| map.get(n).copied().flatten()).collect::<Vec<_>>());
        }
    }
    level.boxes = kept_boxes;
    for zone in &mut level.zones {
        *zone = zone
            .iter()
            .enumerate()
            .filter(|&(b, _)| map.get(b).is_some_and(Option::is_some))
            .map(|(_, &z)| z)
            .collect();
    }
    for r in &mut level.rooms {
        for s in &mut r.sectors {
            s.nav_box = s.nav_box.and_then(|b| map.get(b).copied().flatten());
        }
    }
    level.overlaps.clear();
    let all: BTreeSet<usize> = (0..level.boxes.len()).collect();
    store_lists(level, &kept_lists, &all);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        editor::{NewRoom, Surface, connect, extrude, sector_face},
        world::{WorldGeometry, position},
    };

    /// Two 2 x 1 rooms joined by a doorway, as the editor builds them.
    fn joined() -> Level {
        let texture = crate::world::Texture { uv: [[0.0; 2]; 4], attribute: 0 };
        let mut level = Level { format: 1, textures: vec![texture.clone(), texture], ..Default::default() };
        let spec = |x: [i32; 2]| NewRoom {
            x,
            z: [1, 2],
            floor: 0,
            ceiling: -2048,
            textures: [0, 0, 1],
            shade: 128,
            ambient: Default::default(),
        };
        crate::editor::new_room(&mut level, &spec([1, 3])).unwrap();
        crate::editor::new_room(&mut level, &spec([3, 5])).unwrap();
        let r = &level.rooms[0];
        let wall = (0..r.quads.len())
            .find(|&i| {
                let xs: BTreeSet<i32> = r.quads[i][..4].iter().map(|&v| r.vertices[v][0]).collect();
                xs == BTreeSet::from([3072]) && sector_face(r, &r.quads[i]).is_none()
            })
            .unwrap();
        connect(&mut level, 0, wall).unwrap();
        level
    }

    fn nav_problems(level: &Level, opened: Option<&Level>) -> Vec<String> {
        diagnose(level, opened)
            .into_iter()
            .filter(|p| matches!(p.check, Check::NavBoxMissing | Check::NavBoxOutside | Check::NavBoxStale))
            .map(|p| p.detail)
            .collect()
    }

    /// Lists are symmetric and in range, and every box has its zones.
    fn consistent(level: &Level) {
        let lists = overlap_lists(level);
        for (a, list) in lists.iter().enumerate() {
            for &b in list.iter().filter(|&&b| b != a) {
                assert!(b < level.boxes.len() && lists[b].contains(&a), "{a} lists {b}: {lists:?}");
            }
        }
        assert!(level.zones.iter().all(|z| z.len() == level.boxes.len()));
        crate::editor::validate_level(level).unwrap();
    }

    fn box_of(level: &Level, room: usize, x: usize, z: usize) -> usize {
        let r = &level.rooms[room];
        r.sectors[x * r.z_sectors + z].nav_box.unwrap()
    }

    #[test]
    fn new_rooms_get_boxes_joined_through_their_doorway() {
        let mut level = joined();
        assert_eq!(nav_problems(&level, None).len(), 4);
        let report = rebuild_navigation(&mut level, None).unwrap();
        assert_eq!((report.built, report.removed), (2, 0));
        assert_eq!(nav_problems(&level, None), Vec::<String>::new());
        let (a, b) = (box_of(&level, 0, 1, 1), box_of(&level, 1, 2, 1));
        assert_eq!(box_of(&level, 0, 2, 1), a, "one box covers each room's floor");
        let nav = level.boxes[a];
        assert_eq!((nav.x_min, nav.x_max, nav.z_min, nav.z_max, nav.floor), (1, 3, 1, 2, 0));
        assert_eq!(overlap_lists(&level)[a], [b]);
        assert_eq!(level.zones.len(), 10);
        assert!(level.zones.iter().all(|z| z[a] == z[b]), "level floors share every zone");
        consistent(&level);
        // A creature corridor query now crosses the doorway.
        let geometry = WorldGeometry::new(level.clone());
        let target = geometry.adjacent_path_target(position([1536, 0, 1536]), 0, position([4096, 0, 1536]), 1, 0);
        assert!(target.is_some());
        assert_eq!(rebuild_navigation(&mut level, None), Ok(NavRebuild::default()), "nothing left to do");
    }

    #[test]
    fn stale_boxes_split_and_merge_without_touching_their_neighbours() {
        let mut level = joined();
        rebuild_navigation(&mut level, None).unwrap();
        let opened = level.clone();
        let b = box_of(&level, 1, 2, 1);
        let b_box = level.boxes[b];
        // Raise one sector of room 0 a step: its box is stale.
        let floor = |level: &Level| {
            let r = &level.rooms[0];
            (0..r.quads.len())
                .find(|&i| sector_face(r, &r.quads[i]).is_some_and(|f| (f.x, f.z, f.surface) == (1, 1, Surface::Floor)))
                .unwrap()
        };
        let quad = floor(&level);
        extrude(&mut level, 0, &[quad], -256).unwrap();
        assert_eq!(nav_problems(&level, Some(&opened)).len(), 1);
        let report = rebuild_navigation(&mut level, Some(&opened)).unwrap();
        assert_eq!((report.built, report.removed), (2, 0));
        assert_eq!(nav_problems(&level, Some(&opened)), Vec::<String>::new());
        let (raised, low) = (box_of(&level, 0, 1, 1), box_of(&level, 0, 2, 1));
        assert_ne!(raised, low);
        assert_eq!((level.boxes[raised].floor, level.boxes[low].floor), (-256, 0));
        // The other room keeps its box; only its overlap list names the new neighbour.
        let kept = level.boxes[b];
        assert_eq!(box_of(&level, 1, 2, 1), b);
        assert_eq!(
            (kept.x_min, kept.x_max, kept.z_min, kept.z_max, kept.floor),
            (b_box.x_min, b_box.x_max, b_box.z_min, b_box.z_max, b_box.floor)
        );
        assert!(overlap_lists(&level)[b].contains(&low));
        assert!(overlap_lists(&level)[raised].contains(&low));
        assert!(level.zones.iter().all(|z| z[raised] == z[low]), "one step joins every zone");
        consistent(&level);
        // Lowering only that sector back gives it its own box beside the
        // untouched one: rebuilds never merge with boxes that are fine.
        let edited = level.clone();
        let quad = floor(&level);
        extrude(&mut level, 0, &[quad], 256).unwrap();
        let report = rebuild_navigation(&mut level, Some(&edited)).unwrap();
        assert_eq!((report.built, report.removed), (1, 0));
        let (one, two) = (box_of(&level, 0, 1, 1), box_of(&level, 0, 2, 1));
        assert_ne!(one, two);
        assert!(overlap_lists(&level)[one].contains(&two));
        consistent(&level);
        // Moving both to one new height makes both boxes stale: they merge
        // into one, and the box left over is removed and renumbered away.
        let edited = level.clone();
        let quad = floor(&level);
        extrude(&mut level, 0, &[quad], -512).unwrap();
        let r = &level.rooms[0];
        let other = (0..r.quads.len())
            .find(|&i| sector_face(r, &r.quads[i]).is_some_and(|f| (f.x, f.z, f.surface) == (2, 1, Surface::Floor)))
            .unwrap();
        extrude(&mut level, 0, &[other], -512).unwrap();
        let boxes = level.boxes.len();
        let report = rebuild_navigation(&mut level, Some(&edited)).unwrap();
        assert_eq!((report.built, report.removed), (1, 1));
        assert_eq!(level.boxes.len(), boxes - 1);
        assert_eq!(box_of(&level, 0, 1, 1), box_of(&level, 0, 2, 1));
        assert_eq!(level.boxes[box_of(&level, 0, 1, 1)].floor, -512);
        assert_eq!(nav_problems(&level, None), Vec::<String>::new());
        assert!(overlap_lists(&level)[box_of(&level, 1, 2, 1)].contains(&box_of(&level, 0, 2, 1)));
        consistent(&level);
    }

    #[test]
    fn zones_split_where_steps_exceed_their_limits() {
        let mut level = joined();
        // Room 1 three steps (768) higher than room 0, joined by a doorway.
        let r = &level.rooms[1];
        let floors: Vec<usize> = (0..r.quads.len())
            .filter(|&i| sector_face(r, &r.quads[i]).is_some_and(|f| f.surface == Surface::Floor))
            .collect();
        extrude(&mut level, 1, &floors, -768).unwrap();
        let (a, b) = {
            rebuild_navigation(&mut level, None).unwrap();
            (box_of(&level, 0, 1, 1), box_of(&level, 1, 2, 1))
        };
        assert!(overlap_lists(&level)[a].contains(&b), "boxes overlap at any height difference");
        let same: Vec<bool> = (0..5).map(|k| level.zones[k][a] == level.zones[k][b]).collect();
        assert_eq!(same, [false, false, true, true, true]);
        assert_eq!(level.zones[2][a], level.zones[7][a], "flipped zones follow the same rule");
        consistent(&level);
    }
}
