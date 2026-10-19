//! Splitting a room in two, along a sector line or at a height, and merging
//! two joined rooms back into one.
//!
//! A split along a line leaves the two rooms the way imported neighbours
//! are built (see `connect`): their grids overlap by two sectors, each
//! border sector over the other's open floor becomes a doorway, and a
//! mirrored portal pair covers the opening. A split at a height leaves the
//! upper part's floor and the lower part's ceiling open into each other
//! (`below` / `above`) over every sector that spans the height, as imported
//! holes are. Faces crossing the cut are cut in two; every reference to the
//! room (other rooms' portals and links, objects, cameras) follows the part
//! it now belongs to. Merging is the inverse, for rooms joined only by
//! doorways along one line, or only by holes over the same grid.
use super::connect::{Window, open_window};
use super::*;
use crate::world::{Sector, format::Portal};

/// A face in world coordinates: corners `[x, y, z, shade]` in drawing order.
#[derive(Clone, Debug)]
struct Shape {
    corners: Vec<[i32; 4]>,
    texture: usize,
}

fn shapes(room: &Room) -> Vec<Shape> {
    let world = |v: usize| {
        let p = room.vertices[v];
        [p[0] + room.origin[0], p[1], p[2] + room.origin[2], p[3]]
    };
    let quads = room.quads.iter().map(|q| Shape { corners: q[..4].iter().map(|&v| world(v)).collect(), texture: q[4] });
    let triangles =
        room.triangles.iter().map(|t| Shape { corners: t[..3].iter().map(|&v| world(v)).collect(), texture: t[3] });
    quads.chain(triangles).collect()
}

/// Put `shapes` into `room` (origin already set), sharing identical corners.
fn assemble(room: &mut Room, shapes: &[Shape]) {
    let (mut vertices, mut quads, mut triangles) = (vec![], vec![], vec![]);
    let mut index = BTreeMap::new();
    for s in shapes {
        let corners: Vec<usize> = s
            .corners
            .iter()
            .map(|c| {
                let local = [c[0] - room.origin[0], c[1], c[2] - room.origin[2], c[3]];
                *index.entry(local).or_insert_with(|| {
                    vertices.push(local);
                    vertices.len() - 1
                })
            })
            .collect();
        match corners.as_slice() {
            [a, b, c] => triangles.push([*a, *b, *c, s.texture]),
            [a, b, c, d] => quads.push([*a, *b, *c, *d, s.texture]),
            _ => (),
        }
    }
    (room.vertices, room.quads, room.triangles) = (vertices, quads, triangles);
    fit_height(room);
}

/// Which side of the plane `axis` = `at` a shape is on: -1, +1, or 0 when
/// it crosses. A shape lying in the plane goes to the side it is seen from.
fn side(s: &Shape, axis: usize, at: i32) -> i32 {
    let (low, high) = (s.corners.iter().all(|c| c[axis] <= at), s.corners.iter().all(|c| c[axis] >= at));
    match (low, high) {
        (true, true) => {
            let points: Vec<[i32; 3]> = s.corners.iter().map(|c| [c[0], c[1], c[2]]).collect();
            if normal(&points, axis) > 0 { -1 } else { 1 }
        }
        (true, false) => -1,
        (false, true) => 1,
        _ => 0,
    }
}

/// Cut a shape by the plane into its two sides (Sutherland-Hodgman),
/// interpolating positions and shades; pieces of five or six corners are
/// fanned into quads and triangles.
fn cut(s: &Shape, axis: usize, at: i32) -> [Vec<Shape>; 2] {
    let lerp = |a: [i32; 4], b: [i32; 4]| {
        let t = (at - a[axis]) as f64 / (b[axis] - a[axis]) as f64;
        let mut p = [0; 4];
        for i in 0..4 {
            p[i] = if i == axis { at } else { (a[i] as f64 + (b[i] - a[i]) as f64 * t).round() as i32 };
        }
        p
    };
    let keep = |sign: i32| {
        let mut out = vec![];
        let n = s.corners.len();
        for i in 0..n {
            let (a, b) = (s.corners[i], s.corners[(i + 1) % n]);
            let inside = |p: [i32; 4]| (p[axis] - at) * sign >= 0;
            if inside(a) {
                out.push(a);
            }
            if inside(a) != inside(b) && a[axis] != at && b[axis] != at {
                out.push(lerp(a, b));
            }
        }
        out.dedup();
        if out.len() > 1 && out.first() == out.last() {
            out.pop();
        }
        // Fan pieces of five or more corners into a quad and the rest.
        let mut pieces = vec![];
        while out.len() >= 3 {
            if out.len() <= 4 {
                pieces.push(Shape { corners: out, texture: s.texture });
                break;
            }
            pieces.push(Shape { corners: out[..4].to_vec(), texture: s.texture });
            let mut next = vec![out[0]];
            next.extend_from_slice(&out[3..]);
            out = next;
        }
        pieces
    };
    [keep(-1), keep(1)]
}

