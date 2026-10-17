//! Floor triggers: each tick the trigger lists of the sector under the
//! [`Avatar`] are tested and their actions run.
//!
//! The engine runs two actions itself, `activate` (set an object's
//! activation code bits) and `camera` (cue a fixed camera shot). Every other
//! action is a game's: register its name with
//! [`AppTriggerActions::register_trigger_action`] and read [`TriggerAction`]
//! messages in [`InteractionSystems::Triggers`], after [`TriggerSystems::Dispatch`].
//! Actions nobody registered are listed in [`TriggerDiagnostics`].
use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;

use crate::camera::{CameraCue, CameraCues};
use crate::objects::{CODE_BITS, WorldObject};
use crate::schedule::{Avatar, InteractionSystems, simulation_enabled};
use crate::shell::GameSystems;
pub use crate::world::TriggerKind;
use crate::world::WorldGeometry;

/// The engine's own actions.
const ACTIVATE: &str = "activate";
const CAMERA: &str = "camera";

/// A game action of a trigger that fired this tick.
#[derive(Message, Debug, Clone, PartialEq)]
pub struct TriggerAction {
    pub name: String,
    pub argument: u16,
    pub trigger: TriggerKind,
}

/// Names of the trigger actions some game handles.
#[derive(Resource, Default, Debug)]
pub struct TriggerActions(BTreeSet<String>);

/// Registering trigger actions on the app.
pub trait AppTriggerActions {
    fn register_trigger_action(&mut self, name: &str) -> &mut Self;
}

impl AppTriggerActions for App {
    fn register_trigger_action(&mut self, name: &str) -> &mut Self {
        self.init_resource::<TriggerActions>();
        self.world_mut().resource_mut::<TriggerActions>().0.insert(name.into());
        self
    }
}

/// Trigger actions that fired with no handler registered.
#[derive(Resource, Reflect, Default, Debug)]
#[reflect(Resource)]
pub struct TriggerDiagnostics {
    pub unsupported_actions: BTreeSet<String>,
}

/// Per-level trigger memory. Games report uses and pickups here so switch,
/// key and pickup triggers can test them.
#[derive(Resource, Default)]
pub struct TriggerState {
    level: String,
    once: BTreeSet<usize>,
    used: BTreeSet<usize>,
    collected: BTreeSet<usize>,
    timers: BTreeMap<usize, f32>,
}

impl TriggerState {
    /// The avatar used object `index` (a switch or keyhole) this tick.
    pub fn mark_used(&mut self, index: usize) {
        self.used.insert(index);
    }

    /// The avatar collected object `index`.
    pub fn mark_collected(&mut self, index: usize) {
        self.collected.insert(index);
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum TriggerSystems {
    Dispatch,
}

fn reset_level(world: Option<Res<WorldGeometry>>, mut state: ResMut<TriggerState>) {
    let Some(w) = world else {
        return;
    };
    if state.level != w.data.name {
        *state = TriggerState { level: w.data.name.clone(), ..default() };
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch_triggers(
    time: Res<Time>,
    world: Option<Res<WorldGeometry>>,
    registered: Res<TriggerActions>,
    mut state: ResMut<TriggerState>,
    mut diagnostics: ResMut<TriggerDiagnostics>,
    avatars: Query<&Avatar>,
    mut objects: Query<&mut WorldObject>,
    mut cameras: ResMut<CameraCues>,
    mut fired: MessageWriter<TriggerAction>,
) {
    cameras.0.clear();
    let Some(w) = world else {
        return;
    };
    let Ok(avatar) = avatars.single() else {
        return;
    };
    for (index, timer) in &mut state.timers {
        *timer -= time.delta_secs();
        if *timer <= 0.0 {
            for mut o in &mut objects {
                if o.index == *index {
                    o.active = false;
                    o.flags &= !CODE_BITS;
                }
            }
        }
    }
    state.timers.retain(|_, v| *v > 0.0);
    let Some(floor) = w.trigger_floor(avatar.position, avatar.room) else {
        return;
    };
    for trigger in &floor.triggers {
        let kind = trigger.kind;
        // A shot already running continues before activation and one-shot
        // flags are tested: a spent camera still runs until the avatar
        // leaves its sectors or its own timer expires.
        for action in &trigger.actions {
            if let Some(shot) = action.camera {
                cameras.0.push(CameraCue {
                    index: usize::from(action.argument),
                    timer: shot.timer,
                    once: shot.once,
                    speed: shot.speed,
                    activate: false,
                    restart: kind == TriggerKind::Switch,
                });
            }
        }
        if trigger.once && state.once.contains(&trigger.id) {
            continue;
        }
        let source = trigger.actions.first().filter(|a| a.action == ACTIVATE).map(|a| usize::from(a.argument));
        let allowed = match kind {
            TriggerKind::Floor => true,
            TriggerKind::Pad | TriggerKind::AntiPad => avatar.grounded,
            TriggerKind::Switch | TriggerKind::Key => source.is_some_and(|s| state.used.contains(&s)),
            TriggerKind::Pickup => source.is_some_and(|s| state.collected.contains(&s)),
            TriggerKind::Combat => avatar.armed,
            TriggerKind::Other(_) => false,
        };
        if !allowed {
            continue;
        }
        if trigger.once {
            state.once.insert(trigger.id);
        }
        let mask = u16::from(trigger.mask & 31) << 9;
        for (i, action) in trigger.actions.iter().enumerate() {
            // The first action of a switch, key or pickup trigger names the
            // object that fires it; it does not activate itself.
            if i == 0 && matches!(kind, TriggerKind::Switch | TriggerKind::Key | TriggerKind::Pickup) && action.action == ACTIVATE {
                continue;
            }
            let argument = action.argument;
            match action.action.as_str() {
                CAMERA => {
                    if kind != TriggerKind::Combat
                        && let Some(cue) = cameras.0.iter_mut().rev().find(|cue| cue.index == usize::from(argument))
                    {
                        cue.activate = true;
                    }
                }
                ACTIVATE => {
                    for mut o in &mut objects {
                        if o.index == usize::from(argument) {
                            match kind {
                                TriggerKind::Switch => o.flags ^= mask,
                                TriggerKind::AntiPad => o.flags &= !mask,
                                _ => o.flags |= mask,
                            }
                            o.update_active();
                            if trigger.timer > 0 && o.active {
                                // Timer byte: seconds, except 1 = one tick.
                                state.timers.insert(
                                    o.index,
                                    if trigger.timer == 1 { 1.0 / 30.0 } else { f32::from(trigger.timer) },
                                );
                            }
                        }
                    }
                }
                name => {
                    if registered.0.contains(name) {
                        fired.write(TriggerAction { name: name.into(), argument, trigger: kind });
                    } else {
                        diagnostics.unsupported_actions.insert(name.into());
                    }
                }
            }
        }
    }
    state.used.clear();
}

pub struct TriggersPlugin;

impl Plugin for TriggersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TriggerState>()
            .init_resource::<TriggerActions>()
            .init_resource::<TriggerDiagnostics>()
            .init_resource::<CameraCues>()
            .add_message::<TriggerAction>()
            .configure_sets(FixedUpdate, TriggerSystems::Dispatch.in_set(InteractionSystems::Triggers))
            .add_systems(
                FixedUpdate,
                (
                    reset_level.in_set(InteractionSystems::Reset),
                    dispatch_triggers.in_set(TriggerSystems::Dispatch),
                )
                    .in_set(GameSystems::Game)
                    .run_if(simulation_enabled),
            );
    }
}
