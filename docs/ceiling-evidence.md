# Forward jumps under a low ceiling

Evidence: supplied Tomb Raider II USA Rev 3 disc through `psx`, with no outside
implementation reference. This milestone covers ordinary forward jump state3
against a flat, low room ceiling. Side/corner contacts, forward-top obstructions,
vertical-jump ceiling responses, reach and static meshes remain separate work.

## Controlled starting fixture

`tools/record_ceiling.py --prepare-checkpoint` starts from the existing settled
`.psx/airborne_wall_start/state.bin`. Five frame0 writes set only the initial
position, room and facing: `[37376,3840,91648]`, room0, yaw16384. The original
then runs90 neutral fields to settle and save `.psx/ceiling_low_start/state.bin`.
The corridor's decoded sector has floor3840 and ceiling2816, a1024-unit gap.
The game code and level are unchanged.

This is a **controlled spawn**, not a claim that an ordinary pad route was used
to reach the corridor. Each JSON sidecar retains that preparation command,
source/checkpoint hashes, exact replay, samples, disc/BIOS identity, toolkit
build, and write/trace hashes. The subsequent replays have no pokes. Native
flat instruction costs and two NTSC fields per logic tick match prior captures.

## Binary and runtime observations

- Forward collision `0x8003AB40` supplies a192-unit front-ceiling margin and
  the762-unit standing body probe through `0x8003C318`.
- `GetCollisionInfo` at `0x8001B748..0x8001B774` distinguishes a ceiling hit
  at the centre from a frontal obstruction. It stores a vertical correction
  and collision type8. The front-ceiling comparison at `0x8001B7A4` uses the
  configured192 margin; a slight intrusion into a uniform ceiling is therefore
  not a frontal wall hit.
- `LaraDeflectEdgeJump`'s type8 case `0x8003C748..0x8003C75C` leaves speed,
  heading, clip and goal alone, and sets nonpositive fall speed to1. Its
  `0x8003C608` call applies the collision shift first.
- In `.psx/ceiling_forward`, the retained trace executes that case at run
  field27: fall speed-82 becomes1. The next sampled tick, field28, has feet
  `[37823,3578,91648]`; subtracting762 places the head exactly at ceiling2816.
  Clip77/frame1237, state/goal3 and speed50 survive. At field46 it lands at
  x38273, then uses the ordinary authored recovery.

The previous port rejected any forward head intrusion as a frontal wall,
restored an x contact boundary, quartered speed to12 and reversed heading.
The new test first failed at tick14: x37787 instead of37823 and y3576 instead
of3578. The controller now applies the centre ceiling correction and stops
ascent separately from the frontal response. Only forward-jump state3 receives
the newly verified margin and ceiling rule. No engine format or tuning table
change is needed.

## Recordings and checks

| Case | Input | Purpose |
|---|---|---|
| `forward` | Up+Square[0,22),180 fields | Calibration: ceiling contact and normal landing/recovery |
| `forward_delay_heldout` | Up+Square[16,38),200 fields | Recorded after implementation: delayed contact and landing |
| `forward_release_heldout` | Up+Square[0,8), Square[56,66),200 fields | Recorded after implementation: early release and ignored jump during recovery |

Reproduce with `python3 tools/record_ceiling.py --case NAME` and
`cargo test -p tomb-raider-ii --all-features --test airborne`. The three cases
compare293 sampled logic ticks across13 columns; all match exactly except
the existing0.02-source-unit y conversion allowance. Both independent
validation recordings pass without changing the rule.

`python3 tools/ceiling_smoke.py` checks28 aligned poses in the full standalone
game, using the retained native camera positions. Paired screenshots and the
exact MCP transcript are retained in `validation/ceiling-smoke/`. The original
display trails sampled RAM by about four fields, as in the previous smoke
workflows. This corridor is dark in both games; the smoke's minimum colour
count is8, and pose/clip/frame checks remain exact. Raster, lighting and pixel
parity are not claimed.

No editor fields changed. The same engine sandbox and editor format checks
from the room-visibility repair still apply; the game controller is not used
by either authoring tool or sandbox. All195 workspace tests pass; Clippy
completes with the existing unrelated warnings. Changed Rust files are
formatted. Logs are retained under `validation/ceiling-{tests,clippy}.txt`.
