//! Authored animation: clips that chain into each other, state changes
//! through authored frame ranges, per-clip ground speed, and model posing.
//!
//! Clips are authored at [`ANIMATION_FPS`]. A clip names the *state* it
//! belongs to; a controller asks for a goal state and the clip's authored
//! ranges decide which clip and frame play next ([`AnimationLibrary::dispatch`]).
//! Which goal a controller asks for, and when, is game code.
use bevy::prelude::*;

use crate::schedule::{ItemSystems, simulation_enabled};
use crate::shell::GameSystems;
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
        let range =
            clip.state_ranges.iter().find(|r| r.state == goal && (r.first_frame..=r.last_frame).contains(&frame))?;
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

/// A placed object's current clip and clip-relative frame. Add AnimationGoal
/// to use authored state dispatch and next-clip links.
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

/// A controller's requested state for authored dispatch. Only objects with
/// this component use table-driven transitions; legacy prototype controllers
/// retain their existing clip selection until independently verified.
#[derive(Component, Reflect, Debug, Clone, Default)]
#[reflect(Component)]
pub struct AnimationGoal {
    pub state: usize,
}

/// A cue from an authored frame or clip end. Motion/end commands are exposed
/// for a controller to handle; this pose step does not invent object physics.
#[derive(Debug, Clone)]
pub struct AnimationCue {
    pub animation: usize,
    pub frame: i64,
    pub at_end: bool,
    pub command: Command,
}

#[derive(Message, Debug, Clone)]
pub struct ObjectAnimationCue {
    pub entity: Entity,
    pub cue: AnimationCue,
}

/// Advance one authored frame: increment, dispatch on an inclusive range,
/// then follow the selected clip's end link. PSX AnimateItem 0x8001DD98 and
/// GetChange 0x8001E250; verified per tick on a recorded door cycle.
/// Dispatch retains the requested goal. A natural end link resets it only
/// if the destination's state differs from the state just being played.
pub fn step_authored_animation(data: &Level, a: &mut ObjectAnimation, goal: &mut AnimationGoal) -> Vec<AnimationCue> {
    let mut cues = Vec::new();
    let Some(mut clip) = data.animations.get(a.animation) else { return cues };
    let mut absolute = clip.frame_start as i64 + a.frame.floor() as i64 + 1;
    if clip.state != goal.state
        && let Some(range) = clip
            .state_changes
            .iter()
            .find(|r| r.state == goal.state && (r.first_frame as i64..=r.last_frame as i64).contains(&absolute))
        && let Some(destination) = data.animations.get(range.animation)
    {
        a.animation = range.animation;
        absolute = range.frame as i64;
        clip = destination;
    }
    if absolute > clip.frame_end as i64 {
        for command in &clip.commands {
            if !matches!(command, Command::Sound { .. } | Command::Effect { .. }) {
                cues.push(AnimationCue {
                    animation: a.animation,
                    frame: absolute,
                    at_end: true,
                    command: command.clone(),
                });
            }
        }
        if let Some(destination) = data.animations.get(clip.next_animation) {
            a.animation = clip.next_animation;
            absolute = clip.next_frame as i64;
            if destination.state != clip.state {
                goal.state = destination.state;
            }
            clip = destination;
        }
    }
    a.frame = (absolute - clip.frame_start as i64) as f32;
    for command in &clip.commands {
        let frame = match command {
            Command::Sound { frame, .. } | Command::Effect { frame, .. } => *frame as i64,
            _ => continue,
        };
        if frame == absolute {
            cues.push(AnimationCue { animation: a.animation, frame, at_end: false, command: command.clone() });
        }
    }
    cues
}

/// Advance authored frames with fractional interpolation at other fixed tick
/// rates. The validated game runs at exactly one authored frame per tick.
pub fn advance_authored_animation(
    data: &Level,
    a: &mut ObjectAnimation,
    goal: &mut AnimationGoal,
    frames: f32,
) -> Vec<AnimationCue> {
    if !frames.is_finite() || frames <= 0.0 {
        return vec![];
    }
    let elapsed = a.frame.rem_euclid(1.0) + frames;
    a.frame = a.frame.floor();
    let mut cues = Vec::new();
    for _ in 0..elapsed.floor() as usize {
        cues.extend(step_authored_animation(data, a, goal));
    }
    a.frame += elapsed.fract();
    cues
}

