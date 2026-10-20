# Lara's asset contract

Lara's controller (`game/src/player.rs`, `game/src/player/*.rs`) is driven by
her animation table. It asks for clips by number, waits on the clips' declared
states and authored state-change ranges, reads some clips' commands, and
reads a few absolute frame numbers. Her presentation (arms, guns, head and
braid) turns and swaps joints by number. This page lists everything a
replacement model has to provide for the ported controller to drive it.

The numbers live in one place, `game::player::contract`
(`game/src/player/contract.rs`): `clip::*`, `state::*`, `frame::*` and
`joint::*`. The controller uses those constants. `contract::check(&Level)`
tests a level's model 0 and its table against the rules below and reports
what each feature group is missing (`ContractReport::meets`, `missing`,
`provided`). Tests:

- `game/tests/lara_contract.rs`: Wall passes every group, and every playable
  level passes ground, jump, climb and skeleton.
- `game/src/player/contract.rs` (unit tests): hand-made models that fail.
- `mods/example/tests/rules.rs`: the example mod's character passes ground
  and skeleton.

The numbers and states below are the ones in the levels' shared Lara table
(WALL.PSX, as baked). The controller code cites the PSX routines that use
each one. This page is written from that code and the baked tables, not from
any other reimplementation.

## How numbers are read

- **Clip numbers are absolute.** The controller's `PlayerAnimations` is the
  level's whole animation table (`engine::animation::AnimationLibrary`, built
  from `Level::animations`). So model 0's clips must start at index 0
  (`model.animations[0] == 0`), and the table needs every number the
  controller names: up to `contract::CLIP_COUNT - 1` = 197.
- **A clip's state** is `Animation::state`. The controller compares the
  playing clip's state (`clips.state(n)`) and asks for goal states.
- **State-change frames are absolute** in the file (`StateChange::first_frame`
  and `last_frame` count across the whole table). `AnimationLibrary::dispatch`
  advances one frame and then looks for a range for the goal
  (`engine/src/animation.rs:143`).
- **Speeds** come from clips: `(speed + accel * frame) >> 16` units per frame
  (`AnimationLibrary::speed`). Without a table the controller falls back to
  Wall's built-in speeds (`ground_speed`, `game/src/player.rs:614`).
- A grounded spawn starts at clip 0 frame 0 (`AnimationState::default()`,
  `game/src/player.rs:472`). The first standing tick leaves it through clip 0's
  goal-2 range. A mid-air spawn starts at clip 34 frame 0
  (`game/src/player.rs:472`) with goal 3 (`game/src/player.rs:479`).

## Feature groups

`contract::Feature` splits the contract into groups a character can provide
one at a time:

| Group | What it adds |
|---|---|
| `Ground` | Model 0 and its table's numbering; stand, walk, run, the turns, stopping, running into a wall. **Needed to spawn and walk.** |
| `Steps` | Walking a click (256) up or down. |
| `Backward` | Walking backward, the backward hop and its fall. |
| `Sidestep` | Sidesteps. |
| `Roll` | The roll. |
| `Jump` | Jumps, flight, falls and landings. |
| `Climb` | Vaults, hanging and pulling up. |
| `Slide` | Slopes. |
| `Water` | Wading, the surface and swimming. |
| `Death` | Death clips. |
| `Skeleton` | Lara's 15-joint layout, for the arms, guns, head and braid. |
| `Use` | Picking up, switches, keys and puzzle pieces. |

### Which subset spawns, and why

Only `Ground` is needed to spawn, stand, walk, run, turn and stop on a level
with no slopes, water or drops. The other groups are reached only through
their own input (Square, Circle, Down, R2) or through geometry (drops of more
than 384, slopes, water, ledges).

The table must still have an entry for every clip number up to 193, because
the controller enters numbers directly. A lookup past the table's end panics
in two places:

- walk collision (`game/src/player/forward.rs:77`, `:109`);
- walk-back collision (`game/src/player/backward.rs:113`).

Hanging calls `vertical_bounds(..).unwrap()` on clip 96
(`game/src/player/traversal.rs:727`).

A character without some groups can fill the unused numbers with one-frame
standing clips (state 2, ending in clip 11), as the example mod does. Moves
it lacks then show the standing pose. `check` still reports those groups as
missing, because the fillers declare the wrong states.

## Clips

`file:line` is where the controller uses the number. "Entered" means it
plays the clip directly. "Requested" means it asks for the clip and authored
ranges decide when the request takes effect. "Compared" means it tests the
playing clip.

### Ground

