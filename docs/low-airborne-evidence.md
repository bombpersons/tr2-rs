# Low and middle wall impacts

This extends [high wall rebound evidence](airborne-evidence.md) using only the
supplied Tomb Raider II USA Rev 3 disc and `psx`. Six new ordinary pad routes
compare 676 logic samples across thirteen columns. A seventh diagnostic route
records an outstanding grounded run-start issue and is explicitly excluded
from that passing comparison count. No route uses memory pokes.

## Rules and binary evidence

Use `psx disasm --ram .psx/home_side --addr ADDR --count N --text` to reinspect
these addresses. The RAM image, analysis key and hashes are the same ones
listed in the preceding airborne evidence notes.

- Frontal jump deflection `0x8003C66C..0x8003C71C` has three height branches.
  A centre floor more than512 units below the feet selects fast fall, as
  already ported. At129..512 it preserves the jump clip and goal. At128 or
  less it forces clip31/frame460 and state/goal14, overriding the clip's
  declared state2. All three branches quarter signed speed, reverse movement
  heading and clamp an ascending/stationary fall speed to1. Thresholds are
  evaluated in original integral units, avoiding float noise at a boundary.
- The middle-height recording hits at field30 with clearance340: speed50
  becomes12, fall speed-76 becomes1. The next tick moves backward12, then
  forward collision restores the facing heading. Further contacts at36 and42
  reduce speed to3 then0. It lands through the ordinary forward-jump second
  animation pass into82 at50. It never enters the heavy landing family.
- The low-impact running jump launches from the authored other-foot takeoff
  clip18. At field60 it contacts the wall94 units above the floor: speed75
  becomes18, fall speed-94 becomes1, clip31 starts at460 with state/goal14.
- State14's control handler `0x8003909C` does not process ordinary movement
  requests. Its collision handler `0x8003B2FC` delegates to standing collision
  `0x8003AA20`. On the following tick AnimateLara still applies airborne
  motion using the rebound heading, moving back18. Collision settles the
  feet and clears gravity/fall speed. Horizontal speed18 survives that tick;
  the next grounded animation pass writes0 from clip31.
- AnimateLara updates current state on an accepted dispatch
  (`0x80042E84..0x80042E8C`) or natural end link (`0x80042FAC`). It does not
  overwrite current state from the clip every tick. State14 therefore lasts
  through clip31 even though the asset names state2. Its normal jump-cancel
  range at463..465 cannot take effect because the impact handler keeps its
  goal. Clip11 at field100 restores state2; goal14 remains for that tick.
- A sufficiently close wall prevents a forward jump before takeoff.
  Compression's height-ahead check `0x800390C8`/`0x8003DF60` leaves goal15;
  authored clip73 finishes into91 and the vertical loop28. The new blocked
  takeoff recording verifies this existing branch independently.

These remain game rules over the existing engine geometry and animation
assets. `PlayerMode::JumpImpact` expresses the temporary control lock and
state override; it does not change the authored clip's state. The reusable
engine, asset format, template and editor need no new fields.

## Turning correction exposed by validation

`low_cancel_heldout` was initially recorded after implementation. Movement,
animation and state all matched, but its turn after recovery exposed an
existing goal discrepancy: the original requested goal20 while still playing
slow-turn clip12. This route was used to refine that rule and is labelled
`used_to_refine_rules: true`; it is calibration, not independent validation.

Slow-turn handlers `0x80038DD0` and `0x80038EA0` request goal20 once turn rate
exceeds728 before the shared364 decay. The port now requests that goal above
364 after decay and waits for the asset's dispatch range, replacing its
33-tick substitute. Authored ranges select44 for a right turn and69 for a
left turn. They also preserve a pending request when its window has not yet
opened. The original yaw and animation trajectories continue to pass.

`middle_delay_heldout` and `low_cancel_validation_heldout` pass independently
without fitting. The latter was recorded after the turning correction and
checks a new early cancel request, left turn, entry into69 and release.

