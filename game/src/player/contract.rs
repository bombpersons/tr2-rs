//! Lara's asset contract: the clip, state, frame and joint numbers her
//! controller relies on, and [`check`], which tests a level's player model
//! and animation table against them. `docs/lara-contract.md` explains each
//! entry and where the controller uses it.
//!
//! Clip numbers are absolute indices into the level's animation table (the
//! controller's [`PlayerAnimations`](super::PlayerAnimations) is the whole
//! table), so the player model's clips must start at 0. Numbers and states
//! are those of the levels' shared Lara table (WALL.PSX, as baked); the
//! controller code cites the routines that use them.
use engine::world::{Command, Level};

/// Clips the controller enters, requests or compares by number.
pub mod clip {
    /// Run loop (state 1). Run collision picks the wall-hit clip from its frame.
    pub const RUN: usize = 0;
    /// Walk loop (state 0). Its absolute frames hold the walking foot windows
    /// (see [`super::frame`]).
    pub const WALK: usize = 1;
    /// Walking foot stops (state 0, ending standing).
    pub const WALK_STOP_RIGHT: usize = 2;
    pub const WALK_STOP_LEFT: usize = 3;
    /// Run start (state 1). Run collision never hits a wall from it.
    pub const RUN_START: usize = 6;
    /// Standing (state 2); a mid-air start's camera frames it too.
    pub const STAND: usize = 11;
    /// Slow turns on the spot (states 6 and 7).
    pub const TURN_RIGHT: usize = 12;
    pub const TURN_LEFT: usize = 13;
    /// Running jump takeoff (state 3), only without an animation table.
    pub const RUN_JUMP: usize = 16;
    /// Walk start (state 0).
    pub const WALK_START: usize = 20;
    /// Fast fall (state 9).
    pub const FAST_FALL: usize = 23;
    /// Heavy landing (state 2), and the landing without a dispatch range.
    pub const HEAVY_LANDING: usize = 24;
    /// Fatal landing (state 8).
    pub const FATAL_LANDING: usize = 25;
    /// Vertical jump preparation (state 19), also a high vault.
    pub const JUMP_UP_PREPARE: usize = 26;
    /// Vertical jump (state 28).
    pub const JUMP_UP: usize = 28;
    /// Low airborne impact and recovery (state 2).
    pub const JUMP_IMPACT: usize = 31;
    /// Rebound fast fall (state 9).
    pub const REBOUND_FALL: usize = 32;
    /// Forward flight's fall (state 3): ledge falls and a mid-air spawn.
    pub const FALL: usize = 34;
    /// Walk backward (state 16).
    pub const WALK_BACK: usize = 40;
    /// Three-click vault (state 2, with an end position command).
    pub const VAULT_THREE: usize = 42;
    /// Fast turns (state 20).
    pub const FAST_TURN_RIGHT: usize = 44;
    pub const FAST_TURN_LEFT: usize = 69;
    /// Two-click vault (state 2, with an end position command).
    pub const VAULT_TWO: usize = 50;
    /// Running into a wall (state 2), early and late in the run stride.
    pub const HIT_WALL_EARLY: usize = 53;
    pub const HIT_WALL_LATE: usize = 54;
    /// Walking a click up or down (state 0), by foot.
    pub const STEP_UP_LEFT: usize = 57;
    pub const STEP_UP_RIGHT: usize = 58;
    pub const STEP_DOWN_LEFT: usize = 59;
    pub const STEP_DOWN_RIGHT: usize = 60;
    /// Walking backward a click down (state 16), by foot.
    pub const BACK_STEP_DOWN_LEFT: usize = 61;
    pub const BACK_STEP_DOWN_RIGHT: usize = 62;
    /// Sidesteps (states 22 and 21).
    pub const STEP_LEFT: usize = 65;
    pub const STEP_RIGHT: usize = 67;
    /// Forward slide (state 24) and its stop.
    pub const SLIDE: usize = 70;
    pub const SLIDE_STOP: usize = 71;
    /// Standing jump compression (state 15).
    pub const JUMP_PREPARE: usize = 73;
    /// Back, forward, left and right flight (states 25, 3, 27, 26).
    pub const JUMP_BACK: usize = 75;
    pub const JUMP_FORWARD: usize = 77;
    pub const JUMP_LEFT: usize = 79;
    pub const JUMP_RIGHT: usize = 81;
    /// Forward jump takeoff (state 3, with a jump velocity command).
    pub const JUMP_FORWARD_TAKEOFF: usize = 76;
    /// Underwater stroke (state 17).
    pub const SWIM: usize = 86;
    /// The clip a dead Lara requests (state 18 in the original's table).
    pub const DEAD: usize = 87;
    /// Hop backward (state 5).
    pub const HOP_BACK: usize = 88;
    /// Standing vertical takeoff (state 28, with a jump velocity command).
    pub const STANDING_TAKEOFF: usize = 91;
    /// Backward fall (state 29).
    pub const FALL_BACK: usize = 93;
    /// Hanging from a ledge (state 10) and pulling up (state 19).
    pub const HANG: usize = 96;
    pub const PULL_UP: usize = 97;
    /// Standing idle loop (state 2): what standing requests.
    pub const STAND_IDLE: usize = 103;
    /// Back slide start and loop (state 32).
    pub const BACK_SLIDE: usize = 104;
    pub const BACK_SLIDE_LOOP: usize = 105;
    /// Underwater tread (state 13).
    pub const TREAD: usize = 108;
    /// Surface tread (state 33).
    pub const SURFACE_TREAD: usize = 110;
    /// Climbing out of water onto a high edge (state 55).
    pub const CLIMB_OUT_HIGH: usize = 111;
    /// Jumping into deep water (state 35).
    pub const WATER_ENTRY: usize = 112;
    /// Surfacing from underwater (state 33).
    pub const SURFACING: usize = 114;
    /// Surface strokes (states 34, 47, 48, 49).
    pub const SURFACE_FORWARD: usize = 116;
    pub const SURFACE_BACK: usize = 140;
    pub const SURFACE_LEFT: usize = 143;
    pub const SURFACE_RIGHT: usize = 144;
    /// Diving from the surface (state 35).
    pub const DIVE: usize = 119;
    /// Roll (state 45).
    pub const ROLL: usize = 146;
    /// Wading (state 65): the loop, and the clip the wading gait requests.
    pub const WADE_LOOP: usize = 177;
    pub const WADE: usize = 186;
    /// Stepping out of water (state 55): onto a lower floor, and climbing
    /// out onto a mid and a low edge.
    pub const STEP_OUT: usize = 190;
    pub const CLIMB_OUT_MID: usize = 191;
    pub const STAND_UP: usize = 192;
    pub const CLIMB_OUT_LOW: usize = 193;

