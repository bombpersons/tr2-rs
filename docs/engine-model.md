# Engine model

What Tomb Raider II treats as **data**, which `engine/` interprets, and what it
treats as **code**, which lives in `game/` as per-type controllers. This
document defines the engine's scope; keep it current.

Provenance labels:
- **PSX**: established in this project from the supplied disc with `psx`
  (runtime watches, samples, captures, disassembly). Addresses are for USA Rev 3.
- **Structural**: the importer decodes the file completely and checks every
  reference (`docs/format-notes.md`), but the runtime meaning has not been
  confirmed with `psx`.
- **Inherited**: from work done before the reference restriction (OpenLara /
  TR2Main field names). Treat as provisional until a PSX observation backs it.

## World

**Rooms on a sector grid, joined by portals.** Each level file
(`DATA/*.PSX`) holds rooms. A room is a mesh of quads and triangles plus a
grid of 1024-unit sectors. Each sector carries a floor and ceiling height,
links to the rooms above and below, and an index into a shared floor-data
word list. *Structural.*

- **Collision** is derived from sectors, not from the render mesh. Floor and
  ceiling heights come from the sector, adjusted by slant commands in floor
  data, and room links are followed vertically. Lara's probes go through
  `GetCollisionInfo` `0x8001b018`: quadrant `(facing+8192)>>14`, front probe at
  radius 100, plus left and right corners (*PSX*, `docs/locomotion-evidence.md`).
  The slant rounding (positive tilts truncate, negative tilts round away from
  zero) was confirmed on Wall ramps (*PSX*). Static-mesh boxes and object
  surfaces (bridges, trapdoors) are not ported yet.
- **Visibility** is per room through portals. The port currently hides rooms
  out of view in `engine::render::room_visibility`; the original's portal
  clipping has not been examined with `psx` (*provisional*).
- **Local ground targets** read authored boxes, overlap lists and movement
  zones. Reverse search filters floor differences against per-creature limits
  (`0x800151F0..0x80015224`); the directly adjacent corridor query is shared
  engine machinery. The attendant's step256/drop512 limits and ordinary
  zone selection live with its game tuning. Native Home data and four complete
  roll/contact routes verify a lower-floor target (*PSX*,
  [navigation-height evidence](navigation-height-evidence.md)). The full
  path search (the original's LOT: SearchLOT, UpdateLOT, TargetBox,
  CalculateTarget) is now engine machinery, `engine::world::BoxSearch`.
  Runtime blocked-box flags live in `engine::world::BoxFlags`, kept apart
  from `WorldGeometry` (*PSX*, [creature evidence](creature-evidence.md)).
- **Engine:** `engine::world` (`Level` asset, `WorldGeometry` queries).
  Integer world units and y-down are kept in the format and in collision
  queries, because the trajectory tests match the original's integer rounding
  exactly (`engine/src/world/format.rs`). Conversion to Bevy metres happens once,
  in `world::position`.

## Movement

**Animation tables drive movement; per-state code makes the decisions.**

- **Data:** each animation has a 16.16 speed and acceleration, a frame rate,
  next animation/frame, *state change* ranges (goal state → target clip and
  frame) and commands (position jumps, sounds, flags). These are tables in the
  level file. *Structural*, with runtime use confirmed for Lara: clip
  chaining and goal dispatch (clip 51 → 52 → 6, clip 82 landing, clip 71 slide
  stop) match per tick in `game/tests/player.rs` (*PSX*).
- **Code:** Lara's control `0x80038194` calls a handler per current state
  through the table at `0x80082c98`. Each handler reads input, sets the goal
  state and adjusts the shared turn rate `0x8008c570`, which then decays by 364
  and is added to yaw (*PSX*, `docs/locomotion-evidence.md`). Collision
  responses are separate code, such as run collision `0x8003a844` and the
  vault test `0x8003d924`. Horizontal speed comes from the animation; fall
  speed increases by 6 per tick in code (*PSX*, `docs/original-evidence.md`).
