//! Traps and other moving scenery. The original gives each type its own
//! control routine (the object table's +0x10) and collision routine
//! (+0x20); they are ported per type in the submodules and registered by
//! type id, over the item fields every object shares ([`ItemBody`]).
//!
//! A trap joins the active-item list when a trigger completes its code
//! and it is not already on it (`0x8001F2E4`); from then on its control
//! runs every tick, newest first, until the control takes it off again.
//! Its collision runs in Lara's object collision, for traps near her.
//! Evidence: `docs/trap-evidence.md`.
use bevy::prelude::*;
use engine::controller::{Button, ControllerState};
use engine::{
    animation::{AnimationCue, AnimationGoal, AnimationState, ObjectAnimation, ObjectAnimationCue},
    objects::{AppObjectTypes, Created, Initialized, ObjectType, WorldObject},
    random::GameRandom,
    schedule::{InteractionSystems, ItemSystems, simulation_enabled},
    triggers::{ObjectActivated, TriggerSystems},
    world::WorldGeometry,
};

use crate::contact::ContactPass;
use crate::creature::sync_transform;
use crate::items::{ItemBody, ItemStatus, animate_item};
use crate::player::{CollisionFrame, Motion, Player, PlayerAnimations, PlayerMode, Traversal, WeaponState};

pub mod blades;
pub mod boulder;
pub mod darts;
pub mod floors;
pub mod spikes;
pub mod zipline;

pub use crate::items::NO_HEIGHT;

/// A trap's place on the active-item list, and where it was placed.
#[derive(Component, Reflect, Debug, Clone, Default)]
#[reflect(Component)]
pub struct Trap {
    /// On the active-item list (item status bit 0): its control runs.
    pub listed: bool,
    /// Its placement, for traps that go back to it.
    pub start: [i32; 3],
    pub start_room: usize,
}

/// Lara as a trap sees her: the item fields its routines read.
#[derive(Clone, Copy, Debug)]
pub struct LaraItem {
    /// World units, y down.
    pub position: IVec3,
    pub yaw: i16,
    /// Forward speed and fall speed, world units a tick.
    pub speed: i32,
    pub fall_speed: i32,
    /// In the air (the item's gravity status).
    pub airborne: bool,
}

impl LaraItem {
    pub fn of(player: &Player, motion: &Motion) -> Self {
        let per_tick = |v: f32| (v * 1024.0 / engine::animation::ANIMATION_FPS).round() as i32;
        let forward = player.forward();
        let swimming = matches!(player.mode, PlayerMode::Swimming | PlayerMode::Surface);
        Self {
            position: crate::player::original_units(player.position),
            yaw: crate::player::binary_angle(player.yaw),
            speed: per_tick(motion.velocity.with_y(0.0).dot(forward)),
            fall_speed: per_tick(-motion.velocity.y),
            airborne: !motion.grounded && !swimming,
        }
    }
}

/// A new item a control routine asks for (CreateItem, InitialiseItem and
/// AddActiveItem): it appears with its type's first clip and runs from the
/// next tick.
#[derive(Clone, Copy, Debug)]
pub struct NewItem {
    pub kind: u16,
    pub position: [i32; 3],
    pub yaw: i16,
    pub room: usize,
}

/// What a trap type's control routine works on for one trap.
pub struct TrapContext<'a> {
    pub world: &'a mut WorldGeometry,
    pub random: &'a mut GameRandom,
    pub player: &'a mut Player,
    pub lara: LaraItem,
    pub object: &'a mut WorldObject,
    pub trap: &'a mut Trap,
    pub body: &'a mut ItemBody,
    pub animation: &'a mut ObjectAnimation,
    pub goal: &'a mut AnimationGoal,
    pub cues: &'a mut Vec<AnimationCue>,
    pub created: &'a mut Vec<NewItem>,
}

