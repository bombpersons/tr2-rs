//! Joining rooms: doorways through a shared wall, holes through a floor, and
//! taking either away again, built the way imported levels build them.
//!
//! A doorway joins rooms whose grids overlap by two sectors, so each room's
//! border ring lies over the other's open floor. Each room's border sector in
//! the opening names the other room in its floor data and takes the heights
//! of the open sector under it, so collision walks straight across. Each room
//! keeps wall faces in the shared plane only where its own space reaches
//! beyond the opening (above it, or a riser below it), and a mirrored pair of
//! render portals covers the opening (WALL room 0 and room 4).
//!
//! A hole joins a floor to the ceiling of a room below at the same height:
//! the upper sector loses its floor face and links `below`, the lower one
//! loses its ceiling face and links `above`, and a mirrored pair of
//! horizontal portals covers the hole (WALL room 0 and room 77).
use super::*;
use crate::{
    editor::mirror_portal,
    world::{Sector, format::Portal},
};

/// Which sector of `room` holds world (x, z), if inside its grid.
fn cell(room: &Room, x: i32, z: i32) -> Option<(usize, usize)> {
    let sx = (x - room.origin[0]).div_euclid(SECTOR);
    let sz = (z - room.origin[2]).div_euclid(SECTOR);
    (sx >= 0 && sz >= 0 && (sx as usize) < room.x_sectors && (sz as usize) < room.z_sectors)
        .then_some((sx as usize, sz as usize))
}

fn border(room: &Room, (x, z): (usize, usize)) -> bool {
    x == 0 || z == 0 || x + 1 == room.x_sectors || z + 1 == room.z_sectors
}

fn sector_mut(level: &mut Level, room: usize, (x, z): (usize, usize)) -> &mut Sector {
    let r = &mut level.rooms[room];
    &mut r.sectors[x * r.z_sectors + z]
}

fn sector_of(level: &Level, room: usize, (x, z): (usize, usize)) -> Sector {
    let r = &level.rooms[room];
    r.sectors[x * r.z_sectors + z]
}

/// Set (or clear) the sector's doorway, without changing other sectors that
/// share its floor data.
fn set_doorway(level: &mut Level, room: usize, at: (usize, usize), portal: Option<usize>, slopes: [[i8; 2]; 2]) {
    let sector = sector_of(level, room, at);
    let mut info = level.floor_info(&sector).clone();
    info.portal = portal;
    if portal.is_some() {
        (info.floor_slope, info.ceiling_slope) = (slopes[0], slopes[1]);
    } else {
        (info.floor_slope, info.ceiling_slope) = ([0, 0], [0, 0]);
    }
    let found = if info == FloorInfo::default() {
        None
    } else {
        Some(level.floor_infos.iter().position(|f| *f == info).unwrap_or_else(|| {
            level.floor_infos.push(info);
            level.floor_infos.len() - 1
        }))
    };
    sector_mut(level, room, at).floor_info = found;
}

/// The vertex at `p`, reusing one already there so faces stay joined.
fn shared_vertex(room: &mut Room, p: [i32; 3]) -> usize {
    if let Some(v) = room.vertices.iter().position(|v| [v[0], v[1], v[2]] == p) {
        return v;
    }
    let shade = room
        .vertices
        .iter()
        .min_by_key(|v| (0..3).map(|a| i64::from(v[a] - p[a]).abs()).sum::<i64>())
        .map_or(128, |v| v[3]);
    room.vertices.push([p[0], p[1], p[2], shade]);
    room.vertices.len() - 1
}

/// Add a face through `points` seen from `toward`: a quad, or a triangle
/// when two of its corners meet.
fn add_face(room: &mut Room, points: [[i32; 3]; 4], toward: [i32; 3], texture: usize) {
    let mut unique: Vec<[i32; 3]> = vec![];
    for p in points {
        if !unique.contains(&p) {
            unique.push(p);
        }
    }
    if unique.len() < 3 {
        return;
    }
    let mut corners: Vec<usize> = unique.iter().map(|&p| shared_vertex(room, p)).collect();
    let n: i64 = (0..3).map(|axis| normal(&unique, axis) * i64::from(toward[axis])).sum();
    if n > 0 {
        corners.reverse();
    }
    match corners.as_slice() {
        [a, b, c] => room.triangles.push([*a, *b, *c, texture]),
        [a, b, c, d] => room.quads.push([*a, *b, *c, *d, texture]),
        _ => (),
    }
}