/// Shapes on each side of the plane, cutting those that cross it.
fn partition(shapes: Vec<Shape>, axis: usize, at: i32) -> [Vec<Shape>; 2] {
    let (mut low, mut high) = (vec![], vec![]);
    for s in shapes {
        match side(&s, axis, at) {
            -1 => low.push(s),
            1 => high.push(s),
            _ => {
                let [l, h] = cut(&s, axis, at);
                low.extend(l);
                high.extend(h);
            }
        }
    }
    [low, high]
}

/// Change a sector's floor data without changing other sectors sharing it.
fn edit_info(level: &mut Level, sector: Sector, f: impl FnOnce(&mut FloorInfo)) -> Option<usize> {
    let mut info = level.floor_info(&sector).clone();
    f(&mut info);
    if info == FloorInfo::default() {
        return None;
    }
    Some(level.floor_infos.iter().position(|i| *i == info).unwrap_or_else(|| {
        level.floor_infos.push(info);
        level.floor_infos.len() - 1
    }))
}

/// A portal's world rectangle clamped to `low..=high` on `axis`, or none
/// when nothing is left. Portals on a plane across `axis` are kept or
/// dropped whole by where they stand.
fn clip_portal(room: &Room, p: &Portal, axis: usize, low: i32, high: i32, to: &Room) -> Option<Portal> {
    let world: Vec<[i32; 3]> =
        p.vertices.iter().map(|v| [v[0] + room.origin[0], v[1], v[2] + room.origin[2]]).collect();
    if p.normal[axis] != 0 {
        let at = world[0][axis] + i32::from(p.normal[axis] > 0);
        return (low <= at && at <= high).then(|| Portal {
            room: p.room,
            normal: p.normal,
            vertices: world.iter().map(|w| [w[0] - to.origin[0], w[1], w[2] - to.origin[2]]).collect(),
        });
    }
    let clamped: Vec<[i32; 3]> = world
        .iter()
        .map(|w| {
            let mut w = *w;
            w[axis] = w[axis].clamp(low, high);
            w
        })
        .collect();
    let lo = clamped.iter().map(|w| w[axis]).min()?;
    let hi = clamped.iter().map(|w| w[axis]).max()?;
    (hi > lo).then(|| Portal {
        room: p.room,
        normal: p.normal,
        vertices: clamped.iter().map(|w| [w[0] - to.origin[0], w[1], w[2] - to.origin[2]]).collect(),
    })
}

/// Point every reference to room `from` at whichever of `parts` `choose`
/// picks for a world position (`[x, y, z]`, y the height that matters).
/// `link` says how the reference arrives: 0 sideways, -1 from above (a
/// floor opening down into the room), +1 from below.
/// `choose` also gets the room the reference is in (`usize::MAX` for objects
/// and cameras).
/// Picks the part a reference to a split room goes to: given the level, a
/// world position, how the reference arrives and the room it is in.
type Chooser<'a> = &'a dyn Fn(&Level, [i32; 3], i32, usize) -> usize;

