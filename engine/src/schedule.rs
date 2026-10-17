//! The order of work inside one logic tick, and the pause gate.
//!
//! Every gameplay system belongs to one of [`WorldSystems`], which run chained
//! inside [`GameSystems::Game`]. Games put their controllers into these sets
//! instead of ordering themselves against each other ad hoc, so the engine's
//! systems (triggers, camera) always see the state the original saw.
use bevy::prelude::*;

use crate::shell::GameSystems;

/// The major phases of one logic tick. Item controllers see the avatar from
/// the previous tick; floor triggers see the result of this tick's collision.
/// PSX ControlPhase 0x8001DC54..0x8001DD00 and avatar control at 0x800383E0..0x80038458
/// establish this order (docs/tick-order-evidence.md).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum WorldSystems {
    /// Menus, initial avatar publication and camera setup.
    Prepare,
    /// Per-level reset and object spawn hooks, before any controller runs.
    Initialize,
    /// Placed-object controllers and their animation, before the avatar.
    Items,
    /// Avatar input and state decisions.
    Control,
    /// Avatar movement and level collision.
    Motion,
    /// Commit the avatar's authored pose before object collision.
    Animation,
    /// Nearby objects, weapons and floor triggers.
    Interaction,
    /// Hair, cloth and other attached motion, before the camera.
    SecondaryMotion,
    Camera,
    /// Sound and other work after the settled camera.
    Effects,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum LifecycleSystems {
    Reset,
    Initialize,
}

/// A controller selects its object's state before the generic clip step.
/// AnimationGoal opts objects into authored dispatch and links. Legacy
/// controllers retain their prior clip rules; active-list order is separate.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ItemSystems {
    Control,
    Animation,
}

/// Order inside [`WorldSystems::Interaction`].
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum InteractionSystems {
    /// Pickups, switches and keyholes in the avatar's vicinity.
    Use,
    /// Object collision: doors, traps and pushes.
    Resolve,
    /// Publish the final avatar and visual placement after object collision.
    Publish,
    /// Avatar weapons resolve after object collision, before triggers.
    Combat,
    Triggers,
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
/// whenever the engine is about to read it: at the start of the tick, after
/// object collision ([`InteractionSystems::Publish`]) and before the camera.
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
            (Prepare, Initialize, Items, Control, Motion, Animation, Interaction, SecondaryMotion, Camera, Effects)
                .chain()
                .in_set(GameSystems::Game),
        );
        app.configure_sets(
            FixedUpdate,
            (LifecycleSystems::Reset, LifecycleSystems::Initialize).chain().in_set(Initialize),
        );
        app.configure_sets(FixedUpdate, (ItemSystems::Control, ItemSystems::Animation).chain().in_set(Items));
        use InteractionSystems as I;
        app.configure_sets(
            FixedUpdate,
            (I::Use, I::Resolve, I::Publish, I::Combat, I::Triggers, I::Report).chain().in_set(Interaction),
        );
    }
}
