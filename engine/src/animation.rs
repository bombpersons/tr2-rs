//! Authored animation: clips that chain into each other, state changes
//! through authored frame ranges, per-clip ground speed, and model posing.
//!
//! Clips are authored at [`ANIMATION_FPS`]. A clip names the *state* it
//! belongs to; a controller asks for a goal state and the clip's authored
//! ranges decide which clip and frame play next ([`AnimationLibrary::dispatch`]).
//! Which goal a controller asks for, and when, is game code.
use bevy::prelude::*;

use crate::shell::GameSystems;
use crate::schedule::{WorldSystems, simulation_enabled};
use crate::world::{Command, Level, Model, Pose, WorldGeometry, position, units};

/// Authored clip frames per second.
pub const ANIMATION_FPS: f32 = 30.0;

/// Elapsed animation frames for `seconds` of game time. Bevy's f32 seconds can
/// represent one 30 Hz tick as slightly less than one frame; integer frame
/// boundaries are preserved, otherwise floor() would delay a dispatch by a tick.
pub fn animation_frames(seconds: f32) -> f32 {
    let frames = seconds * ANIMATION_FPS;
    if (frames - frames.round()).abs() < 0.00001 { frames.round() } else { frames }
}

/// The clip an animated actor is playing.
#[derive(Component, Reflect, Debug, Clone, Default)]
#[reflect(Component)]
pub struct AnimationState {
    pub animation: usize,
    /// Frame relative to this clip's `frame_start` (not its absolute frame).
    pub frame: f32,
    requested: usize,
}

impl AnimationState {
    /// Start a clip without treating the first tick as a new request.
    pub fn at_frame(animation: usize, frame: f32) -> Self {
        Self { animation, frame, requested: animation }
    }

    /// The clip the controller last asked for (it may still be playing an
    /// authored lead-in or a chained clip).
    pub fn requested(&self) -> usize {
        self.requested
    }

    /// Accept `index` as the requested clip without changing what plays.
    pub fn acknowledge(&mut self, index: usize) {
        self.requested = index;
    }

    /// Enter `clip` at `frame` on behalf of the request `index`.
    pub fn enter(&mut self, clip: usize, frame: f32, index: usize) {
        self.animation = clip;
        self.frame = frame;
        self.requested = index;
    }

    /// Restart at frame 0 of `animation` when it is a new request.
    pub fn select(&mut self, animation: usize) {
        if self.requested != animation {
            self.requested = animation;
            self.animation = animation;
            self.frame = 0.0;
        }
    }
}

/// Every clip of the loaded level, with its state ranges decoded.
#[derive(Resource, Default)]
pub struct AnimationLibrary {
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Clip {
    pub frame_start: usize,
    pub frame_end: usize,
    pub next_animation: usize,
    pub next_frame: usize,
    /// Ground speed and its per-frame change, 16.16 world units per frame.
    pub speed: i32,
    pub accel: i32,
    #[serde(default)]
    pub state: usize,
    #[serde(default)]
    pub state_ranges: Vec<StateRange>,
    /// Position command (x, y, z in the authored y-down, facing-relative
    /// frame), applied when the clip ends: a climb's final step up.
    #[serde(default)]
    pub end_offset: Option<[i32; 3]>,
}

/// An authored state change: while the clip frame is in the inclusive range
/// and the goal is `state`, play `animation` from `frame`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StateRange {
    pub state: usize,
    pub first_frame: usize,
    pub last_frame: usize,
    pub animation: usize,
    pub frame: usize,
}

impl From<&Level> for AnimationLibrary {
    fn from(data: &Level) -> Self {
        let clips = data
            .animations
            .iter()
            .map(|a| Clip {
                frame_start: a.frame_start,
                frame_end: a.frame_end,
                next_animation: a.next_animation,
                next_frame: a.next_frame,
                speed: a.speed,
                accel: a.accel,
                state: a.state,
                state_ranges: a
                    .state_changes
                    .iter()
                    .map(|c| StateRange {
                        state: c.state,
                        first_frame: c.first_frame,
                        last_frame: c.last_frame,
                        animation: c.animation,
                        frame: c.frame,
                    })
                    .collect(),
                end_offset: a.commands.iter().find_map(|c| match c {
                    Command::SetPosition(offset) => Some(*offset),
                    _ => None,
                }),
            })
            .collect();
        Self { clips }
    }
}

