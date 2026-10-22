//! The attendant's collision callback (PSX `0x8001C65C`): the shared
//! animated-object contact in [`crate::contact`], without stagger. Unlike
//! CreatureCollision it runs whether or not he is active yet.
use crate::contact::ContactPass;
use crate::creature::{ItemBody, ItemStatus};
use crate::player::{CollisionFrame, Player};
use bevy::prelude::*;
use engine::{
    animation::{AnimationState, ObjectAnimation, PoseOverrides},
    objects::WorldObject,
    schedule::{InteractionSystems, simulation_enabled},
    world::WorldGeometry,
};

pub(super) fn install(app: &mut App) {
    app.add_systems(FixedUpdate, push.in_set(InteractionSystems::Resolve).run_if(simulation_enabled));
}

fn push(
    world: Option<Res<WorldGeometry>>,
    mut players: Query<(&mut Player, &AnimationState, &mut CollisionFrame, Option<&PoseOverrides>)>,
    mut attendants: Query<(&mut ItemBody, &WorldObject, &Transform, &ObjectAnimation), Without<Player>>,
) {
    let (Some(world), Ok((mut player, pose, mut collision, aim))) = (world, players.single_mut()) else { return };
    let aim = aim.map_or(&[][..], |a| a.0.as_slice());
    let Some(mut pass) = ContactPass::begin(&world, &player, pose, &collision, aim) else { return };
    for (mut body, object, transform, animation) in &mut attendants {
        if object.kind != super::KIND || object.consumed || !body.collidable || body.status == ItemStatus::Invisible {
            continue;
        }
        let mask = pass.touch(&world, &mut player, pose, &mut collision, object.kind, transform, animation, &[], true);
        if mask != 0 {
            // AnimateItem clears them at his next step (0x8001DDC8).
            body.touch_bits = mask;
        }
    }
}
