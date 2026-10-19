# Forward walking against room walls, ledges and steps

Evidence is the supplied Tomb Raider II USA Rev 3 disc through `psx` only.
The milestone covers ordinary cardinal flat-room walking: a blocked start,
ledge refusal, collision foot-stop, both ascending step feet, both descending
step feet, neutral release after/during a step and an ignored jump request
while the collision stop finishes. Angular/corner boundaries, static/dynamic
items, walking vault entry, unusual ceiling contacts, steep slopes and
walk/run transitions remain separate work.

## Binary rules and ECS implementation

`psx disasm --ram .psx/home_start/ram.bin --addr 0x8003A62C --count 132 --text`
retains walk collision in `validation/forward-walk/walk-collision.txt`.

- Walk collision permits +/-384-unit front-floor changes, zero front-ceiling
  margin and sets slope/deadly-floor probe options7 at0x8003A668..0x8003A684.
  It uses the shared front/left/right room probes and grid correction. The
  existing backward implementation already verifies these common helpers;
  forward walking now reuses them, retaining animation motion before collision.
- Shared deflection0x8003C544 sets current state and goal2 and speed0 for a
  front contact before the walk handler chooses a stop. Absolute frames29..47
  select clip3;22..28 or48..57 select2; outside these ranges the shared stop
  routine restores/advances the preceding standing pose or enters11
  (0x8003A6AC..0x8003A72C). This is why a held walk at the wall never visibly
  enters walk-start20, while a moving walk at the ledge uses clip3/state2.
- A downward centre-floor change of at least129 selects59 at absolute walk
  frames28..45,60 otherwise (0x8003A740..0x8003A79C). Upward changes129..384
  select58 at frames27..44,57 otherwise (0x8003A7A0..0x8003A800). The tick
  retains the movement speed used before entering the step clip. Following
  ticks use that clip's authored speed, and its natural link returns to walk.
- The feet snap to the sampled centre floor after the clip selection
  (0x8003A818..0x8003A828), preserving the collision tick's order.

The game adds a forward-walk collision module, reusing the existing room
probe/grid helpers. `WalkStopping` is an explicit ECS mode for the standing
control override through collision-selected clips2/3. It holds no guessed timer:
its authored end link owns completion. These clips retain walking state0 when
used for an ordinary released-input stop; the new mode distinguishes that
from a collision stop's standing state2. Existing walking trajectories pass.
Walking Action/vault entry continues through the existing provisional path
until independently recorded. No engine format, editor or template change.

## Recordings

All routes use ordinary pad input with no memory pokes. JSON sidecars retain
precise commands, toolkit build, disc/BIOS identity, input, checkpoint hash,
complete sample/write hashes and preparation recipes. The first four routes
are labelled calibration (`used_to_refine_rules: true`). All five independent
routes were recorded after implementation and passed without changing its rules.

| Case | Observation |
|---|---|
| `walk_wall` | Held Up+R1 at the cardinal wall for100 fields; remains standing103 with speed0 |
| `walk_ledge` | Walks to z88987, selects clip3/state2 at138; standing control asks0 at140, authored end enters11 at170, refused retry enters103 at172 |
| `walk_step_up` | Two one-click ascents, clip57 at96 and250, with speed15 retained on contact; authored speed/link and later neutral stop pass |
| `walk_step_down` | One-click descent selects59 at70; returns to walk at96, then stops normally after release |
| `walk_step_up_other_heldout` | Independent shifted starting position: clip58 at124, walk156, release192, authored stop2 at196 |
| `walk_step_down_release_heldout` | Independent delayed start: clip59 at86, walk112, release114 and stop3 at122 |
| `walk_step_down_mid_release_heldout` | Independent: release at98 during59; goal2 at100 cannot interrupt the step; natural link and authored walking stop complete |
| `walk_step_down_other_heldout` | Independent: roll-prepared upper platform, descends via60 at52 and192, returns through authored links into walk |
| `walk_ledge_jump_request_heldout` | Independent delayed ledge stop at156; Square during the committed pose requests15 at178, but releases before its end. Retry Up+R1 at224 is refused without moving |

The other descending-foot fixture is prepared from
`.psx/backward_step_down_start/state.bin` with Circle[0,2),90 fields. It settles
at [19968,2048,94023], facing0. The port starts at that sampled settled state;
this milestone makes no claim about ported roll behavior. Reproduce with
`python3 tools/record_airborne.py --prepare-walk-step`, then `--case NAME`.

Nine complete recordings compare1309 sampled logic ticks across thirteen
columns. All are exact except the existing0.02-source-unit y conversion
allowance. Current-state comparisons explicitly preserve the standing override
rather than treating the stop clip's declared state as authoritative.

## Verification

All238 workspace tests pass, including every earlier walking, airborne,
traversal and backward trajectory. All-target builds and Clippy pass with
existing unrelated warnings. Changed Rust files and `git diff --check` pass.

The full-game MCP smoke checks100 poses across all nine routes. Paired PSX
images use the established four-field display offset. Position, authored clip
and frame are checked; images covering both ascending/descending feet, ledge
stop, recovery and refused walking start were inspected. Reports and exact
MCP operations are retained in `validation/forward-walk/airborne/`. HUD, NPC,
lighting and raster differences remain outside the pose claim.

Engine sandbox movement/rendering passes30 ticks. Editor sandbox and Home
checks preserve untouched bytes and exercise undo/redo and save/reopen; Home
previews descending step60. Logs are in `validation/forward-walk/`.