| Const | Clip | State | Use |
|---|---|---|---|
| `RUN` | 0 | 1 run | The run loop; the grounded spawn clip. Run collision picks the wall-hit clip by its frame (`player.rs:1513`). The jump latch reads its frame 4 (`player.rs:827`). |
| `WALK` | 1 | 0 walk | The walk loop. It must cover absolute frames 22..=57 (see Frames). |
| `WALK_STOP_RIGHT`, `WALK_STOP_LEFT` | 2, 3 | 0 | Walking into a wall stops on a foot (`forward.rs:79`, `:81`), and the stop holds until the clip links out (`forward.rs:41`). |
| `RUN_START` | 6 | 1 | Requested by running (`player.rs:1685`, `forward.rs:25`). Never hits a wall (`player.rs:1511`). Clears the jump latch (`player.rs:825`). |
| `STAND` (`STANDING_CLIP`) | 11 | 2 stand | Standing request and stop (`player.rs:1678`, `forward.rs:94`). The camera frames it at startup (`player.rs:292`). |
| `TURN_RIGHT`, `TURN_LEFT` | 12, 13 | 6, 7 | Slow turns (`player.rs:1666`, `:1668`); a wall stop keeps a held turn (`player.rs:1548`). |
| `WALK_START` | 20 | 0 | Requested by walking (`player.rs:1678`, `forward.rs:25`). |
| `FAST_TURN_RIGHT` | 44 | 20 fast turn | Requested once the turn rate passes 364 (`player.rs:1664`). |
| `FAST_TURN_LEFT` | 69 | 20 | Reached only through clip 13's goal-20 range (not named in code). |
| `HIT_WALL_EARLY`, `HIT_WALL_LATE` | 53, 54 | 2 | Running into a wall before or after run frame 10 (`player.rs:1515`). |
| `STAND_IDLE` | 103 | 2 | What standing requests (`player.rs:1670`). Recovery compares it (`traversal.rs:559`). |

### Other groups

