//! Consistency checks over how rooms join, where objects are, and the
//! navigation boxes, with fixes where only one fix makes sense.
//!
//! Imported levels break most of these rules on purpose: 9,930 doorway
//! sectors lead into rooms on another layer with no render portal between
//! them, 382 portals are one-sided, vertical links are many-to-one, and 57
//! placements are authored in rooms a strict lookup would not give them.
//! Nothing here is therefore an error by itself. Tools compare against the
//! level as it was opened ([`Problem::key`]) and show what an edit
//! introduced. Two rules hold for every imported sector and are checked
//! exactly: a sector's navigation box covers it, and every open sector that
//! is not a doorway or a hole has one. Box floors follow no per-sector rule
//! that the imported data confirms, so a box is reported stale only when
//! its sector's floor changed since the level was opened.
//!
//! Flip rooms are checked in the state they are seen in: the level as it
//! starts, then flipped (see [`super::flip`]), where problems in an
//! alternate's data, or caused by the alternate beside a room, are added.
use super::flip::{flip_bases, flipped};
use super::rooms::locate_room;
use crate::world::{Level, Room, Sector, format::Portal, format::SOLID};

/// What a check found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Check {
    /// A portal with no portal back from the room it opens into.
    OneSidedPortal,
    /// A doorway, floor or ceiling link with no render portal to that room,
    /// so the rooms collide as joined but are not drawn through each other.
    LinkWithoutPortal,
    /// A doorway, floor or ceiling link into a solid sector, or past the
    /// edge of the other room's grid.
    LinkIntoSolid,
    /// Heights that disagree across a link: a doorway's floor and the floor
    /// it leads onto, or a floor and the ceiling of the room below it.
    HeightMismatch,
    /// A placement whose room, looked up from its position, is another room.
    ObjectInOtherRoom,
    /// A placement in solid ground or outside its room.
    ObjectOutside,
    /// A sector outside the navigation box it names.
    NavBoxOutside,
    /// An open sector that is not a doorway or a hole, with no navigation box.
    NavBoxMissing,
    /// A sector whose floor changed since the level was opened while its
    /// navigation box did not.
    NavBoxStale,
    /// An alternate that is not on its room's grid, that two rooms share or
    /// that has an alternate itself, or a link that names an alternate room
    /// rather than the room it replaces.
    FlipPair,
}

impl Check {
    pub const ALL: [Check; 10] = [
        Check::FlipPair,
        Check::LinkIntoSolid,
        Check::ObjectOutside,
        Check::NavBoxOutside,
        Check::NavBoxMissing,
        Check::NavBoxStale,
        Check::OneSidedPortal,
        Check::LinkWithoutPortal,
        Check::HeightMismatch,
        Check::ObjectInOtherRoom,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Check::OneSidedPortal => "One-sided portals",
            Check::LinkWithoutPortal => "Links without render portals",
            Check::LinkIntoSolid => "Links into solid or outside a grid",
            Check::HeightMismatch => "Heights that differ across a link",
            Check::ObjectInOtherRoom => "Objects whose position is in another room",
            Check::ObjectOutside => "Objects in solid ground or outside their room",
            Check::NavBoxOutside => "Sectors outside their navigation box",
            Check::NavBoxMissing => "Open sectors without a navigation box",
            Check::NavBoxStale => "Navigation boxes older than their floor",
            Check::FlipPair => "Flip rooms that do not pair up",
        }
    }
}

/// A change that resolves a problem, where only one makes sense.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fix {
    /// Add the mirror of `room`'s portal `portal` to the room it opens into.
    ReturnPortal { room: usize, portal: usize },
    /// Set a placement's room.
    ObjectRoom { placement: usize, room: usize },
}

/// One thing a check found, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub check: Check,
    pub room: usize,
    /// The sector, in the room's grid.
    pub sector: Option<(usize, usize)>,
    /// The other room, or the placement for object checks.
    pub other: Option<usize>,
    pub detail: String,
    pub fix: Option<Fix>,
    /// Where the problem is, independent of room numbers: a sector's world
    /// sector, floor, ceiling and link kind; a portal's world bounds; an
    /// object's index. Splitting or merging rooms renumbers them, and must
    /// not make the problems a file already had look new.
    pub place: [i32; 6],
    /// Found in the level as it is once flipped.
    pub flipped: bool,
}