    /// Using objects. Each plays in place and ends in [`STAND`], whose
    /// end command frees the hands. Picking up and the key and puzzle
    /// clips are reached through standing's ranges; the switch clips are
    /// entered directly, by switch type (SwitchCollision 0x800507E4).
    pub const USE_KEY: usize = 131;
    pub const USE_PUZZLE: usize = 134;
    /// The pickup lands in the hand on frame [`super::frame::PICKUP_TAKE`].
    pub const PICKUP: usize = 135;
    /// Type 104 and the others not named below: turning a switch off and on.
    pub const SWITCH_OFF: usize = 63;
    pub const SWITCH_ON: usize = 64;
    /// Type 93, the wall button.
    pub const BUTTON_OFF: usize = 195;
    pub const BUTTON_ON: usize = 196;
    /// Type 103, used the same way both ways.
    pub const LEVER: usize = 197;
}

/// Clip states, as clips declare them and as goals the controller asks for.
pub mod state {
    pub const WALK: usize = 0;
    pub const RUN: usize = 1;
    pub const STAND: usize = 2;
    pub const FORWARD_JUMP: usize = 3;
    pub const HOP_BACK: usize = 5;
    pub const TURN_RIGHT: usize = 6;
    pub const TURN_LEFT: usize = 7;
    pub const DEATH: usize = 8;
    pub const FAST_FALL: usize = 9;
    pub const HANG: usize = 10;
    pub const REACH: usize = 11;
    pub const TREAD: usize = 13;
    pub const JUMP_IMPACT: usize = 14;
    pub const JUMP_PREPARE: usize = 15;
    pub const WALK_BACK: usize = 16;
    pub const SWIM: usize = 17;
    pub const GLIDE: usize = 18;
    pub const PULL_UP: usize = 19;
    pub const FAST_TURN: usize = 20;
    pub const STEP_RIGHT: usize = 21;
    pub const STEP_LEFT: usize = 22;
    pub const ROLL_BACK: usize = 23;
    pub const SLIDE: usize = 24;
    pub const JUMP_BACK: usize = 25;
    pub const JUMP_RIGHT: usize = 26;
    pub const JUMP_LEFT: usize = 27;
    pub const JUMP_UP: usize = 28;
    pub const FALL_BACK: usize = 29;
    pub const BACK_SLIDE: usize = 32;
    pub const SURFACE_TREAD: usize = 33;
    pub const SURFACE_FORWARD: usize = 34;
    pub const DIVE: usize = 35;
    pub const WATER_DEATH: usize = 44;
    pub const ROLL: usize = 45;
    pub const SURFACE_BACK: usize = 47;
    pub const SURFACE_LEFT: usize = 48;
    pub const SURFACE_RIGHT: usize = 49;
    pub const SWAN_DIVE: usize = 52;
    pub const CLIMB_OUT: usize = 55;
    pub const WADE: usize = 65;
    pub const TWIST: usize = 68;
    pub const PICKUP: usize = 39;
    /// Using a switch whose state is 1, and one whose state is 0.
    pub const SWITCH_OFF: usize = 40;
    pub const SWITCH_ON: usize = 41;
    pub const USE_KEY: usize = 42;
    pub const USE_PUZZLE: usize = 43;
}

