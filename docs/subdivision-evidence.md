# Room polygon subdivision: PSX-only investigation

Question: how does TR2 keep affine texture warping down on room polygons near
the camera? Answer: **runtime 3D subdivision by depth band** in the room
renderer, plus a crack-filler copy of each split polygon. The renderer
subdivides polygons with projection errors and rejects invalid leaves rather
than clipping those leaves at the near plane.

Evidence: supplied disc, psx native core. Wall settled start
(`.psx/wall_game/state.bin`), where the camera sits against the left rock wall.

## Runs

| Run | Start | What |
|---|---|---|
| `.psx/affine_probe_walk` | wall_game | RIGHT [0,56), UP+CROSS [60,240); snapshots every 20 fields, state at 250 |
| `.psx/affine_gpu_near` | wall_game | first textured frame (run frame 1): camera against the wall |
| `.psx/affine_gpu_far` | affine_probe_walk/state_f000250 | first textured frame: Lara a few metres away at the far wall |
| `.psx/affine_gpu_near_control` | wall_game | textured frame at run frame 7, no pokes |
| `.psx/affine_gpu_near_nosubdiv` | wall_game | same, poking `0x80061A14` and `0x80061D40` to `slti at,v1,0` |

Note: the frame drawn at run frame 1 was built before a frame-0 poke took
effect, so the poke comparison uses run frame 7 for both runs.

## Code (room renderer `0x800617b0`)

Disassemble with `psx disasm --ram .psx/affine_gpu_near --function 800617b0 --text`.
The quad loop starts at `0x80061898`, the triangle loop at `0x80061C34`. Both
do the same thing; the quad addresses are listed.

1. Project the vertices (RTPT + RTPS). If the GTE flag (`cfc2 c31`) has its
   error bit set (near plane or overflow), OR the four SZ values together;
   if the result `>> 7` is 0 (all SZ < 128) the quad is dropped. The
   triangle guard at `0x80061CB8` instead tests the unshifted OR for zero.
   Otherwise it is kept with `s5 = 6` and the whole-polygon NCLIP backface test
   is skipped (`0x80061950`..`0x80061984`).
2. Otherwise NCLIP; drop if back-facing (`0x80061988`).
3. Screen-rect reject `0x80061FC4`: all vertices left, right, above or below
   the clip rect stored at `0x1F800040..0x1F80004C`.
