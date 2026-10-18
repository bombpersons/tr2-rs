# Forward, vertical and reach jumps under low ceilings

Evidence: supplied Tomb Raider II USA Rev 3 disc through `psx`, with no outside
implementation reference. The milestones cover forward state3, vertical state28 and reach state11
against flat room ceilings. Side/corner contacts, forward-top obstructions
and static meshes remain separate work.

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
by either authoring tool or sandbox. At the forward milestone, all195 workspace tests passed; Clippy
completes with the existing unrelated warnings. Changed Rust files are
formatted. Logs are retained under `validation/ceiling-{tests,clippy}.txt`.

## Vertical jump extension

The same controlled checkpoint is reused, without further writes. The three
new sidecars retain the fixture preparation and source/checkpoint hashes;
the pad replays leave code, level and memory unchanged.

Binary evidence from `psx disasm --ram .psx/ceiling_low_start` establishes a
separate response in jump-up collision `0x8003B7B8`:

- `0x8003B7EC` supplies the same192-unit front-ceiling margin; `0x8003B828`
  supplies an870-unit body height, rather than forward jump's762.
- `0x8003B85C` applies the collision shift. Type8 (centre ceiling contact)
  requests goal2 at `0x8003B884..0x8003B888`. Unlike forward deflection, it
  leaves fall speed unchanged. The subsequent collision branch retains
  horizontal speed2 for nonnegative incoming speed (`0x8003B89C..0x8003B8B4`).
- The original animation's clip28 dispatch takes that goal into clip31.
  Standing collision then performs the final airborne move, settles the feet,
  clears gravity and fall speed, and retains speed2 until the following tick.

In `.psx/ceiling_vertical/trace.txt`, run field31 executes the type8 path.
The sample at field32 is `[37380,3686,91648]`, clip28/frame440, state28,
goal2, speed2, fall speed-98. Feet3686 minus body870 exactly meets ceiling2816.
At field34 it is `[37382,3840,91648]`, clip31/frame460, state/goal2, speed2,
fall speed0. Field36 retains the position and has speed0. This is an authored
transition into standing collision while still ascending, not a freefall.

The new calibration first failed at tick16: the port treated the ceiling as
a front wall and shifted x to36763, with y3638 and goal15. The controller now
uses the jump-up probe height and margin, applies the centre correction,
requests goal2, and allows that authored state transition to settle the feet.
No formats, baked assets, template or engine code changed.

| Case | Input | Purpose |
|---|---|---|
| `vertical` | Square[0,8),200 fields | Calibration: ceiling contact and full landing recovery |
| `vertical_delay_heldout` | Square[16,24), Cross[28,80),220 fields | Recorded after implementation: delayed contact with Action held; no grab |
| `vertical_hold_heldout` | Square[0,120),240 fields | Recorded after implementation: five ceiling contacts through the authored landing cancel window |

All333 sampled logic ticks across13 columns pass (the same0.02-source-unit
y conversion allowance). Both independent routes passed without changing the
implemented rule. The held route re-enters compression at field40 via clip31's
cancel range, rather than waiting until its natural end at field74.

Reproduce with `python3 tools/record_ceiling.py --case vertical` (or either
validation name) and the airborne test command above. The trace range is the
jump-up handler for these cases; reports retain339/339/1695 lines respectively,
with zero dropped lines and complete movement-heading watches.

The extended `tools/ceiling_smoke.py` passes all63 poses across the six routes:
35 vertical poses plus the existing28 forward poses. Paired original/port
contact and recovery screenshots were inspected. The same darkness, display
latency and raster/lighting limitations described above apply.

`cargo test --workspace --all-features` passes198 tests (six opt-in tests remain
ignored). Workspace Clippy and builds pass with existing unrelated warnings.
The changed Rust files and `git diff --check` pass; `cargo fmt --all --check`
still reports pre-existing differences in unchanged files. Logs are under
`validation/vertical-ceiling/`. The engine sandbox passed movement and render
checks. Editor sandbox and Home byte-exact save, undo/redo, reopening and
animation preview checks pass; Home placement8 previews vertical clip28.
No authoring format change was required.

## Reach extension and the later-contact fixture