| Group | Const = clip (state) | Where |
|---|---|---|
| Steps | `STEP_UP_LEFT` 57, `STEP_UP_RIGHT` 58, `STEP_DOWN_LEFT` 59, `STEP_DOWN_RIGHT` 60 (all 0) | `forward.rs:113-122` |
| Backward | `WALK_BACK` 40 (16), `BACK_STEP_DOWN_LEFT` 61, `BACK_STEP_DOWN_RIGHT` 62 (16), `HOP_BACK` 88 (5), `FALL_BACK` 93 (29) | `player.rs:1690`, `:1695`, `:1696`; `backward.rs:103`, `:115-119` |
| Sidestep | `STEP_LEFT` 65 (22), `STEP_RIGHT` 67 (21) | `player.rs:1701`, `:1708`; `traversal.rs:540-541` |
| Roll | `ROLL` 146 (45), entered at frame 2 | `rolling.rs:24`, `player.rs:1755` |
| Jump | `RUN_JUMP` 16 (3), `FAST_FALL` 23 (9), `HEAVY_LANDING` 24 (2), `JUMP_UP_PREPARE` 26 (19), `JUMP_UP` 28 (28; entered at frame 9 on letting go of a ledge), `JUMP_IMPACT` 31 (2), `REBOUND_FALL` 32 (9, entered at frame 1), `FALL` 34 (3), `JUMP_PREPARE` 73 (15), `JUMP_BACK` 75 (25), `JUMP_FORWARD_TAKEOFF` 76 (3), `JUMP_FORWARD` 77 (3), `JUMP_LEFT` 79 (27), `JUMP_RIGHT` 81 (26), `STANDING_TAKEOFF` 91 (28) | `player.rs:631-637`, `:1134`, `:1721-1752`; `traversal.rs:373`, `:420`, `:424`, `:491`, `:816` |
| Climb | `VAULT_THREE` 42 (2), `VAULT_TWO` 50 (2), `HANG` 96 (10; entered at frame 0 or 12), `PULL_UP` 97 (19) | `player.rs:548-553`, `:1467`, `:1745`, `:1751`; `traversal.rs:726` |
| Slide | `SLIDE` 70 (24), `SLIDE_STOP` 71 (24), `BACK_SLIDE` 104 (32), `BACK_SLIDE_LOOP` 105 (32) | `player.rs:951`, `:1713-1718`; `traversal.rs:513-515` |
| Water | `SWIM` 86 (17), `TREAD` 108 (13), `SURFACE_TREAD` 110 (33), `CLIMB_OUT_HIGH` 111 (55), `WATER_ENTRY` 112 (35), `SURFACING` 114 (33), `SURFACE_FORWARD` 116 (34), `DIVE` 119 (35), `SURFACE_BACK` 140 (47), `SURFACE_LEFT` 143 (48), `SURFACE_RIGHT` 144 (49), `WADE_LOOP` 177 (65), `WADE` 186 (65), `STEP_OUT` 190 (55), `CLIMB_OUT_MID` 191 (55), `STAND_UP` 192 (55), `CLIMB_OUT_LOW` 193 (55) | `player.rs:1673`, `:1756-1761`; `surface.rs:30-33`, `:375-388`, `:455-459`; `wading.rs:56-59`, `:98` |
| Death | `FATAL_LANDING` 25 (8), `DEAD` 87 (18 in the original table) | `player.rs:1749`, `:1764` |
| Use | `PICKUP` 135 (39; the item is taken on frame 42, `frame::PICKUP_TAKE`), `UNDERWATER_PICKUP` 130 (39; reached from treading's range, ends in tread 108; taken on frame 18, `frame::UNDERWATER_PICKUP_TAKE`), `USE_KEY` 131 (42), `USE_PUZZLE` 134 (43; the piece goes in on frame 80), `SWITCH_OFF` 63 (40), `SWITCH_ON` 64 (41), `BUTTON_OFF` 195 (40), `BUTTON_ON` 196 (41), `LEVER` 197 (40, used both ways). Pickups, keys and puzzles enter through standing's ranges (clip 103 to states 39, 42, 43); the switch clips are set directly, by switch type (`object_types.json` `switch.clips`). Each ends in standing. | `interactions/usable.rs`, `player/using.rs` |

## States

`contract::state` names every state the controller asks for or compares.
Where they matter most:

- **Ground.** 0 walk, 1 run, 2 stand, 6/7 slow turn right/left, and 20 fast
  turn decide `step_animation`'s dispatch (`player.rs:1771-1850`). Run and
  walk keep their mode while their clip's state is still 1 or 0
  (`player.rs:1055`, `:1059`), and turn to standing once the next clip's state
  is 2 (`player.rs:1117`).
- **Back and side.** 5, 16, 21 and 22 always wait for an authored range
  (`player.rs:1795`). 5 asks for 2 (`player.rs:1807`).
- **Jumps.** Takeoff goals are 3, 25, 26 and 27 (`traversal.rs:127-130`).
  Flight goals are 11 reach, 9 fast fall, 52 swan dive and 68 twist
  (`traversal.rs:152-163`). Landing goals are 1, 2 and 8
  (`traversal.rs:155`, `:807`).
- **Climb.** 10 hang, 19 pull up, 28 jump up (`traversal.rs:285`, `:379`,
  `:396`, `:736`).
- **Water.** Underwater: 13 tread, 17 swim, 18 glide, 35 dive, 44 death
  (`swimming.rs:84-88`). Surface: 33, 34, 47, 48, 49 (`surface.rs:24-28`).
  Wading: 65, and 55 climbing out (`wading.rs:126`, `:327`).
- **Roll.** 45 roll and 23 back roll (`rolling.rs:53`, `:130`).
- **Slide.** 24 slide and 32 back slide (`player.rs:60`, `:1376`).

### State changes the controller relies on

`contract::DISPATCHES`: while clip `from` plays, a range for `goal` exists
and leads, through end links, to a clip of state `goal`.

| Group | From clip → goal |
|---|---|
| Ground | 11 and 103 → 0, 1, 6, 7; 0 → 2; 1 → 2; 12 and 13 → 2 and 20; 44 and 69 → 2 |
| Backward | 103 → 16, 5; 40 → 2 |
| Sidestep | 103 → 21, 22; 65 and 67 → 2 |
| Jump | 103 → 15 and 28; 73 → 3; 0 → 3; 77 → 2; 34 → 11 |
| Climb | 96 → 19 |
| Slide | 70 → 2 |
| Water | 110 → 34; 108 → 17 |

These ranges must exist, but their frame windows are the character's to
choose. Wall's run loop, for example, can stop on either foot (frames 0-5
and 10-14 relative), and its standing clips accept every move on every
frame.

### Next-clip links

`contract::LINKS`: a clip's end link must lead to a clip of a given state:

- 6 into run (1), 20 into walk (0);
- 2, 3, 53 and 54 back to standing;
- 88 and 146 back to standing; 76 into forward flight; 24, 50 and 42 back to
  standing.

`check` also requires every clip's end link and range destination to fall
inside model 0's clips.

