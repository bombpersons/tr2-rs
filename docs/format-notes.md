# Tomb Raider II PlayStation asset decoding

The supplied disc is **Tomb Raider II: Starring Lara Croft, USA, revision 3**. The source files used here are the ISO-extracted `research/disc/DATA/*.PSX`, `PIX/*.RAW`, and `PIXUS/*.RAW`. Game runtime assets are written under `game/assets`; no BIN/CUE or extracted `.PSX` is needed at build or runtime.

## Evidence and scope

The decoder is an original Python implementation, cross-checked against the primary source [OpenLara `src/format.h`](https://github.com/XProger/OpenLara/blob/master/src/format.h), especially `loadTR2_PSX`, `readDataArrays`, `readRoom`, `initMesh`, and `readObjectTex`. Skeleton traversal and frame indexing were checked against [OpenLara `src/animation.h`](https://github.com/XProger/OpenLara/blob/master/src/animation.h). The headerless RNC2 image format was checked against [OpenLara `src/texture.h`](https://github.com/XProger/OpenLara/blob/master/src/texture.h), `LoadRNC`. These are format references; the Rust port does not embed OpenLara or emulate the PlayStation.

Observed: all 25 `.PSX` files decode into 2,548 rooms, 6,310 unique object meshes, 1,480 model definitions, 41,029 texture definitions, 2,706 entity placements, 9,691 animations, 178,552 decoded animation keyframes, and 2,789 mono WAV samples. There are 37 decoded screen PNGs. Counts include title, home, demos, and cutscene scenes; they are not counts of campaign stages or distinct assets across the entire disc.

Each level JSON records the SHA-256 and size of its own original extracted file. `manifest.json` records counts. Reader bounds, room geometry lengths, vertex references, texture references, room portals, sector links, model mesh ranges, ADPCM predictors, and final source consumption are checked. This establishes structurally valid assets, not behavioral feature parity.

## Reproduce

Run from the workspace root:

```sh
tools/bake_assets.sh [path/to/game.cue]
```

It runs `psx extract --all` into `research/disc`, `tools/extract_tr2.py`
(raw tables into `research/extracted/levels`), the `tr2-import` importer
(engine format into `game/assets/levels`), `tools/extract_pix.py` and
`tools/extract_cdda.py`. A bake from the supplied disc reproduces the
current assets byte for byte.

Pillow is the only nonstandard Python requirement. `--level WALL` limits extraction to one level; `--input` and `--output` select other directories. A single-level invocation replaces the output manifest with that invocation's results; run without `--level` to rebuild the complete manifest.

## Level JSON contract (schema 1)

Coordinates preserve original TR2 units: `x` right, `y` down, `z` forward. A sector is 1,024 units wide. Signed heights in sectors are expanded to full units. Rotations stored on placements are unsigned 16-bit turns (65,536 = one turn). Convert axes and scale once when importing into Bevy.

| Field | Meaning |
| --- | --- |
| `rooms` | Full room geometry, portals, sectors, lights, flags, and static placements. |
| `rooms[].origin` | `[world_x, 0, world_z]`; add to each room vertex. |
| `rooms[].vertices` | `[local_x, absolute_y, local_z, light_0_to_255, attributes]`. Light is direct brightness. |
| `quads` | `[v0,v1,v2,v3,texture_index]`, perimeter order. Triangulate `(0,1,2),(0,2,3)`. |
| `triangles` | `[v0,v1,v2,texture_index]`. |
| `textures` | Original tile/CLUT/attribute/UV and normalized `atlas_uv`. |
| `atlas_size` | Width and height of `atlas.png`; use nearest sampling. |
| `meshes` | Unique mesh data, indexed through `mesh_indices`; vertices `[x,y,z,original_w]`, normals if present, quads and triangles. |
| `mesh_indices` | Maps an original game mesh reference to the index in `meshes`. |
| `models` | Game object ID, mesh start/count, node/frame offsets, animation start/end, direct joint tree, decoded initial pose. |
| `static_meshes` | Static definition `id`, original mesh reference, visual/collision boxes, flags. |
| `entities` | Original object ID, room, absolute position, rotation, intensities, activation flags. |
| `floor_data` | Original 16-bit floor commands; retain for slants, triggers, and room links. |
| `animations` | Timing, 16.16 speed/acceleration, transitions, command/state offsets, decoded poses. |
| `animation_states/ranges/commands` | Original state dispatch and animation effect tables. |
| `nodes/frame_data` | Original decoded integer arrays retained for audit and extension. |
| `cameras/camera_frames` | Authored cameras and cutscene camera keyframes. |
| `boxes/overlaps/zones` | Original AI navigation boxes, connectivity, and movement zones. |
| `animated_textures` | Original animation sequence block of 16-bit values. |
| `sprite_textures` | Sprite bounds, original tile/CLUT, UV, normalized UV in `sprite_atlas.png`. |
| `sprites/sprite_sequences` | Original packed sprite and object-to-sprite sequence tables. |
| `sound_map/sound_info/audio` | Game sound mapping, sample metadata, and standalone WAV paths. |
| `demo_data` | Original recorded demo controls retained as bytes. |

**UV ordering:** texture `uv` and `atlas_uv` retain PSX strip order `[top-left,top-right,bottom-left,bottom-right]` (possibly rotated/flipped in the original). For a quad's exported perimeter vertex order, use UV indices **`[0,1,3,2]`**. For triangles, use `[0,1,2]`. This also applies to sprite rectangles. Atlas rectangles have one pixel of duplicated-edge padding.

Texture indices below 256 are color swatches used by colored object polygons. Their first texel provides the original material color. RGB555 color zero is transparent, while black with bit 15 set is opaque. PSX semitransparent blend modes require renderer support beyond the atlas alpha channel. Texture `attribute` is retained.

Room geometry uses a 32-bit vertex word: brightness bits 0–7, attributes bits 8–15, Z bits 16–20 times 1,024, Y bits 21–25 times 256 plus room top, X bits 26–30 times 1,024. Room quad vertex references are byte offsets divided by four. Mesh references are divided by eight. Room quads need a strip-to-perimeter index swap; TR2 object mesh quads are already in perimeter order. Treating these the same produces crossed polygons on Lara and other objects.

## Skeletons and animation

`models[].joints` contains `{parent, translation}` in mesh order. Root parent is `-1`. Child translations preserve TR2 coordinates. The source bone stack uses bit 0 = pop and bit 1 = push; these are resolved into ordinary parents during extraction. Model node offsets index 32-bit words, not bytes. Mesh part `j` is `meshes[mesh_indices[model.mesh_start+j]]`.

`initial_pose` and every `animations[].poses[]` contain:

```text
bounds = [minX,maxX,minY,maxY,minZ,maxZ]
root = [x,y,z]
angles = [[angleX,angleY,angleZ], ...one per mesh part]
```

Angles are integers in units of 1/1,024 turn; compose source rotations as **Y then X then Z**. Apply the root offset before root rotation. Each child's transform is parent transform × child translation × joint rotation. Convert source coordinates consistently for translation and rotation. `initial_pose` is the model's referenced frame, not necessarily the standing pose.

Each animation's `frame_rate` is the spacing in 30 Hz game ticks between its stored keyframes. The source frame range is inclusive. Keyframe count is `(frame_end-frame_start)/max(1,frame_rate)+1` with integer division. `next_animation` and `next_frame` are absolute original table indices/frame numbers. Speed and acceleration are signed 16.16 fixed-point values. State/range dispatch and animation commands are retained separately and require game systems to act on them.

## Collision and floor-data semantics

Sector index is `local_sector_x * room.z_sectors + local_sector_z`. `floor` and `ceiling` are already expanded by 256. `above`/`below` use 255 for no link. `floor_data=0` means no command chain. Base `-32,768` is the wall/no-height sentinel from source signed byte `-128`.

A floor command header has function in bits 0–4, triangle information in 5–7, sub/type in 8–14, and end-of-chain in bit 15. Function 1 consumes the next word as horizontal portal room. Function 2 consumes floor slant; 3 consumes ceiling slant. In the following slant word, low byte is signed X and high byte signed Z. Function 4 is a trigger: consume trigger settings, then trigger actions through their end bit. Function 5 is lava/death; 6 is climb flags. Trigger settings contain timer bits 0–7, once bit 8, mask bits 9–13. Action word contains argument bits 0–9, action bits 10–14, end bit 15. Camera actions consume an additional settings word.

For position remainders `dx=x&1023` and `dz=z&1023`, the source floor slant adjustment is:

```text
floor -= (sx * (dx-1023 if sx>0 else dx)) >> 2
floor -= (sz * (dz-1023 if sz>0 else dz)) >> 2
```

Ceiling slant adjustment is:

```text
ceiling -= (sx * (dx-1023 if sx<0 else dx)) >> 2
ceiling += (sz * (dz-1023 if sz>0 else dz)) >> 2
```

Room resolution first follows horizontal floor-data portals, then vertical links according to the current Y. Floor surface calculation follows room-below links down to the surface sector; ceiling calculation follows room-above links up. A complete collision system must also apply animated object surfaces and static collision boxes.

## Audio and screens

`audio/NNN.wav` is signed 16-bit PCM mono at 8,000 Hz. Samples are decoded from 16-byte PSX SPU ADPCM blocks with original predictor state, stopping at end flags. The original packed samples and sample offsets are retained as `audio.vag` and JSON fields. The 8 kHz rate is corroborated by [OpenLara `src/level.h`, TR2 PSX sound pitch correction](https://github.com/XProger/OpenLara/blob/master/src/level.h). Loop metadata and sound variation flags remain in the source sound tables and need runtime handling. WAV generation does not extract Red Book music tracks or FMV sound.

Screen RAW files are headerless RNC2 data decompressing into little-endian RGB555. Screens decode to 384×256 except `PIX/WARNING.RAW` which is 640×256. This larger screen exceeds OpenLara's hardcoded 512×256 buffer assumption; the Python decoder accepts the actual complete stream with checked bounds. `screens/manifest.json` preserves source hashes and sizes. The US title is `screens/pixus_titleus.png`. Generic `pix_title.png` was visually inspected and contains the original Tomb Raider II title artwork.

## Pain points and remaining unknowns

- Shell network DNS was unavailable for source retrieval. The web tool could read primary GitHub sources; no external executable/library was downloaded.
- `.PSX` here is a level container, not a PS-X executable. A generic executable decoder is insufficient.
- Most array counts are 32-bit, but room and camera-frame counts are 16-bit. Confusing these initially failed on a scene with nonempty cutscene camera data; correcting the width resolved it.
- Demo recording bytes follow the camera frames, prefixed by a 32-bit size. BOAT, PLAYABLE, KEEL and SKIDOO contain substantial recordings; all other files still contain the zero size. Preserving this block permits exact final file-boundary validation.
- PSX room and object mesh quad conventions differ, and texture UVs use strip order. This was checked against real Lara mesh vertices and corrected before final asset delivery.
- Some inventory meshes have no valid bone tree. Their joints attach to the root at zero offset, matching the source fallback's lack of bone translation.
- Semitransparency, full sound loop semantics, scripted object behavior, cinematic transforms, and demo playback are retained as data but require runtime implementation. Structural extraction is complete; these behaviors are not established by asset decoding alone.

## Runtime interaction implementation boundary

`game/src/interactions.rs` consumes original placement indices and floor-data actions. It handles item collection, typed pickup-pack inventory, small/large healing, matching key/puzzle consumption, switch and pad activation masks, timed activation, original level-exit actions, logical music requests, and secret counting. Door blockers derive their extents from decoded mesh vertices; spikes use the source falling/running damage distinction. Pistol hits consume a monotonic shot counter once and require a sector line trace. Tiger and spider health values come from the primary setup sources; tiger state IDs, turn limit and bite damage come from OpenLara's tiger controller.

The current interaction implementation does **not** reproduce the complete original creature state machine or collision-sphere contact masks. Tiger/spider pursuit uses decoded clip velocity and floor/step checks. Spider attack selection/damage remain provisional. Navigation box path finding, humanoid gun AI, bosses, vehicles, timed attack windows, pickup/use animations, switch alignment animation, dynamic trapdoors/bridges/falling floors, heavy actors, flipmaps, cinematic cameras, and bespoke trigger effects remain unfinished. Unsupported trigger action IDs are exposed in `InteractionProgress.unsupported_actions`; they are not silently converted into other behavior.

A camera trigger's extra settings word carries the end bit. Actual WALL chain at floor-data offset 3391 is `[2,64768,32772,15872,1026,33024]`: floor slope `(0,-3)` followed only by camera 2. The word 33024 (`0x8100`) is camera settings and end-of-list, **not** object activation 256. Runtime parsers must consume and test this operand. This is covered by an extracted-content regression test.

Primary interaction references: [TR2Main object IDs](https://github.com/Arsunt/TR2Main/blob/master/global/types.h), [TR2Main creature setup](https://github.com/Arsunt/TR2Main/blob/master/game/setup.cpp), [OpenLara tiger controller](https://github.com/XProger/OpenLara/blob/master/src/enemy.h), [OpenLara floor and interaction bounds](https://github.com/XProger/OpenLara/blob/master/src/format.h), and [OpenLara spikes controller](https://github.com/XProger/OpenLara/blob/master/src/objects.h). These guide data and rule interpretation; the implementation is native Bevy ECS.
