# Submerged stroke, glide, tread and steering

Ordinary submerged movement now uses the PSX stroke/glide/tread rules and
extracted animation dispatch. This replaces the provisional constant-target
velocity smoothing and immediate clip selection. The controller is a separate
player module with a reflected `SubmergedMotion` component; its speed, bank
and oxygen state are independent of grounded motion. The rendered body now
uses the recorded pitch and bank as well as yaw.

## Evidence and controlled starting checkpoint

Only the supplied USA Rev3 disc and `psx` were used. `tools/record_swimming.py
--setup` retains both setup commands in `swimming_fixture.json`. From the
existing unmodified Home gameplay checkpoint it changes only Lara's initial
XYZ, room and yaw to the deep pool interior, lets the original settle for120
fields, then sets initial pitch0 and runs two more fields. The original itself
enters water and settles into tread108 before the replay checkpoint is saved.
These initial-position/pitch pokes are explicit setup, not ordinary play and
not a claim of water-entry parity. The six pad replays make no memory pokes.
The Rust game reads only its existing standalone level and tuning assets.

`--inspect` retains PSX listings and byte hashes in `swimming_code.json`:

- Control handlers, table `0x80082C98`: tread13 at `0x800455F4`, stroke17 at
  `0x80045474`, glide18 at `0x8004552C`; shared steering `0x80045384`.
- Underwater wrapper `0x80044FC4..0x80045384`: state control, bank/turn decay,
  angular limits, yaw update, animation, movement, object collision, state
  collision, room update and weapons/triggers.
- Collision table `0x80082DE0` dispatches these three states through wrappers
  to `0x80045CB8`. Its general collision branches remain separate work.
- Native animation `0x80042E08` increments before goal dispatch, follows
  default links and updates the current state while retaining the requested
  goal. This differs from AnimateItem's goal reset on a changed default state.

The cached Ghidra analysis of Wall RAM was used to inspect the underwater
wrapper and collision query, then checked against Home MIPS. Home and Wall
bytes match for both controller ranges: SHA256
`18af5be2de1c5d4e42dd7257756d9f191d4d92d276fc584753a3ca8614b86f93`
for `0x44FC4..0x45384`, and
`467bc5886c93831cb84666f4f56651b42824de62b6e3f749d9fa9255cd6aa869`
for `0x45384..0x4570C`. Static inspection establishes candidate rules; the
retained runtime samples establish the behavior below.

## Verified rules

The source's item+32 stores quarter-units of water speed per tick. Stroke
adds8, capped at200:2 original units/tick acceleration and50 units/tick maximum
(1500 units/second). Glide and tread subtract6 (1.5 units/tick), clamped to0.
Stroke requests glide when Jump is released; dispatch waits for authored stroke
ranges. Glide requests stroke when Jump is re-held, then requests tread if its
new speed is below134 quarter-units. Tread requests stroke on held Jump and
otherwise preserves its previous goal. A released high-speed stroke can thus
end glide199 through its default link into tread108 while goal18 persists.
Starts109, stroke86, multiple glide clips and tread loops come from level data.

Up subtracts364 binary pitch units/tick, Down adds364, with Up priority. This
means Up pitches downward underwater. Pitch is limited to±15470 (about85°).
Left/Right add∓409 to turn rate, capped at±1092 before364 decay. Body bank adds
∓546, decays364 toward0 and clamps to±4004. Bank recovers on release; pitch
remains where the player aimed it. Yaw changes before underwater movement;
the ordinary ground system's later yaw update is skipped for submerged actors.

Movement at `0x80045224..0x800452D4` uses the verified quantized sine table.
Each horizontal yaw-thrust component is shifted to an integer before pitch's
cosine is applied; Y uses the signed pitch-thrust shift. Keeping this sequence
avoids visible per-axis drift during a turn/dive. The component uses ordinary
world speed and radians, with the exact integer conversions confined to this
boundary. Design values are editable under Lara's existing `swimming` entry
in `game/data/object_types.json`.

Air at `0x8008C4FE` starts at1800 and decrements before underwater control
(`0x80042C1C`). The component keeps this integer counter and publishes seconds
to the existing HUD state. All six routes verify ordinary oxygen consumption;
exhaustion/death, surface refill and entry timing remain outside this claim.

## Validation and limits

`game/tests/swimming.rs` compares700 ticks and16 columns across six complete
submerged replays: straight acceleration/full stop, steering with downward
pitch, idle angular caps/recovery, then three post-implementation validations
for a second stroke, opposite steering/upward pitch and idle direction reversal.
XYZ, yaw, pitch, roll, turn rate, both speed fields, clip/frame/state/goal,
room, air and health match exactly. Neither positions nor clips are supplied
after tick0. A separate hand-authored two-state graph replaces the old
non-disc test's provisional Up-moves-up expectation.