fn retarget(level: &mut Level, from: usize, parts: [usize; 2], choose: Chooser) {
    // Sector links in other rooms; the parts' own links are already right.
    for r in 0..level.rooms.len() {
        if parts.contains(&r) {
            continue;
        }
        for i in 0..level.rooms[r].sectors.len() {
            let (zs, origin) = (level.rooms[r].z_sectors, level.rooms[r].origin);
            let s = level.rooms[r].sectors[i];
            let (x, z) = (i / zs, i % zs);
            let centre = |y: i32| [origin[0] + x as i32 * 1024 + 512, y, origin[2] + z as i32 * 1024 + 512];
            if level.floor_info(&s).portal == Some(from) {
                let to = choose(level, centre(s.floor), 0, r);
                let info = edit_info(level, s, |f| f.portal = Some(to));
                level.rooms[r].sectors[i].floor_info = info;
            }
            if s.below == Some(from) {
                level.rooms[r].sectors[i].below = Some(choose(level, centre(s.floor), -1, r));
            }
            if s.above == Some(from) {
                level.rooms[r].sectors[i].above = Some(choose(level, centre(s.ceiling), 1, r));
            }
        }
    }
    for p in 0..level.placements.len() {
        if level.placements[p].room == from {
            level.placements[p].room = choose(level, level.placements[p].position, 0, usize::MAX);
        }
    }
    for c in 0..level.cameras.len() {
        if level.cameras[c].room == Some(from) {
            level.cameras[c].room = Some(choose(level, level.cameras[c].position, 0, usize::MAX));
        }
    }
}

/// Other rooms' portals into `from`, split between `parts` by clipping on
/// `axis` at `at` (the low part gets `..=at`, the high part `at..`).
fn split_incoming(level: &mut Level, from: usize, parts: [usize; 2], axis: usize, at: i32) {
    for r in 0..level.rooms.len() {
        if parts.contains(&r) {
            continue;
        }
        let room = level.rooms[r].clone();
        let mut portals = vec![];
        for p in &room.portals {
            if p.room != from {
                portals.push(p.clone());
                continue;
            }
            for (part, (low, high)) in parts.iter().zip([(i32::MIN, at), (at, i32::MAX)]) {
                if let Some(mut clipped) = clip_portal(&room, p, axis, low, high, &room) {
                    clipped.room = *part;
                    portals.push(clipped);
                }
            }
        }
        level.rooms[r].portals = portals;
    }
}

fn no_flips(level: &Level, rooms: &[usize]) -> Result<(), String> {
    if level
        .rooms
        .iter()
        .enumerate()
        .any(|(i, r)| (rooms.contains(&i) && r.alternate.is_some()) || r.alternate.is_some_and(|a| rooms.contains(&a)))
    {
        return Err("rooms that flip with another room cannot be split or merged yet".into());
    }
    Ok(())
}

fn open_sector(level: &Level, s: &Sector) -> bool {
    s.floor > SOLID && level.floor_info(s).portal.is_none()
}

