# Airborne side and corner contacts

Evidence comes only from the supplied Tomb Raider II USA Rev 3 disc, through
`psx` (build `src-88132a8eecb2461c-release`). This covers forward jumps
(state 3) and reach (state 11) that strike a wall with the body's leading
side corner, in both a low and a tall room, as well as the run-stop wall
decision exposed by one route. Vertical jumps, fast falls, static meshes and
the clamp case remain separate work.

## Native rule

Forward-jump collision `0x8003AB00` and reach collision `0x8003B18C` classify
the whole body with GetCollisionInfo `0x8001B018` (radius 100, height 762,
facing = the movement heading at `0x8008C572`). Their limits differ:

| Handler | bad_pos | bad_neg | bad_ceiling | Writer |
|---|---|---|---|---|
| Forward jump | 32512 | -384 | 192 | `0x8003AB30..0x8003AB48` |
| Reach | 32512 | 0 | 192 | `0x8003B1B4..0x8003B1D0` |

So reach treats any front floor above the feet as a wall. On a reach's
landing tick that restores the across-axis coordinate (`open_reach` field 16492:
the deflection reads contact type 1, x keeps its same-sector move and z is
restored).

GetCollisionInfo tests the left/right corner probes' floors only
(`0x8001B884..0x8001B9B0`): side ceilings never classify a contact. A centre
ceiling hit stores its vertical shift first (`0x8001B768..0x8001B774`), and
a later front/side classification keeps that shift. The shift is applied by
ShiftItem `0x8001C09C` at the start of either deflection routine.

Deflection jump tables, read from runtime RAM:

| Contact | Forward jump `0x8003C5F4`, table `0x80010994` | Reach `0x8003C7F4`, table `0x80010A14` |
|---|---|---|
| Front (1), top-front (16) | rebound: speed/4, heading +0x8000, clip 31/32 by clearance, fall speed ≤0 → 1 | shift only (front); stop ascent (top-front) |
| Left (2) | yaw +910 (`0x8003C720`) | yaw +910 (`0x8003C848`) |
| Right (4) | yaw −910 (`0x8003C734`) | yaw −910 (`0x8003C85C`) |
| Top (8) | fall speed ≤0 → 1 (`0x8003C748`) | fall speed ≤0 → 1 (`0x8003C870`) |
| Clamp (32) | step back 100 along the facing, speed 0, mid floor 0, fall speed ≤0 → 16 (`0x8003C760`) | same (`0x8003C888`) |

The forward front case skips the rebound when the halfword at `0x8008C4F6` is
set and speed is exactly 2 (`0x8003C648..0x8003C664`). Neither the flag's
meaning nor that path has been exercised; the port does not model it. The
clamp branch is ported from the binary but no recording reaches it, and the
port does not reproduce its mid-floor reset.

## Port

`game/src/player/traversal.rs` now routes forward-jump and reach collision
through the existing `collision::info` port with the per-handler limits above,
applies the full shift, and dispatches the contact in `deflect`. The forward
frontal rebound and the grab test keep their existing code, now triggered by
front/top-front contacts. The vertical jump and fast fall keep their
previous probes. No engine, format, editor or template change was needed.

The first calibration failed before the change at tick 15 (`open_left`): the
port stopped x at the side wall and never turned (x 26523 vs 26569, yaw
12384 vs 13294). The reach calibration then failed only on its landing tick
(z 82693 vs 82700) until reach used `bad_neg` 0.

### Run-stop wall clip

`open_reach_release_heldout` ran into a pillar three ticks after Up was
released. The original sets state 12 (so its wall probe `0x8003C91C` hit) but
enters ColStop clip 11. Run collision chooses clip 53 only for animation
frames 0–9, clip 54 for 10–21, and ColStop otherwise
(`0x8003A8EC..0x8003A944`). The released run had already dispatched toward
its stop clip, so the frame was outside both windows. The port predicted that
tick's animation by always requesting the run clip; it now requests the
clip the current controls ask for. The route is therefore labelled as
calibration for this rule (`used_to_refine_rules`), and
`open_right_reach_validation_heldout` was recorded afterwards.

## Recordings

`python3 tools/record_corner_jump.py --prepare SPAWN` writes Lara's position,
room and yaw at frame 0 into the rebuilt `.psx/home_start` checkpoint, then
runs 90 neutral fields. This is a controlled spawn, not an ordinary pad route.
In the open hall the settle moved Lara from z 82600 to 82669 before row 0. The
tests and smoke start from row 0. `--case NAME` replays the pad input with no
pokes, traces both deflection routines and watches the movement heading. Each
sidecar keeps the preparation command, source/checkpoint hashes, disc/BIOS
identity, toolkit build and sample/write/trace hashes.

| Case | Room / input | Role |
|---|---|---|
| `low_left` | 1024 corridor, Up+Square[0,22) | Calibration: ceiling stop, then five left turns |
| `open_left` | 1536 hall, Up+Square[0,22) | Calibration: six left turns while rising |
| `open_reach` | hall, + Cross through reach | Calibration: side turns, reach, front-contact landing |
| `open_right_heldout` | hall south wall, Up+Square[0,22) | Independent: right turns into a low frontal impact (clip 31) |
| `low_right_delay_heldout` | corridor, Up+Square[16,38) | Independent: delayed right contacts under the ceiling |
| `open_reach_release_heldout` | hall, reach, then Up[136,168) | Recorded after the side rule; calibration for the run-stop fix |
| `open_right_reach_validation_heldout` | hall south wall, reach, then Up[140,200) | Independent after both fixes |

The seven routes compare 727 logic ticks across 13 columns with both pad
inputs active. All match exactly except the existing 0.02-unit y conversion
allowance. Reproduce with `cargo test -p tomb-raider-ii --all-features
--test airborne`.

## Verification

`cargo test --workspace --all-features` passes 462 tests (8 opt-in ignored),
including every existing airborne, ceiling, traversal and run-stop route.
All-target/all-feature Clippy passes with existing unrelated warnings.
Changed Rust files are formatted, and `git diff --check` passes.

`python3 tools/corner_jump_smoke.py` (after `cargo build -p tomb-raider-ii
--features dev-mcp`) checks 69 aligned poses across all seven routes in the
standalone game, using the native camera samples. Paired contact images in
`validation/corner-smoke/` were inspected. HUD, display latency and raster
parity are not claimed. The engine sandbox and editor do not use the game
controller and are unchanged.
