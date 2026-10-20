# Using objects: pickups, switches, keys, puzzles and doors

Evidence is the supplied disc (USA Rev 3) only: Ghidra decompilation of the
Wall RAM snapshot (`psx ghidra decompile --ram .psx/wall_game --range
80010000-80080000`), `psx disasm --ram .psx/wall_game --text`, and RAM tables
read from the same snapshot. No other reference was consulted.

**Status:** a pickup on land and one underwater, a switch, a keyhole, the
doors they open and a door push are compared with PSX recordings (below,
"Recordings"). Puzzle holes and the inventory ring are not.

## Object collision

Lara's control calls object collision `0x8001C258` after her animation
step. For every item in her room and the rooms its portals lead to that is
not invisible (status bits `&6 != 6`), whose type has a collision routine
(object table `+0x20`), and which lies within 4095 units on each axis, it
calls that routine with the item, Lara and her collision context. Collision
routines by type ([object-table-evidence](object-table-evidence.md)):

| Routine | Types |
|---|---|
| PickUpCollision `0x800502D4` | pickups |
| SwitchCollision `0x800507E4` | 92, 93, 103, 104 |
| SwitchCollision2 `0x80050AC4` | 105 (underwater) |
| KeyHoleCollision `0x80050F80` | 201-204 |
| PuzzleHoleCollision `0x800512CC` | 182-185 |
| DoorCollision `0x8001C70C` | 106-113 |

The port runs these from the game's `InteractionSystems::Use` (pickups,
switches, keys) and `InteractionSystems::Resolve` (doors), after Lara's
animation step. It tests the 4095-unit box but not the room list.

## Position tests and alignment

TestLaraPosition `0x8001CE04(bounds, item, lara)` first compares Lara's
rotation minus the item's, per axis (pitch `+12/+14`, yaw `+16/+18`, roll
`+20/+22`), then rotates Lara's offset into the item's frame (transposed
`phd_RotYXZ` of the item's rotation) and compares it with the box
`[min x, max x, min y, max y, min z, max z]`. Tables, read from RAM:

| Table | Box | Angles (pitch, yaw, roll) |
|---|---|---|
| Pickup `0x800896D8` | x ±256, y ±100, z ±256 | ±1820, 0, 0 |
| Underwater pickup `0x80089720` | ±512 on each axis | ±8190 each |
| Switch `0x80089744` | x ±220, y 0..0, z 292..512 | ±1820, ±5460, ±1820 |
| Underwater switch `0x80089780` | x ±1024, y ±1024, z -1024..512 | ±14560 each |
| Keyhole `0x800897A4` | x ±200, y 0..0, z 312..512 | ±1820, ±5460, ±1820 |
| Puzzle hole `0x800897C8` | as keyhole | as keyhole |

PickUpCollision first sets the item's yaw to Lara's and its roll to 0 (and
its pitch to 0 on land, -4550 underwater), so a pickup accepts Lara from any
heading. Only its final fall-through restores them (`0x800502D4`, the last
three stores); the paths that start or continue a pickup return early, so
the item keeps Lara's heading from the press on (recorded below).

AlignLaraPosition `0x8001D070(offset, item, lara)` copies the item's
rotation to Lara, then moves her to the item plus the rotated offset, but
only when that spot's floor is within 256 of her height and its ceiling more
than 761 above it (`0x8001D1E4..0x8001D25C`). Offsets: pickup (0, 0, -100)
`0x800896F0`; switch 93 (0, 0, 362) `0x8008975C`, 103 (0, 0, 292)
`0x80089768`, 92 (0, 0, 212) `0x80089774`, 104 none; keyhole and puzzle hole
(0, 0, 362) `0x800897BC`, `0x800897E0`.

MoveLaraPosition `0x8001D278(offset, item, lara)` (underwater pickups and
switch 105) instead swims her there over several ticks. The spot is the
item plus the offset turned by the item's full rotation (phd_RotYXZ
`0x80064338`: yaw, then pitch, then roll, each skipped at 0 and rounded with
a 14-bit shift). Move3DPosTo3DPos `0x8001D4D4(lara, spot, 16, 364)` moves her
`d * 16 / |d|` (C division, |d| from the rounded-down square root
`0x80063E84`) or onto the spot when within 16, and turns each of pitch, yaw
and roll 364 toward the item's or onto it. It returns 1 once position and
all three angles match. TestLaraPosition uses the same rotation, transposed,
for the item frame, after checking each angle's difference against the
table's window.