/// Absolute frame windows (across the whole table, not clip-relative) that
/// the walk collision 0x8003A62C reads. They lie inside the walk loop
/// [`clip::WALK`], which must therefore cover frames 22..=57.
pub mod frame {
    use std::ops::Range;

    /// Pickup collision 0x80050580 takes the item on clip
    /// [`super::clip::PICKUP`]'s frame 42 (relative).
    pub const PICKUP_TAKE: f32 = 42.0;
    /// Puzzle-hole collision 0x800512CC fills the hole on clip
    /// [`super::clip::USE_PUZZLE`]'s frame 80 (relative).
    pub const PUZZLE_PLACE: f32 = 80.0;
    /// The walk loop's absolute frames.
    pub const WALK_LOOP: Range<usize> = 22..58;
    /// Walking into a wall: stop on the left foot ([`super::clip::WALK_STOP_LEFT`]) ...
    pub const WALL_STOP_LEFT: Range<f32> = 29.0..48.0;
    /// ... or the right ([`super::clip::WALK_STOP_RIGHT`]), else stand at once.
    pub const WALL_STOP_RIGHT: [Range<f32>; 2] = [22.0..29.0, 48.0..58.0];
    /// Walking a click down: the left foot steps first in this window.
    pub const STEP_DOWN_LEFT: Range<f32> = 28.0..46.0;
    /// Walking a click up: the right foot steps first in this window.
    pub const STEP_UP_RIGHT: Range<f32> = 27.0..45.0;
    /// Walking backward a click down: the right foot in this window
    /// (backward collision 0x8003B43C).
    pub const BACK_STEP_DOWN_RIGHT: Range<f32> = 964.0..994.0;
}

/// Lara's joints, by the presentation that turns or swaps them.
pub mod joint {
    pub const HIPS: usize = 0;
    pub const LEFT_THIGH: usize = 1;
    pub const RIGHT_THIGH: usize = 4;
    pub const TORSO: usize = 7;
    pub const RIGHT_UPPER_ARM: usize = 8;
    pub const RIGHT_HAND: usize = 10;
    pub const LEFT_UPPER_ARM: usize = 11;
    pub const LEFT_HAND: usize = 13;
    pub const HEAD: usize = 14;
    /// Joints in Lara's layout: hips, left leg (1-3), right leg (4-6),
    /// torso, right arm (8-10), left arm (11-13), head.
    pub const COUNT: usize = 15;
    /// Each joint's parent in that layout.
    pub const PARENTS: [Option<usize>; COUNT] = [
        None,
        Some(0),
        Some(1),
        Some(2),
        Some(0),
        Some(4),
        Some(5),
        Some(0),
        Some(7),
        Some(8),
        Some(9),
        Some(7),
        Some(11),
        Some(12),
        Some(7),
    ];
}

/// One past the highest clip number the controller names.
pub const CLIP_COUNT: usize = clip::LEVER + 1;

/// A group of moves a character can provide on its own. A model meeting
/// [`Feature::Ground`] can stand, walk, run, turn and stop under Lara's
/// controller; the others add moves on top of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    /// Model 0, its table's numbering, and stand, walk, run, turn and stop.
    Ground,
    /// Walking a click up or down.
    Steps,
    Backward,
    Sidestep,
    Roll,
    /// Jumps, falls and landings.
    Jump,
    /// Vaults, hanging and pulling up.
    Climb,
    Slide,
    /// Wading, the surface and swimming.
    Water,
    Death,
    /// The joint layout the arms, guns, head and braid presentation read.
    Skeleton,
    /// Picking up, switches, keys and puzzle pieces.
    Use,
}

impl Feature {
    pub const ALL: [Feature; 12] = [
        Feature::Ground,
        Feature::Steps,
        Feature::Backward,
        Feature::Sidestep,
        Feature::Roll,
        Feature::Jump,
        Feature::Climb,
        Feature::Slide,
        Feature::Water,
        Feature::Death,
        Feature::Skeleton,
        Feature::Use,
    ];
}

