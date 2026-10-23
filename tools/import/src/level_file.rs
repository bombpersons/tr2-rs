//! `DATA/<LEVEL>.PSX`: one Tomb Raider II level as the PlayStation version
//! stores it, decoded into [`raw::Level`] tables plus the level's texture
//! atlas and sound samples.
//!
//! The layout was first written down as `tools/extract_tr2.py` (cross-checked
//! against OpenLara's `format.h` before this project's reference
//! restriction; no engine code copied) and is ported here as it was. The
//! room vertex packing was confirmed on the PSX (RAM `0x800f4144` for Wall
//! room 35, read by the room renderer `0x800617b0`). Every byte of the file
//! must be accounted for.
use image::{Rgba, RgbaImage};

use crate::raw;

pub type Result<T> = std::result::Result<T, String>;

/// Little-endian reads with bounds checks.
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).filter(|&end| end <= self.data.len());
        let end = end.ok_or_else(|| format!("{:#x}+{n} exceeds {:#x}", self.at, self.data.len()))?;
        let bytes = &self.data[self.at..end];
        self.at = end;
        Ok(bytes)
    }
    fn skip(&mut self, n: usize) -> Result<()> {
        self.bytes(n).map(|_| ())
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }
    fn i8(&mut self) -> Result<i8> {
        Ok(self.u8()? as i8)
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }
    fn i16(&mut self) -> Result<i16> {
        Ok(self.u16()? as i16)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }
    /// A `u32` element count, refusing absurd ones before allocating.
    fn count(&mut self) -> Result<usize> {
        let at = self.at;
        let n = self.u32()? as usize;
        if n > 2_000_000 { Err(format!("unreasonable count {n} at {at:#x}")) } else { Ok(n) }
    }
    fn list<T>(&mut self, n: usize, mut read: impl FnMut(&mut Self) -> Result<T>) -> Result<Vec<T>> {
        (0..n).map(|_| read(self)).collect()
    }
    /// A `u32` count, then that many elements.
    fn counted<T>(&mut self, read: impl FnMut(&mut Self) -> Result<T>) -> Result<Vec<T>> {
        let n = self.count()?;
        self.list(n, read)
    }
    fn u16s(&mut self, n: usize) -> Result<Vec<u16>> {
        self.list(n, Self::u16)
    }
    fn i16s<const N: usize>(&mut self) -> Result<[i16; N]> {
        let mut out = [0; N];
        for v in &mut out {
            *v = self.i16()?;
        }
        Ok(out)
    }
}

/// One decoded level file.
pub struct LevelFile {
    pub level: raw::Level,
    /// The textures' tiles, each with a one-texel border, packed 2048 wide.
    pub atlas: RgbaImage,
    /// Sound samples, decoded to 16-bit PCM at 8 kHz.
    pub samples: Vec<Vec<i16>>,
}

pub const SAMPLE_RATE: u32 = 8000;