## Starting a use

All land uses need Action held (input `&0x40`), Lara not airborne (status
`&8`), standing (state 2) and her hands free (gun status `0x8008C4EA` 0).

- **Pickup** (`0x800503E4..0x800504BC`): align, ask for state 39, and run
  AnimateLara `0x80042E08` in a loop until the state is 39 (standing's range
  for 39 covers its whole clip, so one call: clip 135, frame 0). Then goal 2
  and gun status 1 (hands busy). Flares (type 152) use state 67 instead.
- **Pickup underwater** (PickUpCollision's branch for water status
  `0x8008C4F4` 1): the item's pitch is set to -4550 (its yaw is already
  Lara's), the bounds are the underwater table, and Lara must be treading
  (state 13) with Action held and hands free; there is no airborne test. Each such tick
  MoveLaraPosition swims her toward (0, -200, -350) in the tilted item's
  frame (`0x80089738`); on the tick she arrives she is run into state 39
  (treading's range for 39 covers its clip: clip 130, frame 0), goal 13. Her
  hands are never marked busy. The item is taken on clip 130's frame 18
  (`anim[130].frame_base + 0x12`). Every path but the fall-through returns
  without restoring the item's angles, so it keeps the tilt from the first
  in-range tick. At the surface (water status 2) neither branch runs.
- **Switch** (`0x80050844..0x80050A70`): also needs the switch inactive
  (status `&6 == 0`). Lara's yaw becomes the switch's, then the type's
  alignment. A switch in state 1 puts Lara in state 40 with clip 63 (93: 195,
  103: 197) and asks the switch for state 0; otherwise state 41 with clip 64
  (93: 196, 103: 197) and the switch goes to state 1. Lara's clip is set
  directly at its first frame, goal 2, hands busy. The switch becomes active,
  joins the active list and animates once (AnimateItem `0x8001DD98`). Type
  92 cannot be used in state 1 and, from state 0, plays a clip of object 12
  with a flag at `0x8008C510`; that variant is not ported.
- **Keyhole** (`0x80050F80`): Action with no item chosen opens the
  inventory in its keys mode (`0x80030F30(2)`); the item chosen there
  (`0x80089032`) is tested on the next tick. A matching key (201→197,
  202→198, 203→199, 204→200) is removed (`0x8003462C`), Lara aligned and run
  into state 42 (clip 131), goal 2, hands busy, keyhole active. A used
  keyhole or the wrong item plays sound 2 once per position. **The port
  skips the inventory and uses the matching key directly.**
- **Puzzle hole** (`0x800512CC`): as the keyhole with state 43 (clip 134).
  On clip 134's frame 80 the hole's type becomes its filled type (182→186).

## While the clip plays

The use states' handlers (`0x80039930` pickup, `0x80039A20`/`0x80039A8C`
switches, `0x80039AF8` key, `0x80039B58` puzzle) clear the look flag and
collision flags `0x10` and `0x8` (no pushes), and ask the chase camera for an
angle (`0x8008B29C`), elevation (`0x8008B2A0`) and distance 1024
(`0x8008B294`); the switch handlers also set the approach divisor
(`0x8008B2AA`) to 6:

| State | Angle | Elevation | Divisor |
|---|---|---|---|
| 39 pickup | -23660 | -2730 | 10 |
| 40, 41 switch | 14560 | -4550 | 6 |
| 42 key, 43 puzzle | -14560 | -4550 | 10 |

Their collision handlers all call the default collision `0x8003C208`.

PickUpCollision, called again each tick while Lara is in state 39 and in
bounds, takes the item when her absolute frame is clip 135's first frame plus
42 (`0x80050580`): it is added to the inventory (`0x80033A70`), the third
secret starts the secret music, the item becomes invisible (status `|6`) and
leaves its room's list.

