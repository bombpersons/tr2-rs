# Creatures: shared AI, the Wall's creatures and Venice's

The tiger (type 39), the small spider (type 36), the crow (type 38), the
T-Rex (type 214) and Venice's dogs (15), gunmen (16, 17, 18), rats (21) and
club men (32) run on shared creature routines (`game::creature`) with a
control function per type (`game::tiger`, `game::spider`, `game::crow`,
`game::trex`, `game::dog`, `game::gunman`, `game::rat`, `game::club_man`)
registered by `register_creature`. The
earlier rule in `game::interactions` still drives the other creature types.
Only the supplied Tomb Raider II USA Rev 3 disc and `psx` were used; the
earlier OpenLara/TR2Main-derived creature rules no longer apply to these
types.

## Sources

Routines were read with `psx disasm --ram` from Wall checkpoints, and
Venice's from a Venice checkpoint (`.psx/venice_enemies_survey`). The Wall
listings are kept in the ignored `validation/tiger/asm/`. Venice is the
level file `boat` (the file `venice` is Bartoli's Hideout).

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
| Dog control | `0x80021AC8` | `dog::control` |
| Rat control | `0x80051990` | `rat::control` |
| Gunmen: initialise, control, Targetable, ShotLara, GunHit | `0x8004DF34`, `0x8004DFE4`, `0x8004D648`, `0x8004DC90`, `0x8004DA24` | `gunman::control`, `targetable`, `shoot` |
| Club man control | `0x8002BF54` | `club_man::control` |
| CreatureNeck, CreatureVault | `0x80017530`, `0x800177C0` | `creature::neck`, `CreatureContext::vault` |
| GetFloor (its edge clamp) | `0x8001E41C` | `engine::world::WorldGeometry::floor_room` |
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
- **Dog.** Health 10, radius 341, pivot 300, head joint 20. Not violent.
  Bored, it walks, stops and takes up two idle poses (4, 5; state 6 is
  left the same way), one draw a tick. Roused, it runs, leaps from within
  768 (squared `0x8FFFF`) and bites standing within 341 or on the run
  within 682. Each attack bites once (touch `0x13F7000`): the leap for 200,
  the others for 100. Death is its clip 13 in state 10.
- **Rat.** Health 4, radius 102, pivot 50, head joint 4. Not violent; it
  always turns at up to 1092 a tick. Every change of gait goes through its
  stop: it sets the state it wants and stops. It bites once within 341
  (touch `0x7F`) for 20. Death is its clip 9 in state 6.
- **Gunmen.** Types 16, 17 and 18 are one man in three looks: the setup
  gives 17 and 18 type 16's clips and frames (`animations_from` in
  `object_types.json`, applied as a level loads by
  `ObjectTypes::share_animations`), and their initialise only changes which
  meshes show. Health 25, radius 102, pivot 50; joint 1 turns by the head
  angle. Not violent. They fire when Lara is ahead within 8192 and in clear
  sight from 768 above their feet to 768 above hers (Targetable, LOS
  `0x8001FB54`): standing or kneeling (one draw), or on the run beyond 2048
  in her zone. ShotLara rolls to hit: a draw against a chance that falls
  with distance and with her speed across the line of fire (32-bit
  arithmetic kept, overflow included); a hit costs her 50 and GunHit draws
  once more. Only the stop state clears the turn limit. Death is clip 20
  or 21 of type 16's, at random.
- **Club man.** Health 27, radius 102, pivot 0; step 1024 and drop -1024
  (InitialiseSlot gives types 32 and 33 these). Violent. Joint 1 turns by
  the head angle (while it attacks) and joint 5 by the neck (while it
  moves). Three attacks hit once each (touch `0x600`): two for 80, one for
  100. Death is clip 26 in state 11.
