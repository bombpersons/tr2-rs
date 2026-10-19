//! Grid-aware room geometry edits that keep collision equal to what is drawn.
//!
//! A room is drawn from its faces but collides through its sector grid. Each
//! sector's floor and ceiling are one quad covering exactly that sector, and
//! [`plane`] / [`corners`] convert between such a quad's corner heights and the
//! sector's height and floor-data slope, using the same rules as
//! [`crate::world::WorldGeometry::floor_at`]. Every edit here moves geometry on
//! the grid (x and z in whole sectors, heights in 256-unit steps) and then
//! re-derives the sectors it touched, so the two layers never disagree.
//!
//! Faces are wound as the imported levels are: seen from the side they are
//! drawn for, their corners run clockwise.
use crate::world::{FloorInfo, Level, Room, format::SOLID};
use std::collections::{BTreeMap, BTreeSet};

/// World units per sector.
pub const SECTOR: i32 = 1024;
/// Height step: floor data slopes rise in whole steps across a sector.
pub const STEP: i32 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Surface {
    Floor,
    Ceiling,
}

/// A room face: an index into `quads` or `triangles`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Face {
    Quad(usize),
    Triangle(usize),
}

/// Heights at a sector's corners, in the order `[x0 z0, x1 z0, x0 z1, x1 z1]`.
pub type Corners = [i32; 4];

/// A floor or ceiling quad: the sector it covers, its side and its corner heights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectorFace {
    pub x: usize,
    pub z: usize,
    pub surface: Surface,
    pub corners: Corners,
}

fn point(room: &Room, vertex: usize) -> [i32; 3] {
    let v = room.vertices[vertex];
    [v[0], v[1], v[2]]
}

/// Twice the area of a polygon projected along `axis`, signed by its winding
/// (Newell's method). Positive about y means a floor (drawn for viewers above).
fn normal(points: &[[i32; 3]], axis: usize) -> i64 {
    let (a, b) = [(1, 2), (2, 0), (0, 1)][axis];
    (0..points.len())
        .map(|i| {
            let (p, q) = (points[i], points[(i + 1) % points.len()]);
            i64::from(p[a] - q[a]) * i64::from(p[b] + q[b])
        })
        .sum()
}

/// The sector a quad lies across, when it covers exactly one and faces up or down.
pub fn sector_face(room: &Room, quad: &[usize; 5]) -> Option<SectorFace> {
    let points: Vec<_> = quad[..4].iter().map(|&v| point(room, v)).collect();
    let x0 = points.iter().map(|p| p[0]).min()?;
    let z0 = points.iter().map(|p| p[2]).min()?;
    if x0.rem_euclid(SECTOR) != 0 || z0.rem_euclid(SECTOR) != 0 || x0 < 0 || z0 < 0 {
        return None;
    }
    let mut corners = [0; 4];
    let mut seen = 0u8;
    for p in &points {
        let (dx, dz) = (p[0] - x0, p[2] - z0);
        if !matches!(dx, 0 | SECTOR) || !matches!(dz, 0 | SECTOR) {
            return None;
        }
        let corner = usize::from(dx != 0) + 2 * usize::from(dz != 0);
        seen |= 1 << corner;
        corners[corner] = p[1];
    }
    let (x, z) = ((x0 / SECTOR) as usize, (z0 / SECTOR) as usize);
    let surface = match normal(&points, 1) {
        n if n > 0 => Surface::Floor,
        n if n < 0 => Surface::Ceiling,
        _ => return None,
    };
    (seen == 0b1111 && x < room.x_sectors && z < room.z_sectors).then_some(SectorFace { x, z, surface, corners })
}

/// A sector's height and `[x, z]` slope from its face's corner heights.
pub fn plane(surface: Surface, c: Corners) -> Result<(i32, [i8; 2]), String> {
    if c[0] - c[1] != c[2] - c[3] {
        return Err("a floor or ceiling must be flat or slope evenly (move a whole edge, not one corner)".into());
    }
    let (dx, dz) = (c[0] - c[1], c[0] - c[2]);
    if dx % STEP != 0 || dz % STEP != 0 {
        return Err("slopes rise in whole 256-unit steps across a sector".into());
    }
    let tilt = |d: i32| i8::try_from(d / STEP).map_err(|_| "slope too steep".to_string());
    Ok(match surface {
        Surface::Floor => (*c.iter().min().unwrap(), [tilt(dx)?, tilt(dz)?]),
        Surface::Ceiling => (*c.iter().max().unwrap(), [tilt(dx)?, tilt(-dz)?]),
    })
}