/// A clip the controller uses, and the state it must declare.
#[derive(Debug, Clone, Copy)]
pub struct ClipRule {
    pub clip: usize,
    pub state: usize,
    pub feature: Feature,
}

const fn rule(feature: Feature, clip: usize, state: usize) -> ClipRule {
    ClipRule { clip, state, feature }
}

/// Every clip the controller names, with its declared state.
pub const CLIPS: &[ClipRule] = {
    use Feature::*;
    &[
        rule(Ground, clip::RUN, state::RUN),
        rule(Ground, clip::WALK, state::WALK),
        rule(Ground, clip::WALK_STOP_RIGHT, state::WALK),
        rule(Ground, clip::WALK_STOP_LEFT, state::WALK),
        rule(Ground, clip::RUN_START, state::RUN),
        rule(Ground, clip::STAND, state::STAND),
        rule(Ground, clip::TURN_RIGHT, state::TURN_RIGHT),
        rule(Ground, clip::TURN_LEFT, state::TURN_LEFT),
        rule(Ground, clip::WALK_START, state::WALK),
        rule(Ground, clip::FAST_TURN_RIGHT, state::FAST_TURN),
        rule(Ground, clip::FAST_TURN_LEFT, state::FAST_TURN),
        rule(Ground, clip::HIT_WALL_EARLY, state::STAND),
        rule(Ground, clip::HIT_WALL_LATE, state::STAND),
        rule(Ground, clip::STAND_IDLE, state::STAND),
        rule(Steps, clip::STEP_UP_LEFT, state::WALK),
        rule(Steps, clip::STEP_UP_RIGHT, state::WALK),
        rule(Steps, clip::STEP_DOWN_LEFT, state::WALK),
        rule(Steps, clip::STEP_DOWN_RIGHT, state::WALK),
        rule(Backward, clip::WALK_BACK, state::WALK_BACK),
        rule(Backward, clip::BACK_STEP_DOWN_LEFT, state::WALK_BACK),
        rule(Backward, clip::BACK_STEP_DOWN_RIGHT, state::WALK_BACK),
        rule(Backward, clip::HOP_BACK, state::HOP_BACK),
        rule(Backward, clip::FALL_BACK, state::FALL_BACK),
        rule(Sidestep, clip::STEP_LEFT, state::STEP_LEFT),
        rule(Sidestep, clip::STEP_RIGHT, state::STEP_RIGHT),
        rule(Roll, clip::ROLL, state::ROLL),
        rule(Jump, clip::RUN_JUMP, state::FORWARD_JUMP),
        rule(Jump, clip::FAST_FALL, state::FAST_FALL),
        rule(Jump, clip::HEAVY_LANDING, state::STAND),
        rule(Jump, clip::JUMP_UP_PREPARE, state::PULL_UP),
        rule(Jump, clip::JUMP_UP, state::JUMP_UP),
        rule(Jump, clip::JUMP_IMPACT, state::STAND),
        rule(Jump, clip::REBOUND_FALL, state::FAST_FALL),
        rule(Jump, clip::FALL, state::FORWARD_JUMP),
        rule(Jump, clip::JUMP_PREPARE, state::JUMP_PREPARE),
        rule(Jump, clip::JUMP_BACK, state::JUMP_BACK),
        rule(Jump, clip::JUMP_FORWARD_TAKEOFF, state::FORWARD_JUMP),
        rule(Jump, clip::JUMP_FORWARD, state::FORWARD_JUMP),
        rule(Jump, clip::JUMP_LEFT, state::JUMP_LEFT),
        rule(Jump, clip::JUMP_RIGHT, state::JUMP_RIGHT),
        rule(Jump, clip::STANDING_TAKEOFF, state::JUMP_UP),
        rule(Climb, clip::VAULT_THREE, state::STAND),
        rule(Climb, clip::VAULT_TWO, state::STAND),
        rule(Climb, clip::HANG, state::HANG),
        rule(Climb, clip::PULL_UP, state::PULL_UP),
        rule(Slide, clip::SLIDE, state::SLIDE),
        rule(Slide, clip::SLIDE_STOP, state::SLIDE),
        rule(Slide, clip::BACK_SLIDE, state::BACK_SLIDE),
        rule(Slide, clip::BACK_SLIDE_LOOP, state::BACK_SLIDE),
        rule(Water, clip::SWIM, state::SWIM),
        rule(Water, clip::TREAD, state::TREAD),
        rule(Water, clip::SURFACE_TREAD, state::SURFACE_TREAD),
        rule(Water, clip::CLIMB_OUT_HIGH, state::CLIMB_OUT),
        rule(Water, clip::WATER_ENTRY, state::DIVE),
        rule(Water, clip::SURFACING, state::SURFACE_TREAD),
        rule(Water, clip::SURFACE_FORWARD, state::SURFACE_FORWARD),
        rule(Water, clip::DIVE, state::DIVE),
        rule(Water, clip::SURFACE_BACK, state::SURFACE_BACK),
        rule(Water, clip::SURFACE_LEFT, state::SURFACE_LEFT),
        rule(Water, clip::SURFACE_RIGHT, state::SURFACE_RIGHT),
        rule(Water, clip::WADE_LOOP, state::WADE),
        rule(Water, clip::WADE, state::WADE),
        rule(Water, clip::STEP_OUT, state::CLIMB_OUT),
        rule(Water, clip::CLIMB_OUT_MID, state::CLIMB_OUT),
        rule(Water, clip::STAND_UP, state::CLIMB_OUT),
        rule(Water, clip::CLIMB_OUT_LOW, state::CLIMB_OUT),
        rule(Death, clip::FATAL_LANDING, state::DEATH),
        rule(Death, clip::DEAD, state::GLIDE),
        rule(Use, clip::PICKUP, state::PICKUP),
        rule(Use, clip::USE_KEY, state::USE_KEY),
        rule(Use, clip::USE_PUZZLE, state::USE_PUZZLE),
        rule(Use, clip::SWITCH_OFF, state::SWITCH_OFF),
        rule(Use, clip::SWITCH_ON, state::SWITCH_ON),
        rule(Use, clip::BUTTON_OFF, state::SWITCH_OFF),
        rule(Use, clip::BUTTON_ON, state::SWITCH_ON),
        rule(Use, clip::LEVER, state::SWITCH_OFF),
    ]
};