/// Split `room` along the sector line `line` (in its grid) on `axis` (0 = x,
/// 2 = z): the part below the line keeps the room's index, the part above
/// becomes a new room, returned.
pub fn split_along(level: &mut Level, room: usize, axis: usize, line: usize) -> Result<usize, String> {
    if axis != 0 && axis != 2 {
        return Err("split along x or z".into());
    }
    no_flips(level, &[room])?;
    let r = level.rooms.get(room).ok_or("no such room")?.clone();
    let size = if axis == 0 { r.x_sectors } else { r.z_sectors };
    if line < 2 || line + 2 > size {
        return Err("split at least two sectors from the room's edges, so each part keeps open floor".into());
    }
    let other = 2 - axis;
    let other_size = if axis == 0 { r.z_sectors } else { r.x_sectors };
    let at = r.origin[axis] + line as i32 * SECTOR;
    let cell = |a: usize, o: usize| if axis == 0 { (a, o) } else { (o, a) };
    let get = |a: usize, o: usize| {
        let (x, z) = cell(a, o);
        r.sectors[x * r.z_sectors + z]
    };
    // The opening between the two parts: wherever both sides of the line are open.
    let mut window: Option<Window> = None;
    let mut opens = vec![false; other_size];
    for (o, opening) in opens.iter_mut().enumerate() {
        let (near, far) = (get(line - 1, o), get(line, o));
        if !open_sector(level, &near) || !open_sector(level, &far) {
            continue;
        }
        let corners = |a: usize, surface| {
            let (x, z) = cell(a, o);
            sector_corners(level, room, x, z, surface)
        };
        let (Some(nf), Some(nc), Some(ff), Some(fc)) = (
            corners(line - 1, Surface::Floor),
            corners(line - 1, Surface::Ceiling),
            corners(line, Surface::Floor),
            corners(line, Surface::Ceiling),
        ) else {
            continue;
        };
        let (nf, nc, ff, fc) =
            (edge(nf, axis, true), edge(nc, axis, true), edge(ff, axis, false), edge(fc, axis, false));
        let top = [nc[0].max(fc[0]), nc[1].max(fc[1])];
        let bottom = [nf[0].min(ff[0]), nf[1].min(ff[1])];
        if (0..2).all(|e| top[e] >= bottom[e]) {
            continue;
        }
        *opening = true;
        let (mut low, mut high) = ([0; 3], [0; 3]);
        low[other] = r.origin[other] + o as i32 * SECTOR;
        high[other] = low[other] + SECTOR;
        (low[1], high[1]) = (top[0].min(top[1]), bottom[0].max(bottom[1]));
        let w = Window { axis, at, low, high };
        window = Some(window.map_or(w, |old| old.union(w)));
    }
    // Grids: the low part keeps 0..=line, the high part line-1..size.
    let slice = |from: usize, to: usize| {
        let mut sectors = vec![];
        let (w, d) = if axis == 0 { (to - from, r.z_sectors) } else { (r.x_sectors, to - from) };
        for x in 0..w {
            for z in 0..d {
                let (sx, sz) = if axis == 0 { (x + from, z) } else { (x, z + from) };
                sectors.push(r.sectors[sx * r.z_sectors + sz]);
            }
        }
        (sectors, w, d)
    };
    let b = level.rooms.len();
    let mut low = Room { portals: vec![], static_placements: vec![], ..r.clone() };
    let mut high = Room { portals: vec![], static_placements: vec![], ..r.clone() };
    (low.sectors, low.x_sectors, low.z_sectors) = slice(0, line + 1);
    (high.sectors, high.x_sectors, high.z_sectors) = slice(line - 1, size);
    high.origin[axis] += (line as i32 - 1) * SECTOR;
    let [low_shapes, high_shapes] = partition(shapes(&r), axis, at);
    assemble(&mut low, &low_shapes);
    assemble(&mut high, &high_shapes);
    for p in &r.portals {
        low.portals.extend(clip_portal(&r, p, axis, i32::MIN, at, &low));
        high.portals.extend(clip_portal(&r, p, axis, at, i32::MAX, &high));
    }
    for s in &r.static_placements {
        if s.position[axis] < at { &mut low } else { &mut high }.static_placements.push(s.clone());
    }
    level.rooms[room] = low;
    level.rooms.push(high);
    own_doorways(level, room, b);
    // Each part's border sector over the other's open floor becomes a doorway.
    for (o, &opening) in opens.iter().enumerate() {
        for (part, a, target, source) in [(room, line, b, line), (b, 0, room, line - 1)] {
            let s = get(source, o);
            let (x, z) = cell(a, o);
            if !opening {
                // No opening here: a corner of the new border, or open floor
                // whose space does not meet the other side's (a wall stands
                // between them). Doorways a corner held are drawn from the
                // other part.
                let p = &mut level.rooms[part];
                let zs = p.z_sectors;
                p.sectors[x * zs + z] = Sector { floor: SOLID, ceiling: SOLID, ..Default::default() };
                continue;
            }
            let slopes = {
                let info = level.floor_info(&s);
                (info.floor_slope, info.ceiling_slope)
            };
            let info = edit_info(level, Sector::default(), |f| {
                f.portal = Some(target);
                (f.floor_slope, f.ceiling_slope) = slopes;
            });
            let p = &mut level.rooms[part];
            let zs = p.z_sectors;
            p.sectors[x * zs + z] =
                Sector { floor: s.floor, ceiling: s.ceiling, floor_info: info, ..Default::default() };
        }
    }
    split_incoming(level, room, [room, b], axis, at);
    let choose = move |_: &Level, p: [i32; 3], _: i32, _: usize| if p[axis] < at { room } else { b };
    retarget(level, room, [room, b], &choose);
    if let Some(window) = window {
        let mut normal = [0; 3];
        normal[axis] = -1;
        open_window(level, room, b, normal, window);
    }
    Ok(b)
}