/// Corner heights of a sector's floor or ceiling; the inverse of [`plane`].
pub fn corners(surface: Surface, height: i32, slope: [i8; 2]) -> Corners {
    let [tx, tz] = slope.map(|t| i32::from(t) * STEP);
    let mut c = [height; 4];
    for (i, corner) in c.iter_mut().enumerate() {
        let (fx, fz) = ((i & 1) as i32, (i >> 1) as i32);
        *corner += match surface {
            // Rises toward the far side for positive tilts; `height` is the top.
            Surface::Floor => {
                (if tx > 0 { tx * (1 - fx) } else { -tx * fx }) + (if tz > 0 { tz * (1 - fz) } else { -tz * fz })
            }
            // `height` is the lowest point.
            Surface::Ceiling => {
                -(if tx > 0 { tx * fx } else { -tx * (1 - fx) }) - (if tz > 0 { tz * (1 - fz) } else { -tz * fz })
            }
        };
    }
    c
}

/// A sector's floor or ceiling corners now, or none for a solid sector.
pub fn sector_corners(level: &Level, room: usize, x: usize, z: usize, surface: Surface) -> Option<Corners> {
    let r = &level.rooms[room];
    let s = r.sectors.get(x * r.z_sectors + z)?;
    let info = level.floor_info(s);
    match surface {
        Surface::Floor if s.floor > SOLID => Some(corners(surface, s.floor, info.floor_slope)),
        Surface::Ceiling if s.ceiling > SOLID => Some(corners(surface, s.ceiling, info.ceiling_slope)),
        _ => None,
    }
}

/// Every floor and ceiling quad of a room, by sector and side.
fn sector_faces(room: &Room) -> BTreeMap<(usize, usize, Surface), Vec<(usize, Corners)>> {
    let mut faces = BTreeMap::<_, Vec<_>>::new();
    for (i, quad) in room.quads.iter().enumerate() {
        if let Some(f) = sector_face(room, quad) {
            faces.entry((f.x, f.z, f.surface)).or_default().push((i, f.corners));
        }
    }
    faces
}

/// Set a sector's slope without changing other sectors that share its floor data.
fn set_slope(level: &mut Level, room: usize, index: usize, surface: Surface, slope: [i8; 2]) {
    let sector = level.rooms[room].sectors[index];
    let mut info = level.floor_info(&sector).clone();
    let field = match surface {
        Surface::Floor => &mut info.floor_slope,
        Surface::Ceiling => &mut info.ceiling_slope,
    };
    if *field == slope {
        return;
    }
    *field = slope;
    let found = if info == FloorInfo::default() {
        None
    } else {
        Some(level.floor_infos.iter().position(|f| *f == info).unwrap_or_else(|| {
            level.floor_infos.push(info);
            level.floor_infos.len() - 1
        }))
    };
    level.rooms[room].sectors[index].floor_info = found;
}

/// Re-derive sectors from their floor and ceiling faces. `had` lists the
/// faces each sector had before the edit: a sector that loses both becomes
/// solid.
fn derive(
    level: &mut Level,
    room: usize,
    sectors: &BTreeSet<(usize, usize)>,
    had: &BTreeSet<(usize, usize, Surface)>,
) -> Result<(), String> {
    let faces = sector_faces(&level.rooms[room]);
    for &(x, z) in sectors {
        let (width, depth) = (level.rooms[room].x_sectors, level.rooms[room].z_sectors);
        let index = x * depth + z;
        let name = format!("room {room} sector ({x}, {z})");
        let sector = level.rooms[room].sectors[index];
        let portal = level.floor_info(&sector).portal.is_some();
        let mut open = [false; 2];
        for (side, surface) in [Surface::Floor, Surface::Ceiling].into_iter().enumerate() {
            match faces.get(&(x, z, surface)).map(Vec::as_slice) {
                Some([(_, c)]) => {
                    let (height, slope) = plane(surface, *c).map_err(|e| format!("{name}: {e}"))?;
                    let s = &mut level.rooms[room].sectors[index];
                    *(if surface == Surface::Floor { &mut s.floor } else { &mut s.ceiling }) = height;
                    set_slope(level, room, index, surface, slope);
                    open[side] = true;
                }
                Some(_) => return Err(format!("{name}: two {surface:?} faces cover one sector").to_lowercase()),
                None => {
                    let through = if surface == Surface::Floor { sector.below } else { sector.above };
                    let was = had.contains(&(x, z, surface));
                    open[side] = through.is_some() || (!was && sector_corners(level, room, x, z, surface).is_some());
                }
            }
        }
        match open {
            [false, false] if !portal => {
                let s = &mut level.rooms[room].sectors[index];
                (s.floor, s.ceiling) = (SOLID, SOLID);
                set_slope(level, room, index, Surface::Floor, [0, 0]);
                set_slope(level, room, index, Surface::Ceiling, [0, 0]);
                continue;
            }
            [true, true] => (),
            [floor, _] => {
                let missing = if floor { "ceiling" } else { "floor" };
                return Err(format!("{name}: an open sector needs a {missing} face (or a room beyond it)"));
            }
        }
        if (x == 0 || z == 0 || x + 1 == width || z + 1 == depth) && !portal {
            return Err(format!("{name}: border sectors stay walls; push a wall out to grow the room"));
        }
        if let (Some(floor), Some(ceiling)) =
            (sector_corners(level, room, x, z, Surface::Floor), sector_corners(level, room, x, z, Surface::Ceiling))
            && floor.iter().zip(ceiling).any(|(f, c)| *f < c)
        {
            return Err(format!("{name}: the floor would rise above the ceiling"));
        }
    }
    Ok(())
}

