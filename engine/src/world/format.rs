//! The engine's level format: `assets/levels/<name>/level.json`, next to
//! `atlas.png` (the texture atlas) and `audio/NNN.wav` (sound samples).
//!
//! # Units
//!
//! Level files use integer **world units**: 1024 to a sector, and a sector
//! is a metre in the game. The y axis points **down** (a higher floor has a
//! smaller y), as in the engine's collision and camera rules, which are exact
//! integer arithmetic over these values. [`super::position`] converts a
//! point to Bevy's y-up metres.
//!
//! Angles are **binary angles**: 65536 to a full turn (0 faces +z, 16384
//! faces +x). Frame numbers count authored animation frames at 30 per
//! second.
//!
//! Index fields refer to other lists in the same file (`room` to `rooms`,
//! `mesh` to `meshes`, `animation` to `animations`...).
use bevy::{asset::Asset, reflect::TypePath};
use serde::{Deserialize, Serialize};

/// Format version written by importers and checked by the loader.
pub const FORMAT_VERSION: u32 = 1;

/// Floor or ceiling height of a solid sector (no space to stand in).
pub const SOLID: i32 = -32512;

#[derive(Asset, TypePath, Debug, Clone, Default, Serialize, Deserialize)]
pub struct Level {
    pub format: u32,
    pub name: String,
    /// Where the content came from, when it was imported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    pub rooms: Vec<Room>,
    /// Floor descriptions shared by sectors (`Sector::floor_info`).
    pub floor_infos: Vec<FloorInfo>,
    pub textures: Vec<Texture>,
    pub meshes: Vec<Mesh>,
    pub models: Vec<Model>,
    pub static_meshes: Vec<StaticMesh>,
    pub animations: Vec<Animation>,
    pub placements: Vec<Placement>,
    /// Fixed camera positions that triggers can cut to.
    pub cameras: Vec<FixedCamera>,
    /// Navigation boxes: open floor areas the camera keeps its eye inside.
    pub boxes: Vec<NavBox>,
    /// Authored adjacency and zones for creature navigation. Local corridor
    /// queries use overlaps; full per-creature zone search remains unported.
    #[serde(default)]
    pub overlaps: Vec<u16>,
    #[serde(default)]
    pub zones: Vec<Vec<i16>>,
    /// Sound id -> index into `sounds`, or none.
    pub sound_map: Vec<Option<usize>>,
    pub sounds: Vec<Sound>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub file: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Room {
    /// Room origin: x and z of its sector grid corner; y is unused (0).
    pub origin: [i32; 3],
    /// y of the room's highest ceiling and lowest floor.
    pub top: i32,
    pub bottom: i32,
    /// `[x, y, z, shade]`: x and z relative to the origin, y absolute;
    /// shade is the vertex colour (low byte, 128 = neutral).
    pub vertices: Vec<[i32; 4]>,
    /// `[v0, v1, v2, v3, texture]`, corners in drawing order.
    pub quads: Vec<[usize; 5]>,
    /// `[v0, v1, v2, texture]`.
    pub triangles: Vec<[usize; 4]>,
    pub portals: Vec<Portal>,
    /// Sector grid size; sectors are stored x-major (`x * z_sectors + z`).
    pub x_sectors: usize,
    pub z_sectors: usize,
    pub sectors: Vec<Sector>,
    pub static_placements: Vec<StaticPlacement>,
    pub ambient: Ambient,
    pub lights: Vec<Light>,
    /// The room swapped in when the level flips (not yet used).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alternate: Option<usize>,
    #[serde(default)]
    pub water: bool,
    /// Hair and cloth blow in this room's wind.
    #[serde(default)]
    pub wind: bool,
    /// Room flag bits with no engine meaning yet.
    #[serde(default)]
    pub other_flags: u16,
}

/// Light every lit object in the room starts from.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Ambient {
    /// 0 (bright) .. 8191 (dark).
    pub shade: i32,
    pub shade2: i32,
    /// Light mode (0 = steady; flickering modes are not yet used).
    pub mode: i32,
}

