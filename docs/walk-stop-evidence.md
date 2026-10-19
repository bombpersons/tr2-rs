# Authored walking stops, flight requests and braking-wall state

Evidence is only the supplied Tomb Raider II USA Rev 3 disc through `psx`.
This milestone resolves the two diagnostics retained by the
[ledge-fall milestone](ledge-fall-evidence.md): Walk during forward flight and
after landing, and the state override when a braking run meets a wall.
Direction reversal, walk/run transitions, swan-dive execution, unusual floor
contacts and angular/static-obstruction sweeps remain separate work.

## Rules established from the original

Reinspection uses `.psx/home_hang_start/ram.bin`; commands and listings are
retained under `validation/walk-wall/`.

- Walk control `0x800386C4..0x800387AC` requests goal2 when Up is released
  (`0x800387A0`); it keeps the walking state until an authored range accepts
  the request. Clip1 offers standing transitions into2/frame58 at absolute
  frames37..38 and3/frame74 at55..56. Both stop clips retain state0 and have
  no restart dispatch: re-holding Up changes the goal to0 but must wait for
  the natural end into standing11 before a new walk-start20 can begin.
- Walk-start20 and acceleration21 have no stop ranges. Early release must
  finish them, enter the walk loop, and reach its first stop range. Speeds
  continue to come from the frame being advanced, including while Up is
  absent. The original diagnostic released Up during20; the port previously
  forced standing immediately, losing208 source units of movement.
- Forward-flight control `0x80038B88..0x80038C48` resets previous reach11 or
  dive52 requests, then considers input unless landing/death goals1/2/8 are
  pending. With hands free, Action requests11 and Walk requests52 afterward,
  giving Walk priority. Fall34/35 offer no range for52, so these recordings
  retain their falling clip and speed. Landing then requests standing2;
  standing control can ask to walk while the landing animation finishes.
  Only this unaccepted dive request is runtime-verified here; swan-dive entry
  and its controller are not claimed.
- Run collision `0x8003A8C0..0x8003A944` excludes run-start6, checks the wall,
  and writes state12 at `0x8003A8F0` before testing the foot ranges for hit
  clips53/54. Braking10's absolute frame169 is outside both ranges, so the
  shared stop routine enters11 while retaining12. Its next natural end link
  restores standing state2. The port previously reported standing too early.

The game keeps ordinary ECS modes and authored animation assets. Walking
release now retains movement until the authored stop, and the existing
`HitWall` mode also represents the one-tick standing stop outside the hit
foot ranges. Flight submits its requests through the clip's authored ranges.
No engine format, template or editor change is needed.

## Recording identities and independent validation

The two former diagnostics retain their filenames, inputs, complete CSV bytes
and original sample/checkpoint/write hashes. Their JSON now marks
`used_to_refine_rules: true`, `diagnostic_only: false`, and explains their prior
role. Their historical `_heldout` names do not mean independent validation of
this milestone. Tests explicitly call them calibration.

| Case | Role and observation |
|---|---|
| `forward_ledge_walk_heldout` | Calibration: airborne Walk requests52 at60; standing landing82 at80; walk-start20 at118; release132 finishes into stop2 at174, standing206 |
| `forward_ledge_release_heldout` | Calibration: later braking-wall contact196 uses clip11/state12, then clip103/state2 at198 |
| `walk_stop_left_heldout` | Independent: Up+R1[0,100),240 fields; waits for clip2 at128, standing160 |
| `walk_stop_right_heldout` | Independent: Up+R1[0,80),220 fields; waits for clip3 at92, standing124 |
| `walk_stop_rehold_heldout` | Independent: Up+R1[0,80),[110,162),280 fields; re-hold during3 waits for standing124, restarts20 at126, then early release finishes21 and stops via2 at182 |
| `forward_ledge_walk_validation_heldout` | Independent: Up[16,72), Up+R1[72,148),280 fields; airborne52 at74, standing landing96, walking134, early release150 and authored stop190 |
| `braking_wall_release_validation_heldout` | Independent: Up[18,74),[152,198),280 fields; braking10 at208, clip11/state12 at210, standing103/state2 at212 |

The left/right names distinguish stop clips2/3. The first two cases contribute
242 sampled ticks and the five fresh independent routes655, for897 complete
sampled logic ticks across thirteen columns. All pass without fitting controller
rules to the independent recordings. Comparisons retain the existing0.02-source-
unit y allowance; all other columns are exact, including oblique x/z on the
runway walking routes.

Reproduce with `python3 tools/record_airborne.py --case NAME`. Checkpoint recipes
are retained in that script and `tools/record_traversal.py`; JSON sidecars record
the precise command, inputs, hashes, toolkit build, disc/BIOS identity and native
timing model. All routes and checkpoint preparation use ordinary pad input,
without pokes.

One additional fresh recording, `braking_wall_validation_heldout`, is retained
as `diagnostic_only: true` without a passing test. It matches the braking-wall
override and subsequent jump through field226. At228 the original vertical
jump begins moving over the wall (z91037), while the port remains at91035,
despite matching y, speeds and animation. That vertical front-wall clearance
branch is the next concrete gap; this recording has not been used to fit it.

## Verification

`cargo test --workspace --all-features` passes218 tests; six opt-in/documentation
tests remain ignored. Workspace all-target builds and Clippy pass with the
existing unrelated warnings. Changed Rust files and `git diff --check` pass;
workspace-wide rustfmt retains the previously documented23 unchanged files.

`tools/airborne_smoke.py` adds82 full-game pose checks across these seven
passing cases. The complete current sweep passes283 poses across29 routes,
including all previous airborne routes. Three bounded batches retain reports,
paired images and MCP transcripts under `validation/walk-wall/airborne-batch-*/`,
with the combined report at `validation/walk-wall/airborne-report.json`.
Positions and authored clip/frame timing are checked against PSX samples.
Original displayed images use the established
approximately four-field offset. Inspected pairs cover stop clips, recovery,
restart and the braking-wall stop; existing NPC, HUD and rendering differences
remain outside the pose checks.

The engine sandbox passes30 ticks of movement and rendering. The editor passes
sandbox and Home byte-exact untouched saves, undo/redo, save/reopen and Home
placement8 preview of clip3 through its16 authored frames into11. Logs, binary
listings and sandbox evidence are retained under `validation/walk-wall/`.
