//! Which room a position belongs to, and how rooms join, for authoring tools.
//!
//! A placement's `room` is authored data: 57 of the 2,706 imported
//! placements sit where a strict geometric lookup would put them in another
//! room (items floating on water above a wet room, an object over its room's
//! ceiling). Tools therefore never recompute it from scratch. When something
//! moves, [`moved_room`] looks the room up from both the old and the new
//! position and changes it only if that lookup changed, so a move that crosses
//! a portal or a vertical link carries the object across, and any other move
//! keeps what was authored.
use crate::world::{Level, Room, format::SOLID};

/// How one room opens into another, from its render portals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Link {
    /// A doorway in a wall.
    Side,
    /// A hole in the floor into the room underneath.
    Below,
    /// An opening in the ceiling into the room overhead.
    Above,
}

/// Every room `room`'s portals open into, with how. A portal's normal points
/// into its own room: up (negative y) for a hole in the floor.
pub fn neighbours(level: &Level, room: usize) -> Vec<(usize, Link)> {
    let Some(r) = level.rooms.get(room) else { return vec![] };
    let mut out: Vec<_> = r
        .portals
        .iter()
        .map(|p| {
            let link = match p.normal[1] {
                y if y < 0 => Link::Below,
                y if y > 0 => Link::Above,
                _ => Link::Side,
            };
            (p.room, link)
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The sector of `room` under level x and z, if inside its grid.
fn sector(room: &Room, x: i32, z: i32) -> Option<&crate::world::Sector> {
    let sx = (x - room.origin[0]).div_euclid(1024);
    let sz = (z - room.origin[2]).div_euclid(1024);
    if sx < 0 || sz < 0 || sx as usize >= room.x_sectors || sz as usize >= room.z_sectors {
        return None;
    }
    room.sectors.get(sx as usize * room.z_sectors + sz as usize)
}

/// The room a level position is in, walking from `room` by the rules
/// `WorldGeometry::floor_at` uses: horizontal floor-data portals first, then
/// the room below when at or under a sector's floor and the room above when
/// over its ceiling. `None` when the walk ends in a solid sector or outside
/// the grid.
pub fn locate_room(level: &Level, room: usize, p: [i32; 3]) -> Option<usize> {
    let [x, y, z] = p;
    let rooms = level.rooms.len();
    let mut room = room;
    // Portals form cycles. Bound the walk, as floor_at does.
    for _ in 0..rooms.min(64) {
        let s = sector(level.rooms.get(room)?, x, z)?;
        if let Some(next) = level.floor_info(s).portal.filter(|&r| r != room && r < rooms) {
            room = next;
        } else if let Some(below) = s.below.filter(|&r| y >= s.floor && r != room && r < rooms) {
            room = below;
        } else if let Some(above) = s.above.filter(|&r| y < s.ceiling && r != room && r < rooms) {
            room = above;
        } else {
            break;
        }
    }
    let s = sector(&level.rooms[room], x, z)?;
    (s.floor > SOLID).then_some(room)
}

/// Every room whose grid has an open sector under `p` and whose height range
/// contains it: where a position dropped outside its own room may belong.
pub fn rooms_containing(level: &Level, p: [i32; 3]) -> Vec<usize> {
    let [x, y, z] = p;
    (0..level.rooms.len())
        .filter(|&i| {
            let r = &level.rooms[i];
            r.top <= y && y <= r.bottom && sector(r, x, z).is_some_and(|s| s.floor > SOLID)
        })
        .collect()
}

/// What a move from `from` to `to` does to an object authored in `room`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomChange {
    /// The move crossed no boundary: keep the authored room.
    Keep,
    /// The move carried it into this room.
    Into(usize),
    /// The new position is in no room (solid or outside every grid).
    Outside,
}

/// The room an object authored in `room` belongs to after moving from `from`
/// to `to`. See the module notes for why the authored room is kept unless the
/// move crosses into another one.
pub fn moved_room(level: &Level, room: usize, from: [i32; 3], to: [i32; 3]) -> RoomChange {
    if from == to {
        return RoomChange::Keep;
    }
    let before = locate_room(level, room, from);
    match locate_room(level, room, to) {
        Some(after) if Some(after) == before || after == room => RoomChange::Keep,
        Some(after) => RoomChange::Into(after),
        // Dropped where the walk from its room can't reach (typed far away,
        // say): any room that holds the position, preferring its own.
        None => {
            // Not an alternate: objects are authored in the room it replaces.
            let bases = super::flip::flip_bases(level);
            let found: Vec<_> = rooms_containing(level, to)
                .into_iter()
                .filter_map(|r| locate_room(level, r, to))
                .filter(|&r| bases[r].is_none())
                .collect();
            match found.as_slice() {
                found if found.contains(&room) => RoomChange::Keep,
                [first, ..] => RoomChange::Into(*first),
                // Already outside before the move: nothing new to report.
                [] if before.is_none() => RoomChange::Keep,
                [] => RoomChange::Outside,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{FloorInfo, Sector, format::Portal};

    /// Two 3x3 rooms along x overlapping by two sectors, as imported rooms
    /// do, with a doorway at z sector 1, and a room under the first one's
    /// centre.
    fn level() -> Level {
        let open = Sector { floor: 0, ceiling: -2048, ..Default::default() };
        let solid = Sector { floor: SOLID, ceiling: SOLID, ..Default::default() };
        let room = |origin: [i32; 3], top, bottom| Room {
            origin,
            top,
            bottom,
            x_sectors: 3,
            z_sectors: 3,
            sectors: (0..9).map(|i| if i == 4 { open } else { solid }).collect(),
            ..Default::default()
        };
        let mut a = room([0, 0, 0], -2048, 0);
        let mut b = room([1024, 0, 0], -2048, 0);
        let c = room([0, 0, 0], 0, 2048);
        // A's border sector (2, 1) is B's centre: a doorway into B. B's
        // border sector (0, 1) is A's centre: the way back.
        a.sectors[2 * 3 + 1] = Sector { floor_info: Some(0), ..open };
        b.sectors[1] = Sector { floor_info: Some(1), ..open };
        a.sectors[4].below = Some(2);
        let mut c = c;
        c.sectors[4] = Sector { floor: 2048, ceiling: 0, above: Some(0), ..open };
        a.portals = vec![
            Portal { room: 1, normal: [-1, 0, 0], vertices: vec![] },
            Portal { room: 2, normal: [0, -1, 0], vertices: vec![] },
        ];
        b.portals = vec![Portal { room: 0, normal: [1, 0, 0], vertices: vec![] }];
        c.portals = vec![Portal { room: 0, normal: [0, 1, 0], vertices: vec![] }];
        Level {
            rooms: vec![a, b, c],
            floor_infos: vec![
                FloorInfo { portal: Some(1), ..Default::default() },
                FloorInfo { portal: Some(0), ..Default::default() },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn portals_name_their_neighbours_and_which_way_they_open() {
        let level = level();
        assert_eq!(neighbours(&level, 0), [(1, Link::Side), (2, Link::Below)]);
        assert_eq!(neighbours(&level, 2), [(0, Link::Above)]);
    }

    #[test]
    fn lookups_follow_doorways_and_vertical_links_like_floor_at() {
        let level = level();
        assert_eq!(locate_room(&level, 0, [1536, -512, 1536]), Some(0));
        // Through A's doorway sector into B's centre.
        assert_eq!(locate_room(&level, 0, [2560, -512, 1536]), Some(1));
        // At or under A's floor over the hole: the room below.
        assert_eq!(locate_room(&level, 0, [1536, 0, 1536]), Some(2));
        assert_eq!(locate_room(&level, 2, [1536, -512, 1536]), Some(0));
        assert_eq!(locate_room(&level, 0, [512, -512, 512]), None, "solid");
        assert_eq!(locate_room(&level, 0, [99999, 0, 0]), None, "outside the grid");
    }

    #[test]
    fn moves_keep_the_authored_room_unless_they_cross_into_another() {
        let level = level();
        let centre = [1536, -512, 1536];
        assert_eq!(moved_room(&level, 0, centre, [1500, -600, 1600]), RoomChange::Keep);
        assert_eq!(moved_room(&level, 0, centre, [2560, -512, 1536]), RoomChange::Into(1));
        assert_eq!(moved_room(&level, 0, centre, [1536, 512, 1536]), RoomChange::Into(2));
        // Authored in B although it sits in A: small moves leave it in B.
        assert_eq!(moved_room(&level, 1, [1536, -512, 1536], centre), RoomChange::Keep);
        assert_eq!(moved_room(&level, 0, centre, [512, -512, 512]), RoomChange::Outside);
        assert_eq!(moved_room(&level, 0, centre, [99999, 0, 0]), RoomChange::Outside);
        // Typed from the room below straight into B, which its walk can't
        // reach: A's doorway sector holds the point and leads on into B.
        assert_eq!(rooms_containing(&level, [2560, -512, 1536]), [0, 1]);
        assert_eq!(moved_room(&level, 2, [1536, 1024, 1536], [2560, -512, 1536]), RoomChange::Into(1));
    }
}
