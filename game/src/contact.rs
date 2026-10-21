//! Lara against an animated object: the collision callback every creature
//! and the attendant share. Coarse authored bounds first (TestBoundsCollide
//! `0x8001CCA4`), then posed mesh spheres (TestCollision `0x8005B990`),
//! whose overlap mask becomes the object's mesh contacts, then the
//! expanded-box push (ItemPushLara `0x8001C890`). Callers: the attendant's
//! `0x8001C65C` and CreatureCollision `0x8001C598`.
//!
//! ItemPushLara's stagger option for creatures also records a hit direction
//! and frame for Lara's flinch and plays her grunt (`0x8001CB24..0x8001CBC0`);
//! that presentation is not ported. Positions and pushes are.
use crate::player::{CollisionFrame, PLAYER_RADIUS, Player, item_push_is_clear};
use bevy::prelude::*;
use engine::{
    animation::{AnimationState, JointPose, ObjectAnimation, collision_bounds, model_spheres_with},
    camera::chase::{cos, sin},
    world::{WorldGeometry, units},
};

// Products are added before the signed shift: rounding each separately
// changes the expanded edge by a visible unit (0x8001CD48..0x8001CD98).
fn local_xz(delta: Vec3, angle: i32) -> [i32; 2] {
    let (x, z) = ((delta.x * 1024.0).round() as i32, (delta.z * 1024.0).round() as i32);
    let (s, c) = (sin(angle), cos(angle));
    [((c * x - s * z) >> 14), ((c * z + s * x) >> 14)]
}

fn world_xz(local: [i32; 2], angle: i32) -> Vec3 {
    let [x, z] = local;
    let (s, c) = (sin(angle), cos(angle));
    Vec3::new(units(((c * x + s * z) >> 14) as f32), 0.0, units(((c * z - s * x) >> 14) as f32))
}

/// Lara's side of one collision pass: her pose, and the origin her pushes
/// are validated from.
pub(crate) struct ContactPass {
    avatar_bounds: [i32; 6],
    accepted_origin: Vec3,
    /// Lara's own extra joint turns (her aim), as her spheres are posed.
    avatar_overrides: Vec<JointPose>,
}

impl ContactPass {
    /// `None` when Lara is dead or has no model or bounds.
    pub(crate) fn begin(
        world: &WorldGeometry,
        player: &Player,
        pose: &AnimationState,
        collision: &CollisionFrame,
        avatar_overrides: &[JointPose],
    ) -> Option<Self> {
        if player.health <= 0 {
            return None;
        }
        world.data.model(crate::player::LARA)?;
        let avatar_bounds = collision_bounds(&world.data, pose.animation, pose.frame)?;
        Some(Self { avatar_bounds, accepted_origin: collision.origin, avatar_overrides: avatar_overrides.to_vec() })
    }

    /// Test one object of type `kind` and, with `push`, push Lara out of it
    /// when their spheres touch. `overrides` are the object's extra joint
    /// turns (a creature's head). Returns the object's meshes that touched
    /// her (0 for none, or when the coarse tests fail).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn touch(
        &mut self,
        world: &WorldGeometry,
        player: &mut Player,
        pose: &AnimationState,
        collision: &mut CollisionFrame,
        kind: u16,
        transform: &Transform,
        animation: &ObjectAnimation,
        overrides: &[JointPose],
        push: bool,
    ) -> u32 {
        if (player.position - transform.translation).length_squared() > 16.0 {
            return 0;
        }
        let Some(bounds) = collision_bounds(&world.data, animation.animation, animation.frame) else { return 0 };
        let (actor_y, avatar_y) = (-transform.translation.y * 1024.0, -player.position.y * 1024.0);
        if avatar_y + self.avatar_bounds[2] as f32 >= actor_y + bounds[3] as f32
            || avatar_y + self.avatar_bounds[3] as f32 <= actor_y + bounds[2] as f32
        {
            return 0;
        }
        let angle = (transform.rotation.to_euler(EulerRot::YXZ).0 * 65536.0 / std::f32::consts::TAU).round() as i32
            as i16 as i32;
        let mut local = local_xz(player.position - transform.translation, angle);
        // Lara's ordinary collision context radius is 100 at 0x800381E8.
        let radius = (PLAYER_RADIUS * 1024.0) as i32;
        let edges = [bounds[0] - radius, bounds[1] + radius, bounds[4] - radius, bounds[5] + radius];
        if !(edges[0]..=edges[1]).contains(&local[0]) || !(edges[2]..=edges[3]).contains(&local[1]) {
            return 0;
        }
        let (Some(model), Some(avatar_model)) = (world.data.model(kind), world.data.model(crate::player::LARA)) else {
            return 0;
        };
        let obstacle =
            model_spheres_with(&world.data, model, animation.animation, animation.frame, transform, overrides);
        let avatar_root = Transform::from_translation(player.position).with_rotation(Quat::from_rotation_y(player.yaw));
        let avatar = model_spheres_with(
            &world.data,
            avatar_model,
            pose.animation,
            pose.frame,
            &avatar_root,
            &self.avatar_overrides,
        );
        let mask = obstacle.iter().filter(|a| a.joint < 32).fold(0, |mask, a| {
            if avatar.iter().any(|b| a.center.distance_squared(b.center) < (a.radius + b.radius).powi(2)) {
                mask | (1 << a.joint)
            } else {
                mask
            }
        });
        if mask == 0 || !push {
            return mask;
        }
        // Exact tie priority: minX,maxX,maxZ,minZ (0x8001C9AC..0x8001CA30).
        let distances = [local[0] - edges[0], edges[1] - local[0], edges[3] - local[1], local[1] - edges[2]];
        let edge = (0..4).min_by_key(|&i| distances[i]).unwrap();
        match edge {
            0 => local[0] = edges[0],
            1 => local[0] = edges[1],
            2 => local[1] = edges[3],
            _ => local[1] = edges[2],
        }
        let candidate = (transform.translation + world_xz(local, angle)).with_y(player.position.y);
        // The native validator faces the push from the last accepted origin;
        // axis-only probes miss raised geometry at a leading body corner.
        if item_push_is_clear(world, candidate, self.accepted_origin, player.room) {
            player.position = candidate;
            self.accepted_origin = candidate;
            collision.origin = candidate;
            if let Some(floor) = world.floor_at(candidate + Vec3::Y * units(10.0), player.room) {
                player.room = floor.room;
            }
        } else {
            player.position.x = self.accepted_origin.x;
            player.position.z = self.accepted_origin.z;
        }
        mask
    }
}