impl TrapContext<'_> {
    /// AnimateItem `0x8001DD98`.
    pub fn animate(&mut self) {
        self.cues.extend(animate_item(&self.world.data, self.body, self.animation, self.goal));
    }

    /// TriggerActive `0x8001F6C8`.
    pub fn trigger_active(&mut self) -> bool {
        self.object.trigger_active()
    }

    /// RemoveActiveItem `0x80037988`: its control stops until a trigger
    /// adds it back.
    pub fn remove(&mut self) {
        self.trap.listed = false;
    }

    /// KillItem `0x8003749C`: out of the level for good.
    pub fn kill(&mut self) {
        self.trap.listed = false;
        self.body.removed = true;
        self.object.consumed = true;
    }

    /// GetFloor and GetHeight at the trap's position, moving it into the
    /// room found (ItemNewRoom); returns the floor height.
    pub fn settle_room(&mut self) -> i32 {
        let [x, y, z] = self.body.position;
        if let Some(room) = self.world.floor_room(x, y, z, self.body.room) {
            self.body.room = room;
            self.object.room = room;
        }
        self.height_at(x, y, z)
    }

    /// GetHeight at a point, starting from the trap's room.
    pub fn height_at(&self, x: i32, y: i32, z: i32) -> i32 {
        let room = self.world.floor_room(x, y, z, self.body.room).unwrap_or(self.body.room);
        self.world.floor_height(x, y, z, room).unwrap_or(NO_HEIGHT)
    }

    /// Lara loses `amount` hit points; her hit flag is not ported.
    pub fn hurt_lara(&mut self, amount: i32) {
        self.player.damage(amount);
    }
}

/// A trap type's control routine.
pub type TrapControlFn = fn(&mut TrapContext);

/// What a trap type's collision routine works on: the trap and Lara's
/// side of the object collision pass.
pub struct TrapCollision<'a> {
    pub world: &'a WorldGeometry,
    pub random: &'a mut GameRandom,
    pub player: &'a mut Player,
    pub lara: LaraItem,
    pub pose: &'a mut AnimationState,
    pub motion: &'a mut Motion,
    pub(crate) collision: &'a mut CollisionFrame,
    pub(crate) pass: &'a mut ContactPass,
    pub weapon: &'a mut WeaponState,
    pub traversal: &'a mut Traversal,
    pub clips: Option<&'a PlayerAnimations>,
    /// Action held this tick.
    pub action: bool,
    pub object: &'a mut WorldObject,
    pub trap: &'a Trap,
    pub body: &'a mut ItemBody,
    pub transform: &'a Transform,
    pub animation: &'a ObjectAnimation,
    /// Set by the routine to add the trap to the active-item list
    /// (AddActiveItem), unless it is already there.
    pub add_active: bool,
}

impl TrapCollision<'_> {
    /// TestBoundsCollide `0x8001CCA4` and TestCollision `0x8005B990`, with
    /// ItemPushLara `0x8001C890` (big push) when `push`: the trap's meshes
    /// touching Lara, which also become its touch bits.
    pub fn touch(&mut self, push: bool) -> u32 {
        let mask = self.pass.touch(
            self.world,
            self.player,
            self.pose,
            self.collision,
            self.object.kind,
            self.transform,
            self.animation,
            &[],
            push,
        );
        // Only a passing coarse test rewrites them; AnimateItem clears them.
        if mask != 0 {
            self.body.touch_bits = mask;
        }
        mask
    }

    /// TrapCollision `0x8001C65C`, shared by many traps: touch, and push
    /// Lara out (her collision allows pushes whenever she is not using
    /// something, the case the port leaves out).
    pub fn generic(&mut self) {
        self.touch(true);
    }
}

/// A trap type's collision routine.
pub type TrapCollisionFn = fn(&mut TrapCollision);

/// A trap type's own InitialiseItem extra: it may start in another clip.
pub type TrapInitFn = fn(&WorldGeometry, u16, &mut ObjectAnimation, &mut ItemBody);

/// A trap type's routines, as the original's object table holds them.
#[derive(Clone, Copy)]
pub struct TrapType {
    pub name: &'static str,
    pub initialise: Option<TrapInitFn>,
    pub control: Option<TrapControlFn>,
    pub collision: Option<TrapCollisionFn>,
}

impl TrapType {
    pub fn new(name: &'static str) -> Self {
        Self { name, initialise: None, control: None, collision: None }
    }

    pub fn with_initialise(mut self, initialise: TrapInitFn) -> Self {
        self.initialise = Some(initialise);
        self
    }

    pub fn with_control(mut self, control: TrapControlFn) -> Self {
        self.control = Some(control);
        self
    }

    pub fn with_collision(mut self, collision: TrapCollisionFn) -> Self {
        self.collision = Some(collision);
        self
    }
}

/// The routines of each trap type, by object type id.
#[derive(Resource, Default)]
pub struct TrapTypes(std::collections::BTreeMap<u16, TrapType>);

/// Traps on the active-item list, in update order (newest first).
#[derive(Resource, Reflect, Default, Debug, Clone)]
#[reflect(Resource)]
pub struct ActiveTraps(pub Vec<Entity>);

/// Registering a trap type.
pub trait AppTraps {
    /// Panics if `kind` is already registered.
    fn register_trap(&mut self, kind: u16, trap: TrapType) -> &mut Self;
}