/// Decodes `bytes`, the file `DATA/<NAME>.PSX`.
pub fn decode(name: &str, bytes: &[u8]) -> Result<LevelFile> {
    let r = &mut Reader { data: bytes, at: 0 };
    let sample_count = r.count()?;
    let offsets = r.list(sample_count + 1, Reader::u32)?;
    let sound = r.bytes((offsets[sample_count] - offsets[0]) as usize)?;
    r.skip(4)?;
    let room_count = r.u16()?;
    let rooms = (0..usize::from(room_count)).map(|i| room(r, i)).collect::<Result<Vec<_>>>()?;
    let floor_data = r.counted(Reader::u16)?;
    let words = r.count()?;
    let mesh_data = r.bytes(words * 2)?;
    let mesh_offsets = r.counted(Reader::u32)?;
    let mut animations = r.counted(animation)?;
    let animation_states = r.counted(|r| Ok([usize::from(r.u16()?), usize::from(r.u16()?), usize::from(r.u16()?)]))?;
    let animation_ranges = r.counted(|r| Ok(r.i16s::<4>()?.map(i32::from)))?;
    let animation_commands = r.counted(Reader::i16)?;
    let nodes = r.counted(Reader::i32)?;
    let frame_data = r.counted(Reader::u16)?;
    let mut models = r.counted(model)?;
    let static_meshes = r.counted(|r| {
        let id = r.u32()? as u16;
        let mesh = usize::from(r.u16()?);
        let _visual_box = r.i16s::<6>()?;
        let collision_box = r.i16s::<6>()?.map(i32::from);
        Ok(raw::StaticMesh { id, mesh, collision_box, flags: r.u16()? })
    })?;
    let tiles = r.counted(|r| r.bytes(32768))?;
    let cluts = r.counted(|r| r.u16s(16))?;
    r.skip(4)?;
    let textures = r.counted(texture)?;
    let sprites = r.counted(sprite)?;
    let sprite_sequences = r.counted(|r| {
        let id = r.i32()?;
        let length = r.i16()?;
        let first = usize::from(r.u16()?);
        let id = u16::try_from(id).map_err(|_| format!("sprite sequence of object {id}"))?;
        // The file stores the length negated, as the object table's mesh count.
        Ok(raw::SpriteSequence { id, first, count: usize::from(length.unsigned_abs()) })
    })?;
    let cameras = r.counted(|r| Ok([r.i32()?, r.i32()?, r.i32()?, i32::from(r.i16()?), i32::from(r.u16()?)]))?;
    let _sound_sources = r.counted(|r| r.bytes(16))?;
    let boxes = r.counted(|r| {
        let [a, b, c, d] = [r.u8()?, r.u8()?, r.u8()?, r.u8()?].map(i32::from);
        Ok([a, b, c, d, i32::from(r.i16()?), i32::from(r.u16()?)])
    })?;
    let overlaps = r.counted(Reader::u16)?;
    let zones = (0..10).map(|_| r.list(boxes.len(), Reader::i16)).collect::<Result<Vec<_>>>()?;
    let _animated_textures = r.counted(Reader::u16)?;
    let entities = r.counted(|r| {
        let id = r.u16()?;
        let room = r.i16()?;
        let position = [r.i32()?, r.i32()?, r.i32()?];
        let rotation = r.u16()?;
        let _intensity = [r.i16()?, r.i16()?];
        let room = usize::try_from(room).map_err(|_| format!("placement in room {room}"))?;
        Ok(raw::Placement { id, room, position, rotation, flags: r.u16()? })
    })?;
    r.skip(4)?;
    let sound_map = r.list(370, Reader::i16)?;
    let sound_info = r.counted(|r| Ok([r.u16()?, r.u16()?, r.u16()?, r.u16()?]))?;
    r.skip(4)?;
    let camera_frames = r.u16()?;
    r.skip(usize::from(camera_frames) * 16)?;
    let demo = r.count()?;
    r.skip(demo)?;
    if r.at != bytes.len() {
        return Err(format!("{} trailing bytes", bytes.len() - r.at));
    }

    let mut unique = mesh_offsets.clone();
    unique.sort_unstable();
    unique.dedup();
    let meshes = unique.iter().map(|&o| mesh(mesh_data, o as usize)).collect::<Result<Vec<_>>>()?;
    let mesh_indices = mesh_offsets.iter().map(|o| unique.binary_search(o).unwrap()).collect();
    let (atlas, atlas_uvs, sprite_rects) = atlas(&textures, &sprites, &tiles, &cluts)?;
    let sprites = sprites.iter().zip(sprite_rects).map(|(s, atlas)| raw::Sprite { atlas, rect: s.rect }).collect();
    let textures =
        textures.iter().zip(atlas_uvs).map(|(t, atlas_uv)| raw::Texture { atlas_uv, attribute: t.attribute }).collect();
    animate(&mut models, &mut animations, &frame_data, &nodes)?;
    let samples = offsets
        .windows(2)
        .map(|pair| {
            let at = |o: u32| ((o - offsets[0]) as usize).min(sound.len());
            adpcm(&sound[at(pair[0])..at(pair[1])])
        })
        .collect::<Result<Vec<_>>>()?;

    let level = raw::Level {
        name: name.to_string(),
        source_sha256: psx_media::disc::sha256_bytes(bytes),
        source_size: bytes.len() as u64,
        rooms,
        floor_data,
        meshes,
        mesh_indices,
        models,
        static_meshes,
        animations,
        animation_commands,
        frame_data,
        textures,
        entities,
        cameras,
        sound_map,
        sound_info,
        boxes,
        overlaps,
        zones,
        animation_states,
        animation_ranges,
        sprites,
        sprite_sequences,
    };
    validate(&level)?;
    Ok(LevelFile { level, atlas, samples })
}

