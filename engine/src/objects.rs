//! Placed objects and the object type registry.
//!
//! Every placement in a level becomes an entity. What an object *is* (a
//! pickup, a door, a creature, the player) is not engine knowledge: games
//! register each object type id with an [`ObjectType`], whose hooks add the
//! game's components when the object is spawned or first ticks.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use bevy::{ecs::system::EntityCommands, prelude::*};

use crate::psx::PsxMaterial;
use crate::schedule::{LifecycleSystems, simulation_enabled};
use crate::shell::GameSystems;
use crate::world::{Placement, WorldGeometry};

/// Activation code bits of an object's flag word (one bit per trigger mask
/// slot). An object is active when all five are set, unless it is inverted.
pub const CODE_BITS: u16 = 0x3e00;
/// Inverted activation: active while the code is *not* complete.
pub const INVERTED: u16 = 0x4000;
/// Not drawn until activated.
pub const INVISIBLE: u16 = 0x0100;

/// A placed object and its activation state.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
#[require(Transform)]
pub struct WorldObject {
    /// Index of the placement in the level; triggers address objects by it.
    pub index: usize,
    /// Object type id (see [`ObjectTypes`]).
    pub kind: u16,
    pub room: usize,
    pub flags: u16,
    /// The code is complete (or, inverted, not): what a trigger last left.
    pub active: bool,
    /// Ticks the trigger that last named the object holds it for: 0 for no
    /// limit, -1 once run out. Only [`WorldObject::trigger_active`] counts
    /// it, so it runs while the object's controller asks.
    pub timer: i16,
    /// Collected or otherwise removed from play.
    pub consumed: bool,
}

impl WorldObject {
    pub fn new(index: usize, kind: u16, room: usize, flags: u16) -> Self {
        // A complete authored code means "start active": it becomes an
        // inverted object with the code cleared, so triggers deactivate it.
        let flags = if flags & CODE_BITS == CODE_BITS { (flags & !CODE_BITS) | INVERTED } else { flags };
        let mut object = Self { index, kind, room, flags, active: false, timer: 0, consumed: false };
        object.update_active();
        object
    }

    /// The object a level placement starts as.
    pub fn from_placement(index: usize, placement: &Placement) -> Self {
        let flags = placement.other_flags
            | u16::from(placement.code & 31) << 9
            | if placement.inverted { INVERTED } else { 0 }
            | if placement.invisible { INVISIBLE } else { 0 };
        Self::new(index, placement.kind, placement.room, flags)
    }

    /// Recompute `active` from the code bits and the inverted flag.
    pub fn update_active(&mut self) {
        self.active = (self.flags & CODE_BITS == CODE_BITS) ^ (self.flags & INVERTED != 0);
    }

    /// What a controller asks each tick (the original's TriggerActive): the
    /// code is complete and its timer, if any, has not run out. Each call
    /// counts the timer down a tick; the tick it reaches zero still answers
    /// yes and leaves -1. An inverted object reads the other way.
    pub fn trigger_active(&mut self) -> bool {
        let ok = self.flags & INVERTED == 0;
        if self.flags & CODE_BITS != CODE_BITS {
            return !ok;
        }
        match self.timer {
            0 => ok,
            -1 => !ok,
            t => {
                self.timer = if t == 1 { -1 } else { t - 1 };
                ok
            }
        }
    }
}

/// An object created in play rather than placed by the level (the
/// original's CreateItem), such as a fired dart. It has no placement; the
/// renderer draws it from its type's model.
#[derive(Component, Reflect, Debug, Clone, Copy, Default)]
#[reflect(Component)]
pub struct Created;

/// Added once an object's type hooks have run.
#[derive(Component)]
pub struct Initialized;

/// Adds components to a newly placed or newly spawned object of one type.
pub type InitHook = Arc<dyn Fn(&mut EntityCommands, &WorldObject, Option<&WorldGeometry>) + Send + Sync>;
/// Builds the entity for a placement instead of the default [`WorldObject`].
pub type SpawnHook = Arc<dyn Fn(&mut EntityCommands, &SpawnContext) + Send + Sync>;

/// What a spawn hook knows about the placement it builds.
pub struct SpawnContext<'a> {
    /// Placement index in the level.
    pub index: usize,
    pub placement: &'a Placement,
    pub world: &'a WorldGeometry,
    /// Parent of everything drawn for the level.
    pub render_root: Entity,
    /// One mesh per level mesh (`LevelData::meshes`).
    pub meshes: &'a [Handle<Mesh>],
    /// The lit material shared by this object's model, when it has one.
    pub lit_material: Option<Handle<PsxMaterial>>,
}

/// One object type: a name for tools and logs, and the game's hooks.
#[derive(Clone, Default)]
pub struct ObjectType {
    pub name: String,
    init: Vec<InitHook>,
    spawn: Option<SpawnHook>,
}