/// A point light that lights objects in its room.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Light {
    /// Absolute position.
    pub position: [i32; 3],
    pub intensity: i16,
    pub intensity2: i16,
    /// Distance at which the light falls to half.
    pub falloff: i32,
    pub falloff2: i32,
}

/// An opening into another room; `vertices` are relative to this room's origin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portal {
    pub room: usize,
    pub normal: [i32; 3],
    pub vertices: Vec<[i32; 3]>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Sector {
    /// Floor and ceiling heights ([`SOLID`] for solid sectors).
    pub floor: i32,
    pub ceiling: i32,
    /// The room continuing below the floor / above the ceiling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub above: Option<usize>,
    /// Navigation box this sector belongs to.
    #[serde(default, rename = "box", skip_serializing_if = "Option::is_none")]
    pub nav_box: Option<usize>,
    /// Slopes, portals and triggers (index into `Level::floor_infos`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_info: Option<usize>,
}

/// What a sector's floor does besides having a height.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FloorInfo {
    /// The sector continues sideways into this room.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portal: Option<usize>,
    /// Tilt in quarter world units per unit across the sector, `[x, z]`:
    /// the floor rises by `tilt * 256` across the sector.
    #[serde(default)]
    pub floor_slope: [i8; 2],
    #[serde(default)]
    pub ceiling_slope: [i8; 2],
    /// Standing here kills.
    #[serde(default)]
    pub kill: bool,
    /// Climbable walls: the source's climb sub-field (not yet used).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub climb: Option<u8>,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
}

/// A floor trigger: when its condition holds for the avatar in the sector,
/// its actions run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trigger {
    /// Identity for one-shot memory; sectors sharing a trigger share its id.
    pub id: usize,
    pub kind: TriggerKind,
    /// Seconds an activation lasts (0 = until deactivated; 1 = one tick).
    #[serde(default)]
    pub timer: u8,
    /// Fire only once per level.
    #[serde(default)]
    pub once: bool,
    /// Activation code bits this trigger sets (five bits).
    #[serde(default)]
    pub mask: u8,
    pub actions: Vec<Action>,
}

/// What has to be true for a trigger to fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    /// Whenever the avatar is in the sector.
    Floor,
    /// Only while the avatar stands on the floor.
    Pad,
    /// Its first action names a switch the avatar just used; toggles code bits.
    Switch,
    /// Its first action names a keyhole the avatar just used.
    Key,
    /// Its first action names a pickup the avatar collected.
    Pickup,
    /// Like `pad`, but clears code bits.
    AntiPad,
    /// Only while the avatar is armed; does not start camera shots.
    Combat,
    /// Fired by something other than the avatar (not yet supported); the
    /// importer's code for it.
    Other(u16),
}

/// One trigger action. `activate` (argument: placement index) and `camera`
/// (argument: camera index) are the engine's; other names are game actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Action {
    pub action: String,
    pub argument: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<CameraShot>,
}

/// How a `camera` action holds its shot.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct CameraShot {
    /// Seconds; 0 = while the avatar stays on the trigger.
    pub timer: u8,
    pub once: bool,
    /// The eye closes 1/(8*speed+1) of the gap to the camera per tick.
    pub speed: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Texture {
    /// Atlas UV of each corner (0..1).
    pub uv: [[f32; 2]; 4],
    /// The source's texture attribute (its blending; not yet used by the
    /// renderer, which draws every face with the atlas alpha).
    pub attribute: u16,
}

/// A mesh in model space. Lit meshes have normals; unlit ones carry a shade
/// per vertex instead.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Mesh {
    /// Bounding sphere.
    pub center: [i32; 3],
    pub radius: i32,
    /// `[x, y, z, shade]`; shade 0 (bright) .. 8191 (dark) on unlit meshes.
    pub vertices: Vec<[i32; 4]>,
    /// `[x, y, z]`, length 16384.
    pub normals: Vec<[i32; 3]>,
    pub quads: Vec<[usize; 5]>,
    pub triangles: Vec<[usize; 4]>,
}