/// Some imported rooms have doorway sectors naming themselves; in the new
/// part they name the new part.
fn own_doorways(level: &mut Level, room: usize, part: usize) {
    for i in 0..level.rooms[part].sectors.len() {
        let s = level.rooms[part].sectors[i];
        if level.floor_info(&s).portal == Some(room) {
            let info = edit_info(level, s, |f| f.portal = Some(part));
            level.rooms[part].sectors[i].floor_info = info;
        }
    }
}

/// Where a sector stands relative to a cut height.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Layer {
    Solid,
    Above,
    Below,
    Spans,
}

/// Split `room` at height `y` (a whole 256-unit step strictly inside it):
/// the lower part keeps the room's index; the upper part, returned, is new.
pub fn split_height(level: &mut Level, room: usize, y: i32) -> Result<usize, String> {
    no_flips(level, &[room])?;
    let r = level.rooms.get(room).ok_or("no such room")?.clone();
    if y % STEP != 0 || y <= r.top || y >= r.bottom {
        return Err(format!(
            "split at a whole 256-unit step between the room's top ({}) and bottom ({})",
            r.top, r.bottom
        ));
    }
    let mut layers = vec![Layer::Solid; r.sectors.len()];
    for (i, s) in r.sectors.iter().enumerate() {
        if s.floor <= SOLID {
            continue;
        }
        let (x, z) = (i / r.z_sectors, i % r.z_sectors);
        if level.floor_info(s).portal.is_some() {
            // Doorways go wherever their opening reaches.
            layers[i] = match (s.ceiling < y, s.floor > y) {
                (true, true) => Layer::Spans,
                (true, false) => Layer::Above,
                _ => Layer::Below,
            };
            continue;
        }
        let floor = sector_corners(level, room, x, z, Surface::Floor);
        let ceiling = sector_corners(level, room, x, z, Surface::Ceiling);
        let (Some(f), Some(c)) = (floor, ceiling) else { continue };
        layers[i] = if f.iter().all(|&h| h <= y) {
            Layer::Above
        } else if c.iter().all(|&h| h >= y) {
            Layer::Below
        } else if c.iter().all(|&h| h < y) && f.iter().all(|&h| h > y) {
            Layer::Spans
        } else {
            return Err(format!("room {room} sector ({x}, {z}): its sloped floor or ceiling crosses the split height"));
        };
    }
    if !layers.contains(&Layer::Spans) {
        return Err("no open sector reaches across this height, so nothing would join the two parts".into());
    }
    let upper = level.rooms.len();
    let solid = Sector { floor: SOLID, ceiling: SOLID, ..Default::default() };
    let mut low = Room { portals: vec![], static_placements: vec![], ..r.clone() };
    let mut high = Room { portals: vec![], static_placements: vec![], ..r.clone() };
    let mut window: Option<Window> = None;
    for (i, s) in r.sectors.iter().enumerate() {
        let doorway = level.floor_info(s).portal.is_some();
        let (l, h) = match layers[i] {
            Layer::Solid => (*s, *s),
            Layer::Above => (solid, *s),
            Layer::Below => (*s, solid),
            Layer::Spans if doorway => (*s, *s),
            Layer::Spans => {
                let lower_info = edit_info(level, *s, |f| f.ceiling_slope = [0, 0]);
                let upper_info = edit_info(level, *s, |f| {
                    let ceiling = f.ceiling_slope;
                    *f = FloorInfo { ceiling_slope: ceiling, ..Default::default() };
                });
                let (x, z) = (i / r.z_sectors, i % r.z_sectors);
                let (x0, z0) = (r.origin[0] + x as i32 * SECTOR, r.origin[2] + z as i32 * SECTOR);
                let w = Window { axis: 1, at: y, low: [x0, y, z0], high: [x0 + SECTOR, y, z0 + SECTOR] };
                window = Some(window.map_or(w, |old| old.union(w)));
                (
                    Sector { ceiling: y, above: Some(upper), floor_info: lower_info, ..*s },
                    Sector { floor: y, below: Some(room), floor_info: upper_info, ..*s },
                )
            }
        };
        low.sectors[i] = l;
        high.sectors[i] = h;
    }
    let [high_shapes, low_shapes] = partition(shapes(&r), 1, y);
    assemble(&mut low, &low_shapes);
    assemble(&mut high, &high_shapes);
    for p in &r.portals {
        high.portals.extend(clip_portal(&r, p, 1, i32::MIN, y, &high));
        low.portals.extend(clip_portal(&r, p, 1, y, i32::MAX, &low));
    }
    for s in &r.static_placements {
        if s.position[1] > y { &mut low } else { &mut high }.static_placements.push(s.clone());
    }
    // A doorway reaching across the height stays in the part its portal is in.
    for (i, s) in r.sectors.iter().enumerate() {
        let Some(to) = level.floor_info(s).portal.filter(|_| layers[i] == Layer::Spans) else { continue };
        let drawn = |part: &Room| part.portals.iter().any(|p| p.room == to && p.normal[1] == 0);
        match (drawn(&low), drawn(&high)) {
            (true, false) => high.sectors[i] = solid,
            (false, true) => low.sectors[i] = solid,
            _ => (),
        }
    }
    level.rooms[room] = low;
    level.rooms.push(high);
    own_doorways(level, room, upper);
    split_incoming(level, room, [upper, room], 1, y);
    let layer_at = move |p: [i32; 3]| -> Layer {
        let (x, z) = ((p[0] - r.origin[0]).div_euclid(SECTOR), (p[2] - r.origin[2]).div_euclid(SECTOR));
        if x < 0 || z < 0 || x as usize >= r.x_sectors || z as usize >= r.z_sectors {
            return Layer::Solid;
        }
        layers[x as usize * r.z_sectors + z as usize]
    };
    let choose = move |level: &Level, p: [i32; 3], link: i32, source: usize| match (link, layer_at(p)) {
        // Only one part is open here.
        (_, Layer::Above) => upper,
        (_, Layer::Below) => room,
        // A doorway goes to the part its room's portal reaches, if only one.
        (0, _) if source < level.rooms.len() => {
            let drawn = |part: usize| level.rooms[source].portals.iter().any(|q| q.room == part && q.normal[1] == 0);
            match (drawn(room), drawn(upper)) {
                (true, false) => room,
                (false, true) => upper,
                _ if p[1] > y => room,
                _ => upper,
            }
        }
        // Where both are open: a floor opening from above reaches the upper
        // part, a ceiling opening from below the lower one.
        (-1, _) => upper,
        (1, _) => room,
        // At or under the cut is the lower part, as collision finds it.
        _ if p[1] >= y => room,
        _ => upper,
    };
    retarget(level, room, [room, upper], &choose);
    if let Some(window) = window {
        open_window(level, upper, room, [0, -1, 0], window);
    }
    Ok(upper)
}