/// A problem's identity across edits: see [`Problem::place`].
pub type ProblemKey = (Check, [i32; 6]);

impl Problem {
    /// Identifies the problem across edits, to tell those an edit introduced
    /// from those the file already had.
    pub fn key(&self) -> ProblemKey {
        (self.check, self.place)
    }
}

fn sector_at(room: &Room, x: i32, z: i32) -> Option<&Sector> {
    let sx = (x - room.origin[0]).div_euclid(1024);
    let sz = (z - room.origin[2]).div_euclid(1024);
    if sx < 0 || sz < 0 || sx as usize >= room.x_sectors || sz as usize >= room.z_sectors {
        return None;
    }
    room.sectors.get(sx as usize * room.z_sectors + sz as usize)
}

/// The portal in `to` that mirrors `portal` of room `from`: the same
/// opening, wound the other way, facing the other way. Imported pairs place
/// the side whose normal is positive on an axis one unit short of the plane;
/// all 7,478 two-sided imported pairs but 6 follow this exactly.
pub fn mirror_portal(level: &Level, from: usize, portal: &Portal) -> Option<Portal> {
    let (a, b) = (level.rooms.get(from)?, level.rooms.get(portal.room)?);
    let axis = (0..3).find(|&i| portal.normal[i] != 0)?;
    let shift = if portal.normal[axis] > 0 { 1 } else { -1 };
    let vertices = portal
        .vertices
        .iter()
        .rev()
        .map(|v| {
            let mut w = [v[0] + a.origin[0], v[1], v[2] + a.origin[2]];
            w[axis] += shift;
            [w[0] - b.origin[0], w[1], w[2] - b.origin[2]]
        })
        .collect();
    Some(Portal { room: from, normal: portal.normal.map(|n| -n), vertices })
}

/// Every problem in `level`. `opened` is the level as it was opened, for
/// finding navigation boxes an edit left behind.
pub fn diagnose(level: &Level, opened: Option<&Level>) -> Vec<Problem> {
    let bases = flip_bases(level);
    // Before the level flips, alternates are not there.
    let mut out: Vec<Problem> =
        check(level, opened).into_iter().filter(|p| is_object(p.check) || bases[p.room].is_none()).collect();
    out.extend(flip_pairs(level, &bases));
    // Swapped: base numbers whose data is their alternate's once flipped.
    let swapped: Vec<Option<usize>> =
        (0..level.rooms.len()).map(|b| level.rooms[b].alternate.filter(|&a| bases.get(a) == Some(&Some(b)))).collect();
    if swapped.iter().all(Option::is_none) {
        return out;
    }
    let seen: std::collections::BTreeSet<ProblemKey> = out.iter().map(Problem::key).collect();
    let opened = opened.map(flipped);
    for mut p in check(&flipped(level), opened.as_ref()) {
        if is_object(p.check) {
            if seen.contains(&p.key()) {
                continue;
            }
        } else if bases[p.room].is_some() {
            // An original's data, not there once flipped.
            continue;
        } else if let Some(alternate) = swapped[p.room] {
            // The alternate's own data: report it where it is stored.
            p.detail = p.detail.replacen(&format!("Room {} ", p.room), &format!("Room {alternate} "), 1);
            p.room = alternate;
            p.place[5] = 1;
        } else if seen.contains(&p.key()) {
            continue;
        }
        p.detail = format!("When flipped: {}", p.detail);
        // Fixes would change the level before it flips as well.
        p.fix = None;
        p.flipped = true;
        out.push(p);
    }
    out
}

fn is_object(check: Check) -> bool {
    matches!(check, Check::ObjectInOtherRoom | Check::ObjectOutside)
}

