# Room and object lighting: PSX-only investigation

Evidence: supplied disc, psx native core, no pokes. Wall settled start
(`.psx/wall_game/state.bin`) and the climb route (`.psx/climb_visual`).

## Room vertices

`.psx/light_gpu` (GPU frame capture) shows room polygons drawn Gouraud,
textured, with grey vertex colours of 8..47 (128 = neutral modulation). In RAM
Wall room 35's vertex count is stored twice (`0x800f4144` and `0x800f4146`,
both 92). The extractor had read the duplicate as part of vertex 0, pairing
every vertex with its neighbour's shade and losing the last one. The room
renderer `0x800617b0` reads aligned words: xyz in the low 15 bits (x bits
10-14, y 5-9, z 0-4), a 16-bit shade in the high half (traced loads such as
`0x14520401`). The colour it draws is the shade's low byte (`lbu 6(slot)`),
replicated to RGB by `0x80062090`, minus `(depth - 12288) >> 5` beyond depth
12288. `tools/extract_tr2.py` now asserts the duplicate count (all rooms of all
25 levels) and stores the full 16-bit shade; positions are unchanged.

The port also passed encoded GPU multipliers to PsxMaterial as linear vertex
colours, which the shader re-encodes, brightening dark vertices (0.15 drew as
0.42). Vertex colours now go through `Color::srgb(..).to_linear()`, as the
template's TMD path does.

## Items (Lara and objects)

Normals of Lara's hips mesh are read by `0x80064954`, the object vertex
lighter: light vector transformed by the current matrix and scaled by
1/intensity; per vertex `v = ambient + (n . L') >> 16`, clamped 0..8191,
shade `255 - (v >> 5)`. `0x8006c6f4` (S_CalculateLight, called by
CalculateObjectLighting `0x800286ec` at the item position plus its rotated
bounds centre) takes each room light's `intensity*f^2 / (d^2/4096 + f^2)` with
`f^2 = falloff^2 >> 12`, keeps the brightest, halves it (t3), sets ambient =
room ambient - t3, intensity = 2^26 / t3 and the light direction from that
light; beyond depth 12288 the ambient darkens. Watches give ambient 7690/7694
at fields 1/135 (shade about 15, against drawn greys 8..23); the port's
`lighting::item_light` reproduces the same formula. The shade is linear in
n.L, so presentation feeds PsxMaterial two opposite directional lights.

## Verification and limits

Earlier climb comparisons reported an equal median and a 95th percentile
within 3 levels at fields 41, 61 and 135. Those captures predate the current
template integration; the fresh checks below supersede that visual claim.
Not ported: depth darkening (camera fog
approximates it), dynamic lights (gunfire, flares), flickering light modes,
water caustics, and prelit (normal-less) object meshes, whose shade mapping is
unverified. The light direction sign follows the physical reading of
`0x8006c6f4` (darker facing away from the light). The directional scale is
corrected by the fresh evidence below.

## Per-object lighting ownership and directional scale

The backward-movement smoke exposed black characters in Home. The engine
was updating each item's material in `Update`, but the template's private
`apply_look` copied scene lighting over it in `PostUpdate`. With no scene
lights, this erased the original room-light calculation every rendered frame.

`PsxMaterial::use_scene_lighting` now defaults to true, preserving template
behavior. Engine item materials set it false and own their `lighting`.
Global look changes still propagate. This is a generic material option;
no title-specific code or data was added to the template. Its unit test
checks scene-light changes, look changes and opting back into scene lighting.
The local template merge instructions are in [template.md](template.md).

Fresh two-field, no-poke traces start from the original Home and settled
Wall checkpoints. `.psx/item_light_home_complete` and `.psx/item_light_wall`
trace `0x8006C6F4..0x8006CC14` and `0x80064954..0x80064B18`, with no dropped
events. The first CalculateLight call comes from CalculateObjectLighting
`0x800287BC`, before Lara's first mesh is lit. They establish:

| Scene | Room | Sample point, source axes | Ambient | Strength |
|---|---:|---|---:|---:|
| Home | 16 | [19977,2172,95752] | 3865 | 1511 |
| Wall | 35 | [64167,4989,26081] | 7691 | 244 |

Point X/Y are traced arguments; point Z is reconstructed from the observed
light Z plus the subtraction at `0x8006C780`. Ambient is the store at
`0x8006CB6C`, strength is the halving at `0x8006CB1C`, and the directional
denominator is stored at `0x8006CB74`. The JSON reference retains those
values, the first mesh's rotation, view-space light vector, normals, output
shade bytes, complete commands, build identity and checkpoint/trace hashes.

These traces also correct the directional coefficient. The earlier port
assumed a 4096 rotation scale and divided directional strength by four.
The actual rotation, normals and light vector all use **16384 for one**.
For example, Home's traced first-mesh matrix first column is
`[13715,260,8935]`, whose length is approximately 16384. The lighter's
matrix/vector multiply divides by `2^26/strength`, then the normal dot
product shifts by 16 (`0x80064974..0x80064ABC`). In unit vectors those
scales cancel, giving `ambient + strength * dot(normal, direction)`.
Presentation now uses that full coefficient in its opposite material lights.

`game/tests/lighting.rs` checks the sampled ambient and strength exactly,
then compares all 22 Home and 20 Wall first-mesh shade bytes. The float
lighting rule agrees within one shade-byte level, allowing integer matrix
division and shade truncation. Home established the scale; Wall's vertex
results provide an independent check without changing the rule.

Reproduce with psx on PATH (or supply `--psx /path/to/psx`):

```sh
python3 tools/record_item_lighting.py
cargo test --workspace --all-features
cargo build -p tomb-raider-ii --all-features
python3 tools/item_lighting_smoke.py
```

`--summarize-only` retains equivalent psx MCP runs already present. The smoke
renders the full standalone game at retained Home/Wall camera poses and
checks a skin patch in the Home view to catch the silhouette regression.
Its broad brightness bound is a visibility check, not pixel parity. The
before views are retained under `validation/item-lighting-before/`; the final
corrected views, report and MCP transcripts are under `validation/item-lighting/`.

Home's character textures are visible again, and the Wall remains appropriately
dark. Original and port images were inspected; pose, raster and texture
differences remain, as does Home's missing sky. No lighting format changed:
the editor still uses the existing authored room lights and standard renderer,
and the engine-only sandbox retains ordinary scene lighting.

Verification: workspace all-features tests and Clippy pass, including the
lighting ownership test and both PSX-reference checks. The final offscreen
Home skin patch has median brightness 131.7, versus original 127.7 and
pre-fix 5.3. Both scene screenshots and the engine-only sandbox were inspected.
Existing lint warnings and unrelated workspace formatting differences remain.
