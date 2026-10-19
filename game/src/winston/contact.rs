//! Animated box/sphere contact from PSX0x8001C65C,0x8001CCA4,0x8005B990.
//! The expanded authored box chooses a push only after posed spheres overlap.
//! Bone transforms use ordinary Bevy math; GTE rounding is bounded by fixtures.
use super::{Winston, yaw};
use crate::player::{PLAYER_HEIGHT, PLAYER_RADIUS, Player};
use bevy::prelude::*;
use engine::{
    animation::{AnimationState, ObjectAnimation, collision_bounds, model_spheres},
    camera::chase::{cos, sin},
    objects::WorldObject,
    schedule::{InteractionSystems, WorldSystems, simulation_enabled},
    world::{WorldGeometry, units},
};

#[derive(Resource, Default)]
struct ContactOrigin(Option<Vec3>);

pub(super) fn install(app: &mut App) {
    app.init_resource::<ContactOrigin>().add_systems(
        FixedUpdate,
        (remember.in_set(WorldSystems::Control), push.in_set(InteractionSystems::Resolve)).run_if(simulation_enabled),
    );
}

fn remember(players: Query<&Player>, mut origin: ResMut<ContactOrigin>) {
    origin.0 = players.single().ok().map(|p| p.position);
}

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

fn push(
    world: Option<Res<WorldGeometry>>,
    origin: Res<ContactOrigin>,
    mut players: Query<(&mut Player, &AnimationState)>,
    mut attendants: Query<(&mut Winston, &WorldObject, &Transform, &ObjectAnimation), Without<Player>>,
) {
    let (Some(world), Ok((mut player, pose))) = (world, players.single_mut()) else { return };
    if player.health <= 0 {
        return;
    }
    let Some(avatar_model) = world.data.model(crate::player::LARA) else { return };
    let Some(avatar_bounds) = collision_bounds(&world.data, pose.animation, pose.frame) else { return };
    let mut accepted_origin = origin.0.unwrap_or(player.position);
    for (mut actor, object, transform, animation) in &mut attendants {
        if object.consumed || (player.position - transform.translation).length_squared() > 16.0 {
            continue;
        }
        let Some(bounds) = collision_bounds(&world.data, animation.animation, animation.frame) else { continue };
        let (actor_y, avatar_y) = (-transform.translation.y * 1024.0, -player.position.y * 1024.0);
        if avatar_y + avatar_bounds[2] as f32 >= actor_y + bounds[3] as f32
            || avatar_y + avatar_bounds[3] as f32 <= actor_y + bounds[2] as f32
        {
            continue;
        }
        let angle = yaw(transform);
        let mut local = local_xz(player.position - transform.translation, angle);
        // Lara's ordinary collision context radius is100 at0x800381E8.
        let radius = (PLAYER_RADIUS * 1024.0) as i32;
        let edges = [bounds[0] - radius, bounds[1] + radius, bounds[4] - radius, bounds[5] + radius];
        if !(edges[0]..=edges[1]).contains(&local[0]) || !(edges[2]..=edges[3]).contains(&local[1]) {
            continue;
        }
        let Some(model) = world.data.model(object.kind) else { continue };
        let obstacle = model_spheres(&world.data, model, animation.animation, animation.frame, transform);
        let avatar_root = Transform::from_translation(player.position).with_rotation(Quat::from_rotation_y(player.yaw));
        let avatar = model_spheres(&world.data, avatar_model, pose.animation, pose.frame, &avatar_root);
        actor.contact_mask = obstacle.iter().filter(|a| a.joint < 32).fold(0, |mask, a| {
            if avatar.iter().any(|b| a.center.distance_squared(b.center) < (a.radius + b.radius).powi(2)) {
                mask | (1 << a.joint)
            } else {
                mask
            }
        });
        if actor.contact_mask == 0 {
            continue;
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
        // The source validates a762-unit body with at most384 units upward
        // floor correction (0x8001CBC8..0x8001CC44). Flat open contact is
        // runtime-verified; wall/static/step interactions need further captures.
        let clear = [
            Vec3::ZERO,
            Vec3::X * PLAYER_RADIUS,
            -Vec3::X * PLAYER_RADIUS,
            Vec3::Z * PLAYER_RADIUS,
            -Vec3::Z * PLAYER_RADIUS,
        ]
        .into_iter()
        .all(|d| {
            world.floor_at(candidate + d, player.room).is_some_and(|floor| {
                floor.height <= candidate.y + units(384.0) && floor.ceiling >= candidate.y + PLAYER_HEIGHT
            })
        });
        if clear {
            player.position = candidate;
            accepted_origin = candidate;
            if let Some(floor) = world.floor_at(candidate + Vec3::Y * units(10.0), player.room) {
                player.room = floor.room;
            }
        } else {
            player.position.x = accepted_origin.x;
            player.position.z = accepted_origin.z;
        }
    }
}