- **Engine:** `engine::animation` (`AnimationLibrary::dispatch`, clip
  advance, commands, posing): what the original reads from tables.
  **Game:** `game::player`: the state handlers, turn rules, vault and wall
  tests, ported per handler with their constants and quirks.
- **Placed-object pose timing:** `AnimationGoal` opts objects into authored
  dispatch and next links in the engine; doors use this path. Wall type 107
  matches a complete open/close cycle and held-out reversal per tick (*PSX*,
  [door-animation-evidence](door-animation-evidence.md)). Frame/end cues are
  engine messages; object movement/effect commands remain controller work.
  The editor uses the same step for silent model previews. Legacy creature
  clip selection remains provisional.
- **Drawing on top of clips:** Lara's draw code `0x80025BDC` is per-type
  code: it turns torso and head by the gun angles and draws the gun arms
  from another object's frames, oriented from Lara's root, and the gun code
  rewrites her mesh table (*PSX*, `docs/pistols-evidence.md`). **Engine:**
  the generic means (`PoseOverrides`, `JointMeshes`, `JointAttachment`,
  `DynamicLights` for the original's per-frame dynamic light table
  `0x8008BB88`). **Game:** `game::player::arms` chooses joints, frames,
  meshes and lights.

## Objects

**Placed items with a type id; behaviour is code per type.** Each level lists
placements (type id, room, position, rotation, intensity, activation flags);
models and animations are found by type id. *Structural.* Lara's item record
layout (state, goal, animation, frame, room, speed, fall speed, health,
position, rotation) was confirmed at the item pointed to by `0x8008c6b8`
(*PSX*, `docs/original-evidence.md`).

- **The type table** (*PSX*, `docs/object-table-evidence.md`): 265 records
  of 48 bytes at `0x800936D0`, indexed by type id. Each record has
  - **model data from the level file**: mesh count, first mesh, joint tree,
    frame base, first animation, and a "present in this level" flag bit;
  - **function pointers**: initialise (+0x0C), control (+0x10), floor
    (+0x14), ceiling (+0x18), draw (+0x1C, default `0x800257B4`),
    collision (+0x20);
  - **setup values**: hit points (+0x26, default -16384), an unnamed
    creature value (+0x28), radius (+0x2A, default 10), shadow size (+0x2C)
    and behaviour flag bits (+0x2E).
  The level loader `0x8006A6AC` resets every record (`0x8005717C`) and then
  runs three straight-line setup routines (`0x800538A0`, `0x80055774`,
  `0x80055FE4`). For each type present in the level, they store that type's
  function pointers and setup values. Setup values therefore live in a
  table, but the table is written by code, not read from the level file.
  InitialiseItem `0x80037690` copies the first animation and hit points into
  each placed item, then calls the type's initialise.
- **Shared controllers:** one function serves a whole group of ids: switch
  control `0x800516B8` for 92, 93, 103-105; door functions for 106-113;
  pickup collision `0x800502D4` for 135-152, 174-177, 190-196, 205, 206;
  keyholes 201-204 and puzzle holes 182-185. These match the port's
  grouping in `object_types.json` (*PSX*).
- **Who calls what:** control runs from the active-item and active-effect
  lists in the control phase. Collision runs from **Lara's** control
  (`0x8001C258`) for items in her room and its portal neighbours. Draw runs
  from the room draw (`0x800253F8`). Lara has no control pointer: the
  control phase calls her directly, after all items (see Frame).
- **Engine:** `engine::objects`: placements become `WorldObject` entities;
  `register_object_type(id, ObjectType)` attaches a type's hooks; unknown ids
  are counted in `ObjectDiagnostics`.
  **Game:** `game::interactions` registers each type. It reads the types from
  `game/data/object_types.json` (pickup, switch, door, creature health,
  radius and damage). Creature health and radius for 36, 37 and 39 now match
  the setup code (*PSX*). `damage` is not a table field and is still
  *Inherited*. That file must not grow into a generic behaviour schema:
  creature AI stays per-type code.