/// Point everything at room `from` to `to`, then remove `from`, renumbering.
fn remove_room(level: &mut Level, from: usize, to: usize) {
    let map = |r: usize| if r == from { to } else { r };
    let shift = |r: usize| if r > from { r - 1 } else { r };
    for info in &mut level.floor_infos {
        info.portal = info.portal.map(|r| shift(map(r)));
    }
    for room in &mut level.rooms {
        for p in &mut room.portals {
            p.room = shift(map(p.room));
        }
        for s in &mut room.sectors {
            s.below = s.below.map(|r| shift(map(r)));
            s.above = s.above.map(|r| shift(map(r)));
        }
        room.alternate = room.alternate.map(|r| shift(map(r)));
    }
    for p in &mut level.placements {
        p.room = shift(map(p.room));
    }
    for c in &mut level.cameras {
        c.room = c.room.map(|r| shift(map(r)));
    }
    level.rooms.remove(from);
}

/// Merge room `b` into room `a`. They must be joined only by doorways
/// along one line (or sit side by side unjoined, with no floor open in
/// both), or only by holes over the same grid, and be alike in water, wind
/// and flags. Returns `a`'s index after `b` is removed.
pub fn merge_rooms(level: &mut Level, a: usize, b: usize) -> Result<usize, String> {
    if a == b || a >= level.rooms.len() || b >= level.rooms.len() {
        return Err("choose two different rooms".into());
    }
    no_flips(level, &[a, b])?;
    let (ra, rb) = (level.rooms[a].clone(), level.rooms[b].clone());
    if (ra.water, ra.wind, ra.other_flags) != (rb.water, rb.wind, rb.other_flags) {
        return Err("only rooms with the same water, wind and flags can merge".into());
    }
    let side = |r: &Room, to: usize| r.sectors.iter().any(|s| level.floor_info(s).portal == Some(to));
    let vertical = |r: &Room, to: usize| r.sectors.iter().any(|s| s.below == Some(to) || s.above == Some(to));
    let (sideways, stacked) = (side(&ra, b) || side(&rb, a), vertical(&ra, b) || vertical(&rb, a));
    let before = wall_mismatches(level, a).len() + wall_mismatches(level, b).len();
    let mut merged = match (sideways, stacked) {
        (true, false) => merge_sideways(level, a, b)?,
        (false, true) => merge_stacked(level, a, b)?,
        // Rooms side by side but not joined (split along a wall) merge if their grids fit.
        (false, false) => merge_sideways(level, a, b)?,
        (true, true) => return Err("rooms joined both sideways and through holes cannot merge yet".into()),
    };
    let mut shapes_all = shapes(&ra);
    shapes_all.extend(shapes(&rb));
    assemble(&mut merged, &shapes_all);
    merged.portals = vec![];
    for (r, other) in [(&ra, b), (&rb, a)] {
        for p in r.portals.iter().filter(|p| p.room != other) {
            merged.portals.extend(clip_portal(r, p, 0, i32::MIN, i32::MAX, &merged));
        }
    }
    merged.static_placements = [ra.static_placements.clone(), rb.static_placements.clone()].concat();
    merged.lights = ra.lights.clone();
    merged.lights.extend(rb.lights.iter().filter(|l| !ra.lights.contains(l)).copied());
    level.rooms[a] = merged;
    remove_room(level, b, a);
    let a = if a > b { a - 1 } else { a };
    level.rooms[a].portals.retain(|p| p.room != a);
    if wall_mismatches(level, a).len() > before {
        return Err(
            "the rooms are not open to each other along the whole of their shared wall; connect it all first".into()
        );
    }
    Ok(a)
}

