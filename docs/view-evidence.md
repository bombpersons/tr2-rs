# View orientation and camera startup: PSX-only evidence

This work uses the supplied disc and `psx` runtime/disassembly. It does not
consult an external game implementation. Existing decoded room/animation
assets are inputs, with their earlier provenance unchanged.

## Horizontal orientation

Camera movement calls the original look-at routine `0x80065184` from
`0x800182b4`. It derives angles from the eye/target difference, then calls
`0x80064f04`, which writes the world-to-view basis and eye to `0x8008aa60`.
Each row contains three signed Q14 coefficients followed by one eye coordinate.
The same routine writes another copy at `0x80096888`.

At `.psx/wall_spawn`, eye `[61952,384,23040]`, target `[62976,830,23040]`,
the basis rows are:

```text
     0      0 -16384
 -6524  15019      0
 15019   6524      0
```

Thus +X is forward at this camera and -Z points screen-right. The port had
converted only source Y, so Bevy's right-handed look-at put +Z to the right.
This mirrored horizontal framing for both manual and gameplay cameras.

`presentation/view.rs` now defines the render boundary. Rules retain their
existing `(source X, -source Y, source Z)` coordinates. A single level parent
reflects Z for all rooms, static placements, actors and articulated children;
the camera's eye and target receive the same conversion separately. Local
Transforms used by movement, interactions, saves and developer edits remain in
rule coordinates. The 2D title/HUD are outside that level hierarchy.
Level replacement despawns the hierarchy root, including its descendants.

The projection remains an ordinary Bevy perspective camera. The PSX GPU
capture independently reports H=228 and principal point (192,120), with a
368x240 display. The port renders the corresponding crop of a 384x240 view.

`game/tests/data/original/view_projection.json` retains four original view
matrices, snapshot hashes and 64 projected room landmarks. Expected pixels
come from the original matrix and projection values, not Bevy's implementation.
The regression test compares an ordinary Bevy camera and the converted level
coordinates against them, allowing less than one pixel for quantized original
angles/matrix coefficients. Rebuild with `python3 tools/record_view_reference.py`.

## Initial look target

The watch in `.psx/view_initial_target_probe/writes.jsonl` establishes this
ordering during Wall initialization, all at relative field17:

1. `0x80043b1c/0x80043b28` select clip11/frame185.
2. `0x80017db8` seeds target Y to player Y-1024 =512.
3. The camera call at `0x80017e68` eases that to622 using clip11's bounds.
4. Actor initialization changes to clip103, then fall clip34/frame492.
5. The level's camera update eases target Y to709.
6. The next tick advances fall clip34 to493, moves player Y to1542, and eases
   the camera target to776.

The rules initialize the airborne camera using these two framing passes.
The fall clip is selected at spawn, so the first tick advances it rather than
reselecting its first frame. The full player/camera rules match original eye
and target for the first38 fields without injecting a recorded target.
The existing test with original actor poses still validates96 fields separately.

## Captures and verification

New PSX calls and identities are retained in `tools/view-evidence-calls.json`
and `game/tests/data/original/view_runtime.json`. The retained runs are:

* `.psx/view_matrix_probe`: two neutral fields from Wall spawn; watches all
  48 bytes of the matrix and records the expected writers.
* `.psx/view_projection_draw`: three-field textured-draw search from Wall spawn.
* `.psx/view_initial_target_probe`: twenty neutral fields from the pre-load
  checkpoint; watches target Y, player animation/frame and player Y.
* `.psx/view_spawn_reference`: one field from the first initialized Wall state,
  with samples and an exact GPU capture.

The last run's normal `screen.png` still shows the loading screen because the
new draw is in the other display buffer. Its `gpu_frame_screen.png` shows the
captured gameplay draw. Use the latter for visual comparison at these fields.

Build with `cargo build --manifest-path game/Cargo.toml --features dev-mcp`.
Run `python3 tools/port_smoke.py --out validation/view-smoke --cases home,wall`
for aligned Home/Wall captures. Run
`python3 tools/camera_smoke.py --out validation/view-camera-smoke --transitions`
for the fall and Wall → Home → Wall hierarchy/reinitialization checks.
All screenshots were inspected; the reflected framing now agrees with the
original at the aligned Home and Wall poses. The entrance state records also
match the freshly sampled PSX player, animation, camera eye and target.

Remaining visual differences include lighting, the absent Home sky, close-wall
visibility and enemy positions. At first contact the original chooses landing
clip82 while the provisional port uses clip24; their different bounds start
diverging camera target height at field16732. Later camera obstacle correction,
complete chase rules, and cinematic cameras remain unfinished. These checks
establish orientation/startup behavior, not complete rendering parity.
