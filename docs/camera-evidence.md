# Authored cameras: PSX-only investigation

This increment uses the supplied USA Rev 3 disc, `psx` native execution,
memory watches, controlled memory pokes, and MIPS disassembly. No external
game implementation was consulted for this work. Older port code and asset
extraction predate this restriction; these findings independently check the
camera records and changed rules, not every earlier extraction assumption.

## Records and selection

In `.psx/wall_spawn/ram.bin`, the word at `0x8008b2b4` points to physical
`0x00194cac`. Six 16-byte records contain signed i32 x/y/z, signed i16 room,
and u16 flags. The first 14 bytes of the six-record table match the original
WALL.PSX at file offset `0x18abc8`; the entire table matches the existing
standalone camera assets except camera 2's runtime once bit. Camera 2 is
`[61952,384,23040,35]`. Routine `0x80019b28` indexes the table with index*16,
reads those coordinates and room, and passes them to camera movement.

The entrance floor chain at source floor offset 3391 is
`[2,64768,32772,15872,1026,33024]`. An identical adjacent chain begins at
3397. In loaded RAM these begin at `0x0010ad0a` and `0x0010ad16`.
The camera action selects index 2; the operand `0x8100` marks the action list
end and sets once, with no timer or authored easing bits.

Watch `.psx/camera_initialization/writes.jsonl` shows initial camera selection
by `0x8001f344`, then continuation by `0x8001ed0c`. The refresh routine at
`0x8001ecb8` recognizes last camera index (`0x8008b2a6`) before the main
activation path checks the camera record's once flag. Consequently a once
shot continues across its sectors; once prevents a new activation after exit.
The selected index is cleared at each tick's end, so end-of-field samples of
`0x8008b2a4` normally show -1 even while a fixed shot is running. Sample the
last index and observe its writers rather than mistaking this for inactivity.

## Movement, framing and timing

* `0x80017e9c..0x80017f7c`: the eye approaches its destination by 1/divisor
  of the gap per tick, with signed integer division towards zero on each axis.
* `0x8001f3ec..0x8001f400`: initial divisor is
  `((camera_operand & 0x3e00) >> 6) + 1`. End-of-tick `0x8001a22c` restores
  divisor 10 for later refresh ticks. Eye offsets below the divisor remain.
* `0x80019d64..0x80019d88`: Lara target Y uses animated bounds,
  `feet_y + max_y + ((min_y-max_y)*3 >> 2)`, in source coordinates.
  `0x80028904` interpolates bounds by truncating the weighted difference
  before adding the first bound. `0x8001a100..0x8001a128` approaches target Y
  by a quarter of the gap with signed division towards zero.
* `0x8001f3b0..0x8001f3bc`: a nonzero camera timer byte means seconds*30 ticks.
  `0x80019bb0..0x80019bdc` decrements it and changes zero to -1. The final
  tick still renders the shot; refresh stops it on the following tick.
* `0x8001f358` excludes gun-trigger activation of fixed cameras. Camera target
  object actions and heavy-actor cameras remain outside this increment.

## Reproducible observations

All runs explicitly supply neutral input unless the table says otherwise.
Paths are relative to the project; the `psx` server's saved BIOS/media identity
is unchanged. Original sample CSVs and hashes/identity/poke reports are retained
in `game/tests/data/original/cameras.json`. Tool-call recipes are retained in
`tools/camera-evidence-calls.json`.

| Run under `.psx/` | Starting checkpoint | Experiment |
|---|---|---|
| camera_entry_probe | wall_spawn/state.bin | 180 neutral fields, eye/index/state writers |
| camera_initialization | wall_load/state_f000175.bin | 42 fields, save checkpoints at 18 and 20 |
| camera_smoothing_probe | camera_initialization/state_f000020.bin | 8 fields; poke eye X to 62052 at field 0 |
| camera_timer_probe | same | 12 fields; poke camera timer to 3 ticks |
| camera_authored_timer_probe | wall_load/state_f000175.bin | 90 fields; replace both camera operands with `0x8001` at relative field 17 |
| camera_heldout_left | camera_initialization/state_f000020.bin | LEFT for 16 fields, no pokes |
| camera_heldout_right | same | RIGHT for 16 fields, no pokes; recorded after implementation |

Smoothing probe: eye X is 62042,62033,62025,62018 on successive game ticks.
The independent one-second-operand experiment counts 29 down to -1 and does
not restart while the actor remains in the same trigger sectors. Tests replay
these observations rather than generating their expected values from port code.
The target test feeds recorded original actor poses to isolate the camera from
locomotion errors; it matches all target coordinates for the first 96 fields.

## Port integration and limits

`camera.rs` is a renderer-independent rules plugin. Trigger dispatch retains
camera operands and sends eligible/continuing cues. `CameraState` exposes the
eye, look target, room, selected shot and timer through the developer MCP.
Presentation applies that pose and includes rooms visible from the eye.
Camera logic now runs on the fixed game tick and freezes with gameplay pause.
Manual camera edits used for screenshot alignment still override presentation.

Chase framing, chase collision, animated
bounds at shortened final keyframe intervals, fixed-camera obstacle/LOS
adjustment, object-look targets, heavy cameras, cutscenes, and camera state in
saves are not established as matching the original. In particular, the original
eye briefly shifts around an obstruction later in the entrance slide; this
increment does not reproduce that correction. This is an authored-shot increment,
not a claim of complete camera or game parity.

Verification: `cargo test --features dev-mcp`; build with the same feature,
then run `python3 tools/camera_smoke.py` from the workspace root for offscreen
GPU/MCP images and state records under `validation/camera-smoke`.

The offscreen check passed: tick 1 and tick 16 select camera 2 at exactly
`[60.5,-0.375,22.5]` in Bevy units; tick 116 has returned to chase. All three
368x240 PNGs were inspected. Comparison with the original entrance screenshot
still shows different horizontal framing/orientation and initial target height.
Eye-coordinate agreement does not establish rendered pixel parity. Full
`cargo test --features dev-mcp` passes with the original player comparisons
unchanged; the offscreen smoke separately exercises real MCP queries/rendering.

The subsequent [view/startup increment](view-evidence.md) resolves the horizontal
orientation and airborne initial-target differences above. Its retained captures
are under `validation/view-smoke` and `validation/view-camera-smoke`; the original
`validation/camera-smoke` captures document the earlier implementation.