## Reproduction and retained routes

`tools/record_airborne.py --prepare-checkpoints` retains all preparation
recipes. The new checkpoints start from `.psx/airborne_wall_start/state.bin`:

| Checkpoint | Pad route | Settled pose |
|---|---|---|
| `airborne_mid_wall_start` | Up+R1[0,100), neutral through180 | `[19968,2560,96731]`, yaw0, room16, clip103/frame1617 |
| `airborne_blocked_wall_start` | Up[0,40), neutral through140 | `[19968,2560,97124]`, yaw0, room16, clip103/frame1640 |

All intervals below are relative video fields. Record with `python3
tools/record_airborne.py --case NAME`. JSON sidecars retain commands, input,
toolkit build, disc/BIOS identity, timing model and checkpoint/sample/write
hashes. Game logic advances once per two NTSC video fields; native toolkit
timing uses flat instruction costs, not cycle-accurate PS1 timings.

| Route | Input and purpose |
|---|---|
| `middle_wall` | Middle checkpoint, Up+Square[0,22); repeated small rebounds and ordinary landing |
| `running_low_wall` | Original airborne checkpoint, Up[0,40), Up+Square[40,70); low impact and locked recovery |
| `blocked_takeoff` | Blocked checkpoint, Up+Square[0,22); compression falls back to vertical takeoff |
| `middle_delay_heldout` | Middle checkpoint, Up+Square[10,32), Square[40,48); independent delayed impact and ignored airborne jump |
| `low_cancel_heldout` | Airborne checkpoint, Up[0,40), Up+Square[40,62), Square[66,72), Right[80,150); retained calibration for the early fast-turn goal |
| `low_cancel_validation_heldout` | Airborne checkpoint, Up[0,40), Up+Square[40,62), Square[64,74), Left[86,210); fresh independent cancel, recovery and left fast-turn validation |

`game/tests/airborne.rs` now compares 1191 sampled logic ticks across eleven
passing routes, including the preceding five. State is sampled from the
controller override during impact, otherwise from the authored clip. The
existing 0.02-source-unit y conversion allowance remains; all other compared
values are exact. No tolerance was widened.

The original `run_start_retry_probe` diagnostic starts at the middle checkpoint
with Up[0,12), Up+Square[12,30). It hits the wall during run-start before a running jump is
ready. The original stops into clip11 and retries the held jump into
compression; the existing port's grounded run-start stop/held-jump branch
diverged at this milestone. The subsequent
[run-start correction](run-start-evidence.md) closes that mismatch and promotes
the unchanged recording to passing calibration, with two fresh independent
validation routes. Its sidecar now records that classification.

Remaining airborne work includes corner and ceiling deflection, static
obstructions, unassisted fast falls, slide landings and backward/sideways
jump collisions. Exact height-boundary sweeps are binary-backed only; the
new runtime routes exercise clearance94 and340, alongside the earlier high
impact.

## Verification

`cargo test --workspace --all-features` passes 186 tests, with six existing
opt-in/documentation tests ignored. Clippy completes with existing warnings
outside the changed files. Changed Rust files pass rustfmt; unrelated workspace
formatting differences remain. All 80 full-game airborne MCP pose checks pass
across eleven routes. The new paired PSX/Bevy screenshots were inspected,
including repeated middle contacts, the low-impact settle, blocked takeoff
and left fast turn. Existing sky/HUD/shading differences remain outside this
movement milestone.

The engine sandbox builds and runs, moves its Walker for 30 ticks and renders
a nonblank frame. Editor smoke passes on sandbox and disposable Home copies,
including untouched byte-exact saves, undo/redo, reopen and full-resolution
rendering. Home's Lara placement 8 previews clip31 for 20 frames into 11 without
dirtying the document. The editor correctly displays the asset's declared
state2; the temporary state14 override belongs to the game's controller.
