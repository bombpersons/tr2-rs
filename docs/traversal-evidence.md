# Jumps, ledges and landing recovery

PSX-only evidence from the supplied Tomb Raider II USA Rev 3 disc. All routes
use ordinary pad input, without memory pokes. No external reimplementation
was consulted. Structural animation data comes from the existing importer;
the controller rules below were independently checked against the executable
and new native runs.

## Reproduction and retained evidence

`tools/record_traversal.py --prepare-checkpoints` builds the Home approach
checkpoints from the existing `home_start` and `home_side` states documented
in [original-evidence.md](original-evidence.md). It retains the fountain and
assault-course approach separately from the measured traversal. These states
and the disc remain local under `.psx/` and `rom/`.

For each case, run `python3 tools/record_traversal.py --case NAME`. The sixteen
`game/tests/data/original/traversal_*.{csv,json}` pairs retain the exact replay,
sampling addresses, checkpoint/sample hashes, BIOS/disc identity and psx build.
Screens are recorded every two fields for visual comparison.

The measured routes start standing. They cover:

- Three-click climb, release during the clip, and attempted turning before
  its authored end.
- Vertical ledge jump, grabbing, a long hang, pull-up and dropping.
- Early pull-up, release while the grab pose is still playing, and releasing
  Action before takeoff so the ledge is refused.
- Forward jump, reach, catch, hang, pull-up and early drop after a reach catch.
- Standing and running jumps, neutral landings, landing into a run, and a
  second jump through an authored landing-cancel range.

Cases ending `_heldout` were recorded after the relevant implementation.
There is one explicit exception to pure validation: `running_jump_heldout`
exposed the alternate takeoff clip and was used to refine dispatch. Its JSON
has `used_to_refine_rules: true`. `running_jump_validation_heldout` was then
recorded after that correction and passed without fitting. Other held-out
routes also remain independent validation.

## Binary rules and runtime checks

| Rule | Executable evidence | Observed behavior |
|---|---|---|
| Running jump enable | Run handler `0x800387B0`, latch writes `0x800388D4..0x80038944` | Run-start clip6 clears the latch; run clip0 frame4 sets it. A held jump waits for this boundary. Authored ranges choose clip16 or18 according to the running foot. |
| Jump compression | `0x800390C8`, height-ahead query `0x8003DF60` | Up can request forward jump when the floor256 ahead, sampled762 above the feet, is no more than384 above the feet. The request survives direction release. Clip73 dispatches to76 or chains to91 for a vertical jump. |
| Three-click vault | `0x8003D924` | Front floor[-896,-641] after the overlapping two-click test, even front corners and clearance choose42. Collision temporarily sets state19 although the clip table names state2. |
| Higher ledge jump | `0x8003DBB0` branch of the vault test | Ordinary flat ledges[-1920,-897] request the authored vertical preparation26. Launch magnitude is `floor(sqrt(-12*(front+800)))+3`; a1024-unit ledge gives54. Gravity6 makes the first displayed fall speed-48. |
| Edge crossing | `0x8003D2AC`, interpolated bounds `0x80028904` | Compare the front floor to the current top bound and fall speed on either direction of crossing. The old fixed height-tolerance grab has been removed. |
| Vertical catch | `0x8003D3C0`, collision `0x8003B7B8` | Enter96 at relative frame12, snap yaw, align the feet to the new top bound and clear movement. The recorded ledge1024 gives feet1748, settling to1750 with the hang bounds. |
| Reach catch | `0x8003D5BC`, reach collision `0x8003B18C` | Ordinary catch enters96 at0, speed2/fall1. The first following hang pass clears fall but retains speed2 and restores the prior position; the next animation pass writes speed0. |
| Hang and release | `0x8003CDE4` | Hang probes nudge two units into the wall before restoring contact. Release enters28 at+9, lowers by that frame's bottom bound and seeds speed2/fall1; gravity starts next tick. |
| Pull-up request | `0x8003AFCC` | Up requests goal19 after the animation pass when the front floor is[-849,-651] and the three probes have clearance. The next authored pass accepts it. |
| Pull-up offsets | AnimateLara `0x80042E08`, clips97/102 commands | Clip97 raises735 at its end;102 then moves200 forward. Standing collision settles the feet only after the latter command. The previous direct teleport to the ledge was incorrect. |
| Landing passes | Forward jump `0x8003AAE4`, vertical jump `0x8003B7B8` | Forward-jump collision runs animation again on the landing tick. Vertical-jump collision sets a goal for the next tick. Up without Walk selects running goal1 for forward-jump landing; neutral selects2. |
| Landing recovery | State2 `0x8003899C`, clip31/82 ranges | Input requests a goal while the landing pose continues. Authored ranges govern resuming and jump cancellation; these paths no longer use a fixed0.15-second lock. |
| Goal persistence | AnimateLara `0x80042E08..0x80042FD8` | Lara retains her requested goal across a natural clip/state link. The engine's object animation step has different goal-reset semantics, so this controller preserves its own goal. |