Every use clip (63, 64, 131, 134, 135, 195, 196, 197) ends with an
EmptyHands command, which sets gun status 0, and links to clip 11 (standing).

## Switches and their triggers

SwitchControl `0x800516B8`: sets all the switch's code bits; if TriggerActive
`0x8001F6C8` says it is no longer held, asks for state 1 and clears its
timer; then AnimateItem. TriggerActive is true when all code bits are set and
the timer is 0, or counting down (it becomes -1 on reaching 0); a reversed
item (flag `0x4000`) reads the other way.

The switch's clips end with command 4 (`0x8001DF08`), which sets its status
to deactivated.

TestTriggers `0x8001EE30` dispatches trigger types through `0x800100FC`:

- **Switch** (2): SwitchTrigger `0x8005172C(item, timer)` passes only for a
  deactivated switch. In state 0 with a timer it gets that timer (1 tick for
  1, else seconds × 30) and goes back to active; otherwise it leaves the
  active list and goes inactive. The trigger's activations then toggle
  their objects' code bits (`flags ^= mask`, `0x8001F174..0x8001F184`).
  Type 92 tests differently and is not ported.
- **Key** (3): KeyTrigger `0x80051874` passes for an active keyhole once
  Lara's hands are no longer busy (gun status not 1), and deactivates it.
- **Pickup** (4): PickupTrigger `0x800518E4` passes for an invisible pickup
  and deactivates it.

So a switch's targets change on the first tick after its own clip ends,
while Lara stands on the trigger sector; a key's when Lara's clip has ended.
The port: games mark which sources are ready before dispatch
(`engine::triggers::TriggerState::mark_ready`), the engine reports the
triggers that fired (`TriggerSourceFired`), and the game applies the test's
side effects (`game/src/interactions/usable.rs`).

## Doors

InitialiseDoor `0x8004C358` saves, then ShutThatDoor `0x8004C284` makes solid
(no floor data, no box, floor and ceiling -127 clicks, no rooms above or
below, and the old box marked blocked `|0x4000`), up to four sectors: the
sector one step against the door's facing in its room, the door's own sector
in the room through that sector's portal (GetDoor `0x8001FACC`), and both of
those in the flipped rooms. OpenThatDoor `0x8004C2EC` restores them.

DoorControl `0x8004C954`: not held and not open (state ≠ 1): shut; held and
not closed (state ≠ 0): open. The goal changes as in
[door-animation-evidence](door-animation-evidence.md). One exception: when
type 14 is loaded (Venice), `0x80087A48` is 0, and Lara is within x
24501..26499, z 75501..82999, an open door is kept open; not ported.

DoorCollision `0x8001C70C`: TestBoundsCollide, then the mesh spheres
(TestCollision `0x8005B990`), then, if Lara's collision flag `0x8` is set,
ItemPushLara `0x8001C890` with the radius-expanded box (fifth argument 1).
The port runs the shared contact pass (`game::contact`) for every door,
except while Lara is using an object.

## Recordings

Checkpoints come from `psx handoff` (a person playing in the emulator
window); the recordings replay from them with no memory pokes.

**Home's front-door button** (switch 13, type 103; door 12).
`.psx/handoff_home_switch` leaves Lara at x46556 z69988, 156 units in front
of the button: too close for its bounds (z 292..512), so Action does nothing
there. A one-step walk (UP+R1 for run frames 2..6) puts her 378 in front;
Action at run frame 100 then aligns her to 292 (z69852, yaw -32768) and
starts state 40 / clip 197 (`.psx/rec_home_button`, item array
`*0x8008AAAC`, 0x4C bytes per item). Counting ticks from the press:

| Tick | Lara | Button | Door |
|---|---|---|---|
| 0 | clip 197 frame 0, hands busy | state 0, clip 234 frame 2, active | closed |
| 1..46 | clip 197 frame *n* | clip 234 frame 2+*n*; code bits set (tick 1) | closed |
| 47 | clip 11, hands free | clip 233, inactive | code bits set, active |
| 48 | clip 103 | | state 1, clip 245 frame 1 |
| 80 | | | clip 246 (open) |