fn room(r: &mut Reader, id: usize) -> Result<raw::Room> {
    let (x, z, bottom, top) = (r.i32()?, r.i32()?, r.i32()?, r.i32()?);
    let size = r.u32()? as usize;
    let start = r.at;
    let end = start + size * 2;
    // The vertex count is stored twice. Each vertex word is xyz in its low 15
    // bits (x 10-14, y 5-9, z 0-4) and a 16-bit shade in its high half.
    let (count, again) = (r.u16()?, r.u16()?);
    if count != again {
        return Err(format!("room {id}: vertex counts {count} and {again}"));
    }
    let vertices = r.list(usize::from(count), |r| {
        let c = r.u32()?;
        let field = |shift: u32| ((c >> shift) & 31) as i32;
        Ok([field(10) * 1024, field(5) * 256 + top, field(0) * 1024, (c >> 16) as i32, 0])
    })?;
    let n = r.u16()?;
    let quad_textures = r.u16s(usize::from(n))?;
    if !(r.at - start).is_multiple_of(4) {
        r.skip(2)?;
    }
    let quads = quad_textures
        .iter()
        .map(|&t| {
            let [a, b, c, d] = [r.u16()?, r.u16()?, r.u16()?, r.u16()?].map(|v| usize::from(v >> 2));
            Ok([a, b, d, c, usize::from(t & 0x7fff)])
        })
        .collect::<Result<Vec<_>>>()?;
    let n = r.u16()?;
    r.skip(2)?;
    let triangles = r.list(usize::from(n), |r| {
        let t = r.u16()?;
        let [a, b, c] = [r.u16()?, r.u16()?, r.u16()?].map(|v| usize::from(v >> 2));
        Ok([a, b, c, usize::from(t & 0x7fff)])
    })?;
    if r.at > end {
        return Err(format!("room {id}: geometry overruns {:#x} > {end:#x}", r.at));
    }
    let faces = quads.iter().map(|q| &q[..4]).chain(triangles.iter().map(|t| &t[..3]));
    if faces.flatten().any(|&v| v >= vertices.len()) {
        return Err(format!("room {id}: a face uses a vertex past {}", vertices.len()));
    }
    r.skip(end - r.at)?;
    let n = r.u16()?;
    let portals = r.list(usize::from(n), |r| {
        let room = usize::from(r.u16()?);
        let normal = r.i16s::<3>()?.map(i32::from);
        let vertices = r.list(4, |r| {
            let [a, b, c] = r.i16s::<3>()?.map(i32::from);
            Ok([a, b + top, c])
        })?;
        Ok(raw::Portal { room, normal, vertices })
    })?;
    let (z_sectors, x_sectors) = (usize::from(r.u16()?), usize::from(r.u16()?));
    let sectors = r.list(x_sectors * z_sectors, |r| {
        Ok(raw::Sector {
            floor_data: usize::from(r.u16()?),
            navigation_box: usize::from(r.u16()?),
            below: usize::from(r.u8()?),
            floor: i32::from(r.i8()?) * 256,
            above: usize::from(r.u8()?),
            ceiling: i32::from(r.i8()?) * 256,
        })
    })?;
    let ambient = [r.u16()?, r.u16()?, r.u16()?].map(i32::from);
    let n = r.u16()?;
    let lights = r.list(usize::from(n), |r| {
        Ok([
            i64::from(r.i32()?),
            i64::from(r.i32()?),
            i64::from(r.i32()?),
            i64::from(r.u16()?),
            i64::from(r.u16()?),
            i64::from(r.u32()?),
            i64::from(r.u32()?),
        ])
    })?;
    let n = r.u16()?;
    let static_placements = r.list(usize::from(n), |r| {
        let position = [r.i32()?, r.i32()?, r.i32()?];
        let rotation = r.u16()?;
        let _light = r.u16()?;
        let light = i32::from(r.u16()?);
        Ok(raw::StaticPlacement { position, rotation, light, mesh_id: r.u16()? })
    })?;
    let alternate = i32::from(r.i16()?);
    let flags = r.u16()?;
    Ok(raw::Room {
        id,
        origin: [x, 0, z],
        top,
        bottom,
        vertices,
        quads,
        triangles,
        portals,
        x_sectors,
        z_sectors,
        sectors,
        static_placements,
        ambient,
        lights,
        alternate,
        flags,
    })
}