/// A state change the controller relies on: while `from` plays, asking for
/// `goal` has an authored range, and the clip it enters leads (through its
/// end links) to a clip of state `goal`.
#[derive(Debug, Clone, Copy)]
pub struct DispatchRule {
    pub from: usize,
    pub goal: usize,
    pub feature: Feature,
}

const fn dispatch(feature: Feature, from: usize, goal: usize) -> DispatchRule {
    DispatchRule { from, goal, feature }
}

pub const DISPATCHES: &[DispatchRule] = {
    use Feature::*;
    &[
        // Standing leaves into walking, running and the turns.
        dispatch(Ground, clip::STAND, state::WALK),
        dispatch(Ground, clip::STAND, state::RUN),
        dispatch(Ground, clip::STAND, state::TURN_RIGHT),
        dispatch(Ground, clip::STAND, state::TURN_LEFT),
        dispatch(Ground, clip::STAND_IDLE, state::WALK),
        dispatch(Ground, clip::STAND_IDLE, state::RUN),
        dispatch(Ground, clip::STAND_IDLE, state::TURN_RIGHT),
        dispatch(Ground, clip::STAND_IDLE, state::TURN_LEFT),
        // Walking and running stop through a foot window.
        dispatch(Ground, clip::RUN, state::STAND),
        dispatch(Ground, clip::WALK, state::STAND),
        // Turns stop, and the slow ones hand over to the fast ones.
        dispatch(Ground, clip::TURN_RIGHT, state::STAND),
        dispatch(Ground, clip::TURN_LEFT, state::STAND),
        dispatch(Ground, clip::TURN_RIGHT, state::FAST_TURN),
        dispatch(Ground, clip::TURN_LEFT, state::FAST_TURN),
        dispatch(Ground, clip::FAST_TURN_RIGHT, state::STAND),
        dispatch(Ground, clip::FAST_TURN_LEFT, state::STAND),
        dispatch(Backward, clip::STAND_IDLE, state::WALK_BACK),
        dispatch(Backward, clip::STAND_IDLE, state::HOP_BACK),
        dispatch(Backward, clip::WALK_BACK, state::STAND),
        dispatch(Sidestep, clip::STAND_IDLE, state::STEP_LEFT),
        dispatch(Sidestep, clip::STAND_IDLE, state::STEP_RIGHT),
        dispatch(Sidestep, clip::STEP_LEFT, state::STAND),
        dispatch(Sidestep, clip::STEP_RIGHT, state::STAND),
        dispatch(Jump, clip::STAND_IDLE, state::JUMP_PREPARE),
        dispatch(Jump, clip::JUMP_PREPARE, state::FORWARD_JUMP),
        dispatch(Jump, clip::RUN, state::FORWARD_JUMP),
        dispatch(Jump, clip::JUMP_FORWARD, state::STAND),
        dispatch(Jump, clip::FALL, state::REACH),
        dispatch(Jump, clip::STAND_IDLE, state::JUMP_UP),
        dispatch(Climb, clip::HANG, state::PULL_UP),
        dispatch(Slide, clip::SLIDE, state::STAND),
        dispatch(Water, clip::SURFACE_TREAD, state::SURFACE_FORWARD),
        dispatch(Water, clip::TREAD, state::SWIM),
        dispatch(Use, clip::STAND_IDLE, state::PICKUP),
        dispatch(Use, clip::STAND_IDLE, state::USE_KEY),
        dispatch(Use, clip::STAND_IDLE, state::USE_PUZZLE),
    ]
};

