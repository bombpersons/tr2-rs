//! The order of work inside one logic tick, and the pause gate.
//!
//! Every gameplay system belongs to one of [`WorldSystems`], which run chained
//! inside [`GameSystems::Game`]. Games put their controllers into these sets
//! instead of ordering themselves against each other ad hoc, so the engine's
//! systems (triggers, camera) always see the state the original saw.
use bevy::prelude::*;

use crate::shell::GameSystems;

/// One tick, in order:
///
/// 1. `Prepare`: camera initialisation after a level load; games publish the
///    [`Avatar`] here.
/// 2. `Control`: controllers read input and choose what to do (the player's
///    state handlers).
/// 3. `Motion`: integrate movement and collide with the level.
/// 4. `Interaction`: pickups and switches, floor triggers, object collision,
///    creatures ([`InteractionSystems`] orders the work inside it).
/// 5. `Animation`: advance clips for what was decided this tick.
/// 6. `Camera`: follow the avatar, fixed shots.
/// 7. `Effects`: work that only reads the settled tick (sounds, hair).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum WorldSystems {
    Prepare,
    Control,
    Motion,
    Interaction,
    Animation,
    Camera,
    Effects,
}

/// Order inside [`WorldSystems::Interaction`].
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum InteractionSystems {
    /// Forget per-level state when a new level arrives.
    Reset,
    /// Give newly spawned objects their type's components (object registry).
    Initialize,
    /// The avatar uses things: pickups, switches, keyholes.
    Use,
    /// Floor triggers under the avatar fire, then game trigger actions run.
    Triggers,
    /// Objects push the avatar and hurt it (doors, traps).
    Resolve,
    /// Creatures and combat.
    Creatures,
    /// Copy results into menus and HUD state.
    Report,
}

/// Menus suspend gameplay at the fixed-tick boundary; absence means running.
#[derive(Resource, Reflect, Debug, Clone, Copy)]
#[reflect(Resource)]
pub struct SimulationEnabled(pub bool);

/// Run condition for gameplay systems: true unless a menu paused the world.
pub fn simulation_enabled(enabled: Option<Res<SimulationEnabled>>) -> bool {
    enabled.is_none_or(|enabled| enabled.0)
}

/// The entity the player controls, as the engine sees it: the camera follows
/// it, floor triggers fire under it and rooms are drawn around it.
///
/// The game owns the real state (its player component) and writes this copy
/// whenever the engine is about to read it: at the start of the tick, before
/// [`WorldSystems::Interaction`] and before [`WorldSystems::Camera`].
#[derive(Component, Reflect, Debug, Clone, Copy, Default, PartialEq)]
#[reflect(Component)]
pub struct Avatar {
    /// Feet position, level space (y up, metres).
    pub position: Vec3,
    /// Radians; zero faces +Z, positive turns toward +X.
    pub yaw: f32,
    pub room: usize,
    /// Standing on a floor: pad and anti-pad triggers need it.
    pub grounded: bool,
    /// Falling (not jumping): a level that starts with a fall frames the
    /// camera differently.
    pub falling: bool,
    /// Weapons drawn: combat triggers need it.
    pub armed: bool,
    pub alive: bool,
}

pub struct SchedulePlugin;

impl Plugin for SchedulePlugin {
    fn build(&self, app: &mut App) {
        use WorldSystems::*;
        app.configure_sets(
            FixedUpdate,
            (Prepare, Control, Motion, Interaction, Animation, Camera, Effects).chain().in_set(GameSystems::Game),
        );
        use InteractionSystems as I;
        app.configure_sets(
            FixedUpdate,
            (I::Reset, I::Initialize, I::Use, I::Triggers, I::Resolve, I::Creatures, I::Report)
                .chain()
                .in_set(Interaction),
        );
    }
}
