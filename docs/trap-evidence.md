# The Great Wall's traps

Evidence is the supplied disc (USA Rev 3) only: `psx disasm --ram` of a Wall
RAM snapshot, the object table read from the same snapshot, and no-poke PSX
recordings of a played run. No other reference was consulted.

**Status:** every trap type in Wall that Lara can set off is ported
(`game/src/traps/`), and each is compared tick for tick with a PSX recording
(below, "Recordings"). So is the zip-line, with Lara's ride on it
(`game/src/player/zipline.rs`). Trap kills (spikes, boulder) have no
recording yet.

## Routines

Object table (`0x800936D0`, 48-byte records): +0x0C init, +0x10 control,
+0x14 floor, +0x18 ceiling, +0x20 collision
([object-table-evidence](object-table-evidence.md)).

| Type | Trap | Routines | What it does |
|---|---|---|---|
| 55, 57 | falling block | control `0x8005F97C`; floor `0x8005FB5C`, ceiling `0x8005FBAC` | shakes, falls and settles; while state < 2 its top is y − 512 (1024 for 57) |
| 59 | spikes | collision `0x8005F378` | one random draw each touch, then four per blood drop; a fatal fall onto them impales Lara (clip 149) |
| 60 | boulder | init `0x8005EB00`, control `0x8005EB84`, collision `0x8005EFD8` | rolls on its clip, drops off ledges, stops for good where the floor ahead rises, goes home when its trigger lets go; crushes Lara on the ground (clip 139, state 46, set camera 30940/−4550, 15 drops × 4 draws) and hits her for 100 in the air |
| 62 | dart emitter | `0x8005FE80` | creates a type 61 dart 412 behind and 512 up; the +x facing tests yaw `0x3FEC` |
| 61 | dart | `0x80060078` | hurts 50; one random draw on a ricochet |
| 80 | spiked roller | init (clip +3), control `0x8005E25C` | looks 1536 ahead; hurts 100 |
| 81 | wall blade | init (clip +2), control `0x8005E6B8`, collision `0x8001C7F0` | mesh bit 2 hurts 100 |
| 85 | sliding blade | `0x8005DC50` | 16 units a tick while the floor is level; hurts 20 |
| 114 | trapdoor | control `0x8005F57C`, floor `0x8005F608`, ceiling `0x8005F690`, OnTrapDoor `0x8005F718` | opens when triggered; its floor only holds while shut |
| 102 | zip-line | init `0x8005EB00` (as the boulder's: keeps its start), control `0x8004B824`, collision `0x8004B6CC` | Lara takes hold and rides it; below |

## Rules worth knowing

- **Floors and ceilings from objects.** GetHeight `0x8001E944` and
  GetCeiling `0x8001F740` call the floor or ceiling routine of every
  "activate" action in the floor sector's trigger list, whatever the
  trigger's kind. The port keeps these as `engine::world::ObjectSurface`
  entries in `WorldGeometry`, laid by `traps::floors::lay_surfaces` each
  tick.
- **Triggers come from the bottom sector.** GetHeight starts by following
  the sector's room below (byte 4, until 255) whatever the height, and the
  floor data it reads, triggers included, is that bottom sector's. So the
  trapdoor (item 14, room 31) is triggered by the floor of the pool two
  rooms down (31 over 3 over 33), while Lara still stands on it: the
  recording lists it at tick 47 with her in room 31 at y −3072.
  `WorldGeometry::trigger_floor` follows the rooms below the same way.
- **Activation.** TestTriggers (`0x8001F1E4..0x8001F304`) adds an object to
  the active list whenever its code is complete and it is not already
  listed, so `ObjectActivated` is sent on every such action.
- **Touch bits.** AnimateItem clears them; TestCollision only writes them
  when the coarse test passes.
- **Pushes.** ItemPushLara with big-push (traps and doors) grows the box by
  the collision radius.
- **Randoms.** Blood and sound draw from `0x8006E54C`; the control random is
  `0x8006E518`. The draws are kept even though their effects are
  presentation.
- **Created items.** Darts are created items (`engine::objects::Created`),
  drawn by `dress_created`.

## The zip-line

Read from `psx disasm --ram validation/wall_traps/crows`; Lara's state
handlers from the tables at `0x80082C98` (control) and `0x80082DE0`
(collision). Item 93, in the cave before the T-Rex valley, faces +x.

- **Taking hold** (collision `0x8004B6CC`). Action held (input `&0x40`),
  Lara not falling (status bit 8), gun status 0, Lara in state 2, and the
  line waiting (status bits 1-2 clear). TestLaraPosition with the bounds at
  `0x80089644`: x −256..256, y −100..100, z 256..512 in the line's frame,
  yaw within ±4550 of the line's, pitch and roll equal. AlignLaraPosition
  `0x8001D070` to (0, 0, 371) (`0x8008965C`). Then goal 70, gun status 1
  (hands busy), and AnimateItem on Lara until her state is 19, all in the
  collision pass: standing's range for 70 enters clip 215 (state 19) at its
  first frame. The line joins the active list, goes active and gets flag
  `0x100`.
- **Waiting** (control, state 1). Only AnimateItem: clip 321 plays 65
  frames while Lara grabs (clip 215, 65 frames), and its end moves it 1024
  forward (`set_position`) into clip 322 (state 2), the tick Lara's grab
  ends by stepping her 654 forward into the ride loop 216 (state 70).
- **Sliding** (control, state 2). AnimateItem; the fall speed (`+0x20`)
  grows by 5 a tick up to 100; it moves fall speed × sin/cos(yaw) >> 14
  sideways and fall speed >> 2 down, and changes room through GetFloor.
  If Lara is in state 70 it puts her where it is (`0x8004BA34`). It looks
  1024 ahead and 64 below itself: it keeps going while that floor is more
  than 320 below it and that ceiling more than 192 above (sound 280).
- **Stopping.** Otherwise (sound 281) it leaves the list and goes inactive,
  and its `0x100` flag is cleared. If Lara is in state 70 she gets goal 3,
  one AnimateLara (`0x80042E08`), the fall status, speed = its speed and
  fall speed = a quarter of it.
- **Going home.** A listed, active line without `0x100` (a trigger added
  it again) goes back to its start (kept by its init) and room, state and
  goal 1, its first clip, inactive, off the list. Not recorded.
- **Lara's ride.** State 19's control `0x80039354` only clears collision
  flags; its collision `0x8003B57C` is the standing one (not ported for the
  grab: the recordings never move her). State 70's control `0x80039E64`
  asks the camera for angle 12740 (`0x8008B29C`) and probes the floor for
  triggers; with Action let go it asks for goal 3, runs AnimateLara, then
  sets speed 100, fall speed 40 and the fall status. State 70's collision
  `0x8003C310` does nothing. The ride loop 216 has a range for 3 only on
  its frames 6318..6328 (into the drop 217, state 3, whose end lowers her
  240, frees her hands and links into flight clip 77). Released outside
  that window, she stays in state 70 with goal 3: every tick the line puts
  her back on it, her handler's AnimateLara flies her with last tick's
  speeds, the speeds go back to 100 and 40, and her own AnimateLara flies
  her again (the `zipline_release` recording: +200 a tick, falling 98).
- Lara's room follows her body (UpdateLaraRoom `0x8001C0E8`, 381 above
  her feet), so it changes a few ticks after the line's.