/// Alternates that do not pair up with their room, and links naming them.
fn flip_pairs(level: &Level, bases: &[Option<usize>]) -> Vec<Problem> {
    let mut out = vec![];
    let rooms = &level.rooms;
    let mut problem = |room: usize, other: Option<usize>, kind: i32, detail: String| {
        let r = &rooms[room];
        let place = [r.origin[0], r.origin[2], r.top, r.bottom, kind, 2];
        out.push(Problem {
            check: Check::FlipPair,
            room,
            sector: None,
            other,
            detail,
            fix: None,
            place,
            flipped: false,
        });
    };
    for (i, r) in rooms.iter().enumerate() {
        let Some(a) = r.alternate.filter(|&a| a < rooms.len()) else { continue };
        let alt = &rooms[a];
        if a == i {
            problem(i, Some(a), 1, format!("Room {i} is its own alternate"));
        } else if bases[a] != Some(i) {
            problem(
                i,
                Some(a),
                2,
                format!("Room {i} flips to room {a}, which room {} flips to too", bases[a].unwrap_or(a)),
            );
        } else if alt.alternate.is_some() {
            problem(i, Some(a), 3, format!("Room {i} flips to room {a}, which has an alternate of its own"));
        } else if (alt.origin[0], alt.origin[2], alt.x_sectors, alt.z_sectors)
            != (r.origin[0], r.origin[2], r.x_sectors, r.z_sectors)
        {
            problem(i, Some(a), 4, format!("Room {i} and its alternate {a} are on different grids"));
        }
    }
    // Links name the room an alternate replaces; once flipped they lead into it.
    for (i, r) in rooms.iter().enumerate() {
        let mut named: Vec<usize> = r.portals.iter().map(|p| p.room).collect();
        for s in &r.sectors {
            named.extend(level.floor_info(s).portal.into_iter().chain(s.below).chain(s.above));
        }
        named.sort_unstable();
        named.dedup();
        for to in named.into_iter().filter(|&to| to != i && bases.get(to).is_some_and(Option::is_some)) {
            problem(
                i,
                Some(to),
                10,
                format!(
                    "Room {i} links to room {to}, an alternate; links name room {}, which it replaces",
                    bases[to].unwrap()
                ),
            );
        }
    }
    out
}