fn mesh(data: &[u8], offset: usize) -> Result<raw::Mesh> {
    let r = &mut Reader { data, at: offset };
    let center = r.i16s::<3>()?.map(i32::from);
    let radius = i32::from(r.i16()?);
    let _flags = r.u16()?;
    let n = r.i16()?;
    let count = usize::from(n.unsigned_abs());
    let mut vertices = r.list(count, |r| Ok(r.i16s::<4>()?.map(i32::from)))?;
    let mut normals = Vec::new();
    if n > 0 {
        normals = r.list(count, |r| Ok(r.i16s::<4>()?.map(i32::from)))?;
        let n = r.u16()?;
        r.skip(usize::from(n) * 12)?;
        let n = r.u16()?;
        r.skip(usize::from(n) * 10)?;
    } else {
        for v in &mut vertices {
            v[3] = i32::from(r.u16()?);
        }
    }
    let n = r.u16()?;
    let quads = r.list(usize::from(n), |r| {
        let [a, b, c, d, t] = [r.u16()?, r.u16()?, r.u16()?, r.u16()?, r.u16()?].map(usize::from);
        Ok([a >> 3, b >> 3, c >> 3, d >> 3, t & 0x7fff])
    })?;
    let n = r.u16()?;
    let triangles = r.list(usize::from(n), |r| {
        let [a, b, c, t] = [r.u16()?, r.u16()?, r.u16()?, r.u16()?];
        Ok([usize::from(a >> 3), usize::from(b >> 3), usize::from(c >> 3), usize::from(t & 0x7fff)])
    })?;
    let faces = quads.iter().map(|q| &q[..4]).chain(triangles.iter().map(|t| &t[..3]));
    if faces.flatten().any(|&v| v >= vertices.len()) {
        return Err(format!("mesh at {offset:#x}: a face uses a vertex past {}", vertices.len()));
    }
    Ok(raw::Mesh { center, radius, vertices, quads, triangles, normals })
}

fn animation(r: &mut Reader) -> Result<raw::Animation> {
    let frame_offset = r.u32()? as usize;
    let frame_rate = usize::from(r.u8()?);
    let frame_size = usize::from(r.u8()?);
    let state = usize::from(r.u16()?);
    let speed = r.i32()?;
    let accel = r.i32()?;
    let mut words = [0; 8];
    for w in &mut words {
        *w = usize::from(r.u16()?);
    }
    let [frame_start, frame_end, next_animation, next_frame, state_count, state_offset, command_count, command_offset] =
        words;
    Ok(raw::Animation {
        state_count,
        state_offset,
        frame_offset,
        frame_rate,
        frame_size,
        frame_start,
        frame_end,
        next_animation,
        next_frame,
        state,
        speed,
        accel,
        poses: Vec::new(),
        command_count,
        command_offset,
    })
}

fn model(r: &mut Reader) -> Result<raw::Model> {
    let id = r.u16()?;
    let _pad = r.u16()?;
    let mesh_count = usize::from(r.u16()?);
    let mesh_start = usize::from(r.u16()?);
    let node_offset = r.u32()? as usize;
    let frame_offset = r.u32()? as usize;
    let animation = usize::from(r.u16()?);
    let _pad = r.u16()?;
    Ok(raw::Model {
        id,
        mesh_count,
        mesh_start,
        animation,
        joints: Vec::new(),
        initial_pose: None,
        animation_end: 0,
        node_offset,
        frame_offset,
    })
}

/// An object texture: a tile, a CLUT, four texel corners.
struct Texture {
    tile: usize,
    clut: usize,
    uv: [[u8; 2]; 4],
    attribute: u16,
}

/// A sprite's record: its rectangle from where it is drawn, its CLUT and
/// tile, and its first and last texel (the runtime copy at `0x80089B08`
/// is read the same way by the sprite drawer `0x800697CC`).
struct SpriteRecord {
    rect: [i32; 4],
    clut: usize,
    tile: usize,
    uv: [[u8; 2]; 2],
}