/// A vertical face along a sector edge: the axis it faces along (0 = x,
/// 2 = z), the edge's coordinate on that axis, the sector it spans along the
/// edge, and which way it is seen from (+1 or -1 along the axis).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WallFace {
    axis: usize,
    at: i32,
    along: i32,
    toward: i32,
}

fn wall_face(room: &Room, corners: &[usize]) -> Option<WallFace> {
    let points: Vec<_> = corners.iter().map(|&v| point(room, v)).collect();
    for axis in [0, 2] {
        let other = 2 - axis;
        let at = points[0][axis];
        if at.rem_euclid(SECTOR) != 0 || points.iter().any(|p| p[axis] != at) {
            continue;
        }
        let low = points.iter().map(|p| p[other]).min()?;
        let high = points.iter().map(|p| p[other]).max()?;
        if low.rem_euclid(SECTOR) != 0 || high - low != SECTOR {
            return None;
        }
        // Seen from the side its winding's normal points away from.
        let toward = match normal(&points, axis) {
            n if n > 0 => -1,
            n if n < 0 => 1,
            _ => return None,
        };
        return Some(WallFace { axis, at, along: low / SECTOR, toward });
    }
    None
}

/// Heights at the two ends of a sector's edge on `axis` side `at` (in sectors).
fn edge(c: Corners, axis: usize, far: bool) -> [i32; 2] {
    match (axis, far) {
        (0, false) => [c[0], c[2]],
        (0, true) => [c[1], c[3]],
        (_, false) => [c[0], c[1]],
        (_, true) => [c[2], c[3]],
    }
}