/// The checks over one state of the level.
fn check(level: &Level, opened: Option<&Level>) -> Vec<Problem> {
    let mut out = vec![];
    let rooms = &level.rooms;
    for (i, r) in rooms.iter().enumerate() {
        for (k, p) in r.portals.iter().enumerate() {
            let Some(target) = rooms.get(p.room) else { continue };
            if !target.portals.iter().any(|q| q.room == i) {
                let world = p.vertices.iter().map(|v| [v[0] + r.origin[0], v[1], v[2] + r.origin[2]]);
                let (low, high) = world.fold(([i32::MAX; 3], [i32::MIN; 3]), |(lo, hi), w| {
                    (
                        [lo[0].min(w[0]), lo[1].min(w[1]), lo[2].min(w[2])],
                        [hi[0].max(w[0]), hi[1].max(w[1]), hi[2].max(w[2])],
                    )
                });
                out.push(Problem {
                    check: Check::OneSidedPortal,
                    room: i,
                    sector: None,
                    other: Some(p.room),
                    detail: format!("Room {i} sees into room {} but not back", p.room),
                    fix: Some(Fix::ReturnPortal { room: i, portal: k }),
                    flipped: false,
                    // The plane, not the extent: splitting a room clips portals into it.
                    place: {
                        let axis = (0..3).find(|&a| p.normal[a] != 0).unwrap_or(1);
                        let plane = if p.normal[axis] > 0 { high[axis] + 1 } else { low[axis] };
                        [axis as i32, plane, p.normal[axis].signum(), 0, 0, 0]
                    },
                });
            }
        }
        let has_portal =
            |to: usize, vertical: i32| r.portals.iter().any(|p| p.room == to && p.normal[1].signum() == vertical);
        for sx in 0..r.x_sectors {
            for sz in 0..r.z_sectors {
                let s = &r.sectors[sx * r.z_sectors + sz];
                let (x, z) = (r.origin[0] + sx as i32 * 1024 + 512, r.origin[2] + sz as i32 * 1024 + 512);
                let at = Some((sx, sz));
                let (gx, gz) = (r.origin[0].div_euclid(1024) + sx as i32, r.origin[2].div_euclid(1024) + sz as i32);
                let mut problem = |check, other, kind: i32, detail: String| {
                    let place = [gx, gz, s.floor, s.ceiling, kind, 0];
                    out.push(Problem { check, room: i, sector: at, other, detail, fix: None, place, flipped: false });
                };
                let doorway = level.floor_info(s).portal;
                // Doorways, holes and openings: (target, kind, render portal facing).
                let links = [(doorway, "doorway", 0), (s.below, "floor", -1), (s.above, "ceiling", 1)];
                for (code, (to, kind, facing)) in (1..).zip(links) {
                    let Some(to) = to.filter(|&to| to != i) else { continue };
                    if !has_portal(to, facing) {
                        problem(
                            Check::LinkWithoutPortal,
                            Some(to),
                            code,
                            format!(
                                "Room {i} sector ({sx}, {sz}) {kind} leads into room {to} with no portal to draw it"
                            ),
                        );
                    }
                    let Some(t) = rooms.get(to).and_then(|t| sector_at(t, x, z)).filter(|t| t.floor > SOLID) else {
                        problem(
                            Check::LinkIntoSolid,
                            Some(to),
                            code,
                            format!("Room {i} sector ({sx}, {sz}) {kind} leads into solid ground or outside room {to}"),
                        );
                        continue;
                    };
                    let (mine, theirs) = match kind {
                        "doorway" if s.floor > SOLID => (s.floor, t.floor),
                        "floor" => (s.floor, t.ceiling),
                        "ceiling" => (s.ceiling, t.floor),
                        _ => continue,
                    };
                    if mine != theirs {
                        problem(
                            Check::HeightMismatch,
                            Some(to),
                            code,
                            format!("Room {i} sector ({sx}, {sz}) {kind} at {mine} meets room {to} at {theirs}"),
                        );
                    }
                }
                let open = s.floor > SOLID;
                match s.nav_box.map(|b| (b, level.boxes.get(b))) {
                    Some((b, Some(nav))) => {
                        if !(nav.x_min <= gx && gx < nav.x_max && nav.z_min <= gz && gz < nav.z_max) {
                            problem(
                                Check::NavBoxOutside,
                                None,
                                0,
                                format!("Room {i} sector ({sx}, {sz}) is outside navigation box {b}"),
                            );
                        }
                    }
                    Some((_, None)) => (),
                    None if open && doorway.is_none() && s.below.is_none() => problem(
                        Check::NavBoxMissing,
                        None,
                        0,
                        format!("Room {i} sector ({sx}, {sz}) is open but has no navigation box"),
                    ),
                    None => (),
                }
                if let (Some(b), Some(before)) = (s.nav_box, opened.and_then(|o| o.rooms.get(i)))
                    && let Some(old) = sector_at(before, x, z)
                    && old.nav_box == Some(b)
                    && (old.floor != s.floor
                        || level.floor_info(s).floor_slope != opened.map_or([0, 0], |o| o.floor_info(old).floor_slope))
                    && opened.is_some_and(|o| o.boxes.get(b) == level.boxes.get(b))
                {
                    problem(
                        Check::NavBoxStale,
                        None,
                        0,
                        format!(
                            "Sector ({sx}, {sz}) floor changed from {} to {}; navigation box {b} was not rebuilt",
                            old.floor, s.floor
                        ),
                    );
                }
            }
        }
    }
    for (index, p) in level.placements.iter().enumerate() {
        if p.room >= rooms.len() {
            continue;
        }
        match locate_room(level, p.room, p.position) {
            Some(room) if room == p.room => (),
            Some(room) => out.push(Problem {
                check: Check::ObjectInOtherRoom,
                room: p.room,
                sector: None,
                other: Some(index),
                detail: format!("Object {index} is authored in room {} but stands in room {room}", p.room),
                fix: Some(Fix::ObjectRoom { placement: index, room }),
                place: [index as i32, 0, 0, 0, 0, 0],
                flipped: false,
            }),
            None => out.push(Problem {
                check: Check::ObjectOutside,
                room: p.room,
                sector: None,
                other: Some(index),
                detail: format!("Object {index} is in solid ground or outside room {}", p.room),
                fix: None,
                place: [index as i32, 0, 0, 0, 0, 0],
                flipped: false,
            }),
        }
    }
    out
}