/// A clip whose end links must lead to a clip of `state`: a start into its
/// loop, or a stop back to standing.
#[derive(Debug, Clone, Copy)]
pub struct LinkRule {
    pub from: usize,
    pub state: usize,
    pub feature: Feature,
}

const fn link(feature: Feature, from: usize, state: usize) -> LinkRule {
    LinkRule { from, state, feature }
}

pub const LINKS: &[LinkRule] = {
    use Feature::*;
    &[
        link(Ground, clip::RUN_START, state::RUN),
        link(Ground, clip::WALK_START, state::WALK),
        link(Ground, clip::WALK_STOP_RIGHT, state::STAND),
        link(Ground, clip::WALK_STOP_LEFT, state::STAND),
        link(Ground, clip::HIT_WALL_EARLY, state::STAND),
        link(Ground, clip::HIT_WALL_LATE, state::STAND),
        link(Backward, clip::HOP_BACK, state::STAND),
        link(Roll, clip::ROLL, state::STAND),
        link(Jump, clip::JUMP_FORWARD_TAKEOFF, state::FORWARD_JUMP),
        link(Jump, clip::HEAVY_LANDING, state::STAND),
        link(Climb, clip::VAULT_TWO, state::STAND),
        link(Climb, clip::VAULT_THREE, state::STAND),
        link(Use, clip::PICKUP, state::STAND),
        link(Use, clip::USE_KEY, state::STAND),
        link(Use, clip::USE_PUZZLE, state::STAND),
        link(Use, clip::SWITCH_OFF, state::STAND),
        link(Use, clip::SWITCH_ON, state::STAND),
        link(Use, clip::BUTTON_OFF, state::STAND),
        link(Use, clip::BUTTON_ON, state::STAND),
        link(Use, clip::LEVER, state::STAND),
    ]
};

/// What a clip's commands must include.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// An end-of-clip position command (climbs and pull-ups).
    SetPosition,
    /// An end-of-clip launch (takeoffs).
    JumpVelocity,
    /// An end-of-clip release of the hands (using objects).
    EmptyHands,
}

#[derive(Debug, Clone, Copy)]
pub struct CommandRule {
    pub clip: usize,
    pub command: CommandKind,
    pub feature: Feature,
}

pub const COMMANDS: &[CommandRule] = &[
    CommandRule { clip: clip::VAULT_TWO, command: CommandKind::SetPosition, feature: Feature::Climb },
    CommandRule { clip: clip::VAULT_THREE, command: CommandKind::SetPosition, feature: Feature::Climb },
    CommandRule { clip: clip::PULL_UP, command: CommandKind::SetPosition, feature: Feature::Climb },
    CommandRule { clip: clip::JUMP_UP_PREPARE, command: CommandKind::JumpVelocity, feature: Feature::Jump },
    CommandRule { clip: clip::JUMP_FORWARD_TAKEOFF, command: CommandKind::JumpVelocity, feature: Feature::Jump },
    CommandRule { clip: clip::STANDING_TAKEOFF, command: CommandKind::JumpVelocity, feature: Feature::Jump },
    CommandRule { clip: clip::PICKUP, command: CommandKind::EmptyHands, feature: Feature::Use },
    CommandRule { clip: clip::USE_KEY, command: CommandKind::EmptyHands, feature: Feature::Use },
    CommandRule { clip: clip::USE_PUZZLE, command: CommandKind::EmptyHands, feature: Feature::Use },
    CommandRule { clip: clip::SWITCH_OFF, command: CommandKind::EmptyHands, feature: Feature::Use },
    CommandRule { clip: clip::SWITCH_ON, command: CommandKind::EmptyHands, feature: Feature::Use },
    CommandRule { clip: clip::BUTTON_OFF, command: CommandKind::EmptyHands, feature: Feature::Use },
    CommandRule { clip: clip::BUTTON_ON, command: CommandKind::EmptyHands, feature: Feature::Use },
    CommandRule { clip: clip::LEVER, command: CommandKind::EmptyHands, feature: Feature::Use },
];

