# Locomotion evidence: turn rate, vault, wall stop

PSX-only evidence (supplied disc, psx native core, no pokes). Lara item in Wall
is `0x8019cf30` (layout in [original-evidence.md](original-evidence.md)).

## Capture

`.psx/climb_probe` from `.psx/wall_game/state.bin` (field 21000, settled after
the entry slide): RIGHT fields [0,56), UP+CROSS [60,260). Retained as
`game/tests/data/original/climb_two_click.{csv,json}`; the earlier
`.psx/climb_turn_probe` (RIGHT only) located the release point.

## Turn rate (binary)

Lara control `0x80038194`: the state handler runs (table `0x80082c98`), then the
shared turn rate `0x8008c570` (i16) decays by 364 toward zero (|rate| <= 364
becomes 0) and is added to yaw. Handlers seen:

| State | Handler | Turn rule |
|---|---|---|
| 0 walk | `0x800386c4` | LEFT -409 else RIGHT +409, clamp ±728 |
| 1 run | `0x800387b0` | ±409, clamp ±1456 |
| 2 stop | `0x8003899c` | none; LEFT/RIGHT request 7/6; UP calls walk/run handler directly |
| 3 forward jump | `0x80038b88` | ±409, clamp ±546 |
| 6 turn right | `0x80038d94` | +409 *before* reading input; >=729 requests fast turn 20 (walk clamps 728) |
| 16 walk back | `0x80039270` | ±409, clamp ±728 |
| 20 fast turn | `0x80039370` | sets ±1456 (1092 after decay) |

This reproduces the earlier fitted curve (first delta 0, +45/tick, 1092 fast)
and the capture's coast after release: +1260 (handler still adds), then 896,
532, 168, 0. Displacement each tick uses the heading from before that tick's
turn (x/z rounding at yaw 19166 -> 19698).

## Vault and collision (binary + capture)

GetCollisionInfo `0x8001b018`: quadrant = (facing+8192)>>14; front probe is the
radius (100) along the quadrant axis plus (trig*radius)>>14 across it;
left/right are the quadrant's forward corners. Vault test `0x8003d924` (called
from run collision `0x8003a844`): front collision, CROSS, hands free, yaw within
±5460 of a cardinal, |left-right floor| < 60, 762 clearance at all three probes.
Front floor -640..-384 -> clip50/state19, y += 512+front; -896..-640 -> clip42,
y += 768+front. Yaw snaps to the cardinal. The capture's grab tick discards the
move and shifts to 101 from the wall face (x64411), y5370 = 4858+512.

Clip50 end command 1 `(0, -512, 128)` lands at x64539 after 28 frames; standing
collision settles y4857. Clip51's goal-1 range enters clip52 (speed 0, accel
131071: 0,1,3,5,7), which chains into clip6 frame 99. Floors on the +x tilt-1
ramp (4845 at x64584, 4821 at x64681) show positive tilts truncate the rise,
while the Wall entry ramps confirm negative tilts round away from zero.

Wall test `0x8003c91c(256, 0, -640)`: solid or no clearance 256 ahead at 640
up. Run collision skips the hit-wall clip during clip6; otherwise clip53 (run
frame < 10) or 54. The capture enters clip53 at x65435 (101 from the face),
keeps y for that tick, then clip11 -> clip103.

## Tests and limits

`original_turn_run_grab_climb_and_wall_stop_match_every_tick` compares x, y, z,
yaw, animation and absolute frame for every tick through field 259 (the
decay constant mutated to 363 fails at tick 2). Unverified: the negative-axis
wall shift (assumed mirror), 3-click climb timing, jump-up/monkey branches of
the vault test, and other state handlers.

## Blocked run starts on slopes: sampling before the wall shift

PSX-only follow-up, no pokes. `.psx/wall_stop_watch` replays the same route
and watches Lara's y (`0x8019cf68`, i32). At relative field 217 the run
collision writer `0x8003AA04` changes y4633 to y4627; releasing UP at field
260 lets standing collision `0x8003AAC8` restore y4633.

The six-unit difference is a consequence of collision ordering, not a fixed
height adjustment. `.psx/wall_stop_trace` starts from the saved field-216
checkpoint and traces six fields with UP+CROSS. At absolute field 21217:

1. Animation movement writes x65458 (old x65435 + speed23), z26010.
2. Run collision `0x8003A844` calls `0x8003C318`; `0x8003C34C` calls
   GetCollisionInfo with this proposed position. The centre floor query
   `0x8001B0B8` returns y4627, so the stored middle-floor delta is -6.
3. The collision shift (`0x8001C0AC`) restores x65435. Run-start clip6
   skips the hit-wall animation and `0x8003A944` restores the standing
   animation through `0x8003A56C`.
4. Run collision still applies the previously sampled middle-floor delta:
   `0x8003A9D8..0x8003AA04` adds it to y (downward correction capped at 50).
   Repeating the blocked start now produces delta 0 and holds y4627.

The port retains the centre floor at the proposed position when a blocked
run start returns to standing. It recalculates the floor at the resolved
position on subsequent neutral standing ticks. This change adds no format
or editor fields; the slope and animation remain authored engine data.

After implementing the rule, `tools/record_wall_stop.py` captured an
independent 340-field hold/release/retry route from the unchanged settled
Wall checkpoint: RIGHT [0,56), UP+CROSS [60,240), neutral [240,260),
UP+CROSS [260,300), then neutral. The CSV and metadata are retained as
`game/tests/data/original/wall_stop_heldout.{csv,json}`, including checkpoint,
samples, trace and watch hashes and the full psx command. Its six-field trace
independently repeats the ordering above without dropped events.
`heldout_wall_stop_release_and_retry_match_every_tick` matches position
within 0.02 original units and yaw/animation/frame exactly at every sampled
logic tick, including both floor restorations and the retry. The original
climb test now covers the complete recording instead of truncating it.

Reproduce after baking content and building the developer binary:

```sh
python3 tools/record_wall_stop.py                       # psx on PATH, or --psx /path/to/psx
cargo test -p tomb-raider-ii --all-features --test player
python3 tools/climb_smoke.py --wall-stop-heldout --out validation/wall-stop-smoke
```

The existing capture delay applies to visual comparisons: a port field N
corresponds approximately to original screen N+4. Other collision branches,
including the 50-unit descent cap and restoring turn states after a blocked
run, are not established by this route.

Verification: workspace all-features tests and Clippy completed (existing
lint warnings remain). The offscreen held-out smoke completed with 13 captured
poses matching the original samples for position, yaw, clip and absolute
frame. The final port/original screenshots and the engine-only sandbox
screenshot were inspected. Existing rendering differences, including Lara's
brightness and the HUD, remain; this is a collision correction, not a claim
of pixel parity. Workspace formatting checks still report pre-existing
formatting differences in unrelated code; those files were preserved.