impl AppTraps for App {
    fn register_trap(&mut self, kind: u16, trap: TrapType) -> &mut Self {
        engine::add_once(self, TrapRulesPlugin);
        if self.world_mut().resource_mut::<TrapTypes>().0.insert(kind, trap).is_some() {
            panic!("trap type {kind} registered twice");
        }
        let initialise = trap.initialise;
        self.register_object_type(
            kind,
            ObjectType::new(trap.name)
                .on_init(move |entity, object, world| init_trap(entity, object, world, initialise)),
        );
        self
    }
}

/// InitialiseItem `0x80037690` for a placed trap: its pose from the
/// placement, the first clip of its model in that clip's state, then its
/// type's own initialise routine.
fn init_trap(
    entity: &mut bevy::ecs::system::EntityCommands,
    object: &WorldObject,
    world: Option<&WorldGeometry>,
    initialise: Option<TrapInitFn>,
) {
    let Some(world) = world else { return };
    let Some(placement) = world.data.placements.get(object.index) else { return };
    let mut body = item_body(world, object.kind, placement.position, placement.rotation as i16, object.room);
    let mut animation = first_clip(world, object.kind);
    if let Some(initialise) = initialise {
        initialise(world, object.kind, &mut animation, &mut body);
    }
    entity.insert((
        Trap { listed: false, start: placement.position, start_room: object.room },
        AnimationGoal { state: body.current_state },
        body,
        animation,
        engine::animation::ControllerAnimated,
    ));
}

fn first_clip(world: &WorldGeometry, kind: u16) -> ObjectAnimation {
    ObjectAnimation { animation: world.data.model(kind).map_or(usize::MAX, |m| m.animations[0]), frame: 0.0 }
}

fn item_body(world: &WorldGeometry, kind: u16, position: [i32; 3], yaw: i16, room: usize) -> ItemBody {
    let state =
        world.data.model(kind).and_then(|m| world.data.animations.get(m.animations[0])).map_or(0, |clip| clip.state);
    let floor = world.floor_height(position[0], position[1], position[2], room).unwrap_or(NO_HEIGHT);
    ItemBody { position, yaw, room, current_state: state, collidable: true, floor, ..default() }
}

/// The trap's clip `offset` from its model's first, from that clip's
/// first frame, in its state (what controllers that switch clips write).
pub fn set_clip(world: &WorldGeometry, kind: u16, offset: usize, animation: &mut ObjectAnimation, body: &mut ItemBody) {
    let Some(model) = world.data.model(kind) else { return };
    animation.animation = model.animations[0] + offset;
    animation.frame = 0.0;
    body.current_state = world.data.animations.get(animation.animation).map_or(0, |c| c.state);
}

/// A trigger completing a trap's code adds it to the front of the
/// active-item list unless it is already there, active, with no touches.
fn activate(
    mut activated: MessageReader<ObjectActivated>,
    mut active: ResMut<ActiveTraps>,
    mut traps: Query<(&mut Trap, &mut ItemBody)>,
) {
    for event in activated.read() {
        let Ok((mut trap, mut body)) = traps.get_mut(event.entity) else { continue };
        if trap.listed {
            continue;
        }
        trap.listed = true;
        body.status = ItemStatus::Active;
        body.touch_bits = 0;
        active.0.retain(|&e| e != event.entity);
        active.0.insert(0, event.entity);
    }
}