fn sprite(r: &mut Reader) -> Result<SpriteRecord> {
    let rect = r.i16s::<4>()?.map(i32::from);
    let clut = usize::from(r.u16()?);
    let tile = usize::from(r.u16()?);
    let uv = [[r.u8()?, r.u8()?], [r.u8()?, r.u8()?]];
    Ok(SpriteRecord { rect, clut, tile, uv })
}

fn texture(r: &mut Reader) -> Result<Texture> {
    let mut uv = [[0; 2]; 4];
    let mut words = [0; 4];
    for (corner, word) in uv.iter_mut().zip(&mut words) {
        *corner = [r.u8()?, r.u8()?];
        *word = r.u16()?;
    }
    // After each corner: the CLUT, the tile (low 14 bits), unused, the attribute.
    Ok(Texture { tile: usize::from(words[1] & 0x3fff), clut: usize::from(words[0]), uv, attribute: words[3] })
}

/// PlayStation RGB555: zero is transparent; opaque black sets the STP bit.
fn color(c: u16) -> Rgba<u8> {
    let channel = |shift: u16| ((c >> shift) & 31) as u32 * 255 / 31;
    Rgba([channel(0) as u8, channel(5) as u8, channel(10) as u8, if c == 0 { 0 } else { 255 }])
}

const ATLAS_WIDTH: u32 = 2048;

/// Crops each texture's and then each sprite's rectangle from its 4-bit
/// tile through its CLUT, with a one-texel clamped border, and packs them in
/// rows 2048 wide. Returns the atlas, each texture's corners in it (texel
/// centres, 0..1) and each sprite's rectangle in it (pixels).
#[allow(clippy::type_complexity)]
fn atlas(
    textures: &[Texture],
    sprites: &[SpriteRecord],
    tiles: &[&[u8]],
    cluts: &[Vec<u16>],
) -> Result<(RgbaImage, Vec<[[f32; 2]; 4]>, Vec<[u32; 4]>)> {
    let crop = |tile: usize, clut: usize, min: [u32; 2], w: u32, h: u32, what: &str| {
        let (Some(tile), Some(clut)) = (tiles.get(tile), cluts.get(clut)) else {
            return Err(format!("{what} uses tile {tile} / CLUT {clut} of {} / {}", tiles.len(), cluts.len()));
        };
        Ok(RgbaImage::from_fn(w + 2, h + 2, |x, y| {
            let x = min[0] + x.saturating_sub(1).min(w - 1);
            let y = min[1] + y.saturating_sub(1).min(h - 1);
            let byte = tile[((y * 256 + x) / 2) as usize];
            color(clut[usize::from((byte >> (4 * (x % 2))) & 15)])
        }))
    };
    let mut images = Vec::with_capacity(textures.len() + sprites.len());
    for t in textures {
        // A fourth corner of (0, 0) marks a triangle texture.
        let corners = if t.uv[3] == [0, 0] { &t.uv[..3] } else { &t.uv[..] };
        let min = [0, 1].map(|axis| corners.iter().map(|c| u32::from(c[axis])).min().unwrap());
        let max = [0, 1].map(|axis| corners.iter().map(|c| u32::from(c[axis])).max().unwrap());
        let (w, h) = (max[0] - min[0] + 1, max[1] - min[1] + 1);
        images.push((crop(t.tile, t.clut, min, w, h, "texture")?, min));
    }
    for s in sprites {
        let min = s.uv[0].map(u32::from);
        let [w, h] = [0, 1].map(|axis| (u32::from(s.uv[1][axis]) + 1).saturating_sub(min[axis]).max(1));
        images.push((crop(s.tile, s.clut, min, w, h, "sprite")?, min));
    }
    let mut positions = Vec::with_capacity(images.len());
    let (mut x, mut y, mut row) = (0, 0, 0);
    for (image, _) in &images {
        if x + image.width() > ATLAS_WIDTH {
            (x, y, row) = (0, y + row, 0);
        }
        positions.push((x, y));
        x += image.width();
        row = row.max(image.height());
    }
    let bits = u32::BITS - (y + row).saturating_sub(1).leading_zeros();
    let height = 1u32 << bits.max(1);
    let mut atlas = RgbaImage::new(ATLAS_WIDTH, height);
    for ((image, _), &(x, y)) in images.iter().zip(&positions) {
        image::imageops::replace(&mut atlas, image, i64::from(x), i64::from(y));
    }
    let uvs = textures
        .iter()
        .zip(&images)
        .zip(&positions)
        .map(|((t, (_, min)), &(x, y))| {
            t.uv.map(|[u, v]| {
                let at = |origin: u32, texel: u8, min: u32, size: u32| {
                    ((f64::from(origin) + 1.0 + f64::from(texel) - f64::from(min) + 0.5) / f64::from(size)) as f32
                };
                [at(x, u, min[0], ATLAS_WIDTH), at(y, v, min[1], height)]
            })
        })
        .collect();
    let rects = images[textures.len()..]
        .iter()
        .zip(&positions[textures.len()..])
        .map(|((image, _), &(x, y))| [x + 1, y + 1, image.width() - 2, image.height() - 2])
        .collect();
    Ok((atlas, uvs, rects))
}

