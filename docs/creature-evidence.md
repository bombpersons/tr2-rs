# Creatures: shared AI, the tiger, the spider, the crow and the T-Rex

The tiger (type 39), the small spider (type 36), the crow (type 38) and the
T-Rex (type 214) run on shared creature routines (`game::creature`) with a
control function per type (`game::tiger`, `game::spider`, `game::crow`,
`game::trex`) registered by `register_creature`. The
earlier rule in `game::interactions` still drives the other creature types.
Only the supplied Tomb Raider II USA Rev 3 disc and `psx` were used; the
earlier OpenLara/TR2Main-derived creature rules no longer apply to these
types.

## Sources

Routines were read with `psx disasm --ram` from Wall checkpoints. Their
listings are kept in the ignored `validation/tiger/asm/`.

| Routine | Address | Port |
|---|---|---|
| EnableBaddieAI, InitialiseSlot, CreateZone, ClearLOT | `0x80048C2C`, `0x80048EC8`, `0x800490EC`, `0x80049250` | `creature::enable`, `BoxSearch::new/create_zone/clear` |
| CreatureActive | `0x80014C74` | `creature::activate` |
| CreatureAIInfo | `0x80014D00` | `creature::ai_info` |
| CreatureMood, ValidBox, StalkBox, EscapeBox | `0x80015800`, `0x800156DC`, `0x800154B8`, `0x80015608` | `creature::mood` and helpers |
| SearchLOT, UpdateLOT, TargetBox, CalculateTarget | `0x800150D8`, `0x80015320`, `0x800153E0`, `0x80015DD4` | `engine::world::BoxSearch` |
| Idle search | `0x800492B4` (callback `0x800741E8`) | `creature::finish_searches` |
| CreatureTurn, CreatureTilt, CreatureHead | `0x800172E0`, `0x8001745C`, `0x800174A8` | `creature::turn/tilt/head` |
| CreatureAnimation, BadFloor, CreatureCollided | `0x800166F8`, `0x800164A8`, `0x80016354` | `creature::animate`, `bad_floor` |
| CreatureDie, AnimateItem | `0x8001659C`, `0x8001DD98` | `creature::animate`, `animate_item` |
| CreatureCollision, ItemPushLara | `0x8001C598`, `0x8001C890` | `creature::collide`, `contact::ContactPass` |
| TigerControl | `0x80021FCC` | `tiger::control` |
| InitialiseBird, BirdControl (crow and eagle) | `0x800127DC`, `0x800128A4` | `crow::initialise`, `crow::control` |
| TRexControl, CreatureKill | `0x800239C4`, `0x80017978` | `trex::control`, `CreatureContext::kill` |
| SpiderControl, SpiderLeap, ExplodingDeath | `0x8005C1EC`, `0x8005C0D4`, `0x800498D8` | `spider::control`, `spider::leap` |
| InitialiseDoor, ShutThatDoor, DoorControl, AlterFloorHeight | `0x8004C358`, `0x8004C284`, `0x8004C954`, `0x8004AA4C` | `interactions::door_boxes`, `block_level_boxes`, `control_doors` |
| GetRandomControl | `0x8006E518`, seed `0x80089D54` | `engine::random::GameRandom` |

Memory layouts used by the recorders: items are 76 bytes from `0x8019C940`.
Creature slots are 68 bytes from `0x801A29C0`; the slot's LOT starts at +0x10.

## Rules worth knowing

- **Slots.** A creature takes a slot when its trigger activates it, after
  trigger dispatch in Lara's phase. Slot setup gives max turn 182, step 256,
  drop -512 and block mask 0x4000. The spider uses step 512 and drop -1024,
  read from its `object_types.json` entry.
- **Control order.** Creatures run in active-list order, newest first.
  Activation follows the trigger's action order (`ObjectActivated`).
- **Blocked boxes.** Closed doors and unmoved blocks (types 67-70) block
  their boxes at level start. Doors unblock them as they open. Runtime flags
  live in `engine::world::BoxFlags`, apart from `WorldGeometry`. At Wall's
  start the original's blocked boxes are 703, 705 and 712, and the port
  matches (`wall_doors_and_blocks_block_the_original_boxes`).