impl AnimationLibrary {
    /// Clip index and clip-relative frame the authored ranges select for
    /// `goal`. The frame is advanced by one first: the animation step
    /// increments before it dispatches.
    pub fn dispatch(&self, animation: &AnimationState, goal: usize) -> Option<(usize, f32)> {
        let clip = self.clips.get(animation.animation)?;
        let frame = clip.frame_start + animation.frame.max(0.0).floor() as usize + 1;
        let range = clip.state_ranges.iter().find(|r| r.state == goal && (r.first_frame..=r.last_frame).contains(&frame))?;
        let destination = self.clips.get(range.animation)?;
        Some((range.animation, range.frame.saturating_sub(destination.frame_start) as f32))
    }

    /// The state a clip belongs to.
    pub fn state(&self, animation: usize) -> Option<usize> {
        self.clips.get(animation).map(|clip| clip.state)
    }

    /// Length of a clip in frames (inclusive of its last frame).
    pub fn frames(&self, animation: usize) -> Option<f32> {
        self.clips.get(animation).map(|clip| (clip.frame_end + 1).saturating_sub(clip.frame_start) as f32)
    }

    /// Duration in frames and end offset of a clip that moves its actor when
    /// it ends (a climb).
    pub fn climb(&self, animation: usize) -> Option<(f32, [i32; 3])> {
        let clip = self.clips.get(animation)?;
        Some(((clip.frame_end + 1).saturating_sub(clip.frame_start) as f32, clip.end_offset?))
    }

    /// Ground speed of the frame being shown, `(speed + accel * frame) >> 16`
    /// world units per frame, as metres per second.
    pub fn speed(&self, animation: &AnimationState) -> Option<f32> {
        let clip = self.clips.get(animation.animation)?;
        let frame = i64::from(animation.frame.floor() as i32);
        Some(units(((i64::from(clip.speed) + i64::from(clip.accel) * frame) >> 16) as f32 * ANIMATION_FPS))
    }
}

/// Play `ticks` frames, following each finished clip into its authored next
/// clip and frame. A bounded loop also accepts externally edited frames.
pub fn advance(animation: &mut AnimationState, library: Option<&AnimationLibrary>, ticks: f32) {
    animation.frame += ticks;
    let Some(library) = library else { return };
    for _ in 0..8 {
        let Some(clip) = library.clips.get(animation.animation) else {
            break;
        };
        let duration = clip.frame_end.saturating_sub(clip.frame_start) as f32 + 1.0;
        if animation.frame < duration {
            break;
        }
        animation.frame -= duration;
        animation.animation = clip.next_animation;
        if let Some(next) = library.clips.get(clip.next_animation) {
            animation.frame += clip.next_frame.saturating_sub(next.frame_start) as f32;
        }
    }
}

/// A looping clip on a placed object (doors, creatures, scenery).
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct ObjectAnimation {
    pub animation: usize,
    pub frame: f32,
}

/// Switch an object of type `kind` to the first of its model's clips in
/// `state`, unless it is already playing one.
pub fn select_state(world: &WorldGeometry, kind: u16, state: usize, a: &mut ObjectAnimation) {
    let Some(model) = world.data.model(kind) else {
        return;
    };
    if world.data.animations.get(a.animation).is_some_and(|clip| clip.state == state) {
        return;
    }
    if let Some((index, _)) = world
        .data
        .animations
        .iter()
        .enumerate()
        .skip(model.animations[0])
        .take(model.animations[1].saturating_sub(model.animations[0]))
        .find(|(_, clip)| clip.state == state)
    {
        a.animation = index;
        a.frame = 0.0;
    }
}

