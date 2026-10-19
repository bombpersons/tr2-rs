# Home's camera-relative sky

Evidence is only the supplied Tomb Raider II USA Rev 3 disc through `psx`.
The existing baked model/atlas predate the reference restriction; this milestone
independently verifies the mesh, pose selection and draw behavior against the
original's runtime RAM, instruction trace and GPU capture. No new outside
implementation was consulted. The standalone game reads only baked assets.

## Binary and runtime evidence

`psx xrefs --ram .psx/home_start/ram.bin --target 0x80096670-0x800966A0 --text`
locates the sky's object-table record (type254) in room drawing0x80023E50.
The draw listing and focused trace are in `validation/home-sky/`.

- `0x80023ED0..0x80023EE0` tests room flag8; portal traversal propagates the
  exterior-visible state. `0x80023F58..0x80023F7C` requires the model to be
  loaded. The imported `Room.other_flags` already preserves flag8.
- `0x80023F98..0x80023FAC` zeros all three translation components of the
  copied view matrix. Camera rotation remains; moving the camera cannot
  translate the sky relative to the viewer.
- `0x80023FB4..0x80023FD4` reads first animation278, its first frame pointer
  0x16E8CC, and calls the packed rotation reader on frame+18. It skips the
  root translation [0,414,0]. The decoded first rotation is
  [16320,32704,32704]. Only the first mesh is drawn at0x80023FDC..0x80023FF0.
- The native trace resolves mesh index128 in the original pointer table to
  0x1260F4; this is baked mesh100 after importer deduplication. Its header is
  centre[-18,0,528], radius2403,50 vertices. All50 runtime vertex coordinates
  match the baked mesh exactly. Normals and UVs remain ordinary baked assets.
- Sky draw0x8006D3B8 calls0x8006759C with mode0. The latter writes shade134
  to every vertex at0x800676DC..0x800676F0. GPU capture commands8..15 confirm
  that modulation on the cloud-band textures. Solid cap command7 has palette
  colour[168,196,208] and no dithering; the game keeps neutral cap colours.

The engine now provides an isolated background layer/camera that copies the
main projection, display crop and orientation, keeps its translation zero,
and renders before a fresh world depth buffer. The game's sky plugin selects
model254, its first mesh and first rotation, applies the verified texture shade,
and shows it when an exterior room is visible. Mesh construction reuses the
engine's ordinary object mesh builder. All title-specific decisions stay in
`game/src/sky.rs`. No raw GPU packets, captured images or ROM are shipped.

The small `Sky.enabled` developer switch supports aligned on/off checks.
The sky is a separate `LevelEntity` hierarchy, so level replacement despawns it.
No level format, template crate or editor dependency changed. Other games can
use the engine's background layer with their own mesh and material.

## Independent projection and rendering checks

`game/tests/data/original/home_sky.json` retains the first native GPU capture,
source identity, command, checkpoint/GPU hashes and eight textured commands.
The independent `home_sky_turn.json` was recorded after implementation using
Right[0,36),120 fields from `home_start`, GPU field119; ten cloud-band commands
cover a different camera orientation. Reproduce via `tools/record_sky.py`.

`game/tests/sky.rs` checks all72 GPU corners against the baked mesh transformed
with rotation only: the nearest vertex is within6 source units, and its
projection within2 pixels. These allowances account for the GTE's quantized
matrix/projection versus the engine's floating-point camera; they do not imply
pixel parity. The engine also tests background activation/removal, crop/FOV,
orientation and invariance to world-camera translation without disc assets.

`tools/sky_smoke.py` drives the real game over MCP. In Home's start and the new
turn view, enabling the sky changes13676 and10561 pixels respectively. Every
changed pixel was previously black; room/object/HUD pixels remain identical.
The wall-facing Home side view changes only nine empty pixels, exercising
occlusion. An isolated background capture remains pixel-identical after moving
camera and target together by [100000,-50000,100000] source units.

Upper-left sky patches compare against the PSX images with mean absolute RGB
errors8.20 and10.18 on the0..255 scale. Paired captures were inspected. Palette
expansion/quantization, affine rasterization, cap raster behavior and global
rendering differences remain: full pixel parity is not claimed. Other levels'
sky behavior and unusual interior/portal clipping remain unverified. Wall's
no-sky view passes the ordinary full-game render smoke.

## Workspace and authoring checks

All229 workspace tests pass, including all existing movement trajectories.
All-target builds and Clippy pass with existing unrelated warnings. New Rust
files are formatted and `git diff --check` passes; unrelated workspace formatting
is retained. Engine sandbox movement/rendering and editor sandbox/Home byte-exact
save, undo/redo, save/reopen and authored animation preview checks pass. Paired
images, exact MCP operations, GPU evidence and logs are retained under
`validation/home-sky/`.

The editor keeps its ordinary full-resolution rendering. Sky geometry, model,
pose and atlas remain accessible through the existing engine data browser;
no synthetic placement or new save field is needed for this hardcoded draw rule.