The full-game MCP smoke replays all six routes after initial alignment and
checks126 complete-game poses, including reflected motion, animation, oxygen
and the rendered body quaternion. Native/rebuilt views were visually inspected:
body orientation and authored poses agree, while the original's underwater
green palette was subsequently ported in [a separate verified change](underwater-palette-evidence.md); wet-room caustics were later ported in [another independent change](caustics-evidence.md). The camera is
aligned to recorded PSX samples for this pose check; default underwater camera
behavior is not claimed by this comparison.

Current volumes, underwater roll and weapons remain unverified. Oxygen
exhaustion and fatal drowning were later verified in
[a separate milestone](drowning-evidence.md). Surfacing, surface swimming, the dive and deep-water entry were
later ported in [a separate verified change](surface-evidence.md); submerged
collision in the section below.

All-features workspace tests and Clippy completed; existing unrelated warnings
remain. The engine sandbox ran offscreen. The editor Home Lara stroke preview
and untouched save passed. No engine format or template changes were needed.

## Submerged collision (0x80045CB8)

The provisional floor/body-clearance clamp is replaced by the original
collision routine shared by states 13, 17, 18 and 35 (table `0x80082DE0`
entries `0x800457D0`, `0x80045790`, `0x800457B0`, `0x800457F0` each only call
`0x80045CB8`). Code: `swimming::collide` and the general GetCollisionInfo
port `game/src/player/collision.rs`. Static reading with `psx disasm --ram
.psx/home_start/ram.bin`, then checked against runtime routes:

- Wrapper `0x80044FC4`: radius 300, bad_pos 32512, bad_ceiling 400, old
  position saved, slope/lava flags cleared; after control it clamps pitch to
  ±15470 and roll to ±4004 for every submerged state (the port now also
  clamps the dive), moves, runs state collision, then updates the room at the
  feet (`0x8001C0E8` with offset 0).
- `0x80045CB8`: facing is yaw, or yaw+180° beyond vertical pitch, and is also
  written as the move angle a surfacing tick uses. The body height is
  `|762·sin(pitch) >> 14|`, at least 200; bad_neg is minus that height and
  the probe line is half of it below the origin. ShiftItem applies the shift.
  Front: pitch beyond ±6370 deflects a further 364 away from level, otherwise
  speed stops. Top: pitch at or above −8190 drops 364. Top-front: speed stops.
  Left/right: yaw ±910. Clamp: back to the old position, speed 0, nothing else.
  A floor above the probe line (`mid.floor < 0`) lifts the body by it and adds
  364 pitch. Finally `0x80045BA4` undoes the move when GetWaterDepth
  `0x800458CC` finds no water below.
- GetCollisionInfo `0x8001B018`: mid/front/left/right probes by cardinal
  quadrant (front's transverse offset from the sine table), rooms resolved
  160 above the body and chained from probe to probe; floors relative to the
  probe line, ceilings to the body top. No mid floor → front and undo;
  floor ≤ ceiling → clamp and undo; mid ceiling ≥ 0 → top with a downward
  shift; a bad front floor or ceiling → front, undoing the move along the
  facing axis and grid-shifting across it (`0x8001B9EC`); front ceiling exactly
  at the limit → top-front; a bad left/right floor → side, grid-shifted
  relative to the front probe. This is why a 20° stroke into the north wall
  stops dead on both axes instead of sliding.

### Evidence and validation

`tools/record_swim_collision.py` records four routes, all first recorded after
these rules were written. Three start from the submerged pool checkpoint above:
a head-on stroke into the north wall, an angled stroke to the east wall that
turns back along it (right-side contacts deflect yaw by 910 per tick to level)
and stops in the north-east corner, and a downward stroke pushed off the pool
floor (the floor lifts pitch 364 per tick to level) into the wall. `--setup`
builds a second checkpoint in Home's water tunnel: its only pokes place Lara
in room 42 facing west (`swim_collision_fixture.json`); the original enters
the water and settles itself. The fourth route rises into the tunnel (room 46)
and slides under its solid ceiling, with Top contacts fighting held Down,
into room 41. Replays make no pokes.

`game/tests/swimming.rs` compares every column of the four routes, and the two
held-out back-hop entries in `game/tests/surface.rs` now run to their end:
both meet the pool's east wall in the entry dive, are grid-shifted back to
x 30419 and keep pitching down 364 per tick past the ±15470 limit while they
sink, through stroke, glide and tread.

Not ported: static-mesh collision (`0x8001BA24`, no statics in these water
rooms), the slope/lava pit flags (cleared underwater), wading from water 512
or less deep (`0x80045BA4`'s other branch; wading is not ported at all),
water currents (`0x80045EE0`) and the fly cheat's skip.
