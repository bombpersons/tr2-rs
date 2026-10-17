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

Rendered frames along the climb route match the original's brightness: over
Lara's screen region the median is equal and the 95th percentile within 3
levels at fields 41, 61 and 135. Not ported: depth darkening (camera fog
approximates it), dynamic lights (gunfire, flares), flickering light modes,
water caustics, and prelit (normal-less) object meshes, whose shade mapping is
unverified. The light direction sign follows the physical reading of
`0x8006c6f4` (darker facing away from the light); its effect is only about ±2
shade levels in this room.
