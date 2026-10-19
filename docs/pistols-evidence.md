# Pistols: draw, holster, fire, targeting and hits

Evidence comes only from the supplied Tomb Raider II USA Rev 3 disc through
`psx`. No reimplementation or fan documentation was consulted. The port's
rules are in `game/src/player/weapons.rs`; the combat camera is
`engine::camera::aim`; hit points are applied by `game::interactions::combat`.

## Binary rules

`tools/record_pistols.py --inspect` retains the disassembly commands, Wall
RAM hash and per-range byte hashes in
`game/tests/data/original/pistols_code.json` (listings under
`validation/pistols/code/`, together with Ghidra pseudocode of each function).

- **LaraGun `0x8003FC4C`** runs inside Lara's control after object collision
  and room update, before the floor triggers (`0x8003844C`). It first counts
  both muzzle flashes down, frees the hands if health is below 1, then
  dispatches on the gun status `0x8008C4EA` through the table at `0x80010A90`:
  0 hands free, 1 hands busy, 2 drawing, 3 holstering, 4 ready (5 is the flare
  and vehicle path, not ported).
  - Free hands: Draw (input bit `0x20`) re-requests the last gun
    (`0x8008C4F0`). A request different from the current gun (`0x8008C4EC`), or
    Draw held, starts drawing when the hands are free on land (vehicle field
    `0x8008C50A` = -1, water status `0x8008C4F4` = 0). Otherwise the request
    is only remembered.
  - Ready: Draw held, a different request or unsuitable water starts holstering.
  - **Draw and Action (`0x40`) are tested held, not as presses**: holding
    Draw draws, holsters and draws again (`pistols_held_draw`).
  - Statuses 2 and 4 set the combat camera type 3 (`0x8004001C`, `0x800401B8`)
    unless it is 2 or 4. Holstering does not.
- **Gun table `0x80089434`**, 48 bytes per gun; pistols at `0x80089464`:
  lock box ±10920 yaw and pitch, left arm box yaw -30940..10920, right
  -10920..30940, arm pitch ±14560, aim speed 1820, spread 1456, gun height 650,
  damage 1, range 8192, recoil frames 9, flash 3 ticks, sound 8. These are
  Lara's `pistols` entry in `game/data/object_types.json`.
- **DrawPistols `0x80047E60`**: both arms show the left arm's frame. Outside
  4..22 it restarts at 5. Frame 13 moves the guns to the hands (`0x80048324`)
  with sound 6. The tick that would show 23 calls ReadyPistols `0x800482A0`
  (status 4, frames, aim, locks and target cleared). Drawing takes 18 ticks.
- **UndrawPistols `0x80047F1C`**, per arm (left first): recoil (≥24) drops
  to 4. Frames 1..4 step down while easing each rotation by `rotation/frame`
  (`div`, truncated). Frame 0 jumps to 23 and zeroes the aim. Frames 6..23
  step down, and frame 12 holsters that arm with sound 7. When both arms rest
  on 5 the hands are free. Unless Look (`0x200`) is held, head and torso take
  a quarter of the arms' summed aim (`0x80048200`).
- **PistolHandler `0x800484B0`**: holding Action keeps the current target and
  re-measures it (`0x800406B4`); without Action the target is dropped. With no
  target, `0x80040948` searches. Then each arm turns (`0x80040D7C`), head and
  torso follow the locks, and AnimatePistols `0x8004874C` steps the frames.
- **Target search `0x80040948`**: walks the five creature slots
  (`0x8008C1C0`, stride `0x44`). A candidate must be alive and not Lara
  (`0x8008C4E8`), and within the range on each axis and by squared distance,
  measured from Lara's origin raised 650 to the creature's origin. It needs
  line of sight (`0x8001FB54`) to its aim point (`0x80040C40`: centre of its
  nearest-key collision box, a third down from the top, rotated by its yaw).
  Its angles (`0x80065210`), less Lara's yaw and the torso turn, must be
  inside the lock box. The nearest wins while its yaw is less than the best
  so far plus `0xAAA`.