fn signed(word: u16) -> i32 {
    i32::from(word as i16)
}

/// A keyframe at word `offset / 2` of the frame data: bounds, root offset,
/// then one rotation per mesh (1024 to a turn; one packed axis, or all three
/// across two words).
fn pose(frames: &[u16], offset: usize, count: usize) -> Result<raw::Pose> {
    let mut i = offset / 2;
    let word = |i: usize| frames.get(i).copied().ok_or_else(|| format!("frame word {i} past {}", frames.len()));
    let head = (0..9).map(|k| word(i + k).map(signed)).collect::<Result<Vec<_>>>()?;
    i += 9;
    let mut angles = Vec::with_capacity(count);
    for _ in 0..count {
        let a = word(i)?;
        i += 1;
        let axis = usize::from(a >> 14);
        angles.push(if axis != 0 {
            let mut v = [0; 3];
            v[axis - 1] = a & 1023;
            v
        } else {
            let b = word(i)?;
            i += 1;
            [(a & 0x3ff0) >> 4, ((a & 15) << 6) | ((b & 0xfc00) >> 10), b & 1023]
        });
    }
    Ok(raw::Pose { root: [head[6], head[7], head[8]], angles, bounds: head[..6].try_into().unwrap() })
}

/// The model's joints from its node list: a pop and a push flag, then the
/// joint's offset from its parent.
fn skeleton(model: &raw::Model, nodes: &[i32]) -> Vec<raw::Joint> {
    let mut joints = vec![raw::Joint { parent: -1, translation: [0; 3] }];
    let (mut parent, mut stack) = (0, Vec::new());
    for j in 1..model.mesh_count {
        let i = model.node_offset + (j - 1) * 4;
        let Some(&[flags, x, y, z]) = nodes.get(i..i + 4).map(|n| <&[i32; 4]>::try_from(n).unwrap()) else {
            // Some inventory meshes have no bone tree.
            joints.push(raw::Joint { parent: 0, translation: [0; 3] });
            continue;
        };
        if flags & 1 != 0 {
            parent = stack.pop().unwrap_or(0);
        }
        if flags & 2 != 0 {
            stack.push(parent);
        }
        joints.push(raw::Joint { parent, translation: [x, y, z] });
        parent = j as i32;
    }
    joints
}

/// Joints and initial pose per model, and every keyframe of the animations
/// a model owns (from its first animation to the next model's).
fn animate(models: &mut [raw::Model], animations: &mut [raw::Animation], frames: &[u16], nodes: &[i32]) -> Result<()> {
    let mut starts: Vec<usize> = models.iter().map(|m| m.animation).filter(|&a| a != 0xffff).collect();
    starts.sort_unstable();
    starts.dedup();
    for m in models.iter_mut() {
        m.joints = skeleton(m, nodes);
        m.initial_pose = Some(pose(frames, m.frame_offset, m.mesh_count)?);
        let start = m.animation;
        if start == 0xffff {
            continue;
        }
        let end = starts.iter().copied().find(|&s| s > start).unwrap_or(animations.len());
        m.animation_end = end;
        for a in animations.get_mut(start..end).unwrap_or_default() {
            let span = a.frame_end as i64 - a.frame_start as i64;
            let count = span.div_euclid(a.frame_rate.max(1) as i64) + 1;
            a.poses = (0..count.max(0) as usize)
                .map(|i| pose(frames, a.frame_offset + i * a.frame_size * 2, m.mesh_count))
                .collect::<Result<_>>()?;
        }
    }
    Ok(())
}