fn animate_objects(time: Res<Time>, world: Option<Res<WorldGeometry>>, mut objects: Query<&mut ObjectAnimation>) {
    let Some(world) = world else { return };
    for mut a in &mut objects {
        if let Some(clip) = world.data.animations.get(a.animation) {
            a.frame += time.delta_secs() * ANIMATION_FPS;
            if a.frame > clip.frame_end.saturating_sub(clip.frame_start) as f32 {
                a.frame = 0.0;
            }
        }
    }
}

/// Interpolated vertical bounds (y down, world units) of an actor's current
/// frame, interpolated toward the next stored key (clips store a closing key
/// after their last interval). The
/// last-interval rate test compares a clip-relative frame with the absolute
/// end frame, so it only shortens the interval for clips starting at frame 0.
/// The weighted difference is truncated before adding the first bound.
pub fn vertical_bounds(data: &Level, animation: &AnimationState) -> Option<(i32, i32)> {
    let clip = data.animations.get(animation.animation)?;
    let frame = animation.frame.max(0.0).floor() as usize;
    let mut rate = clip.frame_rate.max(1);
    let (key, interp) = (frame / rate, frame % rate);
    let pose = |k: usize, i: usize| clip.bounds.get(k).map(|b| b[i]);
    let first = (pose(key, 2)?, pose(key, 3)?);
    if interp == 0 {
        return Some(first);
    }
    let second_frame = key * rate + rate;
    if clip.frame_end < second_frame {
        rate = clip.frame_end - key * rate;
    }
    let second = (pose(key + 1, 2)?, pose(key + 1, 3)?);
    let lerp = |a: i32, b: i32| a + ((b - a) as i64 * interp as i64 / rate.max(1) as i64) as i32;
    Some((lerp(first.0, second.0), lerp(first.1, second.1)))
}

fn rotation(a: [u16; 3]) -> Quat {
    let [x, y, z] = a.map(|v| f32::from(v) * std::f32::consts::TAU / 65536.0);
    Quat::from_rotation_y(y) * Quat::from_rotation_x(-x) * Quat::from_rotation_z(-z)
}

fn bone_transforms(model: &Model, pose: &Pose) -> Vec<Transform> {
    let mut transforms = Vec::<Transform>::with_capacity(model.meshes.len());
    for (i, j) in model.joints.iter().enumerate() {
        let q = rotation(pose.angles.get(i).copied().unwrap_or_default());
        let local = Transform::from_translation(position(if i == 0 { pose.root } else { j.offset })).with_rotation(q);
        let t = match j.parent {
            Some(parent) => transforms[parent].mul_transform(local),
            None => local,
        };
        transforms.push(t);
    }
    transforms
}

/// Bone transforms relative to the model root (level space, y up), with
/// keys interpolated linearly.
pub fn model_pose(data: &Level, model: &Model, anim: usize, frame: f32) -> Vec<Transform> {
    let clip = data.animations.get(anim).filter(|_| (model.animations[0]..model.animations[1]).contains(&anim));
    if let Some(clip) = clip.filter(|c| !c.poses.is_empty()) {
        let key = frame / clip.frame_rate.max(1) as f32;
        let i = (key.floor() as usize).min(clip.poses.len() - 1);
        let j = (i + 1).min(clip.poses.len() - 1);
        let first = bone_transforms(model, &clip.poses[i]);
        let second = bone_transforms(model, &clip.poses[j]);
        first
            .into_iter()
            .zip(second)
            .map(|(a, b)| Transform {
                translation: a.translation.lerp(b.translation, key.fract()),
                rotation: a.rotation.slerp(b.rotation, key.fract()),
                scale: Vec3::ONE,
            })
            .collect()
    } else if let Some(pose) = &model.initial_pose {
        bone_transforms(model, pose)
    } else {
        vec![]
    }
}

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        // AnimationLibrary is inserted when a level loads; rules treat its
        // absence as "no authored clips".
        app.add_systems(
            FixedUpdate,
            animate_objects.in_set(WorldSystems::Animation).in_set(GameSystems::Game).run_if(simulation_enabled),
        );
    }
}