impl ObjectType {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..default() }
    }

    /// Run `hook` the first tick an object of this type exists.
    pub fn on_init(mut self, hook: impl Fn(&mut EntityCommands, &WorldObject, Option<&WorldGeometry>) + Send + Sync + 'static) -> Self {
        self.init.push(Arc::new(hook));
        self
    }

    /// Build placements of this type with `hook` instead of as a [`WorldObject`].
    pub fn on_spawn(mut self, hook: impl Fn(&mut EntityCommands, &SpawnContext) + Send + Sync + 'static) -> Self {
        self.spawn = Some(Arc::new(hook));
        self
    }

    pub fn spawn_hook(&self) -> Option<&SpawnHook> {
        self.spawn.as_ref()
    }
}

/// Object type id -> type. Unregistered types are plain [`WorldObject`]s.
#[derive(Resource, Default)]
pub struct ObjectTypes {
    types: BTreeMap<u16, ObjectType>,
}

impl ObjectTypes {
    /// Read just the portable names from a game's type table. Extra per-type
    /// controller settings are deliberately ignored by authoring tools.
    pub fn names_from_json(text: &str) -> Result<Self, serde_json::Error> {
        #[derive(serde::Deserialize)]
        struct Entry {
            id: u16,
            name: String,
        }
        #[derive(serde::Deserialize)]
        struct Table {
            types: Vec<Entry>,
        }
        let table: Table = serde_json::from_str(text)?;
        let mut types = Self::default();
        for entry in table.types {
            types.insert(entry.id, ObjectType::new(entry.name));
        }
        Ok(types)
    }

    pub fn insert(&mut self, kind: u16, object_type: ObjectType) {
        self.types.insert(kind, object_type);
    }

    pub fn get(&self, kind: u16) -> Option<&ObjectType> {
        self.types.get(&kind)
    }

    /// Metadata for authoring tools; hooks remain owned by the running game.
    pub fn iter(&self) -> impl Iterator<Item = (u16, &ObjectType)> {
        self.types.iter().map(|(&id, info)| (id, info))
    }
}

/// Object type ids placed in the level that no game registered.
#[derive(Resource, Reflect, Default, Debug)]
#[reflect(Resource)]
pub struct ObjectDiagnostics {
    pub unregistered_types: BTreeSet<u16>,
}

/// Registering object types on the app.
pub trait AppObjectTypes {
    /// Panics if `kind` is already registered: two plugins claiming one type
    /// is a mistake. A mod that means to change a type uses
    /// [`replace_object_type`](Self::replace_object_type).
    fn register_object_type(&mut self, kind: u16, object_type: ObjectType) -> &mut Self;
    /// Replaces a registered type, for a mod that swaps an existing type's
    /// hooks. Panics if `kind` is not registered yet (add the base game's
    /// plugins first).
    fn replace_object_type(&mut self, kind: u16, object_type: ObjectType) -> &mut Self;
}

impl AppObjectTypes for App {
    fn register_object_type(&mut self, kind: u16, object_type: ObjectType) -> &mut Self {
        self.init_resource::<ObjectTypes>();
        let mut types = self.world_mut().resource_mut::<ObjectTypes>();
        if let Some(old) = types.get(kind) {
            panic!("object type {kind} registered twice (first as {:?})", old.name);
        }
        types.insert(kind, object_type);
        self
    }

    fn replace_object_type(&mut self, kind: u16, object_type: ObjectType) -> &mut Self {
        self.init_resource::<ObjectTypes>();
        let mut types = self.world_mut().resource_mut::<ObjectTypes>();
        if types.get(kind).is_none() {
            panic!("object type {kind} replaced before it was registered (as {:?})", object_type.name);
        }
        types.insert(kind, object_type);
        self
    }
}

/// Run each new object's type hooks. Public so systems that finish an
/// object's set-up can order themselves after it.
pub fn initialize_objects(
    mut commands: Commands,
    world: Option<Res<WorldGeometry>>,
    types: Res<ObjectTypes>,
    mut diagnostics: ResMut<ObjectDiagnostics>,
    objects: Query<(Entity, &WorldObject), Without<Initialized>>,
) {
    for (e, o) in &objects {
        let mut entity = commands.entity(e);
        entity.insert(Initialized);
        match types.get(o.kind) {
            Some(object_type) => {
                for hook in &object_type.init {
                    hook(&mut entity, o, world.as_deref());
                }
            }
            None => {
                diagnostics.unregistered_types.insert(o.kind);
            }
        }
    }
}

pub struct ObjectsPlugin;

impl Plugin for ObjectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ObjectTypes>().init_resource::<ObjectDiagnostics>().add_systems(
            FixedUpdate,
            initialize_objects.in_set(LifecycleSystems::Initialize).in_set(GameSystems::Game).run_if(simulation_enabled),
        );
    }
}