/// PlayStation SPU ADPCM: 16-byte blocks of 28 samples, until a block with
/// the end flag.
fn adpcm(data: &[u8]) -> Result<Vec<i16>> {
    const FILTERS: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];
    if !data.len().is_multiple_of(16) {
        return Err(format!("sound sample of {} bytes is not whole blocks", data.len()));
    }
    let (mut h1, mut h2) = (0i32, 0i32);
    let mut pcm = Vec::with_capacity(data.len() / 16 * 28);
    for block in data.as_chunks::<16>().0 {
        let (shift, filter, flags) = (block[0] & 15, usize::from(block[0] >> 4), block[1]);
        let &(a, b) = FILTERS.get(filter).ok_or_else(|| format!("ADPCM filter {filter}"))?;
        for &byte in &block[2..] {
            for nibble in [byte & 15, byte >> 4] {
                let nibble = i32::from(((nibble << 4) as i8) >> 4);
                let v = ((nibble << 12) >> shift) + ((h1 * a + h2 * b + 32) >> 6);
                let v = v.clamp(-32768, 32767);
                pcm.push(v as i16);
                (h2, h1) = (h1, v);
            }
        }
        if flags & 1 != 0 {
            break;
        }
    }
    Ok(pcm)
}

/// Every cross-reference in range.
fn validate(l: &raw::Level) -> Result<()> {
    let (rooms, textures, meshes) = (l.rooms.len(), l.textures.len(), l.mesh_indices.len());
    let check = |ok: bool, what: String| if ok { Ok(()) } else { Err(what) };
    for s in &l.sprite_sequences {
        check(s.first + s.count <= l.sprites.len(), format!("sprite sequence of object {}", s.id))?;
    }
    for room in &l.rooms {
        let id = room.id;
        check(room.alternate == -1 || (0..rooms as i32).contains(&room.alternate), format!("room {id}: alternate"))?;
        for p in &room.portals {
            check(p.room < rooms, format!("room {id}: portal to {}", p.room))?;
        }
        for s in &room.sectors {
            check(s.floor_data < l.floor_data.len(), format!("room {id}: floor data {}", s.floor_data))?;
            check(s.above == 255 || s.above < rooms, format!("room {id}: room above {}", s.above))?;
            check(s.below == 255 || s.below < rooms, format!("room {id}: room below {}", s.below))?;
        }
        for t in room.quads.iter().map(|q| q[4]).chain(room.triangles.iter().map(|t| t[3])) {
            check(t < textures, format!("room {id}: texture {t}"))?;
        }
    }
    for m in &l.meshes {
        for t in m.quads.iter().map(|q| q[4]).chain(m.triangles.iter().map(|t| t[3])) {
            check(t < textures, format!("mesh texture {t}"))?;
        }
    }
    for m in &l.models {
        check(m.mesh_start + m.mesh_count <= meshes, format!("model {}: meshes", m.id))?;
    }
    for e in &l.entities {
        check(e.room < rooms, format!("placement of {} in room {}", e.id, e.room))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adpcm_decodes_a_block_through_its_filter() {
        // Shift 12 keeps nibbles as they are; filter 0 adds no history.
        let mut block = [0u8; 16];
        block[0] = 12;
        block[1] = 1;
        block[2] = 0x71; // 1 then 7
        block[3] = 0x08; // -8 then 0
        let pcm = adpcm(&block).unwrap();
        assert_eq!(&pcm[..4], &[1, 7, -8, 0]);
        assert_eq!(pcm.len(), 28);
    }

    #[test]
    fn rgb555_zero_is_transparent_and_stp_black_is_opaque() {
        assert_eq!(color(0), Rgba([0, 0, 0, 0]));
        assert_eq!(color(0x8000), Rgba([0, 0, 0, 255]));
        assert_eq!(color(0x7fff), Rgba([255, 255, 255, 255]));
    }
}