- **What should move (not done yet):**
  1. `creature.health` and `creature.radius` are per-type setup data in the
     original. They belong in an engine-level per-type setup record:
     `hit_points`, `radius`, `shadow_size`, the +0x28 value and flags, with
     the original's defaults (-16384, 10, 0, 0). The game would fill it from
     a per-type asset, and the engine's InitialiseItem equivalent would copy
     `hit_points` into each spawned object, as `0x80037690` does. They do
     not belong in the level format: the original does not store them per
     level.
  2. `creature.damage` stays in the controller's own config. It is a design
     number inside the controller (placement rule 4). Find each controller's
     constant (`0x8005C1EC`, `0x8005C4DC`, `0x80021FCC`) before relabelling
     it.
  3. Kinds (`switch`, `door`, `pickup`, `keyhole`, `spikes`) are the port's
     equivalent of the function pointers. They stay registrations in game
     code, now backed by PSX for every id listed above.
  4. The engine's `ObjectType` has init and spawn hooks only. The original
     has six hooks per type, and collision is driven by the player's
     controller, not by a separate phase.

## Level scripting

**Floor-data trigger lists, no bytecode.** A sector's floor data can hold a
trigger: a type (on enter, pad, switch, key, pickup, ...), timer, once bit
and activation mask, followed by actions (activate object, camera shot,
music, end level, secret, flip map, ...). *Structural*: the importer
(`tools/import/src/convert.rs`, `floor_info`) decodes each chain up to its
end bit or the first unknown function. Camera actions consume an extra
settings word, and its end bit is honoured. This was confirmed on the WALL
chain at floor-data offset 3391 and is covered by
`real_wall_start_camera_operand_is_never_an_object_activation`
(`game/tests/interactions.rs`).
Fixed-shot selection, once flags, timers and eye easing are *PSX*
(`docs/camera-evidence.md`).

- **Engine:** `engine::triggers` tests the sector under the `Avatar` each
  tick and runs `activate` and `camera` itself. Other actions go through
  `register_trigger_action(name)` and arrive as `TriggerAction` messages;
  unregistered actions are listed in `TriggerDiagnostics`. An `activate`
  gives the object the trigger's timer (`WorldObject::timer`, in ticks) as
  well as toggling its code; the object's own controller counts it with
  `WorldObject::trigger_active`, as the original's controls call
  TriggerActive (*PSX*: the Rig's timed door,
  [interaction-evidence](interaction-evidence.md)).
  **Game:** `game::interactions` handles `end_level`, `music` and `secret`.
  Flip maps, flyby cameras and other actions are not ported and stay reported.

## Frame

**NTSC, logic every second field (30 Hz).** Yaw stores by `0x800383e4`
land on every other field (`.psx/home_right_watch`), so `REGION = Ntsc` and
`FIELDS_PER_TICK = 2` (*PSX*). Display is 368x240 of a 384x240 projection
with the centre at 192x120 (`0x80065c48`, `0x80065c90`) and projection distance
228 (`0x80065cb8`) (*PSX*, `game::display`).

Order within a tick (`engine::schedule::WorldSystems`, chained):
Prepare → Initialize (Reset, Initialize) → Items (Control, Animation) →
Control → Motion → Animation → Interaction (Use, Resolve, Publish, Combat,
Triggers, Report) → SecondaryMotion → Camera → Effects.

The implemented phase boundaries are checked against a fresh native call
trace and a held-out turn ([tick-order-evidence](tick-order-evidence.md)).

- Lara's part is *PSX* at the level of one controller: the handler runs, then
  turn decay and yaw, and displacement uses last tick's heading
  (`docs/locomotion-evidence.md`). Camera after Lara is consistent with the
  camera captures (`docs/chase-camera-evidence.md`).