/// An articulated model: one mesh per joint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Model {
    /// Object type id of the placements that use it.
    pub id: u16,
    pub meshes: Vec<usize>,
    pub joints: Vec<Joint>,
    /// The model's clips: `animations[0]..animations[1]`.
    pub animations: [usize; 2],
    /// Pose of a model without clips.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_pose: Option<Pose>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Joint {
    /// Earlier joint this one hangs from, none for the root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<usize>,
    /// Offset from the parent joint.
    pub offset: [i32; 3],
}

/// One keyframe: root offset and per-joint rotations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pose {
    pub root: [i32; 3],
    /// Per joint `[x, y, z]` binary angles, applied y, then x, then z.
    pub angles: Vec<[u16; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticMesh {
    pub id: u16,
    pub mesh: usize,
    /// `[x_min, x_max, y_min, y_max, z_min, z_max]`.
    pub collision_box: [i32; 6],
    pub flags: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticPlacement {
    pub mesh_id: u16,
    pub position: [i32; 3],
    pub rotation: u16,
    pub shade: i32,
}

/// A placed object.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Placement {
    /// Object type id.
    pub kind: u16,
    pub room: usize,
    pub position: [i32; 3],
    pub rotation: u16,
    /// Activation code bits it starts with (all five = starts active).
    #[serde(default)]
    pub code: u8,
    /// Active while its code is *not* complete.
    #[serde(default)]
    pub inverted: bool,
    /// Not drawn until activated.
    #[serde(default)]
    pub invisible: bool,
    /// Flag bits with no engine meaning yet.
    #[serde(default)]
    pub other_flags: u16,
}

/// An authored clip.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Animation {
    /// Frames between stored keys.
    pub frame_rate: usize,
    /// This clip's frame numbers run `frame_start..=frame_end`.
    pub frame_start: usize,
    pub frame_end: usize,
    /// Where play continues when the clip ends.
    pub next_animation: usize,
    pub next_frame: usize,
    /// The state this clip belongs to.
    pub state: usize,
    /// Ground speed and its change per frame, 1/65536 world units per frame.
    pub speed: i32,
    pub accel: i32,
    /// Keys every `frame_rate` frames.
    pub poses: Vec<Pose>,
    /// Bounding box `[x_min, x_max, y_min, y_max, z_min, z_max]` of each key,
    /// plus the closing key the last interval blends toward when present.
    pub bounds: Vec<[i32; 6]>,
    pub state_changes: Vec<StateChange>,
    pub commands: Vec<Command>,
}

/// While the clip frame is in `first_frame..=last_frame` and the controller
/// asks for `state`, play `animation` from `frame`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateChange {
    pub state: usize,
    pub first_frame: usize,
    pub last_frame: usize,
    pub animation: usize,
    pub frame: usize,
}

/// Something a clip does besides moving joints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    /// When the clip ends, move the actor by this facing-relative offset.
    SetPosition([i32; 3]),
    /// When the clip ends, launch the actor: `[vertical, horizontal]` speed.
    JumpVelocity([i32; 2]),
    EmptyHands,
    Kill,
    /// Play `sound` on `frame`.
    Sound { frame: usize, sound: usize, environment: SoundEnvironment },
    /// Run game effect `effect` on `frame`.
    Effect { frame: usize, effect: u16 },
}

/// Where a clip's sound may play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundEnvironment {
    Anywhere,
    OnLand,
    InWater,
    Never,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct FixedCamera {
    pub position: [i32; 3],
    /// Room the camera is in; shots with no valid room are ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<usize>,
    /// Usable once per level.
    #[serde(default)]
    pub once: bool,
    #[serde(default)]
    pub other_flags: u16,
}

/// An open floor rectangle in sector numbers: `x_min..x_max`, `z_min..z_max`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct NavBox {
    pub z_min: i32,
    pub z_max: i32,
    pub x_min: i32,
    pub x_max: i32,
    pub floor: i32,
    pub overlap: i32,
}

/// A sound: its sample (`audio/NNN.wav`) and how it plays.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Sound {
    pub sample: usize,
    /// 0..32767.
    pub volume: u16,
    pub range: u16,
    pub flags: u16,
}