The port: `game/src/traps/zipline.rs` (the line) and
`game/src/player/zipline.rs` (`PlayerMode::ZipLine`, stages Grabbing,
Riding, LetGo, Dropping). The drop clip is flown by the ride module; from
clip 77 on, Lara is in ordinary forward flight (`PlayerMode::JumpingForward`).
The clips, states, ranges and commands are `Feature::ZipLine` in
`player::contract` ([lara-contract](lara-contract.md)). Not ported: state
19's standing collision, collision during the drop clip (four ticks), the
trigger probe of state 70's handler (Lara's own floor triggers still run),
water under the line, and the sounds.

## Recordings

`tools/record_wall_traps.py` cuts every case from one played run: a
`psx handoff` of Wall from `.psx/use_wall_game` in which the traps were set
off on purpose, replayed from its pad input with no pokes. Rows are one per
control tick, rebuilt from watched stores (lag frames run two ticks in one
field; the inventory stops the world).

| Case | Ticks | Items |
|---|---|---|
| trapdoor | 201 | 14 |
| darts | 703 | emitters 31–44 and the darts they create |
| tiles_entrance | 150 | 46, 47 |
| boulders | 400 | 73, 74 |
| sliding_blades | 400 | 61, 62 |
| wall_blades | 418 | 49–57, 64–67 |
| tiles_and_blades | 325 | 75–84 |
| spindles | 837 | 88, 90, 91 |
| zipline | 450 | 93, with Lara |
| zipline_release | 525 | 93, with Lara; Action let go at field 25200 |

`game/tests/traps.rs` gives the port Lara as input (pose, speeds, air flag
and health from the previous row), each trap's touch bits, trigger code and
timer, and the seed; a trap the original's triggers listed joins the active
list after Lara's phase. Everything a trap does is compared exactly, with
the damage it deals Lara and the darts' count and positions.

The zip-line cases (`game/tests/zipline.rs`) put Lara under test: she
starts standing where the original's stood the tick before she took hold
(row 89) and runs on her own rules, given the input word (`0x8008AAD4`,
sampled with each row) as pad buttons. Her position, state, clip, frame
and room are compared exactly with the line's: `zipline` through the
grab, the ride into room 39, the line stopping, the drop and her landing
(360 ticks); `zipline_release` through a release while the drop range is
shut, the fall and the landing (361 ticks; after that the valley's tiger,
not spawned, pushes her). `zipline` was recorded before the port;
`zipline_release`, whose input is the run's with Action let go by the
tool, after it, as the held-out case. It found one rule the first port
lacked: the handler's own AnimateLara flies her while she still hangs on.

## Not ported

- Heavy triggers (the boulder's TestTriggers with heavy set, `0x8005ED4C`).
- Blood, sparks, sounds and the camera shake: presentation, draws kept.