/// Faces of `room` lying in the vertical plane `axis` = `at` (room-local)
/// within one sector of the other axis starting at `along` (room-local
/// sector). Removes them and returns a texture they used.
fn clear_plane(room: &mut Room, axis: usize, at: i32, along: i32) -> Option<usize> {
    let other = 2 - axis;
    let (lo, hi) = (along * SECTOR, along * SECTOR + SECTOR);
    split_walls(room, axis, at, lo, hi);
    let inside = |corners: &[usize], room: &Room| {
        corners.iter().all(|&v| {
            let p = point(room, v);
            p[axis] == at && (lo..=hi).contains(&p[other])
        })
    };
    let mut texture = None;
    let mut gone = BTreeSet::new();
    let quads = std::mem::take(&mut room.quads);
    for q in quads {
        if inside(&q[..4], room) {
            texture.get_or_insert(q[4]);
            gone.extend(q[..4].iter().copied());
        } else {
            room.quads.push(q);
        }
    }
    let triangles = std::mem::take(&mut room.triangles);
    for t in triangles {
        if inside(&t[..3], room) {
            texture.get_or_insert(t[3]);
            gone.extend(t[..3].iter().copied());
        } else {
            room.triangles.push(t);
        }
    }
    collect_unused(room, &gone);
    texture
}

/// Cut quads in the plane `axis` = `at` that cross `lo..hi` on the other
/// axis but reach past it into one quad per sector, so one sector of a long
/// wall can open (the sandbox's walls span the whole room). Only walls with
/// two vertical edges are cut; heights between them are interpolated.
fn split_walls(room: &mut Room, axis: usize, at: i32, lo: i32, hi: i32) {
    let other = 2 - axis;
    let mut cut = vec![];
    for (i, q) in room.quads.iter().enumerate() {
        let points: Vec<[i32; 3]> = q[..4].iter().map(|&v| point(room, v)).collect();
        if points.iter().any(|p| p[axis] != at) {
            continue;
        }
        let ends: BTreeSet<i32> = points.iter().map(|p| p[other]).collect();
        let [start, end] = ends.iter().copied().collect::<Vec<_>>()[..] else { continue };
        if end <= lo || start >= hi || (start >= lo && end <= hi) {
            continue;
        }
        let span = |at: i32| {
            let ys: Vec<i32> = points.iter().filter(|p| p[other] == at).map(|p| p[1]).collect();
            [*ys.iter().min().unwrap(), *ys.iter().max().unwrap()]
        };
        let toward = if normal(&points, axis) > 0 { -1 } else { 1 };
        cut.push((i, start, end, span(start), span(end), toward, q[4]));
    }
    if cut.is_empty() {
        return;
    }
    let gone: Vec<usize> = cut.iter().map(|c| c.0).collect();
    let corners: BTreeSet<usize> = gone.iter().flat_map(|&i| room.quads[i][..4].to_vec()).collect();
    let mut index = 0;
    room.quads.retain(|_| {
        index += 1;
        !gone.contains(&(index - 1))
    });
    for (_, start, end, a, b, toward, texture) in cut {
        let y = |x: i32, k: usize| a[k] + ((b[k] - a[k]) as i64 * (x - start) as i64 / (end - start) as i64) as i32;
        let mut stops: Vec<i32> =
            (start.div_euclid(SECTOR) + 1..=(end - 1).div_euclid(SECTOR)).map(|s| s * SECTOR).collect();
        stops.insert(0, start);
        stops.push(end);
        let mut dir = [0; 3];
        dir[axis] = toward;
        for pair in stops.windows(2) {
            let p = |x: i32, y: i32| {
                let mut p = [0; 3];
                p[axis] = at;
                p[other] = x;
                p[1] = y;
                p
            };
            let (x0, x1) = (pair[0], pair[1]);
            add_face(room, [p(x0, y(x0, 1)), p(x1, y(x1, 1)), p(x1, y(x1, 0)), p(x0, y(x0, 0))], dir, texture);
        }
    }
    collect_unused(room, &corners);
}