/// Something a level's player model or table lacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractIssue {
    pub feature: Feature,
    pub what: String,
}

/// The result of [`check`]: every issue, by feature.
#[derive(Debug, Clone, Default)]
pub struct ContractReport {
    pub issues: Vec<ContractIssue>,
}

impl ContractReport {
    /// Issues of `feature`.
    pub fn missing(&self, feature: Feature) -> impl Iterator<Item = &ContractIssue> {
        self.issues.iter().filter(move |i| i.feature == feature)
    }

    /// True when the level provides every one of `features`.
    pub fn meets(&self, features: &[Feature]) -> bool {
        self.issues.iter().all(|i| !features.contains(&i.feature))
    }

    /// The features with no issues.
    pub fn provided(&self) -> Vec<Feature> {
        Feature::ALL.into_iter().filter(|f| self.missing(*f).next().is_none()).collect()
    }
}

impl std::fmt::Display for ContractReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for issue in &self.issues {
            writeln!(f, "{:?}: {}", issue.feature, issue.what)?;
        }
        Ok(())
    }
}

/// Check `level`'s player model (type [`LARA`](super::LARA)) and animation
/// table against the contract, reporting what each feature lacks.
pub fn check(level: &Level) -> ContractReport {
    let mut report = ContractReport::default();
    let mut issue = |feature: Feature, what: String| report.issues.push(ContractIssue { feature, what });
    let Some(model) = level.models.iter().find(|m| m.id == super::LARA) else {
        for feature in Feature::ALL {
            issue(feature, format!("no model of type {}", super::LARA));
        }
        return report;
    };
    let clips = &level.animations;
    let [first, end] = model.animations;
    if first != 0 {
        issue(
            Feature::Ground,
            format!("the player model's clips start at {first}; clip numbers are absolute, so they must start at 0"),
        );
    }
    let owned = |n: usize| n >= first && n < end.min(clips.len());
    let state = |n: usize| owned(n).then(|| clips[n].state);

    for r in CLIPS {
        match state(r.clip) {
            None => issue(r.feature, format!("clip {} is missing (needs state {})", r.clip, r.state)),
            Some(s) if s != r.state => issue(r.feature, format!("clip {} has state {s}, needs {}", r.clip, r.state)),
            Some(_) => {}
        }
    }

    // Follow end links from `n` (at most a few) to a clip of `goal`.
    let reaches = |mut n: usize, goal: usize| {
        for _ in 0..8 {
            match state(n) {
                Some(s) if s == goal => return true,
                Some(_) => n = clips[n].next_animation,
                None => return false,
            }
        }
        false
    };
    for r in DISPATCHES {
        if !owned(r.from) {
            continue; // Reported as a missing clip.
        }
        let ranges: Vec<_> = clips[r.from].state_changes.iter().filter(|c| c.state == r.goal).collect();
        if ranges.is_empty() {
            issue(r.feature, format!("clip {} has no range for state {}", r.from, r.goal));
        } else if !ranges.iter().any(|c| reaches(c.animation, r.goal)) {
            issue(r.feature, format!("clip {}'s ranges for state {} lead to no clip of that state", r.from, r.goal));
        }
    }
    for r in LINKS {
        if owned(r.from) && !reaches(clips[r.from].next_animation, r.state) {
            issue(r.feature, format!("clip {} does not end in a clip of state {}", r.from, r.state));
        }
    }
    for r in COMMANDS {
        if !owned(r.clip) {
            continue;
        }
        let has = clips[r.clip].commands.iter().any(|c| match r.command {
            CommandKind::SetPosition => matches!(c, Command::SetPosition(_)),
            CommandKind::JumpVelocity => matches!(c, Command::JumpVelocity(_)),
            CommandKind::EmptyHands => matches!(c, Command::EmptyHands),
        });
        if !has {
            issue(r.feature, format!("clip {} lacks a {:?} command", r.clip, r.command));
        }
    }

    // The walk loop holds the walking foot windows, in absolute frames.
    if owned(clip::WALK) {
        let walk = &clips[clip::WALK];
        if walk.frame_start != frame::WALK_LOOP.start || walk.frame_end + 1 != frame::WALK_LOOP.end {
            issue(
                Feature::Ground,
                format!(
                    "walk loop {} covers frames {}..={}, needs {}..={}",
                    clip::WALK,
                    walk.frame_start,
                    walk.frame_end,
                    frame::WALK_LOOP.start,
                    frame::WALK_LOOP.end - 1
                ),
            );
        }
    }

    // Every key poses every joint, so the model can be drawn and its spheres
    // posed in any clip.
    let joints = model.joints.len();
    if model.meshes.len() != joints {
        issue(Feature::Ground, format!("{} meshes for {joints} joints", model.meshes.len()));
    }
    for (n, a) in clips.iter().enumerate().take(end.min(clips.len())).skip(first) {
        if a.poses.is_empty() {
            issue(Feature::Ground, format!("clip {n} has no keys"));
        } else if let Some(p) = a.poses.iter().find(|p| p.angles.len() != joints) {
            issue(Feature::Ground, format!("clip {n} has a key for {} joints, the model has {joints}", p.angles.len()));
        }
        if a.bounds.len() < a.poses.len() {
            issue(Feature::Ground, format!("clip {n} has {} bounds for {} keys", a.bounds.len(), a.poses.len()));
        }
        if !owned(a.next_animation) {
            issue(Feature::Ground, format!("clip {n} ends in clip {}, outside the player's clips", a.next_animation));
        }
        for c in &a.state_changes {
            if !owned(c.animation) {
                issue(
                    Feature::Ground,
                    format!(
                        "clip {n}'s range for state {} enters clip {}, outside the player's clips",
                        c.state, c.animation
                    ),
                );
            }
        }
    }
    if end < CLIP_COUNT {
        issue(
            Feature::Ground,
            format!("{} clips; the controller names clips up to {}", end.saturating_sub(first), CLIP_COUNT - 1),
        );
    }

    if joints < joint::COUNT {
        issue(Feature::Skeleton, format!("{joints} joints, Lara's layout has {}", joint::COUNT));
    } else {
        for (j, parent) in joint::PARENTS.iter().enumerate() {
            if model.joints[j].parent != *parent {
                issue(
                    Feature::Skeleton,
                    format!("joint {j} hangs from {:?}, needs {:?}", model.joints[j].parent, parent),
                );
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::world::format::{Animation, Joint, Model, Pose, StateChange};

    fn clip(state: usize, next: usize, ranges: &[(usize, usize)]) -> Animation {
        Animation {
            frame_rate: 1,
            frame_start: 0,
            frame_end: 0,
            next_animation: next,
            state,
            poses: vec![Pose { root: [0, 0, 0], angles: vec![[0, 0, 0]] }],
            bounds: vec![[0; 6]],
            state_changes: ranges
                .iter()
                .map(|&(state, animation)| StateChange { state, first_frame: 0, last_frame: 0, animation, frame: 0 })
                .collect(),
            ..Default::default()
        }
    }

    /// A one-joint model whose only clip stands.
    fn standing_only() -> Level {
        let mut level = Level { meshes: vec![Default::default()], ..Default::default() };
        level.models.push(Model {
            id: 0,
            meshes: vec![0],
            joints: vec![Joint { parent: None, offset: [0; 3] }],
            animations: [0, 1],
            initial_pose: None,
        });
        level.animations.push(clip(state::STAND, 0, &[]));
        level
    }

    #[test]
    fn a_standing_only_model_misses_ground_locomotion() {
        let report = check(&standing_only());
        assert!(!report.meets(&[Feature::Ground]));
        let ground: Vec<_> = report.missing(Feature::Ground).map(|i| i.what.as_str()).collect();
        assert!(ground.contains(&"clip 0 has state 2, needs 1"), "{report}");
        assert!(ground.contains(&"clip 11 is missing (needs state 2)"), "{report}");
        assert!(report.missing(Feature::Skeleton).any(|i| i.what == "1 joints, Lara's layout has 15"), "{report}");
    }

    #[test]
    fn a_wrong_state_and_a_missing_range_are_reported() {
        let mut level = standing_only();
        level.animations = (0..CLIP_COUNT).map(|_| clip(state::STAND, clip::STAND, &[])).collect();
        level.models[0].animations = [0, CLIP_COUNT];
        let report = check(&level);
        assert!(report.missing(Feature::Ground).any(|i| i.what == "clip 0 has state 2, needs 1"), "{report}");
        assert!(report.missing(Feature::Ground).any(|i| i.what == "clip 11 has no range for state 1"), "{report}");
        // Standing clips that stand are fine.
        assert!(!report.missing(Feature::Ground).any(|i| i.what.starts_with("clip 103 has state")), "{report}");
    }

    #[test]
    fn no_player_model_fails_every_feature() {
        let mut level = standing_only();
        level.models.clear();
        assert!(check(&level).provided().is_empty());
    }
}