/// The control phase for traps: each listed trap's control routine, in
/// list order, then items it created join the front of the list.
#[allow(clippy::type_complexity)]
fn control(
    mut commands: Commands,
    world: Option<ResMut<WorldGeometry>>,
    types: Res<TrapTypes>,
    mut active: ResMut<ActiveTraps>,
    mut random: ResMut<GameRandom>,
    mut players: Query<(&mut Player, &Motion)>,
    mut traps: Query<
        (&mut WorldObject, &mut Trap, &mut ItemBody, &mut ObjectAnimation, &mut AnimationGoal, &mut Transform),
        Without<Player>,
    >,
    mut writer: MessageWriter<ObjectAnimationCue>,
) {
    let Some(mut world) = world else { return };
    let Ok((mut player, motion)) = players.single_mut() else { return };
    let mut created = Vec::new();
    for entity in active.0.clone() {
        let Ok((mut object, mut trap, mut body, mut animation, mut goal, mut transform)) = traps.get_mut(entity) else {
            continue;
        };
        let Some(control) = types.0.get(&object.kind).and_then(|t| t.control) else { continue };
        let lara = LaraItem::of(&player, motion);
        let mut cues = Vec::new();
        let mut context = TrapContext {
            world: &mut world,
            random: &mut random,
            player: &mut player,
            lara,
            object: &mut object,
            trap: &mut trap,
            body: &mut body,
            animation: &mut animation,
            goal: &mut goal,
            cues: &mut cues,
            created: &mut created,
        };
        control(&mut context);
        sync_transform(&body, &mut transform);
        crate::creature::emit_cues(entity, cues, &mut writer);
    }
    active.0.retain(|&e| traps.get(e).is_ok_and(|(_, trap, ..)| trap.listed));
    for item in created {
        let mut body = item_body(&world, item.kind, item.position, item.yaw, item.room);
        body.status = ItemStatus::Active;
        let animation = first_clip(&world, item.kind);
        let mut transform = Transform::default();
        sync_transform(&body, &mut transform);
        let entity = commands
            .spawn((
                WorldObject::new(usize::MAX, item.kind, item.room, 0),
                Created,
                Initialized,
                Trap { listed: true, start: item.position, start_room: item.room },
                AnimationGoal { state: body.current_state },
                body,
                animation,
                transform,
                engine::animation::ControllerAnimated,
            ))
            .id();
        active.0.insert(0, entity);
    }
}

/// Lara's object collision for traps: each collidable, visible trap's
/// collision routine.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn collide(
    world: Option<Res<WorldGeometry>>,
    types: Res<TrapTypes>,
    pad: Res<ControllerState>,
    clips: Option<Res<PlayerAnimations>>,
    mut random: ResMut<GameRandom>,
    mut active: ResMut<ActiveTraps>,
    mut players: Query<(
        &mut Player,
        &mut Motion,
        &mut AnimationState,
        &mut CollisionFrame,
        &mut WeaponState,
        &mut Traversal,
        Option<&engine::animation::PoseOverrides>,
    )>,
    mut traps: Query<
        (Entity, &mut WorldObject, &mut Trap, &mut ItemBody, &Transform, &ObjectAnimation),
        Without<Player>,
    >,
) {
    let Some(world) = world else { return };
    let Ok((mut player, mut motion, mut pose, mut collision, mut weapon, mut traversal, aim)) = players.single_mut()
    else {
        return;
    };
    if matches!(player.mode, PlayerMode::Using { .. }) {
        return;
    }
    let aim = aim.map_or(&[][..], |a| a.0.as_slice());
    let Some(mut pass) = ContactPass::begin(&world, &player, &pose, &collision, aim) else { return };
    for (entity, mut object, mut trap, mut body, transform, animation) in &mut traps {
        let Some(collision_fn) = types.0.get(&object.kind).and_then(|t| t.collision) else { continue };
        if !body.collidable || body.removed || object.consumed || body.status == ItemStatus::Invisible {
            continue;
        }
        let lara = LaraItem::of(&player, &motion);
        let mut context = TrapCollision {
            world: &world,
            random: &mut random,
            player: &mut player,
            lara,
            pose: &mut pose,
            motion: &mut motion,
            collision: &mut collision,
            pass: &mut pass,
            weapon: &mut weapon,
            traversal: &mut traversal,
            clips: clips.as_deref(),
            action: pad.held(Button::Cross),
            object: &mut object,
            trap: &trap,
            body: &mut body,
            transform,
            animation,
            add_active: false,
        };
        collision_fn(&mut context);
        if context.add_active && !trap.listed {
            trap.listed = true;
            active.0.retain(|&e| e != entity);
            active.0.insert(0, entity);
        }
    }
}

/// The shared trap systems; each type's module registers its routines.
pub struct TrapRulesPlugin;

impl Plugin for TrapRulesPlugin {
    fn build(&self, app: &mut App) {
        engine::add_once(app, engine::EngineRulesPlugin);
        app.init_resource::<GameRandom>().init_resource::<TrapTypes>().init_resource::<ActiveTraps>();
        app.register_type::<Trap>().register_type::<ActiveTraps>().add_systems(
            FixedUpdate,
            (
                control.in_set(ItemSystems::Control),
                activate.in_set(InteractionSystems::Triggers).after(TriggerSystems::Dispatch),
                collide.in_set(InteractionSystems::Resolve),
            )
                .run_if(simulation_enabled),
        );
    }
}

/// Every trap type of the game.
pub struct TrapsPlugin;

impl Plugin for TrapsPlugin {
    fn build(&self, app: &mut App) {
        floors::register(app);
        darts::register(app);
        blades::register(app);
        spikes::register(app);
        boulder::register(app);
        zipline::register(app);
    }
}
