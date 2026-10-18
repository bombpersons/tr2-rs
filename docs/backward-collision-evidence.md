# Backward collision and hop-falls

New evidence uses only the supplied Tomb Raider II USA Rev 3 disc and `psx`.
All fourteen retained recordings use ordinary pad input, without memory pokes.
They extend [clear-ground backward motion](backward-evidence.md); unrelated
inherited controller branches retain their previous provenance.

## Rules established from the binary

The collision dispatch table at `0x80082DE0` selects backward-hop collision
`0x8003AC48`, backward-walk collision `0x8003B414` and backward-fall collision
`0x8003B9A8`. These are game rules over the engine's existing sectors and
animation tables; no engine format, template or editor field changed.

- Both ground handlers clear gravity and fall speed, write yaw plus a half
  turn to movement heading `0x8008C572`, and request radius 100 probes with
  a maximum upward step384. Walk-back limits the downward probes to 384;
  hop-back allows any downward floor. This differs from the old shared
  prototype's symmetric256 step limit.
- GetCollisionInfo `0x8001B018` probes the front and both corners in the
  facing quadrant. Its transverse front offset uses the sine/cosine table.
  With flags 3, a steep uphill probe becomes an impassable height; a steep
  downhill probe becomes512. The relevant slope rewriting starts around
  `0x8001B2E0`. The new recordings exercise uphill rejection and ordinary
  tilt 1 descent; downhill steep entry is still unverified.
- A frontal collision applies the grid shift, forces standing and clears
  speed (`0x8003C544`). `0x8001B9EC` computes the exact shift: no correction
  within one sector, otherwise push back across the edge with a one-unit
  margin. With radius 100, ordinary wall/ledge contacts therefore stop101
  units from the edge on either side.
- LaraColStop `0x8003A56C` normally chooses standing clip 11. If the preceding
  state was standing or turning, it restores that earlier pose and advances
  it with a standing/turn goal. This allows an idle pose to continue during
  repeated blocked walk requests instead of restarting clip 11 every tick.
- Collision retains its backward movement heading after forcing standing.
  Thus another backward request uses that heading on its first tick. This
  is visible at the Wall ramp: z25701 ->25699 ->25701, alternating walk
  entry and forced stop while Down+R1 remains held. A first request from
  ordinary standing still uses the forward heading, as before.
- Walk-back snaps to the centre floor after horizontal collision. A downward
  step129..383 selects clip 62 if the absolute animation frame is964..993,
  otherwise clip 61 (`0x8003B4D8..0x8003B538`). The retained Home descending
  step exercises61; the62 selection remains binary-backed only.
- Hop-back switches to clip 93/state 29 when the centre floor is more than200
  below the feet (`0x8003ACBC..0x8003AD08`). It retains horizontal speed,
  starts with fall speed0 and applies its first gravity6 on the next tick.
- Hop-fall lands when the centre floor is reached while descending, requests
  goal 2, clears gravity/fall speed and settles the feet (`0x8003B9A8`). It
  does not advance animation again on that collision tick. The following
  authored step enters landing clip 82. Its ranges/end links determine when
  input can take effect; the port reuses the traversal recovery controller.
- Landing damage remains the shared binary-backed rule at `0x8003E044`:
  no damage through 140, quadratic through 154, lethal above 154. The new
  ordinary hop-fall lands at 96 and takes no damage. Runtime threshold
  sweeps are still pending.

The source stores positions in original integer units. The port keeps Bevy
coordinates (y up,1024 source units per metre), ECS player/motion components
and authored animation assets. `Motion::movement_heading` retains collision
heading per actor. It is refreshed by ordinary standing/forward collision;
its backward value survives an immediate forced stop. The original turning
hop's declared one-unit float-sine bound remains in its existing test; it
was not widened by this work.

Reinspect the source using `psx disasm --ram .psx/home_side --addr ADDR
--count N --text`. The existing psx Ghidra analysis of `.psx/wall_game/ram.bin`
is also reusable: analysis key
`81851f7f0f07d25e404efd4e5af7c7665abdb2a13a7af60fbb2b6db01a7e54e5`,
RAM SHA256 `745062c01811c58f18071e7eda7fdd43ca7412fea513d219f4950ae417d177b0`.

## Recorded routes

`tools/record_backward_collision.py` retains commands, input schedules,
checkpoint/sample/write hashes, toolkit build, disc/BIOS identity and timing
model in each JSON sidecar. These runs use toolkit build
`src-6406590b7cd127b1-release`, native flat instruction timing rather than
cycle-accurate PS1 timing. Logic runs once per two NTSC video fields.

