//! Home's attendant: authored local follow, turn, start and stop.
//!
//! State decisions and free-space motion come from PSX0x800500A0 and
//! 0x800166F8. Navigation between boxes and obstacle avoidance remain pending;
//! the current target is Lara's position, verified for the recorded local route.
use bevy::prelude::*;
use engine::{
    animation::{AnimationGoal, ObjectAnimation},
    camera::chase::{cos, heading, sin},
    objects::WorldObject,
    schedule::{ItemSystems, WorldSystems, simulation_enabled},
    world::WorldGeometry,
};

use crate::player::Player;

pub const KIND: u16 = 260;

/// Moddable local follow thresholds. The authored clip table supplies speeds.
#[derive(Resource, Reflect, serde::Deserialize, Debug, Clone)]
#[reflect(Resource)]
pub struct WinstonTuning {
    pub follow_distance: i32,
    /// Creature slot initialization0x80048F40:182 angle units/tick.
    pub max_turn: i32,
}

#[derive(Component, Reflect, Debug, Clone, Default)]
#[reflect(Component)]
pub struct Winston {
    /// Previous authored movement speed, in source units per 30Hz tick.
    pub speed: i32,
}

pub struct WinstonRulesPlugin;

impl Plugin for WinstonRulesPlugin {
    fn build(&self, app: &mut App) {
        engine::add_once(app, engine::EngineRulesPlugin);
        app.insert_resource(crate::interactions::winston_tuning());
        app.register_type::<Winston>().add_systems(
            FixedUpdate,
            (
                follow.in_set(ItemSystems::Control),
                move_authored.in_set(WorldSystems::Items).after(ItemSystems::Animation),
            )
                .run_if(simulation_enabled),
        );
    }
}

fn yaw(transform: &Transform) -> i32 {
    (transform.rotation.to_euler(EulerRot::YXZ).0 * 65536.0 / std::f32::consts::TAU).round() as i32 as i16 as i32
}

fn follow(
    world: Option<Res<WorldGeometry>>,
    tuning: Res<WinstonTuning>,
    players: Query<&Player>,
    mut attendants: Query<
        (&Winston, &WorldObject, &mut Transform, &ObjectAnimation, &mut AnimationGoal),
        Without<Player>,
    >,
) {
    let (Some(world), Ok(player)) = (world, players.single()) else { return };
    for (actor, object, mut transform, animation, mut goal) in &mut attendants {
        if object.consumed {
            continue;
        }
        let delta = ((player.position - transform.translation) * 1024.0).round();
        let (dx, dz) = (delta.x as i32, delta.z as i32);
        let distance = i64::from(dx).pow(2) + i64::from(dz).pow(2);
        let old_yaw = yaw(&transform);
        let angle = (heading(dx, dz) - old_yaw) as i16 as i32;
        let ahead = angle.abs() < 16384;
        // Awareness is evaluated before turning. Moving starts beyond1536
        // or when Lara is behind; stopping requires strictly less1536 ahead.
        let Some(clip) = world.data.animations.get(animation.animation) else { continue };
        if clip.state == 1 {
            if distance > i64::from(tuning.follow_distance).pow(2) || !ahead {
                goal.state = 2;
            }
        } else if distance < i64::from(tuning.follow_distance).pow(2) && ahead {
            goal.state = 1;
        }
        if actor.speed == 0 || tuning.max_turn == 0 {
            continue;
        }
        let mut cap = tuning.max_turn;
        let turn_radius = actor.speed * 16384 / cap;
        if angle.abs() > 16384 && distance < i64::from(turn_radius).pow(2) {
            cap /= 2;
        }
        let next = (old_yaw + angle.clamp(-cap, cap)) as i16;
        transform.rotation = Quat::from_rotation_y(f32::from(next) * std::f32::consts::TAU / 65536.0);
    }
}

fn move_authored(
    world: Option<Res<WorldGeometry>>,
    mut attendants: Query<(&mut Winston, &mut WorldObject, &mut Transform, &ObjectAnimation)>,
) {
    let Some(world) = world else { return };
    for (mut actor, mut object, mut transform, animation) in &mut attendants {
        if object.consumed {
            continue;
        }
        let Some(clip) = world.data.animations.get(animation.animation) else { continue };
        actor.speed = ((i64::from(clip.speed) + i64::from(clip.accel) * animation.frame.floor() as i64) >> 16) as i32;
        let yaw = yaw(&transform);
        let delta = Vec3::new(((sin(yaw) * actor.speed) >> 14) as f32, 0.0, ((cos(yaw) * actor.speed) >> 14) as f32);
        let proposed = transform.translation + delta / 1024.0;
        // The measured local paths stay on one flat floor. Keep unverified
        // stair/ledge navigation bounded rather than walking through geometry.
        if let Some(hit) = world.floor_at(proposed, object.room)
            && (hit.height - transform.translation.y).abs() < 0.0001
        {
            transform.translation = proposed;
            object.room = hit.room;
        }
    }
}
