//! Lara's braid, ported from HairControl 0x8002f67c, InitialiseHair 0x8002f594
//! and the draw at 0x800305ac (docs/braid-evidence.md).
//!
//! Seven points (root plus six segment ends) in original units (y down). Each
//! tick the root follows the head, every later point keeps three quarters of
//! its velocity, falls 10 units (dry), is held above the floor and out of five
//! body spheres, then is pulled back to its segment length behind the previous
//! point. The six braid meshes are drawn at points 0..5 with the angles that
//! point at the next one.
use bevy::prelude::*;

use engine::animation::model_pose;
use engine::objects::SpawnContext;
use engine::render::{InRoom, LevelEntity};
use engine::world::{Level, WorldGeometry, position};

use crate::player::{AnimationState, Player, SimulationEnabled};

/// Object id of the braid model (6 meshes, no animation).
pub const BRAID_MODEL: u16 = 2;
/// Root offset in the head mesh's frame (0x8002fbd8: translate 0, -23, -55).
const ROOT_OFFSET: [i32; 3] = [0, -23, -55];
const NO_HEIGHT: i32 = -32512;

#[derive(Component, Clone, Debug)]
pub struct Braid {
    pub points: [Vec3; 7],
    pub velocity: [Vec3; 7],
    /// Yaw/pitch of each segment (65536 = full turn), original convention.
    pub angles: [(i32, i32); 7],
    pub wind: i32,
    first: bool,
    seed: u32,
}

impl Default for Braid {
    fn default() -> Self {
        // InitialiseHair: every segment hangs straight down (pitch -16384).
        Self {
            points: [Vec3::ZERO; 7],
            velocity: [Vec3::ZERO; 7],
            angles: [(0, -16384); 7],
            wind: 0,
            first: true,
            seed: 0x2d5d,
        }
    }
}

#[derive(Component)]
pub struct BraidSegment(pub usize);

/// Give Lara her braid and spawn its six segment meshes, when the level has
/// the braid model.
pub fn spawn(entity: &mut bevy::ecs::system::EntityCommands, ctx: &SpawnContext) {
    let data = &ctx.world.data;
    let (Some(hair), Some(lit)) = (data.model(BRAID_MODEL), ctx.lit_material.clone()) else {
        return;
    };
    if data.model(ctx.placement.kind).is_none() {
        return;
    }
    entity.insert(Braid::default());
    let room = ctx.placement.room;
    let render_root = ctx.render_root;
    for (j, &mi) in hair.meshes.iter().enumerate() {
        entity.commands().spawn((
            Mesh3d(ctx.meshes[mi].clone()),
            MeshMaterial3d(lit.clone()),
            Transform::default(),
            Visibility::default(),
            InRoom(room),
            BraidSegment(j),
            LevelEntity,
            ChildOf(render_root),
            Name::new(format!("Braid{j}")),
        ));
    }
}

fn to_original(v: Vec3) -> Vec3 {
    Vec3::new(v.x, -v.y, v.z) * 1024.0
}

fn angle(v: i32) -> f32 {
    v as f32 * std::f32::consts::TAU / 65536.0
}

/// phd_atan(x, y) 0x80063ec4: the angle of (x, y) measured from the x
/// argument towards the y argument, in 65536 units (yaw = atan(dz, dx)).
fn atan(x: f32, y: f32) -> i32 {
    (y.atan2(x) * 65536.0 / std::f32::consts::TAU).round() as i32
}

/// Offset of a segment of `length` at (yaw, pitch): RotY(yaw)*RotX(pitch)*(0,0,L).
fn segment(length: f32, (yaw, pitch): (i32, i32)) -> Vec3 {
    let (sy, cy) = angle(yaw).sin_cos();
    let (sp, cp) = angle(pitch).sin_cos();
    Vec3::new(length * cp * sy, -length * sp, length * cp * cy)
}

/// Segment lengths: the braid model's joint offsets, the last one repeated
/// (0x80030450 reuses joint 5 for point 6).
fn lengths(data: &Level) -> Option<[f32; 6]> {
    let model = data.model(BRAID_MODEL)?;
    let length = |j: usize| model.joints.get(j).map(|j| Vec3::from_array(j.offset.map(|v| v as f32)).length());
    Some([length(1)?, length(2)?, length(3)?, length(4)?, length(5)?, length(5)?])
}