- **CreatureVault.** Animates as CreatureAnimation with no tilt. If that
  took the creature across one block edge (x or z) onto a floor that rose
  more than 384, or over a drop of more than 896, it is put back at its
  old height and room, 260 short of the edge and facing across it; the
  club man then starts a climb clip: 27 (rise over 896, state 12), 28 (up
  to 640, 13), 29 (up to 896, 14) or 30 (the drop, 15). The dead club man
  still calls it.
- **GetFloor's edge clamp.** A point outside the room it is asked about is
  clamped onto the room's edge sectors (on the first and last z rows x is
  kept off the corners) and that sector's portal followed. Line-of-sight
  sweeps sample points ahead of the room they start in, so the gunmen's
  sight depended on it.
- **Lara as creatures see her.** Lara is not in the active list: the
  control phase runs the items, then the effects, then Lara
  (`0x800424F0`), then counts the tick. So creatures see her as she ended
  the tick before, unless an item earlier in the list moves her: the
  motorboat she drives (item 26 in Venice) writes her position in its
  control (`0x80014698`, `0x80014A5C` as she dives off), and creatures
  woken before she boarded run after it.
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

### Venice

The same tool cuts Venice's cases from one played run: `psx handoff` from
`.psx/use_wall_game`, the level-skip cheat into Venice, then dogs, gunmen,
a club man and rats fought on foot and shot (and the motorboat driven),
29264 fields replayed from pad input with no pokes. Venice places its
item list and boxes elsewhere than the Wall (the item pointer is
`0x8008AAAC`, the slots `0x8008C1C0`), and runs more creatures at once, so
its idle-search chunks are 10 fields. Two more things are recorded for
these cases:

- **Blocked boxes.** Doors Lara opened earlier unblock their boxes. Each
  case's JSON lists the boxes blocked (overlap bit 0x4000, watched) from
  the first row and wherever they change, and the harness applies them
  (`with_blocked_boxes`); doors themselves are not replayed.
- **Lara as seen.** `seen_lara_*` columns note her pose as CreatureAIInfo
  first reads her x (`0x80014EB0`) after the type's control began; the
  harness gives the creatures that pose.

| Case | Ticks | What happens |
|---|---|---|
| dogs | 1365 | the first two dogs leap and bite and are shot; a gunman is about |
| dog_shot | 311 | a dog bites Lara twice and is shot |
| rats | 826 | three rats (one trigger wakes 37 before 36); one bites and is shot |
| gunman | 1115 | the first gunman (type 17) shoots at Lara until shot |
| gunman_long | 1161 | a gunman (18) fought while Lara drives the boat, swims, takes a medipack |
| gunman_rats | 226 | a gunman (18) shot among the rats |
| club_man | 401 | a club man hits Lara once and is shot |

All were recorded before the routines were written. What they found:
the dog's leap range (first read as `0x7FFFF`), the gunmen's turn limit
(read as cleared in every state), type 16's death clips for 17 and 18,
the rats' wake order, the blocked boxes, GetFloor's edge clamp and the
boat moving Lara first.

## Not ported

- The flinch ("spaz") branch of ItemPushLara: it is presentation only.
- Door sector walls and floors while doors open and close.
- Pushable-block floor changes.
- Effect pieces, blood and the spider's death sound. The random draws they
  cost are kept. Clip sound cues are emitted as for other objects.
- Flip-map zones: no ported type needs them yet.
- The eagle (type 47) shares the crow's routine but has no recording.
- Lara's hit flag (status 0x10), set by every bite, blow and gunshot, and
  her hands-busy gun status during the T-Rex's kill (she is dead; the
  port's Killed mode stops her anyway).
- The gunmen's shots smashing objects on the line of fire (ObjectOnLOS
  `0x80020618`, SmashItem `0x80041444`), their muzzle flashes and misses'
  ricochets (presentation; GunMiss draws only from the presentation
  generator), and the meshes their initialise hides.
- The motorboat: its control moves Lara at its place in the active list.
  When it is ported it must run there, among the creatures.
- The frame catch-up after Lara's death.
- Other creature types, which keep the older provisional rule.