## Frames

`contract::frame`. These are absolute frame numbers, compared with
`frame_start + frame` of the playing clip:

| Const | Frames | Use |
|---|---|---|
| `WALK_LOOP` | 22..=57 | Where clip 1 must lie, because the windows below are inside it. |
| `WALL_STOP_LEFT` | 29..48 | Walking into a wall stops through clip 3 (`forward.rs:78`). |
| `WALL_STOP_RIGHT` | 22..29, 48..58 | ...through clip 2 (`forward.rs:80`). At any other frame, Lara stands at once. |
| `STEP_DOWN_LEFT` | 28..46 | Clip 59 rather than 60 (`forward.rs:113`). |
| `STEP_UP_RIGHT` | 27..45 | Clip 58 rather than 57 (`forward.rs:119`). |
| `BACK_STEP_DOWN_RIGHT` | 964..994 | Clip 62 rather than 61 (`backward.rs:114`). Inside Wall's backward clips; the window is unchecked. |

Other frame literals are clip-relative:

- the run-jump latch at clip 0 frame 4 (`player.rs:827`);
- the wall-hit split at run frame 10 (`player.rs:1515`);
- roll entry at frame 2 (`rolling.rs:24`);
- clip 28 at frame 9 (`traversal.rs:373`);
- clip 32 at frame 1 (`traversal.rs:420`);
- clip 96 at frame 0 or 12 (`traversal.rs:726`).

## Animation commands

| Command | Clips | Use |
|---|---|---|
| `SetPosition` at the clip's end | 50, 42, 97 (`contract::COMMANDS`) | Vault duration and offset (`AnimationLibrary::climb`, `player.rs:1466`). Pull-up end step (`traversal.rs:211`). |
| `JumpVelocity` at the clip's end | 26, 76, 91; also 74/78/80 (back and side takeoffs) and 16/18 (run takeoffs) | Launch velocity (`traversal.rs:216`, `backward.rs:253`). A vertical ledge jump overrides the vertical part. |
| `Effect { effect: 0 }` | twists 210 and 212, roll 147 | Turn 180 degrees (`traversal.rs:212`, `backward.rs:229`, `rolling.rs:34`). |
| `Sound` | any | Played by `game/src/audio.rs:55` from the playing clip, filtered by land or water. |
| `EmptyHands` at the clip's end | the use clips 63, 64, 131, 134, 135, 195, 196, 197 | Frees Lara's hands as the clip ends into standing (`player/using.rs`). |
| `Kill` | — | Not read by the controller (switches and creatures read their own). |

## Joints and meshes

`contract::joint`, in Lara's layout of 15 joints. Each joint has one mesh,
and the meshes are in joint order:

| Joint | Part | Read by |
|---|---|---|
| 0 | hips (root) | braid body sphere (`braid.rs:130`) |
| 1, 4 | left and right thigh | holstered gun meshes (`arms.rs` `THIGHS`) |
| 1-3, 4-6 | left and right leg | — |
| 7 | torso | gun torso turn (`arms.rs:48`), braid sphere |
| 8-10 | right upper arm, forearm, hand | arm keys from the pistol model; hand mesh swap; gun flash at the hand (`arms.rs:53`) |
| 11-13 | left upper arm, forearm, hand | the same for the left arm |
| 14 | head | gun head turn (`arms.rs:49`); braid attachment (`braid.rs:126`) |

Parents follow `joint::PARENTS`: legs and torso hang from the hips, and arms
and head hang from the torso. Without this layout (`Feature::Skeleton`)
nothing panics. The overrides use `get`, so arm, head and braid presentation
just skip missing joints (`arms.rs:77`, `braid.rs:113`).

Other models the presentation reads, all optional:

- the pistol model, object 1 (`arms.rs:43`). Its meshes are indexed by
  Lara's joint numbers, and its clips hold the arm keys.
- the gun flash, object 240 (`arms.rs:45`).
- the braid, object 2 (`braid.rs:21`).

Object contact poses every joint's sphere (`contact.rs:100`), and its bounds
come from the playing clip's key. So every key needs one angle per joint
and a bounding box. `check` tests both.

## Sound ids

The controller itself names only gun sounds:

- 6, drawing (`weapons.rs:180`);
- 7, holstering (`weapons.rs:181`);
- 8, a shot (`game/data/object_types.json:22`, `shot_sound`).

Footsteps and every other movement sound come from the clips' own `Sound`
commands, so a replacement character brings its own. The ids index the
level's `sound_map`.