fn animate_objects(
    time: Res<Time>,
    world: Option<Res<WorldGeometry>>,
    mut objects: Query<(Entity, &mut ObjectAnimation, Option<&mut AnimationGoal>)>,
    mut cues: MessageWriter<ObjectAnimationCue>,
) {
    let Some(world) = world else { return };
    for (entity, mut a, goal) in &mut objects {
        if let Some(mut goal) = goal {
            for cue in advance_authored_animation(&world.data, &mut a, &mut goal, animation_frames(time.delta_secs())) {
                cues.write(ObjectAnimationCue { entity, cue });
            }
        } else if let Some(clip) = world.data.animations.get(a.animation) {
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
    let bounds = frame_bounds(data, animation.animation, animation.frame)?;
    Some((bounds[2], bounds[3]))
}

/// Interpolated authored bounds at a clip-relative frame, in source model
/// units (`x_min,x_max,y_min,y_max,z_min,z_max`, y down). Avatar clearance
/// uses these; object contact instead uses [`collision_bounds`].
pub fn frame_bounds(data: &Level, animation: usize, frame: f32) -> Option<[i32; 6]> {
    let clip = data.animations.get(animation)?;
    let frame = frame.max(0.0).floor() as usize;
    let mut rate = clip.frame_rate.max(1);
    let (key, interp) = (frame / rate, frame % rate);
    let first = *clip.bounds.get(key)?;
    if interp == 0 {
        return Some(first);
    }
    let second_frame = key * rate + rate;
    if clip.frame_end < second_frame {
        rate = clip.frame_end - key * rate;
    }
    let second = *clip.bounds.get(key + 1)?;
    Some(std::array::from_fn(|i| {
        first[i] + ((second[i] - first[i]) as i64 * interp as i64 / rate.max(1) as i64) as i32
    }))
}

/// Nearest authored collision key, with exact halfway choosing the earlier
/// key. The final shortened interval follows the stored absolute end-frame
/// comparison (PSX key selection0x80028824 and bounds0x800289D4).
fn collision_key(data: &Level, animation: usize, frame: f32) -> Option<usize> {
    let clip = data.animations.get(animation)?;
    let frame = frame.max(0.0).floor() as usize;
    let rate = clip.frame_rate.max(1);
    let (key, remainder) = (frame / rate, frame % rate);
    let interval = if (key + 1) * rate > clip.frame_end { clip.frame_end.saturating_sub(key * rate) } else { rate };
    Some(key + usize::from(remainder > interval / 2))
}

/// The authored box used by animated object contact, rather than the
/// interpolated box used by avatar vertical clearance.
pub fn collision_bounds(data: &Level, animation: usize, frame: f32) -> Option<[i32; 6]> {
    data.animations.get(animation)?.bounds.get(collision_key(data, animation, frame)?).copied()
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

/// A mesh's authored bounding sphere after the current skeletal pose.
#[derive(Debug, Clone, Copy)]
pub struct PosedSphere {
    /// Joint index, retained when a mesh has no positive-radius sphere.
    pub joint: usize,
    pub center: Vec3,
    pub radius: f32,
}

/// Mesh collision spheres in world space. The actor supplies its root
/// transform; model data supplies joint offsets, pose keys and sphere geometry.
/// Uses the engine's ordinary posed bones; it does not emulate a fixed-point
/// geometry processor or apply game-specific aiming/head rotations.
pub fn model_spheres(data: &Level, model: &Model, animation: usize, frame: f32, root: &Transform) -> Vec<PosedSphere> {
    let frame = collision_key(data, animation, frame)
        .and_then(|key| data.animations.get(animation).map(|clip| (key * clip.frame_rate.max(1)) as f32))
        .unwrap_or(frame);
    model_pose(data, model, animation, frame)
        .into_iter()
        .zip(&model.meshes)
        .enumerate()
        .filter_map(|(joint, (bone, &index))| {
            let mesh = data.meshes.get(index)?;
            (mesh.radius > 0).then(|| PosedSphere {
                joint,
                center: root.transform_point(bone.transform_point(position(mesh.center))),
                radius: units(mesh.radius as f32) * root.scale.abs().max_element(),
            })
        })
        .collect()
}

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        // AnimationLibrary is inserted when a level loads; rules treat its
        // absence as "no authored clips".
        app.add_message::<ObjectAnimationCue>().add_systems(
            FixedUpdate,
            animate_objects.in_set(ItemSystems::Animation).in_set(GameSystems::Game).run_if(simulation_enabled),
        );
    }
}