- **Bad floor.** BadFloor's step-back compares the new x sector against the
  old z sector. This is a quirk of the original and is kept.
- **Collisions with other creatures.** CreatureCollided walks the room's item
  list from its head and stops at itself. An item joins the head of a list
  when it is placed (in placement order) and when it changes room. A
  creature therefore only collides with creatures that entered its room after
  it did. The port keeps a room-entry stamp (`creature::RoomOrder`).
- **Death.** CreatureDie sets hit points to -16384, clears collidable, and
  releases the slot. The item's state can differ from its clip's state (the
  death clip's state is 0 while the item's is 9), so the current state is
  kept apart (`step_animation_frame_from`).
- **Tiger.** When it stops out of boredom, it draws a random number for a
  roar. A later request then overwrites the roar.
- **Spider.**
  - A step up of more than 384 is undone and replays its leap clip.
  - It bites for 25.
  - Its death throws its 30 meshes as pieces, with three draws each.
- **Idle search.** While the original waits for the next field, it visits
  the five slots round robin (cursor `0x80089628`), giving an occupied
  slot's search one box (UpdateLOT with 1), until the field arrives or a
  whole round expands nothing. How far a search gets therefore depends on
  how long the frame took to draw. Each visit stores the cursor once at
  `0x80049350`; a wrap then stores 0 as well (`0x80049360`), so only the
  first counts visits. Most searches finish within one wait and no fixed
  budget fits every recording, so the game completes searches between
  ticks (`IdleSearch::steps` None). Recordings of creatures note the
  visits after each tick, and their tests replay them exactly
  (`idle_steps`, `search_cursor`).
- **Slot fields on reuse.** InitialiseSlot does not clear the steering
  target, so a reused slot shows its last holder's until its new holder's
  first control: the tests skip it on that tick.
- **Fliers.** InitialiseSlot gives types 35, 38 and 47 step and drop
  0x5000 and a climb of 16; they search the fly zones. CreatureMood aims
  them at the top of Lara's collision box while she is out of the water.
  CreatureAnimation's fly branch (`0x80016FB4..0x80017194`) clamps the
  climb to ±16, tests floor and ceiling at y plus the box top, and pitches
  toward the climb by at most 182 a tick, within ±3640.
- **Crow.** InitialiseBird turns it by `(rand − 0x4000) >> 1` and rests it
  on its clip 14 in state 7 (eating). Roused, it flies at Lara, dives when
  she is ahead within 512 and pecks once per dive for 20. Shot, it falls
  (clip 1) and lies where it lands.
- **T-Rex.** Health 100, pivot 1800, radius 341, block mask 0x8000 (big
  creatures keep out of boxes flagged 0x8000). Its bone flags turn joints
  11 and 12 by the head and neck angles (setup `0x80055648..0x800556C4`);
  the neck copies the head. It walks while bored, or while Lara is out of
  sight but facing it, or within reach of a bite but beyond 1500; it runs
  otherwise, roars at random (512 in 32768) when she is ahead, and bites
  within 1500. Touching her costs her 10 a tick while it runs and 1
  otherwise. Its jaws (meshes 0x3000) take 10000 and call CreatureKill.
- **CreatureKill.** The creature goes to its clip 11 in state 8. Lara is
  put at its position and facing on the first clip of object 12 (a one-frame
  clip of state 0) with goal 6 and animated once, so its state change takes
  her into the kill (clip 260). Her hands are made busy and bit 0x40000 of
  `0x8008C510` set: her control then uses the extra-animation state table
  `0x80082DB4` and skips object collision. Its state 6 handler
  (`0x8003A134`) holds the camera at the crushed view (30940, −4550).

## Recordings