The early-Action control `ceiling_reach` uses the original1024-unit corridor.
It passes unchanged: contact at field28 still occurs in forward clip77, before
Reach can enter clip94 at field40. It does not verify a Reach ceiling response.

`tools/record_ceiling.py --prepare-reach-checkpoint` therefore prepares a
second controlled spawn: `[26112,-1280,80384]`, room7, yaw16384, from the same
`.psx/airborne_wall_start/state.bin`, followed by90 neutral fields. The decoded
room has floor-1280 and ceiling-2816, a1536-unit gap. The taller gap delays
contact until Reach is active. This changes only the initial actor placement;
the level and game code remain unmodified. Each new late-contact sidecar
retains that exact preparation, source/checkpoint hashes, disc/BIOS identity,
input, samples and complete trace/watch hashes.

Binary evidence, re-read from the supplied disc's runtime RAM with `psx`:

- Reach collision `0x8003B18C` supplies the192-unit front-ceiling margin at
  `0x8003B1C4..0x8003B1D0` and uses the usual762-unit body probe. It tries
  grab before calling its separate deflection routine at `0x8003B1E8`.
- Deflection `0x8003C7F4` first applies the collision shift. Its jump table at
  `0x80010A14` maps type8 through entry `0x80010A2C` to `0x8003C870`.
  `0x8003C870..0x8003C884` sets a nonpositive fall speed to1, leaving heading,
  horizontal speed, animation and goal alone. Type16 shares this branch in
  the binary; that forward-top case has not been exercised here.
- In `.psx/ceiling_reach_late/trace.txt`, run field51 executes that table
  entry, reads fall speed-10 and writes1. Field52 is
  `[27159,-2054,80384]`, clip94/frame1489, state/goal11, speed50, fall speed1.
  Feet-2054 minus762 meets ceiling-2816. The next tick moves another50 and
  descends7. Landing at field84 retains speed50; field86 enters recovery82.

The new calibration initially failed at tick26: the port shifted x to26523
rather than27159, left y at-2060 rather than-2054, and retained fall speed-10.
The game controller now gives Reach the verified ceiling margin/correction
and stop-ascent response, preserving its distinct landing timing and clip.

| Case | Input | Role |
|---|---|---|
| `reach` | Up+Square+Cross[0,22), Cross[22,100),200 fields | Early-contact control; passes before this change |
| `reach_late` | Up+Square+Cross[0,22), Cross[22,120),220 fields | Calibration for contact during Reach |
| `reach_late_release_heldout` | Up+Square+Cross[0,22), Cross[22,44), Up[128,160),240 fields | Ceiling rule passed, but later Up release exposed the run-stop defect; explicitly marked calibration for that rule |
| `reach_late_delay_heldout` | Up+Square+Cross[16,38), Cross[38,108), Cross+Square[108,118), Cross[118,136),240 fields | Independent delayed contact and ignored recovery jump |
| `reach_late_validation_heldout` | Up+Square+Cross[8,30), Cross[30,50), Up[136,168),240 fields | Fresh independent release, landing, running and authored stop |

These five new routes compare575 sampled logic ticks across13 columns with
the existing0.02-source-unit y allowance. The source filenames retain the
originally intended held-out names; JSON `used_to_refine_rules` and the test
name distinguish the release calibration from independent validation.
The run-stop fix and its additional independent recordings are documented in
[run-stop-evidence.md](run-stop-evidence.md). No engine format changed.

The combined ceiling smoke passes120 poses over11 routes, including57 new
Reach/control poses. Paired ceiling contact and recovery images were inspected;
movement/clip/frame parity is checked, while raster, lighting, HUD and other
actor rendering differences remain outside this claim. Reports and exact MCP
transcripts are under `validation/ceiling-smoke/`.

The completed workspace pass has208 passing tests and six opt-in tests ignored.
Clippy and all-target builds pass with existing unrelated warnings. Changed
Rust files and `git diff --check` pass. Engine sandbox movement/rendering and
editor sandbox/Home byte-exact save, undo/redo, reopening and clip10 preview
checks pass. No engine, editor format or template change was needed. Logs are
retained under `validation/reach-run-stop/`.