/// The union of two rooms' grids, taking each sector's open floor over the
/// other's border or doorway.
fn merge_sideways(level: &Level, a: usize, b: usize) -> Result<Room, String> {
    let (ra, rb) = (&level.rooms[a], &level.rooms[b]);
    let cells = |r: &Room| {
        let (x, z) = (r.origin[0].div_euclid(SECTOR), r.origin[2].div_euclid(SECTOR));
        (x, z, x + r.x_sectors as i32, z + r.z_sectors as i32)
    };
    let (ax0, az0, ax1, az1) = cells(ra);
    let (bx0, bz0, bx1, bz1) = cells(rb);
    let (x0, z0, x1, z1) = (ax0.min(bx0), az0.min(bz0), ax1.max(bx1), az1.max(bz1));
    let mut merged = Room {
        origin: [x0 * SECTOR, 0, z0 * SECTOR],
        x_sectors: (x1 - x0) as usize,
        z_sectors: (z1 - z0) as usize,
        sectors: vec![],
        ..ra.clone()
    };
    let at = |r: &Room, gx: i32, gz: i32| {
        let (x, z) = (gx - r.origin[0].div_euclid(SECTOR), gz - r.origin[2].div_euclid(SECTOR));
        (x >= 0 && z >= 0 && (x as usize) < r.x_sectors && (z as usize) < r.z_sectors)
            .then(|| r.sectors[x as usize * r.z_sectors + z as usize])
    };
    let solid = Sector { floor: SOLID, ceiling: SOLID, ..Default::default() };
    let mut shared = 0;
    for gx in x0..x1 {
        for gz in z0..z1 {
            let (sa, sb) = (at(ra, gx, gz), at(rb, gx, gz));
            shared += usize::from(sa.is_some() && sb.is_some());
            let class = |s: Option<Sector>, other: usize| match s {
                None => 0,
                Some(s) if s.floor <= SOLID => 0,
                Some(s) if level.floor_info(&s).portal == Some(other) => 1,
                Some(s) if level.floor_info(&s).portal.is_some() => 2,
                Some(_) => 3,
            };
            // The sector to keep, and whether it comes from `a`.
            let (pick, from_a) = match (class(sa, b), class(sb, a)) {
                (3, 3) => {
                    return Err(format!("both rooms are open at world sector ({gx}, {gz}); they overlap"));
                }
                (3, _) => (sa, true),
                (_, 3) => (sb, false),
                (2, _) => (sa, true),
                (_, 2) => (sb, false),
                _ => (None, true),
            };
            let s = pick.unwrap_or(solid);
            // A border doorway of either room must stay on the merged border.
            let on_border = |r: &Room| {
                let (x, z) = (gx - r.origin[0].div_euclid(SECTOR), gz - r.origin[2].div_euclid(SECTOR));
                x == 0 || z == 0 || x + 1 == r.x_sectors as i32 || z + 1 == r.z_sectors as i32
            };
            let from_border = if from_a { on_border(ra) } else { on_border(rb) };
            let inside = gx > x0 && gz > z0 && gx + 1 < x1 && gz + 1 < z1;
            if inside && from_border && level.floor_info(&s).portal.is_some() {
                return Err(format!(
                    "a doorway would end up inside the merged room at world sector ({gx}, {gz}); merge rooms that sit side by side"
                ));
            }
            merged.sectors.push(s);
        }
    }
    if shared < 2 {
        return Err("the rooms' grids do not overlap; merge rooms that sit side by side".into());
    }
    Ok(merged)
}