/// Faces between two open sectors may only fill the height difference
/// between them; anything else would be a wall the avatar walks through.
fn check_walls(level: &Level, room: usize, faces: impl IntoIterator<Item = Face>) -> Result<(), String> {
    match wall_problems(level, room, faces).into_iter().next() {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

/// Every wall face of a room that stands in the open between two sectors.
pub fn wall_mismatches(level: &Level, room: usize) -> Vec<String> {
    let r = &level.rooms[room];
    let faces = (0..r.quads.len()).map(Face::Quad).chain((0..r.triangles.len()).map(Face::Triangle));
    wall_problems(level, room, faces)
}

/// Vertical faces on the edges of `sectors`.
fn faces_on_edges(room: &Room, sectors: &BTreeSet<(usize, usize)>) -> Vec<Face> {
    let quads = (0..room.quads.len()).map(|i| (Face::Quad(i), &room.quads[i][..4]));
    let triangles = (0..room.triangles.len()).map(|i| (Face::Triangle(i), &room.triangles[i][..3]));
    quads
        .chain(triangles)
        .filter(|(_, corners)| {
            wall_face(room, corners).is_some_and(|w| {
                let line = w.at / SECTOR;
                [line - 1, line].into_iter().any(|a| {
                    let cell = if w.axis == 0 { (a, w.along) } else { (w.along, a) };
                    cell.0 >= 0 && cell.1 >= 0 && sectors.contains(&(cell.0 as usize, cell.1 as usize))
                })
            })
        })
        .map(|(face, _)| face)
        .collect()
}

fn wall_problems(level: &Level, room: usize, faces: impl IntoIterator<Item = Face>) -> Vec<String> {
    let mut problems = vec![];
    let r = &level.rooms[room];
    for face in faces {
        let corners: &[usize] = match face {
            Face::Quad(i) => &r.quads[i][..4],
            Face::Triangle(i) => &r.triangles[i][..3],
        };
        let Some(w) = wall_face(r, corners) else { continue };
        let line = w.at / SECTOR;
        let along = w.along as usize;
        let sector = |a: i32| -> Option<(usize, usize)> {
            let (x, z) = if w.axis == 0 { (a, w.along) } else { (w.along, a) };
            (a >= 0 && (x as usize) < r.x_sectors && (z as usize) < r.z_sectors).then_some((x as usize, z as usize))
        };
        let (Some(near), Some(far)) = (sector(line - 1), sector(line)) else { continue };
        // Through a doorway the opening belongs to the room beyond.
        let open = |(x, z): (usize, usize), far: bool| {
            if level.floor_info(&r.sectors[x * r.z_sectors + z]).portal.is_some() {
                return None;
            }
            Some((
                edge(sector_corners(level, room, x, z, Surface::Floor)?, w.axis, far),
                edge(sector_corners(level, room, x, z, Surface::Ceiling)?, w.axis, far),
            ))
        };
        let (Some((floor_a, ceiling_a)), Some((floor_b, ceiling_b))) = (open(near, true), open(far, false)) else {
            continue;
        };
        let other = 2 - w.axis;
        for end in 0..2 {
            let at = (along as i32 + end) * SECTOR;
            let ys: Vec<_> = corners.iter().map(|&v| point(r, v)).filter(|p| p[other] == at).map(|p| p[1]).collect();
            let (Some(&top), Some(&bottom)) = (ys.iter().min(), ys.iter().max()) else { continue };
            let gap_top = ceiling_a[end as usize].max(ceiling_b[end as usize]);
            let gap_bottom = floor_a[end as usize].min(floor_b[end as usize]);
            if top < gap_bottom && bottom > gap_top {
                problems.push(format!(
                    "room {room}: a wall face would stand in the open between sectors {near:?} and {far:?}; extrude floors or push walls instead"
                ));
                break;
            }
        }
    }
    problems
}

fn faces_using(room: &Room, vertices: &BTreeSet<usize>) -> Vec<Face> {
    let quads = room.quads.iter().enumerate().filter(|(_, q)| q[..4].iter().any(|v| vertices.contains(v)));
    let triangles = room.triangles.iter().enumerate().filter(|(_, t)| t[..3].iter().any(|v| vertices.contains(v)));
    quads.map(|(i, _)| Face::Quad(i)).chain(triangles.map(|(i, _)| Face::Triangle(i))).collect()
}

/// Grow `top` and `bottom` to cover the room's vertices.
fn fit_height(room: &mut Room) {
    for v in &room.vertices {
        room.top = room.top.min(v[1]);
        room.bottom = room.bottom.max(v[1]);
    }
}

fn snapped(delta: [i32; 3]) -> Result<(), String> {
    if delta[0] % SECTOR != 0 || delta[2] % SECTOR != 0 || delta[1] % STEP != 0 {
        return Err("geometry moves in whole sectors across and 256-unit steps up and down".into());
    }
    Ok(())
}

/// The vertices of some faces.
pub fn face_vertices(room: &Room, faces: &[Face]) -> BTreeSet<usize> {
    let mut set = BTreeSet::new();
    for face in faces {
        match *face {
            Face::Quad(i) => set.extend(room.quads.get(i).map_or(&[][..], |q| &q[..4])),
            Face::Triangle(i) => set.extend(room.triangles.get(i).map_or(&[][..], |t| &t[..3])),
        }
    }
    set
}

/// Move vertices (and so every face using them) by `delta`, then re-derive
/// the sectors whose floors or ceilings moved. Floor and ceiling faces must
/// keep covering one whole sector, and stay evenly sloped.
pub fn move_vertices(
    level: &mut Level,
    room: usize,
    vertices: &BTreeSet<usize>,
    delta: [i32; 3],
) -> Result<(), String> {
    snapped(delta)?;
    let r = level.rooms.get(room).ok_or("no such room")?;
    if vertices.iter().any(|&v| v >= r.vertices.len()) {
        return Err("no such vertex".into());
    }
    let touched = faces_using(r, vertices);
    let had: BTreeSet<_> = sector_faces(r).keys().copied().collect();
    let before: Vec<_> = touched
        .iter()
        .filter_map(|f| if let Face::Quad(i) = f { sector_face(r, &r.quads[*i]).map(|s| (*i, s)) } else { None })
        .collect();
    let (width, depth) = (r.x_sectors as i32 * SECTOR, r.z_sectors as i32 * SECTOR);
    let r = &mut level.rooms[room];
    for &v in vertices {
        let p = &mut r.vertices[v];
        for axis in 0..3 {
            p[axis] += delta[axis];
        }
        if !(0..=width).contains(&p[0]) || !(0..=depth).contains(&p[2]) {
            return Err("geometry must stay inside the room's sector grid; push a wall out to grow it".into());
        }
    }
    fit_height(r);
    let mut sectors: BTreeSet<_> = before.iter().map(|(_, s)| (s.x, s.z)).collect();
    for &(i, _) in &before {
        let after = sector_face(r, &r.quads[i]).ok_or("a floor or ceiling face must keep covering one whole sector")?;
        sectors.insert((after.x, after.z));
    }
    derive(level, room, &sectors, &had)?;
    let edges = faces_on_edges(&level.rooms[room], &sectors);
    check_walls(level, room, touched.into_iter().chain(edges))
}

/// Whether vertical faces already cover the edge `ends` between heights
/// `span` (`[top, bottom]` at the first end, then at the second).
fn covered(room: &Room, ends: [[i32; 3]; 2], span: [i32; 4]) -> bool {
    let axis = if ends[0][0] == ends[1][0] { 0 } else { 2 };
    let other = 2 - axis;
    (0..2).all(|end| {
        let at = ends[end][other];
        let mut ranges: Vec<_> = room
            .quads
            .iter()
            .filter_map(|q| {
                let points: Vec<_> = q[..4].iter().map(|&v| point(room, v)).collect();
                let low = points.iter().map(|p| p[other]).min()?;
                let high = points.iter().map(|p| p[other]).max()?;
                let on_edge = points.iter().all(|p| p[axis] == ends[0][axis])
                    && low == ends[0][other].min(ends[1][other])
                    && high == ends[0][other].max(ends[1][other]);
                let ys: Vec<_> = points.iter().filter(|p| p[other] == at).map(|p| p[1]).collect();
                if !on_edge {
                    return None;
                }
                Some((*ys.iter().min()?, *ys.iter().max()?))
            })
            .collect();
        ranges.sort();
        let (mut reach, bottom) = (span[end * 2], span[end * 2 + 1]);
        for (top, low) in ranges {
            if top <= reach {
                reach = reach.max(low);
            }
        }
        reach >= bottom
    })
}

/// The face with exactly these corner positions, in any order.
fn same_place(room: &Room, corners: &[[i32; 3]]) -> Option<usize> {
    let mut want = corners.to_vec();
    want.sort();
    room.quads.iter().position(|q| {
        let mut have: Vec<_> = q[..4].iter().map(|&v| point(room, v)).collect();
        have.sort();
        have == want
    })
}

/// Wind a vertical or horizontal quad to be seen from direction `toward`.
fn facing(room: &Room, mut corners: [usize; 4], toward: [i32; 3]) -> [usize; 4] {
    let points: Vec<_> = corners.iter().map(|&v| point(room, v)).collect();
    let n: i64 = (0..3).map(|axis| normal(&points, axis) * i64::from(toward[axis])).sum();
    if n > 0 {
        corners.reverse();
    }
    corners
}

/// The vertex at `p`, added when there is none.
fn vertex_at(room: &mut Room, p: [i32; 3], shade: i32, added: &mut BTreeMap<[i32; 3], usize>) -> usize {
    *added.entry(p).or_insert_with(|| {
        room.vertices.push([p[0], p[1], p[2], shade]);
        room.vertices.len() - 1
    })
}

/// Raise (negative `dy`) or lower a region of floor or ceiling faces,
/// building step walls around its edge. Returns the region's faces.
pub fn extrude(level: &mut Level, room: usize, quads: &[usize], dy: i32) -> Result<Vec<usize>, String> {
    snapped([0, dy, 0])?;
    if dy == 0 {
        return Ok(quads.to_vec());
    }
    let r = level.rooms.get(room).ok_or("no such room")?;
    let region: Vec<_> = quads
        .iter()
        .map(|&i| r.quads.get(i).and_then(|q| sector_face(r, q)).map(|f| (i, f)))
        .collect::<Option<_>>()
        .ok_or("extrude needs floor or ceiling faces")?;
    let surface = region.first().ok_or("select floor or ceiling faces to extrude")?.1.surface;
    if region.iter().any(|(_, f)| f.surface != surface) {
        return Err("extrude floors or ceilings, not both at once".into());
    }
    let had: BTreeSet<_> = sector_faces(r).keys().copied().collect();
    let cells: BTreeSet<_> = region.iter().map(|(_, f)| (f.x, f.z)).collect();
    // The step faces the side that is now lower (floors) or higher (ceilings).
    let outward = (surface == Surface::Floor) == (dy < 0);
    let mut r = level.rooms[room].clone();
    let mut moved = BTreeMap::new();
    let mut steps = vec![];
    for &(i, f) in &region {
        let quad = r.quads[i];
        for k in 0..4 {
            let (a, b) = (quad[k], quad[(k + 1) % 4]);
            let (pa, pb) = (point(&r, a), point(&r, b));
            // Across this edge: the sector on the other side of it.
            let mid = [(pa[0] + pb[0]) / 2, (pa[2] + pb[2]) / 2];
            let centre = [f.x as i32 * SECTOR + SECTOR / 2, f.z as i32 * SECTOR + SECTOR / 2];
            let out = [(mid[0] - centre[0]).signum(), (mid[1] - centre[1]).signum()];
            let beyond = (f.x as i32 + out[0], f.z as i32 + out[1]);
            if beyond.0 >= 0 && beyond.1 >= 0 && cells.contains(&(beyond.0 as usize, beyond.1 as usize)) {
                continue;
            }
            let open = beyond.0 >= 0
                && beyond.1 >= 0
                && sector_corners(level, room, beyond.0 as usize, beyond.1 as usize, surface).is_some();
            // A step into a solid neighbour would hide inside its wall.
            if outward && !open {
                continue;
            }
            let toward = if outward { [out[0], 0, out[1]] } else { [-out[0], 0, -out[1]] };
            steps.push((a, b, pa, pb, toward, quad[4], open));
        }
        for v in &quad[..4] {
            if !moved.contains_key(v) {
                let mut p = r.vertices[*v];
                p[1] += dy;
                r.vertices.push(p);
                moved.insert(*v, r.vertices.len() - 1);
            }
        }
        for k in 0..4 {
            r.quads[i][k] = moved[&quad[k]];
        }
    }
    let mut remove = BTreeSet::new();
    for (a, b, pa, pb, toward, texture, open) in steps {
        let up = |p: [i32; 3]| [p[0], p[1] + dy, p[2]];
        // Undoing an earlier step leaves two opposite faces in one place: drop both.
        if let Some(existing) = same_place(&r, &[pa, pb, up(pb), up(pa)]) {
            remove.insert(existing);
            continue;
        }
        // Below a wall that already reaches the new floor, nothing is missing.
        let span = [pa[1].min(pa[1] + dy), pa[1].max(pa[1] + dy), pb[1].min(pb[1] + dy), pb[1].max(pb[1] + dy)];
        if !open && covered(&r, [pa, pb], span) {
            continue;
        }
        let corners = facing(&r, [a, b, moved[&b], moved[&a]], toward);
        r.quads.push([corners[0], corners[1], corners[2], corners[3], texture]);
    }
    let mut kept = vec![];
    let mut map = vec![None; r.quads.len()];
    for (i, q) in r.quads.iter().enumerate() {
        if !remove.contains(&i) {
            map[i] = Some(kept.len());
            kept.push(*q);
        }
    }
    r.quads = kept;
    let orphans: BTreeSet<_> = moved.keys().copied().collect();
    collect_unused(&mut r, &orphans);
    fit_height(&mut r);
    level.rooms[room] = r;
    derive(level, room, &cells, &had)?;
    check_walls(level, room, faces_on_edges(&level.rooms[room], &cells))?;
    Ok(region.iter().filter_map(|&(i, _)| map[i]).collect())
}

/// Remove `candidates` that no face uses any more, renumbering the rest.
fn collect_unused(room: &mut Room, candidates: &BTreeSet<usize>) {
    let used: BTreeSet<_> = room
        .quads
        .iter()
        .flat_map(|q| q[..4].to_vec())
        .chain(room.triangles.iter().flat_map(|t| t[..3].to_vec()))
        .collect();
    let gone: BTreeSet<_> = candidates.iter().filter(|v| !used.contains(v)).copied().collect();
    if gone.is_empty() {
        return;
    }
    let mut map = vec![0; room.vertices.len()];
    let mut next = 0;
    let mut vertices = vec![];
    for (i, v) in room.vertices.iter().enumerate() {
        if !gone.contains(&i) {
            map[i] = next;
            next += 1;
            vertices.push(*v);
        }
    }
    room.vertices = vertices;
    for q in &mut room.quads {
        for v in &mut q[..4] {
            *v = map[*v];
        }
    }
    for t in &mut room.triangles {
        for v in &mut t[..3] {
            *v = map[*v];
        }
    }
}

/// Add a row of solid sectors to one side of a room's grid. Growing toward
/// negative x or z moves the origin; vertices and portals keep their places.
pub fn grow(level: &mut Level, room: usize, axis: usize, negative: bool) {
    let r = &mut level.rooms[room];
    let solid = crate::world::Sector { floor: SOLID, ceiling: SOLID, ..Default::default() };
    let (width, depth) = (r.x_sectors, r.z_sectors);
    let mut sectors = Vec::with_capacity(r.sectors.len() + width.max(depth) + 1);
    if axis == 0 {
        if negative {
            sectors.extend(std::iter::repeat_n(solid, depth));
        }
        sectors.extend_from_slice(&r.sectors);
        if !negative {
            sectors.extend(std::iter::repeat_n(solid, depth));
        }
        r.x_sectors += 1;
    } else {
        for column in r.sectors.chunks(depth) {
            if negative {
                sectors.push(solid);
            }
            sectors.extend_from_slice(column);
            if !negative {
                sectors.push(solid);
            }
        }
        r.z_sectors += 1;
    }
    r.sectors = sectors;
    if negative {
        r.origin[axis] -= SECTOR;
        for v in &mut r.vertices {
            v[axis] += SECTOR;
        }
        for portal in &mut r.portals {
            for v in &mut portal.vertices {
                v[axis] += SECTOR;
            }
        }
    }
}

/// Push a wall out by one sector: the solid sector behind it opens, with a
/// floor and ceiling at the wall's foot and top, and the wall (with every face
/// stacked on the same edge) moves to the far side. The room's grid grows
/// when that sector was on its border. Returns the moved wall faces.
pub fn push_wall(level: &mut Level, room: usize, quad: usize) -> Result<Vec<usize>, String> {
    let r = level.rooms.get(room).ok_or("no such room")?;
    let w =
        r.quads.get(quad).and_then(|q| wall_face(r, &q[..4])).ok_or("push a wall face that spans one sector edge")?;
    // Sector numbers across the wall: `inside` is seen from, `beyond` opens.
    let line = w.at / SECTOR;
    let (inside, beyond) = if w.toward > 0 { (line, line - 1) } else { (line - 1, line) };
    let size = [r.x_sectors as i32, 0, r.z_sectors as i32][w.axis];
    if beyond < 0 || beyond >= size {
        return Err("this wall is on the room's outer edge".into());
    }
    if beyond == 0 || beyond == size - 1 {
        grow(level, room, w.axis, beyond == 0);
        return push_wall(level, room, quad);
    }
    let cell = |a: i32| if w.axis == 0 { (a as usize, w.along as usize) } else { (w.along as usize, a as usize) };
    let (bx, bz) = cell(beyond);
    let b = &r.sectors[bx * r.z_sectors + bz];
    if sector_corners(level, room, bx, bz, Surface::Floor).is_some() || level.floor_info(b).portal.is_some() {
        return Err("the sector behind this wall is already open".into());
    }
    if b.below.is_some() || b.above.is_some() {
        return Err("the sector behind this wall joins another room".into());
    }
    let (ix, iz) = cell(inside);
    if sector_corners(level, room, ix, iz, Surface::Floor).is_none() {
        return Err("push a wall seen from an open sector".into());
    }
    let had: BTreeSet<_> = sector_faces(r).keys().copied().collect();
    let stack: Vec<usize> = (0..r.quads.len()).filter(|&i| wall_face(r, &r.quads[i][..4]) == Some(w)).collect();
    let other = 2 - w.axis;
    // The opening's foot and top at each end of the edge.
    let mut ends = [[i32::MAX, i32::MIN]; 2];
    for &i in &stack {
        for &v in &r.quads[i][..4] {
            let p = point(r, v);
            let end = usize::from(p[other] != w.along * SECTOR);
            ends[end] = [ends[end][0].min(p[1]), ends[end][1].max(p[1])];
        }
    }
    let texture =
        |surface| sector_faces(r).get(&(ix, iz, surface)).and_then(|f| f.first()).map(|&(i, _)| r.quads[i][4]);
    let wall_texture = r.quads[quad][4];
    let (floor_texture, ceiling_texture) =
        (texture(Surface::Floor).unwrap_or(wall_texture), texture(Surface::Ceiling).unwrap_or(wall_texture));
    let shade = r.vertices[r.quads[quad][0]][3];
    let mut r = r.clone();
    let step = -w.toward * SECTOR;
    let at = |end: usize, y: i32, far: bool| {
        let mut p = [0; 3];
        p[w.axis] = w.at + if far { step } else { 0 };
        p[other] = (w.along + end as i32) * SECTOR;
        p[1] = y;
        p
    };
    let mut added = BTreeMap::new();
    for &i in &stack {
        for k in 0..4 {
            let mut p = point(&r, r.quads[i][k]);
            p[w.axis] += step;
            let shade = r.vertices[r.quads[i][k]][3];
            r.quads[i][k] = vertex_at(&mut r, p, shade, &mut added);
        }
    }
    let mut corner = |r: &mut Room, end: usize, top: bool, far: bool| {
        let p = at(end, ends[end][usize::from(!top)], far);
        match r.vertices.iter().position(|v| [v[0], v[1], v[2]] == p) {
            Some(v) if !far => v,
            _ => vertex_at(r, p, shade, &mut added),
        }
    };
    let foot = [
        corner(&mut r, 0, false, false),
        corner(&mut r, 1, false, false),
        corner(&mut r, 1, false, true),
        corner(&mut r, 0, false, true),
    ];
    let head = [
        corner(&mut r, 0, true, false),
        corner(&mut r, 1, true, false),
        corner(&mut r, 1, true, true),
        corner(&mut r, 0, true, true),
    ];
    let mut new = vec![];
    let f = facing(&r, foot, [0, -1, 0]);
    new.push([f[0], f[1], f[2], f[3], floor_texture]);
    let c = facing(&r, head, [0, 1, 0]);
    new.push([c[0], c[1], c[2], c[3], ceiling_texture]);
    for end in 0..2 {
        let side = w.along + if end == 0 { -1 } else { 1 };
        let (sx, sz) = if w.axis == 0 { (beyond, side) } else { (side, beyond) };
        let inside_grid = sx >= 0 && sz >= 0 && (sx as usize) < r.x_sectors && (sz as usize) < r.z_sectors;
        let solid = !inside_grid || sector_corners(level, room, sx as usize, sz as usize, Surface::Floor).is_none();
        if solid {
            let mut toward = [0; 3];
            toward[other] = if end == 0 { 1 } else { -1 };
            let s = facing(&r, [foot[end], foot[3 - end], head[3 - end], head[end]], toward);
            new.push([s[0], s[1], s[2], s[3], wall_texture]);
        }
    }
    r.quads.extend(new);
    let orphans = stack.iter().flat_map(|&i| level.rooms[room].quads[i][..4].to_vec()).collect();
    collect_unused(&mut r, &orphans);
    fit_height(&mut r);
    level.rooms[room] = r;
    let cells = BTreeSet::from([(bx, bz)]);
    derive(level, room, &cells, &had)?;
    check_walls(level, room, faces_on_edges(&level.rooms[room], &cells))?;
    Ok(stack)
}

/// Set the texture of faces.
pub fn paint(level: &mut Level, room: usize, faces: &[Face], texture: usize) -> Result<(), String> {
    if texture >= level.textures.len() {
        return Err("no such texture".into());
    }
    let r = level.rooms.get_mut(room).ok_or("no such room")?;
    for face in faces {
        match *face {
            Face::Quad(i) => r.quads.get_mut(i).ok_or("no such face")?[4] = texture,
            Face::Triangle(i) => r.triangles.get_mut(i).ok_or("no such face")?[3] = texture,
        }
    }
    Ok(())
}

/// Whether every floor and ceiling face of a room agrees with its sector.
pub fn sector_mismatches(level: &Level, room: usize) -> Vec<String> {
    let mut out = vec![];
    for ((x, z, surface), faces) in sector_faces(&level.rooms[room]) {
        for (_, c) in faces {
            if sector_corners(level, room, x, z, surface) != Some(c) {
                out.push(format!("room {room} sector ({x}, {z}) {surface:?}: face {c:?}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
