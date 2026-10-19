# Running off a ledge and authored landing

Evidence comes only from the supplied Tomb Raider II USA Rev 3 disc through
`psx`. This milestone verifies a cardinal running departure from a three-click
Home ledge, landing into a run or standing recovery, and restarting after that
recovery. Unassisted fast falls, slides, angular/corner contacts, intermediate
step heights and water entry remain separate work.

## Observed rule and implementation

The earlier `.psx/fall_forward_probe` diagnostic identified the mismatch:
forward flight uses clips34/35, and holding Up on landing enters clip92 with
speed47. The former port instead used `freefall_landing` and a provisional
0.15-second landing lock, losing the landing tick's extra movement and the
authored running continuation.

Reinspection with `psx disasm --ram .psx/home_hang_start` establishes:

- Run collision calls the shared ledge-fall test at `0x8003A94C`.
  `0x8003A500..0x8003A568` accepts a centre-floor drop greater than384 source
  units, enters clip34 at its authored first frame, sets state/goal3 and
  gravity, and clears fall speed. Horizontal speed is retained. The source
  exempts water status4; water behavior is not claimed by these dry routes.
- The resulting flight uses the already verified forward handler
  `0x8003AAE4`. Landing `0x8003AB74..0x8003AC0C` requests running when Up is
  held without Walk, otherwise standing. It snaps to the floor, clears speed,
  fall speed and gravity, then calls AnimateLara again on the landing tick.
  The destination clip supplies the grounded speed and the second move.
- Extracted clip35 accepts goal1 into clip92/frame1463 and goal2 into
  clip82/frame1361. Clip92 runs at47 and naturally links to the run loop;
  clip82 holds its authored18-frame standing recovery. These timings and
  transitions remain asset data interpreted by the existing animation system.

The game now hands a running ledge departure greater than384 to the existing
`PlayerMode::JumpingForward` flight/collision path. It enters34 on the departure
tick without advancing it, preserving horizontal speed and starting with zero
vertical speed. The same ECS controller then handles flight and landing.
No additional landing timer, engine format, template change or editor field is
needed. Intermediate drops retain their prior provisional path.

## Recordings and provenance

`tools/record_airborne.py --prepare-checkpoints` now also creates
`.psx/forward_ledge_start/state.bin`: from `.psx/home_hang_start/state.bin`,
Circle[0,2), neutral through field90. This checkpoint is grounded and idle at
`[19968,2048,88165]`, yaw0, room44, clip103/frame1622. There are no pokes.
The preceding roll is checkpoint preparation, not verified by these tests:
the original roll diagnostic also exposes existing roll frame/yaw mismatches.
Its raw `.psx/fall_forward_probe` and `.psx/airborne_forward_ledge_fall`
recordings remain available separately.

Record each route with `python3 tools/record_airborne.py --case NAME`. JSON
sidecars retain the exact command and input, checkpoint/sample/heading-write
hashes, toolkit build, disc/BIOS identities and timing model. The supplied native
core uses flat instruction costs; these game trajectories advance once per
two NTSC fields.

| Case | Input and observed result |
|---|---|
| `forward_ledge_run` | Up[0,90); departure48, clip35 at58, landing into92 at80, braking8 at102 and standing134 |
| `forward_ledge_delay_heldout` | Up[14,146); independently recorded after implementation; departure62, running landing94, then wall-hit54 at142 and standing172 |
| `forward_ledge_release_validation_heldout` | Up[14,68), Square[74,84), Up[146,160); independent release and ignored airborne jump; landing82 at94, standing130, later run-start and braking10 |

All348 sampled logic ticks across these three complete recordings pass all
thirteen compared columns. Only y uses the existing0.02-source-unit float
allowance; the recorded cardinal routes are exact even there. No controller
rule was fitted to either independent validation route. The observation harness
now reports the existing `HitWall` mode as state12, because wall-hit clips53/54
declare state2 while their controller overrides it until the natural end link.

Two additional post-implementation recordings were retained as diagnostics at
this milestone. The subsequent [walking-stop milestone](walk-stop-evidence.md)
resolves both using these unchanged recordings as explicitly labelled calibration:

- `forward_ledge_release_heldout` matches the fall, standing landing and
  restart, but later braking into the wall at field196 uses clip11 with a
  one-tick state12 override that the port did not preserve.
- `forward_ledge_walk_heldout` exposes Walk requesting goal52 while airborne,
  and a walk-release dispatch mismatch after landing. Neither diagnostic was
  used to change the flight controller in the ledge-fall milestone; the fresh
  release validation avoided the later wall. Both full trajectories now pass
  with the walking-stop changes and their separate independent validation.

## Verification

`game/tests/airborne.rs` covers the three complete passing routes. The workspace
suite passes211 tests, with six opt-in/documentation tests ignored. Clippy and
all-target builds pass with the existing unrelated warnings. Changed Rust files
and `git diff --check` pass formatting checks. Workspace-wide rustfmt still
reports the same23 unchanged files documented by the preceding milestones.

`tools/airborne_smoke.py` adds41 full-game pose checks for these routes, including
departure, falling, both landing clips, running continuation, stop and restart.
The full sweep passes201 poses across22 routes, including all prior airborne
and run-stop cases.
Each checks position and authored clip/frame and retains paired images and MCP
transcripts under `validation/airborne-smoke/`. The original displayed images
use the established four-field delay relative to sampled RAM. Inspected landing
pairs match Lara's pose; the existing HUD and scene-rendering differences remain.

The engine sandbox passes30 ticks of movement and a rendering check. The editor
passes sandbox and Home byte-exact untouched saves, undo/redo, save/reopen, and
Home placement8 preview of clip92 through its10 authored frames. Verification
logs and binary listings are under `validation/ledge-fall/`.
