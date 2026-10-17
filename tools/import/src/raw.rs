//! The extraction `tools/extract_tr2.py` writes: Tomb Raider II's level
//! tables as JSON, still in the original's layout (word streams, offsets,
//! packed flag words, sentinel values). Only the importer reads it.
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Level {
    pub name: String,
    #[serde(default)]
    pub source_sha256: String,
    #[serde(default)]
    pub source_size: u64,
    pub rooms: Vec<Room>,
    pub floor_data: Vec<u16>,
    pub meshes: Vec<Mesh>,
    pub mesh_indices: Vec<usize>,
    pub models: Vec<Model>,
    pub static_meshes: Vec<StaticMesh>,
    pub animations: Vec<Animation>,
    pub animation_commands: Vec<i16>,
    pub frame_data: Vec<u16>,
    pub textures: Vec<Texture>,
    pub entities: Vec<Placement>,
    /// x, y, z, room, flags.
    #[serde(default)]
    pub cameras: Vec<[i32; 5]>,
    pub sound_map: Vec<i16>,
    /// sample, volume, range, flags.
    pub sound_info: Vec<[u16; 4]>,
    /// z_min, z_max, x_min, x_max (sectors), floor, overlap index.
    #[serde(default)]
    pub boxes: Vec<[i32; 6]>,
    #[serde(default)]
    pub overlaps: Vec<u16>,
    #[serde(default)]
    pub zones: Vec<Vec<i16>>,
    /// state, range count, first range.
    #[serde(default)]
    pub animation_states: Vec<[usize; 3]>,
    /// first frame, last frame, animation, frame.
    #[serde(default)]
    pub animation_ranges: Vec<[i32; 4]>,
}

#[derive(Debug, Deserialize)]
pub struct Room {
    pub id: usize,
    pub origin: [i32; 3],
    pub top: i32,
    pub bottom: i32,
    pub vertices: Vec<[i32; 5]>,
    pub quads: Vec<[usize; 5]>,
    pub triangles: Vec<[usize; 4]>,
    pub portals: Vec<Portal>,
    pub x_sectors: usize,
    pub z_sectors: usize,
    pub sectors: Vec<Sector>,
    pub static_placements: Vec<StaticPlacement>,
    /// Ambient shade, second ambient, light mode.
    #[serde(default)]
    pub ambient: [i32; 3],
    /// x, y, z, intensity, intensity2, falloff, falloff2.
    #[serde(default)]
    pub lights: Vec<[i64; 7]>,
    pub alternate: i32,
    pub flags: u16,
}

#[derive(Debug, Deserialize)]
pub struct Portal {
    pub room: usize,
    #[serde(default)]
    pub normal: [i32; 3],
    pub vertices: Vec<[i32; 3]>,
}

#[derive(Debug, Deserialize)]
pub struct Sector {
    #[serde(rename = "box", default = "no_box")]
    pub navigation_box: usize,
    pub floor_data: usize,
    pub below: usize,
    pub above: usize,
    pub floor: i32,
    pub ceiling: i32,
}

fn no_box() -> usize {
    0xffff
}

#[derive(Debug, Deserialize)]
pub struct Texture {
    pub atlas_uv: [[f32; 2]; 4],
    pub attribute: u16,
}

#[derive(Debug, Deserialize)]
pub struct Mesh {
    #[serde(default)]
    pub center: [i32; 3],
    #[serde(default)]
    pub radius: i32,
    pub vertices: Vec<[i32; 4]>,
    pub quads: Vec<[usize; 5]>,
    pub triangles: Vec<[usize; 4]>,
    pub normals: Vec<[i32; 4]>,
}

#[derive(Debug, Deserialize)]
pub struct Model {
    pub id: u16,
    pub mesh_count: usize,
    pub mesh_start: usize,
    pub animation: usize,
    pub joints: Vec<Joint>,
    pub initial_pose: Option<Pose>,
    #[serde(default)]
    pub animation_end: usize,
}

#[derive(Debug, Deserialize)]
pub struct Joint {
    pub parent: i32,
    pub translation: [i32; 3],
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pose {
    pub root: [i32; 3],
    /// 1024 to a turn.
    pub angles: Vec<[u16; 3]>,
    #[serde(default)]
    pub bounds: [i32; 6],
}

#[derive(Debug, Deserialize)]
pub struct StaticMesh {
    pub id: u16,
    pub mesh: usize,
    pub collision_box: [i32; 6],
    pub flags: u16,
}

#[derive(Debug, Deserialize)]
pub struct StaticPlacement {
    pub mesh_id: u16,
    pub position: [i32; 3],
    pub rotation: u16,
    pub light: i32,
}

#[derive(Debug, Deserialize)]
pub struct Placement {
    pub id: u16,
    pub room: usize,
    pub position: [i32; 3],
    pub rotation: u16,
    pub flags: u16,
}

#[derive(Debug, Deserialize)]
pub struct Animation {
    #[serde(default)]
    pub state_count: usize,
    #[serde(default)]
    pub state_offset: usize,
    /// Byte offset into `frame_data`.
    pub frame_offset: usize,
    pub frame_rate: usize,
    /// Words per key.
    pub frame_size: usize,
    pub frame_start: usize,
    pub frame_end: usize,
    pub next_animation: usize,
    pub next_frame: usize,
    pub state: usize,
    pub speed: i32,
    pub accel: i32,
    pub poses: Vec<Pose>,
    pub command_count: usize,
    pub command_offset: usize,
}