Prepare checkpoints with `python3 tools/record_traversal.py
--prepare-checkpoints` if the earlier traversal states are absent, then
`python3 tools/record_backward_collision.py --prepare-checkpoints`. The new
preparations are deterministic pad routes from the existing Home/Wall starts:

| Checkpoint | Source and relative-field input |
|---|---|
| `backward_rolled_start` | Home start, Circle[0,2), neutral through 90 |
| `backward_step_down_start` | Home start, Up+R1[0,160), neutral through 240 |
| `backward_angled_start` | Home start, Left[0,12), neutral through 100 |
| `backward_shallow_start` | Wall start, Right[0,56), Up+Cross[60,240) and[260,300), neutral through 340 |

The shallow preparation's final position is `[65435,4633,26010]`, yaw16384,
room35, clip 103/frame1669, matching the preceding Wall hold/release/retry
recording. The backward comparison starts at the resulting settled checkpoint.

| Case | Observation |
|---|---|
| `wall_walk` | positive-Z wall contact at97179, clip 11 then idle while input remains held |
| `wall_hop` | second hop hits the same wall; repeated hop requests are stopped immediately |
| `angled_wall` | oblique approach, transverse coordinate restored at the wall; no corner sweep claim |
| `ledge_walk` | stops at88987 before the768-unit drop at89088 |
| `ledge_hop` | first hop ends88997; second falls, keeps speed27, lands and recovers |
| `step_up` | walks backward up256, then refuses the768-unit drop at93285 |
| `step_down` | descends256 through clip 61 at field102, returns to 40, then authored stop38 |
| `shallow_slope` | tilt 1 floor heights and release into stop39, frame by frame |
| `slope_walk` | rejects the steep ramp and reproduces the repeated two-unit entry/stop motion |
| `slope_hop` | rejects the steep ramp and repeated hop requests |
| `ledge_release_heldout` | releases Down immediately after the second hop starts; fall and recovery finish |
| `wall_delay_heldout` | independent delayed walk input, wall contact and continued idle |
| `step_release_heldout` | independent delayed start and earlier release after the descending step |
| `slope_release_heldout` | releases Down at field4; the committed hop still reaches and stops at the ramp |

All four `heldout` routes were recorded after the rules were written and pass
without using them to fit the implementation. Across fourteen recordings,
`game/tests/backward_collision.rs` compares 1870 logic ticks and thirteen
columns: x/y/z, yaw, clip/absolute frame, state/goal, room, speed/fall speed,
health and turn rate. All columns are exact, apart from a 0.02 source-unit
allowance for floating-point y conversion. The retained recordings also
sample the original camera for pose comparisons.

## Reproduction and limits

```sh
python3 tools/record_backward_collision.py --case wall_walk
# Repeat --case for the other entries listed by --help.
cargo test --workspace --all-features
cargo build -p tomb-raider-ii -p editor --all-features
python3 tools/backward_collision_smoke.py
python3 tools/editor_smoke.py
```

The smoke replays the standalone Bevy game over MCP, checks position, clip
and frame at captured checkpoints, and renders using sampled original camera
positions. Reference screenshots use field N+4 for sampled field N, preserving
the previously established display delay. This is a pose/collision check,
not a claim that camera behavior, room visibility, lighting or the HUD match.

Verification passed: all53 full-game pose checkpoints over eleven routes,
`cargo test --workspace --all-features`, Clippy, the engine-only sandbox
build/run, and the editor's byte-exact save/edit/undo/reopen/animation smoke.
The Home/Wall pose montages, sandbox and editor screenshots were inspected.
Known sky, lighting, shadow and HUD differences remain visible. Reports and
images are retained under `validation/backward-collision-smoke/`,
`validation/backward-collision-sandbox/` and `validation/editor-smoke/`.
Changed Rust files pass rustfmt; the workspace-wide format check still reports
pre-existing differences outside these files. Clippy's existing unrelated
warnings remain, with no warnings in the changed Rust.

Remaining collision work: static meshes and moving-object pushes; a systematic
corner/angle boundary sweep; low ceilings and crush restoration; wading;
steep downhill entry and backward slide transitions; airborne wall deflection
and fast falls. The new hop-fall path currently covers unobstructed descent
and ordinary landing, not the wall-deflection branches of `0x8003C5F4`.
Those branches need dedicated runtime recordings before extending this rule.
Unassisted/slide landings and backward/sideways jumps are separate pending
traversal milestones. Water entry continues to use the provisional controller.
