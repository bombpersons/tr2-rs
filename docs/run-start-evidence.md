# Run-start wall stop and held-jump retry

This closes the ordinary room-wall diagnostic retained by the
[low/middle airborne milestone](low-airborne-evidence.md). Evidence comes only
from the supplied Tomb Raider II USA Rev 3 disc and `psx`. Three no-poke pad
routes compare 333 sampled logic ticks across thirteen columns. Two routes
were recorded after implementation and passed without fitting.

## Binary evidence and rule

The RAM snapshot is `.psx/home_side/ram.bin`, SHA256
`745062c01811c58f18071e7eda7fdd43ca7412fea513d219f4950ae417d177b0`.
Reinspect with `psx disasm --ram .psx/home_side --addr ADDR --count N --text`.

- Run collision `0x8003A844` calls ground deflection at `0x8003C544`.
  Frontal contact applies the collision shift, sets current/goal state2,
  clears horizontal speed and gravity (`0x8003C56C..0x8003C598`).
- Run-start clip6 skips the hit-wall animation (`0x8003A8C0..0x8003A8CC`)
  and calls the shared stop routine at `0x8003A56C`. The shared routine
  checks the saved state from before this tick's animation. For standing or
  turn states2/6/7/20, it restores the preceding animation and advances it
  toward standing or the held left/right turn goal. Otherwise it selects
  standing clip11 at its authored start without advancing
  (`0x8003A600..0x8003A618`). The port previously selected idle clip103.
- Standing control `0x80038AF4..0x80038B08` tests held Jump before delegating
  Up to running. A run-start collision can therefore retry a Jump that was
  pressed while running, without another button edge. Run control still
  clears its jump-enable latch during clip6 and checks it before accepting
  a running jump (`0x800388D4..0x80038944`).

The readable game rule is: stop through the collision's standing pose, then
let the following standing tick consume held Jump. This remains game
controller code over the existing engine animation assets; no format,
template or editor change is required. The existing slope-floor sampling
order is preserved and its two earlier trajectories still pass.

## Runtime routes and reproduction

All routes start at `.psx/airborne_mid_wall_start/state.bin`: Home room16,
position `[19968,2560,96731]`, yaw0, clip103/frame1617. Checkpoint preparation
is retained in `tools/record_airborne.py --prepare-checkpoints`. Intervals
below are relative NTSC video fields, with one logic tick per two fields.

| Route | Input | Coverage |
|---|---|---|
| `run_start_retry_probe` | Up[0,12), Up+Square[12,30), neutral to220 | Original calibration diagnostic, now passing |
| `run_start_release_heldout` | Up[0,12), Up+Square[12,24), Up[24,80), neutral to180 | Release Jump before contact; repeated blocked run requests advance the restored standing/idle animation |
| `run_start_hold_heldout` | Up[0,8), Up+Square[8,100), neutral to260 | Earlier held request, wall stop, vertical fallback and landing |

At field28, the original resolves z97179 and selects clip11/frame185 with
state/goal2 and speed0. The two held-Jump routes enter compression73/frame1178
at30. The wall blocks forward takeoff, so compression links to91 at52, launches
into28 at58 with fall speed-104 and speed2, then lands into31 at130.
The release route remains grounded and follows clip11 into idle103 at30;
holding Up against the wall preserves idle progression instead of restarting
the clip every tick.

Record with `python3 tools/record_airborne.py --case NAME`. To update only
the former diagnostic's classification, use `--case run_start_retry_probe
--summarize-only`: its CSV and original command/checkpoint/sample/write hashes
are unchanged. Its sidecar now marks `used_to_refine_rules: true` and
`diagnostic_only: false`; the historical name is retained for traceability.
The two new sidecars identify independent post-implementation recordings.
Toolkit timing uses flat instruction costs, not cycle-accurate PS1 timings.

`game/tests/airborne.rs` compares x/y/z, yaw, animation/frame, current state,
goal, room, speed, fall speed, health and turn rate. All are exact apart from
the established 0.02-source-unit y float-conversion allowance, which was not
widened. Together with preceding routes, the suite covers 1524 sampled ticks.

## Verification and limits

`cargo test --workspace --all-features` passes 189 tests. Clippy completes
with existing warnings outside these changes. Changed Rust files pass
rustfmt; the workspace check still reports unrelated pre-existing differences.
Logs are retained in `validation/run-start-{tests,clippy,fmt,build}.txt`.

The full-game smoke workflow is `python3 tools/airborne_smoke.py --out
validation/run-start-smoke`: all 106 pose checks pass across fourteen routes,
including 26 checks for the three run-start routes. The report and exact MCP
transcripts are retained there. New comparison images are retained in
`validation/run-start-comparison.png`; standing stop, compression, vertical
takeoff, landing and released-Jump idle poses were inspected against the
aligned PSX frames. Existing HUD and shading differences remain.

The engine-only sandbox builds and runs, moves its Walker for 30 ticks and
renders the authored scene. Editor smoke passes on disposable sandbox and
Home copies, including untouched byte-exact saves, undo/redo and reopen.
Home placement 8 previews clip11's link to103 without dirtying the document;
the full-resolution editor screenshots were inspected.

Runtime coverage here is the cardinal ordinary room wall. The shared stop's
turn-state branches have binary evidence but no new dedicated angular sweep.
Corner/ceiling collision, static obstructions, unassisted fast falls and
slide/backward/sideways jump landing behavior remain separate work.