- **Lock update `0x800406B4`**: without line of sight both arms unlock.
  Inside the lock box both lock. Otherwise each locked arm keeps its lock only
  inside its own box. The angles are stored in `0x8008C56C/E` either way.
- **AimWeapon `0x80040D7C`**: a locked arm turns toward the target angles, an
  unlocked one toward zero, by at most the aim speed per axis.
- **AnimatePistols `0x8004874C`**, right arm then left. An arm aims while
  locked, or while Action is held with no target. Aiming raises 0..4; at 4
  with Action it fires (flash 3, sound 8 unless the right arm already
  played it) and jumps to 24, and recoil returns to 4 at `24 + 9`. Not
  aiming lowers 4..1 and drops recoil straight to 4. So with no target,
  re-tapping Action fires every 2 ticks; with a locked target, recoil runs
  its full length.
- **FireWeapon `0x80040E8C`**: pistol ammo `0x8008C5A8` is set to 1000 before
  each shot. Two control-random draws (`0x8006E518`: seed at `0x80089D54`,
  ×1103515245 + 12345, bits 10..24) jitter pitch, then yaw, by
  `(r - 0x4000) * 1456 / 65536` (truncated). `0x80064F04` builds the gun frame
  at the muzzle. The target's spheres (`0x8005BAF8`) in that frame are tested
  with `|x|,|y| < r < z`, `x² + y² ≤ r²`, and the nearest `z - r` is hit.
  HitTarget `0x8004132C` subtracts the damage (no clamp), sets the hit flag
  and counts a kill when the hit takes the last hit points. A miss traces the
  line of sight for a ricochet (not ported).
- **ResetLook `0x800385EC`**: Lara's control (`0x80038258`) calls it at the
  start of every tick unless Look is held. Head pitch and yaw ease an eighth
  toward zero (truncated), snapping to zero within ±363, and the torso copies
  the head.
- **Movement hooks**: the slow-turn handlers request the fast turn at once
  while the guns are ready (`0x80038DC4`, `0x80038E94`). Grabs, reaches, swan
  dive and pickups require status 0 (`0x80038BEC`, `0x8003D408`,
  `0x8003D968`, `0x80050474`). Combat triggers require status 4 (`0x8001F0B0`).
- **Combat camera `0x80019578`** (type 3 branch `0x8001A038`): the target is
  Lara's x/z, its height a quarter of the way (arithmetic shift) toward 256
  above the chase height, speed 8. The eye is 2560 behind Lara's yaw plus the
  target angles, or plus torso and head with no target, with no elevation
  clamp. The shift reuses the squared chase distance `0x8008B298`, which only
  the chase camera writes: it was 1536² in `.psx/wall_game` and unchanged
  after 110 combat ticks.

## Presentation

`game/src/player/arms.rs` turns the gun state into drawing; the engine
supplies the generic parts: `engine::animation::PoseOverrides` (per-joint
keys from another clip, orientation from the model root, extra turns),
`engine::render::JointMeshes` (per-joint mesh swaps), `JointAttachment`
(meshes that follow a joint) and `engine::lighting::DynamicLights`.