/// Head attachment and the five body spheres (hips, torso, head, both upper
/// arms; the left arm reuses mesh 8's centre and radius as 0x8002fadc does).
fn body(data: &Level, player: &Player, animation: &AnimationState) -> Option<(Vec3, [(Vec3, f32); 5])> {
    let model = data.model(crate::player::LARA)?;
    let pose = model_pose(data, model, animation.animation, animation.frame);
    let root = Transform::from_translation(player.position).with_rotation(Quat::from_rotation_y(player.yaw));
    let mesh = |j: usize| data.meshes.get(*model.meshes.get(j)?);
    let at = |joint: usize, local: [i32; 3]| -> Option<Vec3> {
        let bone = pose.get(joint)?;
        Some(to_original(root.transform_point(bone.transform_point(position(local)))))
    };
    let sphere = |joint: usize, mesh_index: usize, scale: f32| -> Option<(Vec3, f32)> {
        let m = mesh(mesh_index)?;
        Some((at(joint, m.center)?, m.radius as f32 * scale))
    };
    let attach = at(14, ROOT_OFFSET)?;
    Some((attach, [sphere(0, 0, 1.0)?, sphere(7, 7, 1.0)?, sphere(14, 14, 1.0)?, sphere(8, 8, 1.5)?, sphere(11, 8, 1.5)?]))
}

fn rand(seed: &mut u32) -> i32 {
    *seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
    ((*seed >> 16) & 0x7fff) as i32
}

/// One HairControl tick.
pub fn step(braid: &mut Braid, world: &WorldGeometry, player: &Player, animation: &AnimationState) {
    let data = &world.data;
    let (Some((attach, spheres)), Some(lengths)) = (body(data, player, animation), lengths(data)) else { return };
    braid.points[0] = attach;
    if braid.first {
        braid.first = false;
        for i in 0..6 {
            braid.points[i + 1] = braid.points[i] + segment(lengths[i], braid.angles[i]);
        }
        braid.velocity = [Vec3::ZERO; 7];
        braid.wind = 0;
        return;
    }
    let level = |p: Vec3| position([p.x as i32, p.y as i32, p.z as i32]);
    let hit = world.floor_at(level(attach), player.room);
    let mut floor = hit.map_or(NO_HEIGHT, |h| (-h.height * 1024.0).round() as i32);
    if floor < attach.y as i32 {
        floor = (-player.position.y * 1024.0).round() as i32;
    }
    // Rooms flagged 0x20 blow the braid along +z with a random walk 0..7.
    let windy = data.rooms.get(hit.map_or(player.room, |h| h.room)).is_some_and(|r| r.wind);
    if windy {
        let r = rand(&mut braid.seed) & 7;
        if r != 0 {
            braid.wind += r - 4;
            if braid.wind < 0 {
                braid.wind = 0;
            } else if braid.wind >= 8 {
                braid.wind -= 1;
            }
        }
    } else {
        braid.wind = 0;
    }
    for i in 1..7 {
        let old = braid.points[i];
        let v = braid.velocity[i];
        let mut p = old + (v * 3.0 / 4.0).trunc();
        p.y += 10.0;
        if (floor as f32) < p.y {
            p.y = floor as f32;
        } else {
            p.z += braid.wind as f32;
        }
        for &(centre, radius) in &spheres {
            let d = p - centre;
            let distance2 = d.length_squared();
            if distance2 < radius * radius {
                let distance = distance2.sqrt().max(1.0);
                p = centre + (d * radius / distance).trunc();
            }
        }
        let previous = braid.points[i - 1];
        let d = p - previous;
        let yaw = atan(d.z, d.x);
        let pitch = -atan(Vec2::new(d.x, d.z).length(), d.y);
        braid.angles[i - 1] = (yaw, pitch);
        braid.points[i] = (previous + segment(lengths[i - 1], (yaw, pitch))).trunc();
        braid.velocity[i] = braid.points[i] - old;
    }
}

pub fn simulate(
    world: Option<Res<WorldGeometry>>,
    enabled: Option<Res<SimulationEnabled>>,
    mut players: Query<(&Player, &AnimationState, &mut Braid)>,
) {
    if enabled.is_some_and(|e| !e.0) {
        return;
    }
    let Some(world) = world else { return };
    for (player, animation, mut braid) in &mut players {
        step(&mut braid, &world, player, animation);
    }
}

/// Draw 0x800305ac: mesh j at point j, RotY then RotX.
pub fn pose_segments(
    players: Query<(&Braid, &Player)>,
    mut segments: Query<(&BraidSegment, &mut Transform, &mut InRoom)>,
) {
    let Ok((braid, player)) = players.single() else { return };
    for (segment, mut transform, mut room) in &mut segments {
        let p = braid.points[segment.0];
        let (yaw, pitch) = braid.angles[segment.0];
        transform.translation = position([p.x as i32, p.y as i32, p.z as i32]);
        transform.rotation = Quat::from_rotation_y(angle(yaw)) * Quat::from_rotation_x(-angle(pitch));
        room.0 = player.room;
    }
}
