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
yaw, animation and absolute frame for every tick through field 217 (the
decay constant mutated to 363 fails at tick 2). Unverified: the negative-axis
wall shift (assumed mirror), 3-click climb timing, jump-up/monkey branches of
the vault test, other state handlers, and the 6-unit higher standing height
(4627) the original shows from field 219 while UP is held at the wall.
