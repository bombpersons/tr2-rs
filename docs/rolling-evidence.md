# Authored grounded rolling and roll departures

Rolling through animated item contact beside a floor-height change is now
verified separately: see [the contact ordering evidence](item-push-evidence.md#rolling-contact-before-floor-response).
The roll's floor response follows the item push in the same tick.

Evidence is the supplied Tomb Raider II USA Rev 3 disc through `psx` only.
This milestone replaces the former0.7-second roll with authored animation,
movement, turn and recovery. Verified routes cover ordinary flat/oblique motion,
one-click steps, a cardinal room wall, repetition while Roll stays held,
recovery into walking, walking's refusal of Roll, and a second-half departure
into backward fall/landing. Static/dynamic objects, corner/ceiling boundaries,
steep slopes and other launch states remain unverified. The later first-half
departure verification is recorded below.

## Binary rules

Listings, a yaw-write watch and reports are retained under `validation/rolling/`.
The collision dispatch table at0x82DE0 maps state45 to0x8003BC50 and23 to
0x8003BD14; control table0x82C98 maps them to0x80039BB8/0x80039BC0.
Those control handlers do not interrupt their committed clips for input.

- Standing control0x800389D4..0x80038A1C forces clip146 at relative frame2,
  state45 and goal2 on held Roll. Run control0x800387CC..0x80038800 has the
  same entry; walk control0x800386C4 has no Roll request. The same tick's
  authored advance yields frame3896 (+3), speed50. Entry timing comes from
  code; movement speeds, lengths and end links come from the baked clips.
- Clip146 ends into147 at frame3903. Its authored effect0 calls the turn
  routine0x8002A034: yaw adds180 degrees and pitch reverses. A native write
  watch attributes the yaw store at run field15 to0x8002A048, visible in
  sample16. Animation movement still uses the preceding collision heading
  on that tick; a direction change must not reverse physical travel.
- First-half collision follows yaw and second-half collision follows yaw+180
  (0x8003BC84/0x8003BD50). Both permit downward floor changes,384-unit steps
  up, zero front-ceiling margin, and uphill steep-slope rejection. They apply
  room grid shifts without cancelling the roll or zeroing its authored speed.
- Clip147's authored acceleration gives speeds50,47,45,43,40,...,20. Clip148
  is standing state2 with speed0. Its authored links own recovery; held Roll
  can force another roll immediately on the next standing-control tick.
- Second-half centre-floor drops above200 enter93/state/goal29 and seed
  gravity (0x8003BD9C..0x8003BDE4), retaining that tick's speed. The existing
  backward-fall controller owns subsequent descent and landing. First-half
  collision calls the existing >384 forward ledge-fall test. The later
  first-half departure recordings below verify that branch without changing it.

The game keeps a small ECS `Rolling` mode with an entry flag, advances the
engine's ordinary authored clips/cues, and applies the existing verified sine
movement and room probe/grid helpers. It hands standing recovery to the existing
recovery controller, and departures to the existing forward/backward flight
modes. The guessed timer and delayed manual turn are removed. No raw PSX state
machine dispatch or emulation loop is introduced.

## Recordings and independent validation

The first four routes are explicitly labelled calibration. Six fresh independent
routes were recorded after implementation and passed without fitting the roll
rules to them. The earlier `roll_ledge_heldout` ends before the edge: its name
is preserved, and it is documented as a near-ledge control, not a falling route.

| Case | Observation |
|---|---|
| `roll_runway` | Oblique heading-15647: entry146 at2, effect/147 at16, recovery148 at44, standing11 at60, idle103 at62; total displacement[-834,0,49] |
| `roll_step_up` | One-click ascent during147 at30; authored turn, deceleration and recovery remain unchanged |
| `roll_step_down` | One-click descent at14 before the turn; returns to the initial Home floor/spawn and completes recovery |
| `roll_wall` | Cardinal wall correction at12 holds z91035 while authored speed/clip keep advancing |
| `roll_runway_delay_heldout` | Independent delayed entry; Right[22,38) during the roll has no effect |
| `roll_wall_hold_heldout` | Independent Roll[0,100): standing recovery at44 allows another roll46; third starts90; all three complete with the original wall correction |
| `roll_recovery_walk_heldout` | Independent delayed roll: Up+R1 changes goal0 at64 during148; natural link11 at72, walk-start20 at74, then authored release/stop |
| `roll_ledge_heldout` | Independent near-edge control stops at z88999 before departure; ordinary authored recovery |
| `roll_ledge_fall_heldout` | Independent closer starting point: turn16, second-half departure93 at18, retained speed47 through descent, landing50, recovery82 at52, idle90 |
| `roll_during_walk_heldout` | Independent Up+R1 walk with Circle held[40,64); walk controller ignores Roll and continues through its step-up/stop |

All1220 sampled ticks compare thirteen columns. Everything is exact except the
existing0.02-source-unit y allowance. Sidecars retain full command/input,
source/checkpoint/sample/write hashes, toolkit build and disc/BIOS identities.
No route or checkpoint preparation uses memory pokes or altered game code.

`tools/record_airborne.py --prepare-roll-ledge` prepares the departure fixture
from `forward_ledge_start`: Up+R1[0,60),160 fields, settling at
[19968,2048,88642], yaw0. The recipe and source hash are retained in the sidecar.
Reproduce recordings with `--case NAME`.

## Verification

All248 workspace tests pass, including all previous movement and rendering
regressions. All-target builds and Clippy pass with existing unrelated warnings.
Changed Rust files and `git diff --check` pass.

Full-game MCP checks119 poses across all ten routes. Paired original images
use the established four-field display offset; position, clip and authored frame
are checked. Images covering the turn, both step directions, wall correction,
repeated entry, recovery walk and backward departure/landing were inspected.
Reports, paired images and exact MCP operations are retained under
`validation/rolling/airborne/`. Raster, NPC, HUD and unmeasured sound differences
remain outside the pose claim.

Engine sandbox movement/rendering passes30 ticks. Editor sandbox and Home
checks preserve untouched bytes, exercise undo/redo and save/reopen, and preview
clip146 through its authored link into147. No engine format, editor or template
change was required.

## First-half departure verification

Three additional no-poke recordings were made after the rolling controller was
implemented. All pass unchanged; none was used to fit its rules. Their checkpoint
is the end of the earlier `walk_ledge` route: Up+R1[0,220),280 fields from
`forward_ledge_start`, settled at [19968,2048,88987], yaw0. The new sidecars
retain this preparation, checkpoint/source/sample hashes and full toolkit identity.

- `roll_first_half_fall`: roll enters146 at field2, crosses the edge into
  34/state/goal3 at6 with speed50 and no vertical movement on that tick. The
  subsequent flight uses35 at16, lands into82 at38, links11 at74 and103 at76.
  Yaw stays0: leaving before147 skips its authored turn.
- `roll_first_half_walk_heldout`: independent delayed roll, R1 during flight
  requests goal52 without interrupting34/35. Landing resets goal2; later
  Up+R1 starts the existing authored walk and stop.
- `roll_first_half_run_heldout`: independent delayed entry ignores a second
  Jump while airborne, completes landing, then runs and brakes through the
  existing authored clips.

All413 sampled ticks compare the same thirteen columns exactly except the
existing0.02 y allowance. The full-game MCP checks44 poses, including the edge
crossing, descent, landing and both recoveries. Paired original and port images
are retained under `validation/first-half-roll/airborne/`. All261 workspace
tests, all-target builds and Clippy pass. This step adds evidence, regressions
and replay coverage; it changes no game/engine/editor runtime code or format.