/// One grid from two stacked rooms over the same grid: each hole becomes
/// open space from the lower floor to the upper ceiling.
fn merge_stacked(level: &mut Level, a: usize, b: usize) -> Result<Room, String> {
    let (ra, rb) = (level.rooms[a].clone(), level.rooms[b].clone());
    if (ra.origin, ra.x_sectors, ra.z_sectors) != (rb.origin, rb.x_sectors, rb.z_sectors) {
        return Err("rooms joined through holes merge only when their grids are the same".into());
    }
    let (upper, lower, ui, li) =
        if ra.sectors.iter().any(|s| s.below == Some(b)) { (&ra, &rb, a, b) } else { (&rb, &ra, b, a) };
    let mut merged = Room { sectors: vec![], ..ra.clone() };
    for i in 0..ra.sectors.len() {
        let (u, l) = (upper.sectors[i], lower.sectors[i]);
        let s = if u.below == Some(li) {
            if l.above != Some(ui) || l.ceiling != u.floor {
                return Err("a hole between the rooms does not meet on both sides".into());
            }
            let (floor, ceiling) = (level.floor_info(&l).clone(), level.floor_info(&u).ceiling_slope);
            let info = edit_info(level, Sector::default(), |f| {
                *f = FloorInfo { ceiling_slope: ceiling, portal: None, ..floor };
            });
            Sector {
                floor: l.floor,
                ceiling: u.ceiling,
                below: l.below,
                above: u.above,
                nav_box: l.nav_box,
                floor_info: info,
            }
        } else {
            match (u.floor > SOLID, l.floor > SOLID) {
                (true, true)
                    if level.floor_info(&u).portal.is_some()
                        && level.floor_info(&u).portal == level.floor_info(&l).portal =>
                {
                    u
                }
                (true, true) => {
                    return Err(format!(
                        "both rooms are open at sector {} {} without a hole between them",
                        i / ra.z_sectors,
                        i % ra.z_sectors
                    ));
                }
                (true, false) => u,
                (false, true) => l,
                (false, false) => u,
            }
        };
        merged.sectors.push(s);
    }
    Ok(merged)
}