Write watches on `0x8008B29C..0x8008B2AA` show the state-40 handler's camera
request (14560, -4550, 1024, divisor 6) on ticks 1..47, and the camera reset
after each. `game/tests/using.rs`
(`the_front_door_button_matches_the_recording`) checks every one of these
ticks.

**Wall's first keyhole** (1, type 201; door 0, type 106).
`.psx/handoff_wall_key` leaves Lara 411 in front of the keyhole, holding
key 193 (picked up underwater). Action at run frame 4 opens the inventory;
a second Action at frame 80 chooses it (`0x80089032` becomes 197). No game
tick runs while the ring is open. The first one after it (run frame 149,
`.psx/rec_wall_keyhole`) aligns Lara 362 in front (x30870 z33280, yaw
-16384) in state 42, clip 131 frame 0, hands busy, the keyhole active.
Clip 131 plays 135 ticks; on tick 135 Lara is in clip 11 with her hands
free, KeyTrigger has deactivated the keyhole, and door 0 is active; it
enters clip 328 on tick 136 and 329 on tick 168. The key-state handler asks
for the camera (-14560, -4550, 1024, divisor unchanged) on ticks 1..135.
`the_first_wall_keyhole_matches_the_recording` checks these, using the key
on the press since the port has no ring.

**Wall's ladder key** (15, type 194). `.psx/handoff_wall_pickup` leaves
Lara on the key (x25059 z36378, yaw -11119). Action at run frame 4
(`.psx/rec_wall_pickup`): on the next tick she is aligned 100 behind it
along her heading (x25175 z36303) in state 39, clip 135 frame 0, hands busy,
and the key's yaw becomes hers (it stays so). The key turns invisible on
tick 42 (clip frame 42), the clip ends into clip 11 with her hands free on
tick 66, and the camera request (-23660, -2730, 1024) runs on ticks 1..66.
`the_ladder_key_pickup_matches_the_recording` checks these.

**Wall's underwater key** (17, type 193), on the floor of the flooded
corridor. Replaying `.psx/handoff_wall_key`'s input from `use_wall_game`
gives a checkpoint at run frame 10380 (`.psx/rec_wall_underwater_pickup`,
`state_f010380.bin`), with Lara treading above the key at x45211 y6838
z39505, pitch -3822, yaw 17073, Action held. From it
(`.psx/rec_wall_underwater_pickup2`) she swims (10, -10, -5) a tick and
tilts to -4550 while Action is held, stops for the ten ticks it is
released, and arrives at x45335 y6711 z39439 on tick 22, entering clip 130
frame 0. The key turns invisible on tick 40, the clip ends into clip 108
frame 21 (state 13) on tick 55, hands stay free throughout, and the pickup
camera (-23660, -2730, 1024) runs on ticks 23..55, written by the same
state-39 handler as on land (`0x8003996C..0x80039984`).
`the_underwater_key_pickup_matches_the_recording` checks every tick.

**Running into the closed front door.** From the same checkpoint, RIGHT for
run frames 2..52 and UP from frame 120 (`.psx/rec_home_door_push`). Lara runs
west into the door's shut sector and is pinned at x46185 while still in
state 1, sliding 2 units a tick along z. Position writes show why: each
tick AnimateLara (`0x80043274`) moves her to about x46140, ItemPushLara
(`0x8001CA78`) puts her back to 46185, and the run collision (`0x8003A844`,
GetCollisionInfo from `0x8003C34C`) then probes from 46185, whose front
point (46085) is still in the open sector. The state collision follows
object collision (see [tick-order-evidence](tick-order-evidence.md)). The
port keeps that order on the ground: the move happens in Motion, and its
state collision waits until object pushes have run and probes from where
they left her, restoring to the origin a push advanced. The test matches
all forty recorded ticks.

## Not ported

- Switch 105 (underwater, MoveLaraPosition), flares (state 67).
- The inventory ring at keyholes and puzzle holes.
- Type 92's own clip and trigger test; Venice's door hold.
- The room list of object collision.
- Sounds: the "no" sound at a used keyhole.
- Airborne state collision after object pushes: jumps and falls still
  collide before them (ground moves and rolls wait).