/// Apply a fix.
pub fn apply_fix(level: &mut Level, fix: &Fix) -> Result<(), String> {
    match *fix {
        Fix::ReturnPortal { room, portal } => {
            let p = level.rooms.get(room).and_then(|r| r.portals.get(portal)).ok_or("no such portal")?.clone();
            if level.rooms[p.room].portals.iter().any(|q| q.room == room) {
                return Err(format!("room {} already has a portal back", p.room));
            }
            let mirror = mirror_portal(level, room, &p).ok_or("the portal has no normal")?;
            level.rooms[p.room].portals.push(mirror);
        }
        Fix::ObjectRoom { placement, room } => {
            if room >= level.rooms.len() {
                return Err("no such room".into());
            }
            level.placements.get_mut(placement).ok_or("no such object")?.room = room;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{FloorInfo, NavBox, Placement};

    /// Two 3x3 rooms overlapping by two sectors along x, joined by a doorway
    /// and a pair of portals, both open centres in navigation box 0 or 1.
    fn level() -> Level {
        let open = Sector { floor: 0, ceiling: -2048, ..Default::default() };
        let solid = Sector { floor: SOLID, ceiling: SOLID, ..Default::default() };
        let room = |x: i32| Room {
            origin: [x, 0, 0],
            top: -2048,
            bottom: 0,
            x_sectors: 3,
            z_sectors: 3,
            sectors: (0..9)
                .map(|i| if i == 4 { Sector { nav_box: Some((x / 1024) as usize), ..open } } else { solid })
                .collect(),
            ..Default::default()
        };
        let (mut a, mut b) = (room(0), room(1024));
        a.sectors[2 * 3 + 1] = Sector { floor_info: Some(0), ..open };
        b.sectors[1] = Sector { floor_info: Some(1), ..open };
        a.portals = vec![Portal {
            room: 1,
            normal: [-1, 0, 0],
            vertices: vec![[2048, 0, 1024], [2048, -2048, 1024], [2048, -2048, 2048], [2048, 0, 2048]],
        }];
        b.portals = vec![
            mirror_portal(&Level { rooms: vec![a.clone(), b.clone()], ..Default::default() }, 0, &a.portals[0])
                .unwrap(),
        ];
        let nav = |x: i32| NavBox { x_min: x, x_max: x + 1, z_min: 1, z_max: 2, floor: 0, overlap: 0 };
        Level {
            rooms: vec![a, b],
            floor_infos: vec![
                FloorInfo { portal: Some(1), ..Default::default() },
                FloorInfo { portal: Some(0), ..Default::default() },
            ],
            boxes: vec![nav(1), nav(2)],
            placements: vec![Placement { kind: 1, room: 0, position: [1536, 0, 1536], ..Default::default() }],
            ..Default::default()
        }
    }

    fn checks(level: &Level, opened: Option<&Level>) -> Vec<Check> {
        diagnose(level, opened).into_iter().map(|p| p.check).collect()
    }

    #[test]
    fn a_consistent_pair_of_rooms_has_no_problems_and_mirrors_round_trip() {
        let level = level();
        assert_eq!(diagnose(&level, Some(&level)), []);
        // Imported convention: the positive-facing side sits one unit short.
        let b = &level.rooms[1].portals[0];
        assert_eq!(b.normal, [1, 0, 0]);
        assert_eq!(b.vertices[0], [2048 - 1024 - 1, 0, 2048]);
        let back = mirror_portal(&level, 1, b).unwrap();
        assert_eq!(back.vertices, level.rooms[0].portals[0].vertices);
        assert_eq!(back.normal, level.rooms[0].portals[0].normal);
    }

    #[test]
    fn missing_return_portals_and_object_rooms_are_found_and_fixed() {
        let mut level = level();
        let fixed = level.clone();
        level.rooms[1].portals.clear();
        level.placements[0].position[0] = 2560;
        let problems = diagnose(&level, None);
        assert_eq!(
            problems.iter().map(|p| p.check).collect::<Vec<_>>(),
            [Check::OneSidedPortal, Check::LinkWithoutPortal, Check::ObjectInOtherRoom]
        );
        for p in &problems {
            if let Some(fix) = &p.fix {
                apply_fix(&mut level, fix).unwrap();
            }
        }
        let portals =
            |l: &Level| l.rooms[1].portals.iter().map(|p| (p.room, p.normal, p.vertices.clone())).collect::<Vec<_>>();
        assert_eq!(portals(&level), portals(&fixed));
        assert_eq!(level.placements[0].room, 1);
        assert!(apply_fix(&mut level, &Fix::ReturnPortal { room: 0, portal: 0 }).is_err(), "already two-sided");
        assert_eq!(diagnose(&level, None), []);
    }

    #[test]
    fn links_heights_and_navigation_boxes_are_checked() {
        let opened = level();
        let mut level = opened.clone();
        // The doorway's floor no longer meets B's, and B's floor moved under its box.
        level.rooms[0].sectors[2 * 3 + 1].floor = -256;
        level.rooms[1].sectors[4].floor = -512;
        assert_eq!(checks(&level, Some(&opened)), [Check::HeightMismatch, Check::NavBoxStale]);
        assert_eq!(checks(&level, None), [Check::HeightMismatch], "staleness needs the opened level");
        let mut level = opened.clone();
        level.rooms[1].sectors[4] = Sector { floor: SOLID, ceiling: SOLID, ..Default::default() };
        level.rooms[0].sectors[4].nav_box = None;
        level.rooms[0].sectors[2 * 3 + 1].nav_box = Some(0);
        level.placements[0].position = [512, 0, 512];
        assert_eq!(
            checks(&level, None),
            [Check::NavBoxMissing, Check::LinkIntoSolid, Check::NavBoxOutside, Check::ObjectOutside]
        );
        let key = diagnose(&level, None)[0].key();
        assert_eq!(key, (Check::NavBoxMissing, [1, 1, 0, -2048, 0, 0]), "keyed by world place, not room number");
    }

    #[test]
    fn flip_rooms_are_checked_in_the_state_they_are_seen_in() {
        // Room 2 is room 1's alternate: the same room, at first.
        let mut level = level();
        let mut alternate = level.rooms[1].clone();
        level.rooms[1].alternate = Some(2);
        level.rooms.push(alternate.clone());
        assert_eq!(diagnose(&level, Some(&level)), []);
        // Once flipped, A's doorway steps up onto the alternate's raised floor.
        level.rooms[2].sectors[4].floor = -256;
        let problems = diagnose(&level, None);
        assert_eq!(problems.len(), 1, "{problems:?}");
        let p = &problems[0];
        assert_eq!((p.check, p.room, p.other, p.flipped, &p.fix), (Check::HeightMismatch, 0, Some(1), true, &None));
        assert!(p.detail.starts_with("When flipped: Room 0 "), "{}", p.detail);
        // A problem in the alternate's own data is reported in the alternate.
        level.rooms[2].sectors[4].floor = 0;
        level.rooms[2].sectors[4].nav_box = None;
        let problems = diagnose(&level, None);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!((problems[0].check, problems[0].room, problems[0].sector), (Check::NavBoxMissing, 2, Some((1, 1))));
        assert!(problems[0].detail.starts_with("When flipped: Room 2 "), "{}", problems[0].detail);
        assert_eq!(problems[0].place[5], 1, "distinct from the same problem in room 1");
        // Pairs on different grids, and links naming the alternate.
        alternate.origin[0] += 1024;
        level.rooms[2] = alternate;
        level.rooms[0].portals[0].room = 2;
        let pairs: Vec<_> = diagnose(&level, None)
            .into_iter()
            .filter(|p| p.check == Check::FlipPair)
            .map(|p| (p.room, p.other))
            .collect();
        assert_eq!(pairs, [(1, Some(2)), (0, Some(2))]);
    }
}