The front-facing grab cone is +/-6370 binary-angle units in both catch
functions; vault's cone is narrower at5460. Grab rejects an uneven pair of
front corners (difference60 or more), insufficient overhead clearance, drawn
weapons or missing Action. Angular and obstruction boundary sweeps have not
yet been recorded; only the ordinary sector paths are claimed here.

Negative-Z contact exposed a pre-existing mirror error. The margin is101
units on that side too: z92261 at the z92160 edge. A collision can push back
past the previous coordinate: the forward-jump route reaches z88164 before
collision restores88165. Clamping that correction to the attempted segment
was incorrect. The existing Wall positive-X paths still pass.

`.psx/reach_catch_writers/writes.jsonl` independently records the transient
catch state: at run field81, `0x8003D77C` sets y1765,
`0x8003D7C4` sets speed2 and `0x8003D7D4` sets fall1. At field83,
animation moves y to1772, `0x8003CE2C` clears fall, and `0x8003D1CC`
restores y1765. At field85, `0x800431C4` writes speed0 and hang alignment
`0x8003D288` sets y1776. The run begins at `home_hang_run_start`, with the
same forward-jump input as `forward_climb` through field88, watches on
`8017c8b4:4`, `8017c89a:4`, and `8017c8c4:4`.

## Native implementation and verification

`game/src/player/traversal.rs` handles jump, hang, pull-up and recovery rules
over ordinary `Player`, `Motion`, `AnimationState` and `Traversal` components.
It uses the engine's existing authored animation data, end commands and
interpolated bounds. Its animation pass runs before its collision and marks
that tick as already animated. Existing locomotion retains its established
path. No engine/editor format or template file changed.

`game/tests/traversal.rs` compares every second field against thirteen
columns: position, yaw, clip/frame, state, room, speed, fall speed, health,
turn rate and goal. All columns are exact except y's0.02-original-unit float
allowance. Existing trajectory tests remain part of the workspace check.

`tools/traversal_smoke.py` drives the real Bevy binary through MCP and checks
numeric state before capturing aligned views. It uses recorded camera poses;
the PSX image follows sampled state by four fields, matching the earlier
locomotion smoke convention. This checks traversal poses, not camera parity.
The visual differences in Home's sky, portal visibility and HUD remain.

## Remaining scope

The ordinary vertical/forward paths and three-click clip timing are covered.
Climbable-wall variants, shimmy/corner traversal, static obstruction/contact
flags, the alternate reach-catch150 path, angular rejection boundaries,
backward/sideways jump collision and ceiling-impact behavior remain unverified
or incomplete. Fast-fall animation/friction and damage-threshold runtime
coverage also remain; landing damage retains the binary rule at `0x8003E044`.
Unassisted falls and slide landings still use the existing
controller. Water entry hands off to its existing provisional controller.
Traversal sound/effect commands are not newly implemented here.

These limits are tracked separately in [TODO.md](../TODO.md); this is not a
claim that every Lara collision branch or the whole game has reached parity.

Validation: `cargo test --workspace --all-features` passed, as did the sixteen
traversal tests and the real game MCP smoke (nine routes,52 poses). The
engine-only sandbox was built, run, stepped and captured. The editor's sandbox
smoke passed save/reopen, byte-exact untouched saving and authored animation
preview; its screenshots were inspected. The aligned traversal comparisons
were also inspected. Clippy completes with existing warnings outside these
changes. Touched Rust files pass rustfmt; the whole-workspace format check
still reports pre-existing differences (`validation/traversal-fmt-check.txt`).