/// Any texture the room uses for `surface`, else for anything.
fn some_texture(room: &Room, surface: Option<Surface>) -> usize {
    let faces = sector_faces(room);
    surface
        .and_then(|s| faces.iter().find(|((_, _, f), _)| *f == s).map(|(_, v)| room.quads[v[0].0][4]))
        .or_else(|| room.quads.first().map(|q| q[4]))
        .unwrap_or(0)
}

/// A doorway's place: the wall plane between a room's open sector `inside`
/// and its border sector `beyond`, in world units.
#[derive(Clone, Copy, Debug)]
struct Opening {
    /// The axis the plane faces along (0 = x, 2 = z).
    axis: usize,
    /// World coordinate of the plane on `axis`.
    at: i32,
    /// World sector number along the other axis.
    along: i32,
    /// Which way, along `axis`, the room's open side lies (+1 or -1).
    toward: i32,
}

impl Opening {
    fn other(self) -> usize {
        2 - self.axis
    }
    /// World (x, z) of the sector centre on side `side` (+1 or -1) of the plane.
    fn centre(self, side: i32) -> (i32, i32) {
        let mut p = [0; 3];
        p[self.axis] = self.at + side * SECTOR / 2;
        p[self.other()] = self.along * SECTOR + SECTOR / 2;
        (p[0], p[2])
    }
    /// The open sector of `room` on its side and the border sector beyond.
    fn cells(self, room: &Room, toward: i32) -> Option<((usize, usize), (usize, usize))> {
        let (ix, iz) = self.centre(toward);
        let (bx, bz) = self.centre(-toward);
        Some((cell(room, ix, iz)?, cell(room, bx, bz)?))
    }
    /// Heights of a sector's floor or ceiling along the plane, low end first.
    fn edge_heights(
        self,
        level: &Level,
        room: usize,
        at: (usize, usize),
        toward: i32,
        surface: Surface,
    ) -> Option<[i32; 2]> {
        // The plane is the sector's low edge when its room lies on the plane's + side.
        Some(edge(sector_corners(level, room, at.0, at.1, surface)?, self.axis, toward < 0))
    }
    /// Rebuild `room`'s faces in the plane over this sector: wall from each
    /// `spans` entry's top to bottom, at both ends.
    fn rebuild(self, level: &mut Level, room: usize, toward: i32, spans: &[([i32; 2], [i32; 2])]) {
        let r = &mut level.rooms[room];
        let (axis, other) = (self.axis, self.other());
        let at = self.at - r.origin[axis];
        let along = self.along - r.origin[other].div_euclid(SECTOR);
        let texture = clear_plane(r, axis, at, along).unwrap_or_else(|| some_texture(r, None));
        let mut dir = [0; 3];
        dir[axis] = toward;
        for &(top, bottom) in spans {
            if top[0] >= bottom[0] && top[1] >= bottom[1] {
                continue;
            }
            let p = |end: i32, y: i32| {
                let mut p = [0; 3];
                p[axis] = at;
                p[other] = (along + end) * SECTOR;
                p[1] = y;
                p
            };
            let (top, bottom) = ([top[0].min(bottom[0]), top[1].min(bottom[1])], bottom);
            add_face(r, [p(0, bottom[0]), p(1, bottom[1]), p(1, top[1]), p(0, top[0])], dir, texture);
        }
        fit_height(r);
    }
}

/// A rectangle of render portal in world units: the plane on `axis` at
/// `at`, spanning `low..high` on the other two axes (`[x, y, z]` with the
/// plane's axis ignored).
#[derive(Clone, Copy, Debug)]
pub(super) struct Window {
    pub(super) axis: usize,
    pub(super) at: i32,
    pub(super) low: [i32; 3],
    pub(super) high: [i32; 3],
}