- **DrawLara `0x80025BDC`** draws hips, legs and torso from Lara's clip.
  The torso (joint 7) turns by `0x8008C57A/C/E` after its key
  (`0x800261C0`) and the head (joint 14) by `0x8008C574/6/8`
  (`0x80026240`); the first of each is the yaw (the item's own rotation is
  passed the same way: `+0x42, +0x40, +0x44`). While the gun status is 2..4
  it dispatches on the gun through `0x800101F0` (pistols `0x80026654`;
  unarmed `0x800263BC` draws the arms from Lara's clip under the torso).
- **Pistol arms**: push, translate the upper arm's offset under the torso,
  then copy the rotation of the matrix two levels up the stack (Lara's item
  matrix, `0x8002667C..0x800266D0`), RotYXZ by the arm's aim
  (`0x8008C59E/A0/A2` right, `0x8008C58A/C/E` left), then the pistol frame's
  key for that joint (`0x800280D8` skipping 8 or 11 rotations). Forearm and
  hand follow with the next keys. The key pointer is the arm's frame base
  (`0x8008C594`/`0x8008C580`) plus `(frame - clip start) * size`: in Wall
  RAM the four pistol clips are contiguous 72-byte frames
  (218: 0-4, 219: 5-12, 220: 13-23, 221: 24-32), so a frame is the key of the
  clip that holds it (`engine::animation::model_frame_key`).
- **Mesh table `0x8008C52C`**: DrawLara draws whatever the table holds. At
  draw frame 13 `0x80048324` puts the pistol object's hand meshes
  (object 1 meshes 10/13) in the hands and Lara's own thighs on the legs;
  at holster frame 12 `0x800483C8` (left) / `0x8004843C` (right) swap one
  side back. Wall RAM holds the pistol thighs from level start
  (`0x80043B9C`; `0x8008C530` = object 1 mesh 1 = `0x8010F5C8`); Home,
  without pistols, keeps Lara's own (`.psx/home_start`). The table is not
  reset when dying frees the hands, so the port keeps it as
  `WeaponState::in_hand`.
- **DrawGunFlash `0x80028454`** (pistols take its default branch): from the
  hand, translate (0, 185, 40), RotYXZ(0, -16380, draw-random * 2),
  S_CalculateStaticLight(5120) (`0x8006CC14`: shade offset +1024) and the
  mesh of object 240. DrawLara draws it per arm while that arm's flash
  counter is non-zero. In the GPU capture of a flash frame
  (`.psx/pistols_flash_gpu`, field 85) its quads are flat grey 73
  (= 255 - (4801 + 1024) / 32) and semi-transparent mode 0;
  `engine::render::static_lit_mesh` builds that shade.
- **Gunfire light**: after AnimatePistols, PistolHandler adds a dynamic light
  while either flash is lit (`0x800486A8..0x8004872C`): Lara's feet plus
  `sin/cos(yaw) >> 4` (1024 units ahead), intensity 12, falloff 11, through
  AddDynamicLight `0x80028A28` (ten slots at `0x8008BB88`, count
  `0x8008B240`). Inside a cube and sphere of radius 2^11 it gives
  `2^12 - d^2 >> 10`. Items (S_CalculateLight `0x8006C9E4..0x8006CBC8`) add
  it to the brightest static light before halving and take its direction
  when it is the brightest; room vertices (`0x8006CE30`) add a 32nd of it to
  their grey byte, clamped, skipping 255, in rooms whose sector area its
  cube reaches.

Checked:

- `game/tests/pistol_presentation.rs` steps the ported gun rules on Wall
  data: holstered thighs at the start, keys `(219, 7)` at draw frame 12, the
  swap at 13, aim and recoil keys, per-arm holstering with one arm still in
  recoil, the unchanged table after death, the unarmed case, the gunfire
  light's position, and that a torso turn moves but does not turn the
  upper arm. Engine unit tests cover the dynamic light formulas.
- `tools/pistols_visual_smoke.py` replays the draw_fire_holster pad input in
  the port from the Wall checkpoint, puts the port's camera at the recorded
  original camera and writes side-by-side sheets (`validation/pistols-visual/`)
  against `psx run --snapshot-every 1` screens of the same input
  (`.psx/pistols_visual_ref`). The original's screen at field f shows the
  state sampled at f - 4 (the flash counter is set at field 80; flash and
  light are on screen at 84..89). Inspected: draw frames 9-16 (arm reach to
  the holsters, the frame-13 swap), ready frame 0, aim frame 4 and recoil
  frames 24/26 match in arm and gun placement to about a pixel; the gunfire
  light brightens the floor, walls and Lara in both. The original's bright
  yellow halo is the glow sprite (not drawn). A GPU capture at field 27
  (`.psx/pistols_draw_gpu`) shows that the shape beside Lara's neck during
  the draw is a braid segment, which the port's braid places differently.

## Recordings

All routes are ordinary pad input without pokes
(`tools/record_pistols.py`, `game/tests/data/original/pistols_*.{csv,json}`).

- From `.psx/wall_game` (pistols selected, hands free): draw/fire/holster,
  held Draw, aim taps, rapid taps, holster while firing, turning with guns
  ready, drawing while running. Held out, recorded after the rules were
  written: rapid re-tap after a shot (fires every 2 ticks), Walk-held turning,
  Draw taps during the draw, running while firing, and draw-turn-holster. Draw-turn-holster was recorded after the
  combat camera's resting-distance fallback was fixed; `run_fire_heldout`
  had exposed that fallback.
- Tiger routes from `.psx/pistols_tiger_start`, made by 640 fields of pad
  input from `.psx/wall_game` (run north, turn, run onto the room 36/37 trigger
  that activates tiger 18). Calibration: volley and lock-only. Held out:
  long left turn with late fire, re-taps, lock release/re-acquire, and a
  700-field held fire that kills the tiger at field 328. Each JSON lists every
  control-random store (`random_writes`).

## Checked

`game/tests/pistols.rs` compares 18 routes tick by tick with tolerance 0:

- Gun status, both arm frames, flashes and arm yaw, Lara's position, yaw,
  animation, state, goal, speed and turn rate, the combat/chase camera eye
  and target, and the number of arm shots in all twelve Wall-start routes.
  The last held-out route is compared to field 178: there Lara runs into an
  obstruction that the port's run collision does not stop at yet.
- In the tiger routes, Lara's and the tiger's recorded poses are replayed into
  the port just before LaraGun, because the tiger AI and Lara's knock-down
  are not under test. On ticks with shots, the random state before the gun's
  draws (the last two stores per arm shot of that field; nothing after the
  gun draws from this generator) is injected. Compared: gun status, frames,
  flashes, both locks, arm yaw and pitch, target angles, head and torso, and
  the tiger's hit points every tick. Every shot's hit or miss therefore
  matches, including spread. The combat camera eye and target match every
  tick, through the slide the tiger knocks Lara into at tick 102.
  After the long held-out kill, the hit points are compared until the
  tiger's own death code rewrites them near -16384 (tick 190).
- `engine::random` steps exactly like two consecutive recorded stores.
- `game/tests/player.rs` covers draw timing, volley spacing and the unarmed
  no-op; `game/tests/interactions.rs` covers hit points below zero and the
  single kill count.

## Not verified / not ported

- The gun-flash glow sprite (`0x80066180` from DrawGunFlash: sprite of
  object 236, tint 0x0F383F, additive, scale 512). Sprite sequences are not
  imported, so only the flash mesh is drawn.
- The braid: HairControl `0x8002F67C` builds its arm spheres from the gun
  arm frames (`0x8008C594`/`0x8008C59A`), and the hair root follows the
  turned head. `game::braid` still uses Lara's own clip for both.
- The flash roll's random sequence (`0x8006E54C`, the draw generator) and
  the static-light depth fade beyond view depth 12288 (`0x8006CC38`).
- Dynamic lights brighten room vertices only through a room mesh policy
  (the game has one; the sandbox and editor keep static rooms).
- Guns other than pistols, flares, vehicles, wading depth, ammo pickup
  selection and the inventory's gun requests.
- Smashable objects on the line of fire (`0x80020618`), ricochets and blood
  effects (`0x800291A8`).
- Hands-busy status 1 during climbs and pickups (`0x8003D924`, `0x800502D4`)
  and its release by the EmptyHands animation command are not ported. Draw is
  therefore not yet refused while climbing.
- Creature slots: active creatures stand in for the five slots, in entity
  order rather than slot order.
- Look (`0x200`) is mapped to L1 like the existing look camera; the look-flag
  test at `0x8008C510` and the camera-type-2 skip in ResetLook are not ported.