4. `maxZ` = largest SZ. If over 20480, drop (or clamp to 20480 when the
   caller's `s6` flag is set).
5. If `maxZ < 5000` (`0x80061A14`), subdivide via `0x800620DC` (quads) /
   `0x80062474` (triangles). Level `t7` is `maxZ < 2500 ? 1 : 0`, or
   `s5 & 3 = 2` for a near-plane polygon. The first call always splits once;
   children re-enter at `0x800620D4`, which decrements `t7` and emits at -1.
   So the splits are:

   | Condition | Split | UV size of a 64-texel tile |
   |---|---|---|
   | 2500 <= maxZ < 5000 | 2x2 | 32 / 33 |
   | maxZ < 2500 | 4x4 | 16 / 17 |
   | GTE error, some SZ >= 128 | 8x8 | 8 / 9 |

   Each split averages the **pre-transform** vertex positions (edge midpoints
   `(a+b)>>1`, centre `(a+b+c+d)>>2` per axis), the UV bytes and the colours.
   Colours use a carry-masked byte average with mask `0x01010100`. Every new
   vertex is then re-projected with RTPS/RTPT, so the split is
   perspective-correct, not a screen-space split. Triangles split into 4 by
   edge midpoints.
6. Leaf emit (`0x80062314` quad, `0x8006261C` triangle): if the leaf still has
   a GTE error it is dropped. If `s5 & 4` (near-plane parent) it gets its own
   NCLIP test. It goes through the same screen-rect reject, and its OT slot is
   max SZ / 8. Packets are GT4 (`0x0C`, 52 bytes) / GT3 (`0x09`, 40 bytes).
7. **Crack filler**: after splitting, if the polygon was not a near-plane one
   (`s5 == 0`), `maxZ >= 512` and the byte at `0x1F8000AA` is 0, the
   unsplit polygon is also emitted. It has no depth darkening and OT depth
   `maxZ + 1024`, so it sits behind its own pieces and fills T-junction
   gaps against unsplit neighbours (`0x80061AA4`..`0x80061AD8`). The `0xAA`
   byte is the pad byte of the texture record's fourth word (uv3), read from
   the object-texture table at `gp+0x2208`. Meaning provisional: probably
   a per-texture "no filler" flag (e.g. semi-transparent).
8. Unsplit polygons (`maxZ >= 5000`) get depth darkening
   `shade - (SZ - 12288) >> 5` (see [lighting-evidence](lighting-evidence.md)).
   At `maxZ >= 8192` (`0x80061B80`) each UV becomes `(uv >> 1) | 0x8080`, which
   selects a half-size copy of the texture at +128,+128 in its page: a
   distance texture LOD. The LOD is noted here and not yet ported.

## Runtime confirmation

- `.psx/affine_gpu_near`: 405 textured primitives. UV extents split as the
  code predicts. Integer midpoints of 0..63 give 32/33-wide halves,
  16/17-wide quarters and 8/9-wide eighths. The 8x8 (near-plane) pieces
  appear only in the near frame. Whole 64-texel quads share corners with
  their 16/17-wide pieces (e.g. primitive 124 and 180..186), as the crack
  filler predicts.
- The quad at index 103 has one vertex at view z 164 and a 1059 px screen
  extent but full UVs. It is a crack filler (whole copy) and is drawn first
  because it sits deeper in the OT.
- Poke test, same frame: 401 draw commands normally, 297 with the 5000
  threshold patched to 0. In the patched frame the near floor and left wall
  smear diagonally (affine warp); the original does not
  ([subdivision-compare.png](subdivision-compare.png), brightened 3x).

## Not covered

- Object meshes (Lara, enemies, static meshes) use a different renderer. Not
  checked whether they subdivide; Lara's polygons in these frames are small
  and unsplit.
- The `s5 & 8` OT-depth branch (`gte_d7 >> 1` instead of max SZ) is never
  set by the room path. It is presumably used by another caller.

## Port implementation

`game/src/subdivision.rs` supplies TR2's rule through the engine's optional
`render::rooms::RoomMeshPolicy` / `RoomTessellator`. No template shader or
level-format changes are needed. Object and static-model meshes are unchanged.
The editor and engine sandbox do not install a policy and keep static meshes.

The matrix loader `0x80062E58` takes the Q14 world/view coefficients, shifts
them right by two and loads the GTE's Q12 rotation. Room coordinates remain
in source-world units, so SZ is **1024 units per Bevy world unit**. The two
split boundaries are therefore 2500/1024 and 5000/1024 world units. The
captured face test below independently checks this scale against native view
depths. This resolves the previously unchecked scale in this document.

For each visible room, the engine caches its authored faces and regenerates
writable mesh attributes after the current camera pose and room visibility.
A still camera reuses the mesh; camera, atlas-size, projection and policy
changes invalidate it. Hidden rooms catch up when shown. Mesh handles and CPU
buffer allocations are reused. Conservative authored room bounds cover all
future pieces and fillers. An empty result removes the mesh from the entity
instead of uploading an empty asset. Level replacement owns and despawns the
whole hierarchy, including underlays.

The game chooses each parent's band using the largest forward view depth.
It recursively splits quads into four quads and triangles into four
triangles. Positions retain integer source units and use floor averages;
UVs average in integer atlas texels, accounting for the importer's half-texel
centres; shade bytes average before conversion into Bevy vertex colours.
The quad centre uses a direct four-position average but paired edge-colour
averages, as the disassembly does. Quad leaves use the original GPU strip's
triangle diagonal. Leaves with remaining projection overflow are rejected;
parents and leaves receive the observed winding and screen-rectangle tests.

Filler copies use a separate ordinary Bevy mesh/material with
`PsxDepthBias::BEHIND`. The original's `0x1F8000AA` test maps to the low byte
of the texture's existing `attribute`: the fourth texture word is stored at
`0x1F8000A8` (`0x800619A8`), so its first pad byte is at +2. The port tests
that byte for zero and observes the parent-error and SZ>=512 guards. This
needs no guessed flag name or new serialized field. Meaning beyond this
specific zero test remains unknown.

### Verification

- `tools/record_subdivision_reference.py` matches room35 quad60 against
  `.psx/affine_gpu_near` primitive124 and retains its fourteen actual leaf
  packets (171..186, with two neighbouring-face packets excluded) in
  `game/tests/data/original/subdivision.json`. Snapshot hashes are retained.
  Source corners agree with native view positions within eight source units
  and their shade bytes agree exactly. The earlier importer provenance of
  the decoded geometry is unchanged; these new packet matches are PSX-only.
- `game/tests/subdivision.rs` checks all emitted leaf UVs and shade bytes
  exactly against those packets, including the GPU strip diagonal's repeated
  triangle corners. View coordinates agree within sixteen source units under
  the ordinary floating-point camera, and one unsplit filler is emitted.
- Unit tests cover both strict depth boundaries, farthest-corner selection,
  triangle/quad splits, signed midpoint rounding, 63-texel UV spans becoming
  31/15/7 midpoint offsets, near-camera leaf rejection, filler flags and
  camera movement. The engine test checks cache reuse, visibility, policy
  revisions and stable mesh handles.
- `python3 tools/subdivision_smoke.py` compares off/split/filled through MCP
  at the near and far retained Wall camera poses. Screenshots/transcripts are
  in `validation/subdivision/`. Room35 changes from156 base triangles to202
  visible split triangles plus28 filler triangles at the near camera; at the
  far camera it emits138 plus18. Counts include screen/backface rejection.
  Both remain368x240. The split views reduce close-wall/floor smearing.
  Filler on/off screenshots are identical at these two sampled poses: no
  exposed cracks were found there. Far-camera comparison keeps the same
  paused actor state and checks camera-dependent geometry, not a full replay.
- `cargo test --workspace --all-features` passes, including all existing
  gameplay trajectories and editor save tests. The sandbox was built and
  driven headlessly with MCP movement input; it has no `RoomMeshStats` and
  keeps its static320x240 scene. `tools/editor_smoke.py` passed save/undo/
  redo/reopen and byte-exact untouched saves at1280x800. Screenshots were
  inspected. Clippy completes with existing warnings and none in the new
  subdivision code; the workspace format check still reports pre-existing
  differences. The new Rust modules/tests are rustfmt-formatted.

### Remaining differences

The camera and geometric projection-limit tests use floating point, not a
GTE implementation. Exact fixed-point projection, overflow edge cases and
pixel-level equivalence are not claimed. The near-plane policy now drops
invalid split leaves instead of relying solely on Bevy clipping.

The filler's GPU depth bias reproduces precedence over its coplanar pieces,
but does not reproduce the original's maxZ+1024 ordering relative to unrelated
geometry. Distant room darkening, the >=8192 texture LOD and complete portal
screen rectangles remain separate unported rendering features. No conclusions
about subdivision of object meshes are added by this work.