impl Window {
    pub(super) fn union(self, o: Window) -> Window {
        Window {
            low: [0, 1, 2].map(|i| self.low[i].min(o.low[i])),
            high: [0, 1, 2].map(|i| self.high[i].max(o.high[i])),
            ..self
        }
    }
    /// The portal of room `from` into `to`, facing `normal` (into `from`).
    /// Imported portals sit one unit short of the plane on the side their
    /// normal is positive, and wind against their normal.
    fn portal(self, level: &Level, from: usize, to: usize, facing: [i32; 3]) -> Portal {
        let r = &level.rooms[from];
        let (u, v) = match self.axis {
            0 => (2, 1),
            1 => (0, 2),
            _ => (0, 1),
        };
        let mut points: Vec<[i32; 3]> = [(false, false), (false, true), (true, true), (true, false)]
            .iter()
            .map(|&(hu, hv)| {
                let mut p = [0; 3];
                p[self.axis] = self.at - i32::from(facing[self.axis] > 0);
                p[u] = if hu { self.high[u] } else { self.low[u] };
                p[v] = if hv { self.high[v] } else { self.low[v] };
                p
            })
            .collect();
        if normal(&points, self.axis).signum() == i64::from(facing[self.axis]).signum() {
            points.reverse();
        }
        let vertices = points.into_iter().map(|p| [p[0] - r.origin[0], p[1], p[2] - r.origin[2]]).collect();
        Portal { room: to, normal: facing, vertices }
    }
    /// The window an existing portal of `room` covers.
    fn of(level: &Level, room: usize, p: &Portal) -> Option<Window> {
        let r = &level.rooms[room];
        let axis = (0..3).find(|&i| p.normal[i] != 0)?;
        let world: Vec<[i32; 3]> = p.vertices.iter().map(|v| [v[0] + r.origin[0], v[1], v[2] + r.origin[2]]).collect();
        let first = *world.first()?;
        let low = world.iter().fold(first, |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
        let high = world.iter().fold(first, |a, p| [a[0].max(p[0]), a[1].max(p[1]), a[2].max(p[2])]);
        Some(Window { axis, at: first[axis] + i32::from(p.normal[axis] > 0), low, high })
    }
}

/// Widen (or add) `from`'s portal into `to` facing `normal` to cover
/// `window`, and give `to` its mirror.
pub(super) fn open_window(level: &mut Level, from: usize, to: usize, normal: [i32; 3], window: Window) {
    let existing = level.rooms[from].portals.iter().position(|p| p.room == to && p.normal == normal);
    let window = existing
        .and_then(|i| Window::of(level, from, &level.rooms[from].portals[i]))
        .map_or(window, |w| w.union(window));
    let portal = window.portal(level, from, to, normal);
    let mirror = mirror_portal(level, from, &portal).expect("portals have a normal");
    match existing {
        Some(i) => level.rooms[from].portals[i] = portal,
        None => level.rooms[from].portals.push(portal),
    }
    let back = normal.map(|n| -n);
    let r = &mut level.rooms[to];
    match r.portals.iter().position(|p| p.room == from && p.normal == back) {
        Some(i) => r.portals[i] = mirror,
        None => r.portals.push(mirror),
    }
}

/// The wall plane a wall face of `room` stands in, as a world opening.
fn opening_of(level: &Level, room: usize, quad: usize) -> Result<Opening, String> {
    let r = &level.rooms[room];
    let w =
        r.quads.get(quad).and_then(|q| wall_face(r, &q[..4])).ok_or("select wall faces that span one sector edge")?;
    let other = 2 - w.axis;
    Ok(Opening {
        axis: w.axis,
        at: w.at + r.origin[w.axis],
        along: w.along + r.origin[other].div_euclid(SECTOR),
        toward: w.toward,
    })
}

/// Open a doorway through the wall face `quad` of `room` into the room
/// beyond it. Returns that room.
pub fn connect(level: &mut Level, room: usize, quad: usize) -> Result<usize, String> {
    let o = opening_of(level, room, quad)?;
    let a = &level.rooms[room];
    let Some((ai, ab)) = o.cells(a, o.toward) else {
        return Err("this wall is on the room's outer edge".into());
    };
    if sector_corners(level, room, ai.0, ai.1, Surface::Floor).is_none() {
        return Err("connect a wall seen from an open sector".into());
    }
    if !border(a, ab) {
        return Err("the sector behind this wall belongs to this room; push the wall out instead".into());
    }
    if level.floor_info(&sector_of(level, room, ab)).portal.is_some() {
        return Err("this wall is already a doorway".into());
    }
    let (a_floor, a_ceiling) = (
        o.edge_heights(level, room, ai, o.toward, Surface::Floor).ok_or("no floor here")?,
        o.edge_heights(level, room, ai, o.toward, Surface::Ceiling).ok_or("no ceiling here")?,
    );
    // The room beyond: its open sector lies over our border, and its border over our open sector.
    let mut reasons = vec![];
    let found = (0..level.rooms.len()).filter(|&b| b != room).find_map(|b| {
        let r = &level.rooms[b];
        let (bi, bb) = o.cells(r, -o.toward)?;
        if border(r, bi) || !border(r, bb) || level.floor_info(&sector_of(level, b, bb)).portal.is_some() {
            return None;
        }
        let floor = o.edge_heights(level, b, bi, -o.toward, Surface::Floor)?;
        let ceiling = o.edge_heights(level, b, bi, -o.toward, Surface::Ceiling)?;
        let top = [a_ceiling[0].max(ceiling[0]), a_ceiling[1].max(ceiling[1])];
        let bottom = [a_floor[0].min(floor[0]), a_floor[1].min(floor[1])];
        if (0..2).any(|e| top[e] >= bottom[e]) {
            reasons.push(format!("room {b} is there but its space does not meet this room's at this wall"));
            return None;
        }
        Some((b, bi, bb, floor, ceiling, top, bottom))
    });
    let Some((b, bi, bb, b_floor, b_ceiling, top, bottom)) = found else {
        return Err(reasons.into_iter().next().unwrap_or_else(|| {
            "no room beyond this wall overlaps this one by two sectors (each room's border must lie over the other's open floor)"
                .into()
        }));
    };
    // Walls where each room's own space reaches past the opening.
    o.rebuild(level, room, o.toward, &[(a_ceiling, top), (bottom, a_floor)]);
    o.rebuild(level, b, -o.toward, &[(b_ceiling, top), (bottom, b_floor)]);
    // Each border sector takes the heights of the open sector under it.
    for (from, at, to, under) in [(room, ab, b, bi), (b, bb, room, ai)] {
        let s = sector_of(level, to, under);
        let info = level.floor_info(&s).clone();
        let target = sector_mut(level, from, at);
        (target.floor, target.ceiling) = (s.floor, s.ceiling);
        set_doorway(level, from, at, Some(to), [info.floor_slope, info.ceiling_slope]);
    }
    let other = o.other();
    let mut low = [0; 3];
    let mut high = [0; 3];
    low[other] = o.along * SECTOR;
    high[other] = o.along * SECTOR + SECTOR;
    (low[1], high[1]) = (top[0].min(top[1]), bottom[0].max(bottom[1]));
    let mut normal = [0; 3];
    normal[o.axis] = o.toward;
    open_window(level, room, b, normal, Window { axis: o.axis, at: o.at, low, high });
    for (r, at) in [(room, ai), (b, bi)] {
        check_walls(level, r, faces_on_edges(&level.rooms[r], &BTreeSet::from([at])))?;
    }
    Ok(b)
}

/// Open a hole through floor face `quad` of `room` into the room below,
/// whose ceiling there must meet the floor exactly. Returns that room.
pub fn open_hole(level: &mut Level, room: usize, quad: usize) -> Result<usize, String> {
    let r = &level.rooms[room];
    let f = r.quads.get(quad).and_then(|q| sector_face(r, q)).filter(|f| f.surface == Surface::Floor);
    let f = f.ok_or("select floor faces to open holes in")?;
    let a_cell = (f.x, f.z);
    if sector_of(level, room, a_cell).below.is_some() {
        return Err("this floor already opens below".into());
    }
    let (x, z) = (r.origin[0] + f.x as i32 * SECTOR + SECTOR / 2, r.origin[2] + f.z as i32 * SECTOR + SECTOR / 2);
    let below = (0..level.rooms.len()).filter(|&c| c != room).find_map(|c| {
        let cr = &level.rooms[c];
        let at = cell(cr, x, z)?;
        let ceiling = sector_corners(level, c, at.0, at.1, Surface::Ceiling)?;
        (!border(cr, at) && ceiling == f.corners && sector_of(level, c, at).above.is_none()).then_some((c, at))
    });
    let (c, c_cell) = below.ok_or("no room below has its ceiling exactly at this floor; match their heights first")?;
    let had_a: BTreeSet<_> = sector_faces(&level.rooms[room]).keys().copied().collect();
    let had_c: BTreeSet<_> = sector_faces(&level.rooms[c]).keys().copied().collect();
    remove_quads(&mut level.rooms[room], &[quad]);
    let ceilings: Vec<usize> = sector_faces(&level.rooms[c])
        .get(&(c_cell.0, c_cell.1, Surface::Ceiling))
        .map(|v| v.iter().map(|&(i, _)| i).collect())
        .unwrap_or_default();
    remove_quads(&mut level.rooms[c], &ceilings);
    sector_mut(level, room, a_cell).below = Some(c);
    sector_mut(level, c, c_cell).above = Some(room);
    derive(level, room, &BTreeSet::from([a_cell]), &had_a)?;
    derive(level, c, &BTreeSet::from([c_cell]), &had_c)?;
    let y = sector_of(level, room, a_cell).floor;
    let (x0, z0) = (x - SECTOR / 2, z - SECTOR / 2);
    let window = Window { axis: 1, at: y, low: [x0, y, z0], high: [x0 + SECTOR, y, z0 + SECTOR] };
    open_window(level, room, c, [0, -1, 0], window);
    Ok(c)
}

fn remove_quads(room: &mut Room, quads: &[usize]) {
    let gone: BTreeSet<usize> = quads.iter().flat_map(|&q| room.quads[q][..4].to_vec()).collect();
    let mut i = 0;
    room.quads.retain(|_| {
        i += 1;
        !quads.contains(&(i - 1))
    });
    collect_unused(room, &gone);
}

/// The border sector's doorway plane: the open sector inside it and the
/// direction toward the room's inside.
fn doorway_plane(level: &Level, room: usize, (x, z): (usize, usize)) -> Option<(Opening, (usize, usize))> {
    let r = &level.rooms[room];
    let (w, d) = (r.x_sectors, r.z_sectors);
    let (axis, toward, inside) = if x == 0 {
        (0, 1, (1, z))
    } else if x + 1 == w {
        (0, -1, (w - 2, z))
    } else if z == 0 {
        (2, 1, (x, 1))
    } else if z + 1 == d {
        (2, -1, (x, d - 2))
    } else {
        return None;
    };
    let other = 2 - axis;
    let line = [inside.0, 0, inside.1][axis] as i32 + if toward > 0 { 0 } else { 1 };
    let along = [x, 0, z][other] as i32;
    Some((
        Opening { axis, at: line * SECTOR + r.origin[axis], along: along + r.origin[other].div_euclid(SECTOR), toward },
        inside,
    ))
}

/// Take away every doorway and hole between rooms `a` and `b`, closing them
/// with walls, floors and ceilings, and remove their portals.
pub fn disconnect(level: &mut Level, a: usize, b: usize) -> Result<(), String> {
    if a == b || a >= level.rooms.len() || b >= level.rooms.len() {
        return Err("no such pair of rooms".into());
    }
    let mut changed = false;
    for (from, to) in [(a, b), (b, a)] {
        let r = &level.rooms[from];
        let cells: Vec<(usize, usize)> = (0..r.x_sectors).flat_map(|x| (0..r.z_sectors).map(move |z| (x, z))).collect();
        let had: BTreeSet<_> = sector_faces(r).keys().copied().collect();
        let mut derived = BTreeSet::new();
        for at in cells {
            let s = sector_of(level, from, at);
            if level.floor_info(&s).portal == Some(to) {
                if let Some((o, inside)) = doorway_plane(level, from, at) {
                    let spans = match (
                        o.edge_heights(level, from, inside, o.toward, Surface::Ceiling),
                        o.edge_heights(level, from, inside, o.toward, Surface::Floor),
                    ) {
                        (Some(top), Some(bottom)) => vec![(top, bottom)],
                        _ => vec![],
                    };
                    o.rebuild(level, from, o.toward, &spans);
                }
                set_doorway(level, from, at, None, [[0, 0]; 2]);
                let s = sector_mut(level, from, at);
                (s.floor, s.ceiling) = (SOLID, SOLID);
                changed = true;
            }
            for surface in [Surface::Floor, Surface::Ceiling] {
                let link = if surface == Surface::Floor { s.below } else { s.above };
                if link == Some(to) && close_link(level, from, at, surface) {
                    derived.insert(at);
                    changed = true;
                }
            }
        }
        derive(level, from, &derived, &had)?;
        level.rooms[from].portals.retain(|p| p.room != to);
        fit_height(&mut level.rooms[from]);
    }
    if !changed {
        return Err(format!("rooms {a} and {b} have no doorway or hole between them"));
    }
    Ok(())
}

/// Give a sector back the floor (or ceiling) face a hole took, at its stored
/// height and slope, and drop the link through it. Returns false for a
/// solid sector. The caller re-derives the sector.
fn close_link(level: &mut Level, room: usize, at: (usize, usize), surface: Surface) -> bool {
    let Some(c) = sector_corners(level, room, at.0, at.1, surface) else { return false };
    let r = &mut level.rooms[room];
    // Renderable surfaces may cover a sector whose collision still links
    // through to a neighbour. Reuse that face, including its texture/shades,
    // instead of making an overlapping duplicate when the link closes.
    if !sector_faces(r).contains_key(&(at.0, at.1, surface)) {
        let (x0, z0) = (at.0 as i32 * SECTOR, at.1 as i32 * SECTOR);
        let p = |i: usize| [x0 + (i & 1) as i32 * SECTOR, c[i], z0 + (i >> 1) as i32 * SECTOR];
        let toward = if surface == Surface::Floor { [0, -1, 0] } else { [0, 1, 0] };
        let texture = some_texture(r, Some(surface));
        add_face(r, [p(0), p(1), p(3), p(2)], toward, texture);
    }
    let s = sector_mut(level, room, at);
    if surface == Surface::Floor {
        s.below = None;
    } else {
        s.above = None;
    }
    fit_height(&mut level.rooms[room]);
    true
}

/// Fit the horizontal portal pair between `upper` and `lower` to the holes
/// left between them, or remove it when none are.
fn refit_holes(level: &mut Level, upper: usize, lower: usize) {
    let r = &level.rooms[upper];
    let mut window: Option<Window> = None;
    for x in 0..r.x_sectors {
        for z in 0..r.z_sectors {
            let s = r.sectors[x * r.z_sectors + z];
            if s.below != Some(lower) {
                continue;
            }
            let (x0, z0) = (r.origin[0] + x as i32 * SECTOR, r.origin[2] + z as i32 * SECTOR);
            let w = Window { axis: 1, at: s.floor, low: [x0, s.floor, z0], high: [x0 + SECTOR, s.floor, z0 + SECTOR] };
            window = Some(window.map_or(w, |old| old.union(w)));
        }
    }
    level.rooms[upper].portals.retain(|p| !(p.room == lower && p.normal[1] != 0));
    level.rooms[lower].portals.retain(|p| !(p.room == upper && p.normal[1] != 0));
    if let Some(window) = window {
        open_window(level, upper, lower, [0, -1, 0], window);
    }
}

/// Close the hole beside each of several faces of `room`: a floor face seen
/// through a hole above it (the upper room's floor comes back), or a ceiling
/// face seen through a hole below it (the lower room's ceiling comes back).
/// Both rooms get their face again, the links go, and the portal pair
/// shrinks to the holes left. The upper sector's navigation box was the
/// floor beneath's, so it is cleared for a rebuild. Returns the rooms the
/// holes joined `room` to.
pub fn close_holes(level: &mut Level, room: usize, quads: &[usize]) -> Result<BTreeSet<usize>, String> {
    let r = &level.rooms[room];
    let mut holes = BTreeSet::new();
    for &q in quads {
        let f =
            r.quads.get(q).and_then(|f| sector_face(r, f)).ok_or("select floor or ceiling faces seen through holes")?;
        let s = sector_of(level, room, (f.x, f.z));
        let (upper, lower) = match f.surface {
            Surface::Floor => (s.above.ok_or("a selected floor has no hole above it")?, room),
            Surface::Ceiling => (room, s.below.ok_or("a selected ceiling has no hole below it")?),
        };
        let (x, z) = (r.origin[0] + f.x as i32 * SECTOR + SECTOR / 2, r.origin[2] + f.z as i32 * SECTOR + SECTOR / 2);
        let (Some(u), Some(l)) = (cell(&level.rooms[upper], x, z), cell(&level.rooms[lower], x, z)) else {
            return Err("the hole's rooms do not both cover it".into());
        };
        if sector_of(level, upper, u).below != Some(lower) || sector_of(level, lower, l).above != Some(upper) {
            return Err("this hole does not link both rooms both ways; Diagnostics shows why".into());
        }
        holes.insert((upper, u, lower, l));
    }
    if holes.is_empty() {
        return Err("select floor or ceiling faces seen through holes".into());
    }
    let rooms: BTreeSet<usize> = holes.iter().flat_map(|&(u, _, l, _)| [u, l]).collect();
    let had: BTreeMap<usize, BTreeSet<_>> =
        rooms.iter().map(|&r| (r, sector_faces(&level.rooms[r]).keys().copied().collect())).collect();
    let mut derived: BTreeMap<usize, BTreeSet<(usize, usize)>> = BTreeMap::new();
    for &(upper, u, lower, l) in &holes {
        close_link(level, upper, u, Surface::Floor);
        close_link(level, lower, l, Surface::Ceiling);
        sector_mut(level, upper, u).nav_box = None;
        derived.entry(upper).or_default().insert(u);
        derived.entry(lower).or_default().insert(l);
    }
    for (r, cells) in &derived {
        derive(level, *r, cells, &had[r])?;
    }
    let pairs: BTreeSet<(usize, usize)> = holes.iter().map(|&(u, _, l, _)| (u, l)).collect();
    for &(upper, lower) in &pairs {
        refit_holes(level, upper, lower);
    }
    Ok(pairs.into_iter().map(|(u, l)| if u == room { l } else { u }).collect())
}

/// [`connect`] through each of several wall faces of `room`, as one edit.
/// Faces come and go as doorways open, so each face is found again by its
/// place. Returns the rooms joined.
pub fn connect_faces(level: &mut Level, room: usize, quads: &[usize]) -> Result<BTreeSet<usize>, String> {
    let places: Vec<WallFace> = quads
        .iter()
        .map(|&q| level.rooms[room].quads.get(q).and_then(|f| wall_face(&level.rooms[room], &f[..4])))
        .collect::<Option<_>>()
        .ok_or("select wall faces that span one sector edge")?;
    let mut joined = BTreeSet::new();
    let mut done = BTreeSet::new();
    for w in places {
        if !done.insert((w.axis, w.at, w.along)) {
            continue;
        }
        let r = &level.rooms[room];
        let Some(quad) = (0..r.quads.len()).find(|&i| wall_face(r, &r.quads[i][..4]) == Some(w)) else { continue };
        joined.insert(connect(level, room, quad)?);
    }
    Ok(joined)
}

/// [`open_hole`] through each of several floor faces of `room`, as one edit.
pub fn open_holes(level: &mut Level, room: usize, quads: &[usize]) -> Result<BTreeSet<usize>, String> {
    let r = &level.rooms[room];
    let cells: Vec<(usize, usize)> = quads
        .iter()
        .map(|&q| {
            r.quads.get(q).and_then(|f| sector_face(r, f)).filter(|f| f.surface == Surface::Floor).map(|f| (f.x, f.z))
        })
        .collect::<Option<_>>()
        .ok_or("select floor faces to open holes in")?;
    let mut joined = BTreeSet::new();
    for (x, z) in cells {
        let r = &level.rooms[room];
        let Some(quad) = (0..r.quads.len())
            .find(|&i| sector_face(r, &r.quads[i]).is_some_and(|f| (f.x, f.z, f.surface) == (x, z, Surface::Floor)))
        else {
            continue;
        };
        joined.insert(open_hole(level, room, quad)?);
    }
    Ok(joined)
}
