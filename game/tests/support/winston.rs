//! Home's attendant placed as a recording has him, on the shared creature
//! components: active, holding an AI slot whose zone is built from where he
//! stands (InitialiseSlot), bored until his first mood update.
use bevy::prelude::*;
use engine::{
    animation::{AnimationGoal, ControllerAnimated, ObjectAnimation},
    objects::{Initialized, WorldObject},
    world::{BoxFlags, WorldGeometry},
};
use game::{
    creature::{self, ActiveCreatures, ItemBody, ItemStatus, OwnCollision, spawn_components},
    interactions::Enemy,
    winston,
};

/// Winston's recorded item fields.
pub struct Pose {
    pub position: [i32; 3],
    pub yaw: i16,
    pub room: usize,
    pub state: usize,
    pub goal: usize,
    /// Absolute frame of `animation`.
    pub animation: usize,
    pub frame: usize,
    pub speed: i16,
    pub touch_bits: u32,
}

/// Spawn Winston (placement 9) at `pose`, first on the active list, or
/// take over `entity` (a placed Winston not yet initialised).
pub fn place(app: &mut App, world: &WorldGeometry, pose: &Pose, entity: Option<Entity>) -> Entity {
    let object = WorldObject { room: pose.room, active: true, ..WorldObject::new(9, winston::KIND, pose.room, 0) };
    let (mut creature, mut body) = spawn_components(world, &object, Enemy::setup_for_kind(winston::KIND).unwrap());
    body = ItemBody {
        position: pose.position,
        yaw: pose.yaw,
        room: pose.room,
        current_state: pose.state,
        speed: pose.speed,
        touch_bits: pose.touch_bits,
        status: ItemStatus::Active,
        ..body
    };
    body.floor = world.floor_height(pose.position[0], pose.position[1], pose.position[2], pose.room).unwrap_or(0);
    creature::enable(world, &mut creature, &mut body);
    // The recordings start long after his slot was enabled: it has a target
    // box (his own, as an unaimed slot takes) and its search is complete.
    let search = creature.search.as_mut().unwrap();
    search.required_box = body.box_number;
    while search.update(&world.data, &BoxFlags::new(&world.data), false, 1) {}
    let mut transform = Transform::default();
    creature::sync_transform(&body, &mut transform);
    let start = world.data.animations[pose.animation].frame_start;
    let components = (
        object,
        creature,
        body,
        Enemy::for_kind(winston::KIND).unwrap(),
        OwnCollision,
        Initialized,
        ControllerAnimated,
        transform,
        ObjectAnimation { animation: pose.animation, frame: (pose.frame - start) as f32 },
        AnimationGoal { state: pose.goal },
    );
    let entity = match entity {
        Some(entity) => app.world_mut().entity_mut(entity).insert(components).id(),
        None => app.world_mut().spawn(components).id(),
    };
    app.world_mut().get_resource_or_init::<ActiveCreatures>().0.insert(0, entity);
    entity
}