- **The original's tick** (*PSX*, `docs/object-table-evidence.md`, runs
  `.psx/main_loop_trace` and `.psx/object_table_reads`). The game loop
  `0x8006DD98` alternates ControlPhase `0x8001D7FC` and DrawPhase
  `0x80023DEC`. DrawPhase returns the number of fields elapsed. ControlPhase
  adds that number (at most 10) to an accumulator and runs one tick per 2
  fields, carrying the remainder. Each tick, in order:
  1. level hook `0x80011F2C`, input `0x80069320`, then menu/demo exits;
  2. **every active item's control**, in active-list order; the next link is
     read before each call;
  3. **every active effect's control**;
  4. **Lara** `0x800424F0`, which runs, in order: state handler, turn and
     yaw, animation step `0x80042E08`, **object collision** `0x8001C258`
     (pickups, switches, doors, keyholes, creature push), state collision,
     room update, weapons (LaraGun `0x8003FC4C`, ported in
     `game::player::weapons`, [pistols-evidence](pistols-evidence.md)),
     **TestTriggers** `0x8001EE30`;
  5. hair `0x8002F674`;
  6. camera `0x80019BF0`;
  7. sound sources, flip effect and sound update `0x800290E4`;
  8. timers.
  Drawing (including each item's draw pointer) happens after all ticks of
  that frame.
- **Current `engine::schedule` boundaries** (*PSX* call ordering;
  [tick-order-evidence](tick-order-evidence.md)):
  - `LifecycleSystems` resets level state and initializes object components
    before the first item controller runs. Native initialization is at load /
    spawn; the port still defers hooks until the first eligible tick.
  - Creature and door controllers run in `ItemSystems::Control`, before
    Lara's Control/Motion. Ported creatures run in active-list order (newest
    first) through `game::creature`; activation follows the trigger's action
    order via `engine::triggers::ObjectActivated`. They see the previous player position and cannot
    respond to a floor trigger until the next tick's item phase.
  - Generic object animation follows those controllers. Lara's pose commit
    runs after Motion and before nearby object interaction; recorded
    player/camera trajectories remain exact.
  - LaraGun is `PlayerSystems::Weapons` in `InteractionSystems::Combat`,
    after Publish and before the floor triggers; gun hits are `GunHit`
    messages applied by `game::interactions::combat` in the same set.
    Gameplay random draws use `engine::random::GameRandom` (the original's
    control generator at `0x80089D54`); the gun's pistol spread is its only
    ported consumer. The combat camera is the engine's aiming mode, requested
    through `Avatar::aim`.
  - Use and Resolve now both precede floor triggers. The Publish set copies
    the collision result to Avatar before combat resolution and triggers.
    This fixes triggers using the position before a door's push.
  - Hair is in SecondaryMotion before Camera; sound remains after Camera.
  - Remaining differences: active-item order and room-list order are
    modelled only for ported creatures; no effect lists; legacy creature/scenery clip stepping remains provisional;
    state/level collision is still in Motion before object collision;
    use and pushes remain separate passes. The new tests verify the phase
    order and its consequences, not unverified per-type AI/damage/animation.

## Open questions (evidence to collect)

1. ~~The object type table~~: answered (*PSX*, `docs/object-table-evidence.md`).
   Still open: what the +0x28 value means, flag bits 17-23, and creature
   damage constants inside the controllers.
2. ~~The main loop's per-tick order~~: answered (*PSX*, same note).
3. Room visibility: portal clipping rules.
4. Floor-data trigger types and actions beyond those exercised in Wall. The
   dispatchers are known: 8 trigger types through `0x800100FC` and 12
   actions through `0x8001011C`, both in TestTriggers `0x8001EE30`.

Chase framing accepts an optional per-tick elevation request on `Avatar` (radians);
the game owns water/state selection and the engine owns camera geometry. This is
runtime state and does not change level serialization. Wading evidence is in
[the dedicated report](wading-evidence.md).
