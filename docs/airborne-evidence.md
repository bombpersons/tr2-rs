# Airborne wall rebound and heavy landing

Evidence comes only from the supplied Tomb Raider II USA Rev 3 disc through
`psx`. Five ordinary pad recordings use no memory pokes. This milestone covers
a forward jump hitting an ordinary room wall more than512 units above the
floor, the resulting fast fall, its heavy landing, and the contrasting reach
against that wall. The subsequent [low/middle-impact milestone](low-airborne-evidence.md)
extends the same controller. Ceilings, corner deflection, static meshes,
unassisted fast falls and slide landings remain unverified.

## Binary evidence

The existing PSX RAM analysis is reused, with RAM SHA256
`745062c01811c58f18071e7eda7fdd43ca7412fea513d219f4950ae417d177b0` and analysis
key `81851f7f0f07d25e404efd4e5af7c7665abdb2a13a7af60fbb2b6db01a7e54e5`.
The following addresses can be reinspected with `psx disasm --ram
.psx/home_side --addr ADDR --count N --text`. The deflection handler contains
an indirect jump that Ghidra does not fully resolve; these claims use the
instructions, its jump table at `0x80010994`, and runtime recordings.

- Forward-jump collision `0x8003AAE4` refreshes movement heading after
  animation motion: facing yaw for positive speed, yaw plus a half turn for
  negative speed. It calls deflection `0x8003C5F4`. Fast-fall collision
  `0x8003AEEC` retains the preceding heading instead. Collision sees the yaw
  after this tick's turn, while animation movement uses the heading retained
  from the preceding collision. The turning-jump recording checks this
  distinction; the Bevy schedule commits facing after integration.
- Frontal deflection `0x8003C66C..0x8003C71C` applies the collision shift.
  With centre floor more than512 below the feet it enters clip32 at absolute
  frame481 (relative1), state/goal9. It quarters signed horizontal speed
  with truncation toward zero, reverses movement heading, and clamps an
  ascending/stationary fall speed to1. The source also has a low-impact
  branch below129 that forces state14; it is independently verified and
  ported in the subsequent low-impact milestone, not inferred from this
  higher-wall recording.
- Fast-fall control `0x80038F6C` multiplies signed speed by95/100, truncating
  toward zero. AnimateLara `0x800431C8..0x80043220` then adds the difference
  between consecutive integer authored speeds of the destination clip.
  This also applies on a clip link. In these recordings speed12 becomes
  -4 on the final frame of32, then5 on entry to33, briefly moving back
  toward the wall before the rebound continues. A vector magnitude would
  lose that reversal; the test samples signed speed during fast fall.
- Fast-fall collision `0x8003AF64..0x8003AFB0` forces heavy landing24 on
  contact, clears gravity/fall speed, snaps to the floor and preserves
  horizontal speed on that tick. The next standing control pass clears it.
  Clip24's40 frames and end link own recovery; a fixed landing timer cannot
  reproduce the input lock. Existing engine animation data already carries
  these frames and links.
- Reach collision `0x8003B18C` calls grab first, then the separate deflection
  routine `0x8003C7F4`. Its frontal contact retains speed and animation.
  At this tall wall the hands cannot catch its top: clip95 stays at frame1492
  while Lara falls in place, then requests goal2 and dispatches to82 on the
  following tick. Releasing Action during the descent does not cancel reach.

The game keeps y-up Bevy positions, velocity in metres/second and an ECS
`PlayerMode::FastFalling`. Collision heading lives in the existing per-player
`Motion` component; authored animation remains an engine asset. No engine
format, editor field or template changes are needed. Other inherited fall,
slide and exceptional collision branches retain their prior provenance.

## Retained routes

`python3 tools/record_airborne.py --prepare-checkpoints` creates
`.psx/airborne_wall_start` from `.psx/backward_rolled_start/state.bin` using
Up+R1[0,160), neutral through field240. The final settled pose is
`[19968,2560,95984]`, yaw0, room16, clip103/frame1622. The earlier checkpoint
recipe is retained in `tools/record_backward_collision.py`.

Record with `python3 tools/record_airborne.py --case NAME`. Each CSV's JSON
sidecar retains the exact command, inputs, checkpoint/sample/write hashes,
toolkit build, disc/BIOS identity and timing model. The toolkit uses native
flat instruction costs rather than cycle-accurate PS1 timing; game logic
advances once per two NTSC video fields.

| Case | Observation |
|---|---|
| `forward_wall` | Up+Square[0,22); contact at field58/z97179, clip32/frame481, speed12; clip24 begins at88 and finishes at168 |
| `reach_wall` | Up+Square+Action[0,22), Action[22,110); wall contact58 retains speed50; lands88, enters82 at90, finishes126 |
| `forward_steer` | Right[30,48) during flight; yaw1360, contact60, x/z rebound rounding and heavy landing at88 |
| `forward_delay_heldout` | Recorded after implementation: delayed takeoff14..36, another Square100..120 during heavy recovery; the extra jump is ignored |
| `reach_release_heldout` | Recorded after implementation: release Action64, request backward walk96..170; landing finishes before backward walk starts128 |

The first five routes in `game/tests/airborne.rs` compare515 sampled logic
ticks and thirteen columns, including two independent validation routes. All
columns are exact except the existing0.02-source-unit y float allowance.
The initial validation harness treated backward-entry speed as signed against
the post-collision heading; its observation mapping was corrected to match
the existing backward tests. No controller rule was fitted to either
validation recording.

`tools/airborne_smoke.py` drives the full game with the retained inputs and
PSX camera poses, checks position and authored animation, and saves paired
screenshots plus the MCP transcript under `validation/airborne-smoke/`.
As in the preceding smoke scripts, original displayed images trail sampled
RAM poses by approximately four video fields; comparisons use that offset.

## Verification

At the high-rebound checkpoint, `cargo test --workspace --all-features` passed 180 tests;
six opt-in/documentation tests remain ignored. Clippy completes with existing
warnings outside the changed files. Changed Rust files pass rustfmt; the
workspace-wide formatting check still reports 23 pre-existing unformatted
files. The engine sandbox builds and runs through MCP, moves its Walker for
30 ticks and produces a nonblank screenshot.

All 39 full-game airborne pose checks pass, and the paired screenshots were
inspected. The existing visual differences in sky, HUD and shading are outside
this movement milestone. The full-resolution editor smoke passes on both the
sandbox and a disposable Home level copy. Home's Lara placement8 previews
clip24 for 40 authored frames into standing clip11; playback leaves the
document clean and untouched saves remain byte-exact. No original assets
were edited by these checks.
