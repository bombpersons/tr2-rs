# Shallow-water wading

Evidence comes only from the supplied USA Rev3 disc through `psx`. Wading
now has independent contact state (`Wading`) and an authored forward gait
(`PlayerMode::Wade`). Standing, turning, backing and sidestepping keep their
ordinary locomotion identities while the feet remain in shallow water.
The game uses its existing baked animation and level assets, with no disc
dependency or level-format change.

## Source and reproduction

`tools/record_wading.py --boot --setup --inspect` rebuilds Home, prepares the
fixtures and retains the source listings. This session's core writes checkpoint
format9 and cannot load the older format8 checkpoints. The recorded menu route
is embedded in the script; the fresh Home reaches the original courtyard spawn
[19968,2560,95744]. The native build, BIOS/disc identity, commands, checkpoint
hashes and sample hashes are retained beside each CSV.

Setup changes only initial XYZ, room and yaw. The original itself enters the
512-deep fountain, selects stand-up192 and settles into standing. A checkpoint
before that entry tests the whole handoff. Additional fixtures preserve native
gait/surface state while placing the actor in the 256-deep room33, deep pool14,
or fountain17. These are explicitly controlled placements, not claims that a
player can walk between those disconnected rooms. Each replay is pad input
only; no positions, goals or animations are corrected after its first row.

Retained MIPS and byte hashes in `wading_code.json` cover:

- LaraControl `0x800424F0..0x80042B00`: water-mode handoffs.
- Ground wrapper `0x80038194`: turn/bank recovery, animation, movement,
  state collision and body-room update.
- State65 `0x80039D24`, collision `0x8003C028`.
- Standing/walking/running/turn handlers and state55's query-only collision
  `0x8003BFF0 → 0x8003C208`.
- Surface step-out `0x80044BB4` and submerged shallow test `0x80045BA4`.

## Verified behavior

- Mode4 leaves shallow water when feet depth is below384, retaining the
  current stride until its authored running dispatch. Depth731 or greater
  starts the matching surface stroke; the forward branch snaps one unit below
  the surface and clears swim speed, pitch and bank.
- Submerged water512 deep or less enters stand-up192, goal2, clears motion
  and places the feet on the floor. Surface forward movement can instead enter
  step-out190, five units above the floor. State55 queries collision without
  snapping those five units away; the later gait collision settles them.
- Standing starts the wading gait through clip186. Gait177 moves20 original
  units/tick (600/second); all start, stride and stop speeds are read from the
  existing authored animation data. Stops184/185 finish before a later restart.
- State65 adds409 to turning, capped1456 before364 recovery. Its bank adds273,
  caps2002 and recovers182 per tick. A standing Up request delegates to walking
  control, while Up+Walk delegates to the wading handler; both request gait65.
  The modifier consequently does not select ordinary walking in the fountain.
- Back needs no Walk modifier while wading. Both sidesteps and the slow/fast
  standing turns retain their authored dispatches. Movement uses the preceding
  collision heading, including the first backward/sidestep entry tick. Held
  Roll is ignored in shallow standing. Held Action against this fountain's wall
  produces the ordinary wall stop, not a vault.
- Grounded contact restores air to1800. Mode4 and the wading handler request
  camera elevation−4004. The generic engine receives an optional elevation in
  radians; the game owns when to request it. Body presentation applies bank.

Thresholds and banking/camera design values are in the existing per-type JSON
table under `swimming.wading`; animation velocities remain authored data.

## Timing and validation

The current native core occasionally pauses a display field and catches up
afterward (for example the turning route at fields118–122). Each recording
therefore watches native input `0x8008AAD4` and collision heading `0x8008C572`.
There is one heading write per controller tick and one input write per tick;
the recorder checks the counts agree. Metadata retains each tick's field,
writer PC, native input mask and equivalent pad buttons.

`game/tests/wading.rs` replays all1599 observed controller ticks across thirteen
complete routes, comparing every even-field sample through the end. It preserves
paused samples and catch-up groups instead of fitting a delay into the rules.
The compared gameplay columns are XYZ, yaw, pitch, bank, turn rate, swim speed,
clip/frame/state/goal, room, water mode, air and health; ground speed is also
checked whenever the native mode is ground/wading. Native intermediate poses
inside a two-tick catch-up field are not separately sampled. This establishes
controller parity, not display-field scheduling parity.

Six routes were first recorded after implementation and pass without fitting:
Walk-modified steering, re-holding forward during a stop, shallow exit into
running, deeper entry into surface swimming, right fast-turn followed by left
wading, and a turned surface step-out. `turn_in_place_heldout` retains its
historical filename but is explicitly calibration: it refined the left
fast-turn goal before the fresh opposite-turn validation.

Nine complete routes also compare all six chase eye/target coordinates exactly.
The four transition routes exclude camera comparison explicitly in metadata:
the existing state55 camera flags1 and general surface-camera rules are still
incomplete. Their gameplay comparisons run to the end.

`tools/wading_smoke.py` checks121 complete-game poses over MCP, including
body bank/pitch orientation, and renders aligned PSX views. Poses agree in the
inspected wading and stand-up images. Existing caustic phase/HUD and water-effect
differences remain; this is not a claim of identical pixels.

## Remaining scope

The retained `dry_exit_static_diagnostic` overlaps room61's static mesh47 at
[25088,-128,65024]. Its first native static push moves X by−99 and yaw by−910;
the existing room-only collision query does not implement that contact. The
ordinary shallow-exit fixture is one tile east, outside that static. The
diagnostic remains separate from passing parity routes.

Steep/sloped wading collision, exceptional step/fall branches, general wading
vault/jump combinations, other directional deep-water handoffs, currents,
swan dives and effects remain unverified. The 50-unit downward settlement cap
and unexercised directional handoffs are supported by the retained MIPS, not
claimed as independently runtime-validated here.

Workspace tests (441 passed including doc tests), all-target builds and Clippy
pass; existing unrelated lint warnings remain. The engine sandbox moves and
renders headlessly. Editor sandbox/Home saves and Home Lara gait177 preview
are checked using disposable copies; no persistent engine format changed.