`tools/record_tiger.py` starts from `.psx/tiger_pre_start`, which is made by
pad input from the settled Wall checkpoint. `tools/record_spider.py` starts
from `.psx/spider_corridor_start`, which is made by five frame-0 writes
(Lara's x, y, z, yaw and room) plus 90 neutral fields. None of the replays
poke memory. Each JSON beside a CSV holds the command, the checkpoint digest
and every random-seed write.

The tests give the port Lara as input, using the previous tick's sample:
- her pose, speed and health;
- each creature's contact mask and hit flag;
- the random seed;
- activation and gun damage, in Lara's phase.

Everything the creature does is compared exactly:
- position and yaw;
- clip, frame and state, goal and required state;
- speed, room and hit points;
- the slot's max turn, flags and mood;
- the target box and the steering target.

| Route | Ticks | Notes |
|---|---|---|
| tiger stand, flee, shoot | 250 each | calibration |
| tiger long stand (held out) | 471 | ends at Lara's death, where the original catches up four ticks at once |
| tiger walk, circle (held out) | 450 each | |
| tiger run back (held out) | 600 | |
| spider stand, retreat | 300 each | calibration |
| spider walk, shoot (held out) | 450 each | |
| spider long stand (held out) | 460 of 600 | ends where the draw-time idle search budget shows (above) |

That is 2721 tiger ticks and 1960 spider ticks.

- **Seed.** The seed is compared except on routes where Lara fires, because
  her shots draw from the same generator.
- **Tiger contact.** The tiger's contact masks are replayed rather than
  compared. The recording only holds Lara's pose after the push, and the
  port's masks agree on 323 of 329 bite ticks.
- **Spider death.** Each of the three spider deaths in the shoot route shows
  exactly 90 random draws in that field.
- **Held-out spider routes.** Spider walk and long stand both failed before
  the room-entry rule. That rule was diagnosed on the retreat route only, at
  tick 284: spider 26 overlapped spider 25 in the port but not in the
  original. After the fix, both held-out routes match. The long stand's
  shorter window is the one change made with a held-out route in view, and
  it is labelled in the test.

### Crows and T-Rexes

`tools/record_wall_traps.py` cuts these from two played runs (`psx
handoff`, replayed from pad input, no pokes): the traps run (case `crows`)
and a run from the cave before the zip-line down into the T-Rex valley
(cases `trex_shot`, `trex_kill`). Rows are one per control tick. The idle
search's visits after each tick are counted in a second pass, in 25-field
chunks under a memory cap: the cursor is stored some 5000 times a field and
the emulator keeps every watched event. The T-Rex cases also note the seed
as the control loop reads the T-Rex's control pointer (`--watch-reads` on
object table +0x10), because the original's other active creatures (tigers
and a spider elsewhere) draw from the generator earlier in the tick.

`game/tests/support/creature_replay.rs` replays these as above, adding the
idle visits, each creature's slot, and that seed.

| Case | Ticks | What happens |
|---|---|---|
| crows | 676 | three crows roused, fly at Lara, glide, dive and peck; one shot down |
| trex_shot | 1250 | the far T-Rex wakes, runs, roars, walks and is shot dead |
| trex_kill | 2250 | the hidden T-Rex chases and bites Lara and takes her in its jaws |

All three were recorded after the routines were written. The crow case
first diverged at tick 31 while the port completed searches at once, then
at tick 32 while wraps were counted twice; `trex_kill` diverged at tick 169
until the seed at its control was recorded. The kill tick also checks
Lara's clip, frame, position, facing, room and set camera.

## Not ported

- The flinch ("spaz") branch of ItemPushLara: it is presentation only.
- Door sector walls and floors while doors open and close.
- Pushable-block floor changes.
- Effect pieces, blood and the spider's death sound. The random draws they
  cost are kept. Clip sound cues are emitted as for other objects.
- Flip-map zones: no ported type needs them yet.
- The eagle (type 47) shares the crow's routine but has no recording.
- Lara's hit flag from the T-Rex's jaws, and her hands-busy gun status
  during the kill (she is dead; the port's Killed mode stops her anyway).
- The frame catch-up after Lara's death.
- Other creature types, which keep the older provisional rule.
