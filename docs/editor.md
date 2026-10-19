# Level editor

The editor is a normal, full-resolution Bevy application. It uses standard
perspective rendering and `StandardMaterial`, with the level's atlas and
vertex colours. It has no PS1 screen pass, affine texture mapping, vertex
snapping, dithering or colour quantization. The game retains its PS1 rendering.

Camera-dependent room subdivision is a game-supplied `RoomMeshPolicy` over
the authored room faces. The editor does not install that policy: room
vertices, UVs and shades remain editable in their original engine format,
and its ordinary perspective-correct viewport uses static meshes. No level
fields or save representation were added for subdivision.

Camera-relative backdrops are a game rendering rule over ordinary model and
mesh assets. They add no level field or synthetic placement. The existing
data browser can inspect their model, mesh, pose and texture data; the editor
keeps its ordinary scene rendering.

```sh
cargo run -p editor
cargo run -p editor -- --level game/assets/levels/wall/level.json --types game/data/object_types.json
cargo run -p editor -- --software
```

The default opens the hand-authored engine sandbox; no disc, baked game data
or game crate is needed. `--level` takes a file path. The atlas is `atlas.png`
next to the level file. `--types` optionally reads `{"types":[{"id":1,"name":"walker"}]}`
metadata through the engine registry. Extra controller-specific fields in a
game's type table are ignored. The editor never runs the registered controllers.

## Current authoring surface

The 3D view fills the window. A **File / Edit / View** menu bar sits on top,
a status bar along the bottom, and the tool windows float over the view.
Windows can be moved, resized, collapsed or closed; **View** reopens them.
The UI is egui (`bevy_egui`), drawn by its own camera over the level camera.

- **Objects** lists every placement, grouped by room or by type (registry
  names when `--types` is given). Type in the filter to match a name or an
  exact placement number. Click selects, Shift- or Ctrl-click adds or
  removes, and double-click frames the selection.
- **Properties** shows the active (last selected) object: type, room,
  position, heading in degrees, activation code, inverted and hidden flags,
  and other flag bits. Unknown properties are listed read-only. With several
  objects selected, a typed value is set on all of them, and dragging a
  number moves all of them by the same amount. Click a number to type it.
- **Data** browses every field of `engine::world::format::Level`: rooms,
  vertices, faces, portals, sectors and floor data; placements; models,
  joints and animations; fixed cameras and navigation; texture coordinates
  and sound tables; and source provenance. Click a table to open it, or a
  value to edit it. **Edit as JSON** edits the open table or field as text,
  which also allows adding and removing entries. Tables over 256 KB of JSON
  are edited entry by entry instead. Strings need no quotes, and numbers stay
  numbers.
- **Animation** previews authored clips (see below).

In the 3D view, click an object to select it. Shift- or Ctrl-click adds it
to the selection, or removes it. Clicking a room or empty space clears the
selection. Selected objects get boxes drawn through walls; the active one is
yellow. Room shells cull their outward faces, so from outside a room you
look and click straight into it. Objects without a rendered model can still
be selected through the Objects window.

The selection's centre carries a handle. Drag an arrow to move every selected
object along level X (red), height (green, up is negative Y) or Z (blue).
Drag the ring to turn the selection about its centre, which also turns each
object's heading. Hold Ctrl while dragging to snap to 256 units (a quarter
sector) or 15 degrees. A drag shows live and becomes one undo step when you
let go. Moves and turns update the objects in place; other edits rebuild the
preview.

All edits are validated against the whole document before they are kept,
and an edit that is invalid for any selected object changes none of them.
Undo and redo restore optional fields that an edit added. Coordinates are
the level format's units, with Y increasing downward.

| Control | Action |
|---|---|
| Ctrl+O / File > Open... | Native file dialog; unsaved work asks Save / Discard / Cancel first |
| File > Open project level | Levels found under any `<crate>/assets/levels` in the workspace |
| File > Open recent | Levels opened earlier in this session |
| Ctrl+S / File > Save | Save |
| Ctrl+Shift+S / File > Save As... | Native save dialog; continue editing the new file |
| Ctrl+Z, Ctrl+Y or Ctrl+Shift+Z | Undo / redo (also under Edit) |
| Ctrl+A / Edit > Select all objects | Select every placement |
| Click / Shift-click / Ctrl-click | Select / add or remove (viewport and Objects window) |
| F / View > Frame selection | Frame the selection (or the Data window's room) |
| View > Overview | Frame the first room |
| Hold right mouse + drag | Look around |
| Middle mouse + drag / mouse wheel | Pan / zoom |
| WASD, Q/E (after clicking the view) | Fly, descend/ascend; Shift moves faster |
| Ctrl+Q / window close | Quit; unsaved work asks Save / Discard / Cancel |
| Ctrl+Shift+Q | Discard unsaved changes and quit |

Clicking a window gives it the keyboard; click the 3D view to fly again.
While a text field has focus, Ctrl+Z and Ctrl+A act on the text.

Native dialogs run on their own thread so the view keeps drawing. Save As can
replace an existing file once the dialog has asked about it. When the chosen
folder has no `atlas.png`, the level's atlas is copied there so the copy still
shows its textures. Discarding unsaved work to open another level keeps the
current document until a valid level actually opens; canceling the dialog,
or a level that fails to open, leaves the current work in place.

Static scenery belongs to its room. Individual vertices, faces, unrendered
placements and fixed camera markers are reached through the Data window.
Geometry editing (vertex and face selection, extrusion and texture painting)
is the next planned mode; see "Mesh editing" below. Sound tables are
editable, but audio playback and atlas/image editing belong to future tools.
There are no authored tuning RON assets in the current port; add an authoring
surface when those formats become used, rather than inventing a controller
schema for the editor.

## Mesh editing (planned)

Rooms hold two layers: the render mesh (`vertices`, `quads`, `triangles`)
and the sector grid (`sectors`, floor data and `portals`) used for collision.
Geometry editing will keep them in step rather than move vertices freely.
Selection works on grid-aligned quads and vertices: x and z stay on sector
corners and heights move in 256-unit steps. Each edit re-derives the touched
sectors' floor and ceiling heights and slopes, and rebuilds portals where a
room's boundary changes. Extrude raises or lowers a floor region, or pushes
a wall out to add sectors. A Texture window assigns texture-table entries to
faces. Rooms that are not touched keep saving byte for byte.

## Animation preview

Open an entry under `animations` in the Data window, or select a placement
with authored clips, then click **Preview selection** in the Animation window
(View > Animation). The editor frames the first
placed model using that clip and isolates it. **Play / Pause** runs authored
frames at 30 Hz; **Step** advances one frame including dispatch/end links;
**Frame -** seeks backward within the current clip. **Next goal** cycles the
states of the selected model. **Isolate** toggles the surrounding
level, and **Stop** restores the level's initial preview poses. The caption
shows the current clip, absolute frame, current state and requested goal.

This preview uses only engine animation data and suppresses sound, effects,
movement and game controllers. It never changes level data or dirty state.
Models that have no placement in the level cannot yet be previewed here.

Player traversal uses these existing clips, bounds and end commands without
adding an authoring format. The preview remains an inspection of authored
object animation; it does not simulate player collision or Lara's distinct
goal-persistence rule. See [traversal-evidence.md](traversal-evidence.md).
The hand-authored sandbox has four pillar clips to exercise the workflow
without game assets. [Door animation evidence](door-animation-evidence.md)
describes the PSX checks behind the shared animation step.

## Files and validation

An untouched save reproduces the original bytes exactly, including whitespace
and final newline. Edit followed by undo does too. Changed files use pretty
JSON. Unknown properties are kept because the document holds a JSON tree,
not just a deserialized `Level`. The preview receives a validated typed level
through `engine::render::PreviewLevel`; no temporary saved file is needed.
Each undo step holds every field one action changed, such as a group move.

Validation checks typed fields and format version, mesh indices, sector grid
sizes, room/floor/navigation references, skeleton parents, placements,
trigger masks and engine action references, model clip ranges and sound maps.
The engine permits empty/reversed model clip ranges (including the imported
no-animation marker). Clip links and dispatch targets must reference valid
animations; original jumps outside the destination's ordinary key range are
preserved as signed relative frames. Validation does not claim to prove
unknown game actions or the gameplay meaning of inherited fields.

Save uses a sibling temporary file and an atomic replacement. If the source
changed on disk after opening, Save reports the conflict and keeps the draft.
Open again after saving your work elsewhere. Save As writes the JSON and, when
the destination folder has none, a copy of the atlas; audio files remain in
their level folder. Copy that folder first when making a separately playable
level. The game loads `levels/<name>/level.json`, so use that layout for
authored levels.

The tick-order port changes simulation phases only; it adds no engine asset
fields. Level authoring and standard-rendering preview remain independent of
those controllers (see [tick-order evidence](tick-order-evidence.md)).

## Keeping it current

The dependency direction is `editor -> engine -> psx-bevy`; the editor has no
`game` dependency. Engine document/validation code is in `engine/src/editor`,
while application UI and standard-rendering adaptation live in `editor/`.
Names come from registry metadata. The game-specific type table stays game
content, loaded optionally as data, not linked as code.

When an engine format grows, keep its generic field browser working, extend
its reference validation, add a save/reopen case, and exercise it in the
sandbox before adding a specialised editing tool. Every gameplay trajectory
must continue to pass exactly. Editor rendering is an authoring view; fidelity
comparisons still use the game's PS1 renderer and recorded PSX references.

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features
cargo build -p editor --all-features
python3 tools/editor_smoke.py
python3 tools/editor_smoke.py --level game/assets/levels/wall/level.json --pose-placement 72 --pose-clip 334 --pose-steps 61
```

The smoke script copies the level and atlas under `validation/editor-smoke/`,
checks no-op save, Save As, multi-selection, edit, undo, redo, invalid
references, the unsaved-changes question, save, and reopen in a new process,
and records real 1280x800 screenshots and the MCP transcript. It never edits
the source level. Unit tests cover click and Shift-click selection in the
viewport (through posed model hierarchies, rooms and empty space) and in the
shared selection model; relative drags and absolute typed values across
several objects as one undo step; gizmo axis moves, snapping and ring turns
about the centre; JSON drafts keeping their types; Save As replacing a file
and copying the atlas; and the unsaved-work question. All 25 baked levels
pass the editor validator in `level_assets.rs`.

For automation:

```sh
cargo run -p editor --all-features -- --level engine/assets/levels/sandbox/level.json --mcp --headless --software
```

Use `game_query` on the exact entity `Editor` with `EditorStatus`. Set
`EditorCommand` fields through `game_set_component`, e.g.
`{"pointer":"/placements/1/position/0","json":"4700","apply":true}`.
`undo`, `redo`, `save`, `frame` and `cancel` (dismiss the unsaved-changes
question) are one-shot booleans. `open` and `save_as` take a file path in place
of the native dialogs; `select` takes placement numbers such as `"1,2"`, or
`"none"`. Set `pointer` and `navigate: true` to open a field in the Data
window. `EditorStatus` reports `selection` and `asking_to_save`. These commands use the UI's document
transactions. Headless `game_screenshot(target="screen")` captures the
full-resolution editor; use `target="window"` in a desktop session.

`AnimationPreview` is a reflected component on the `Editor` entity. Automation
can set `active`, `isolate`, `placement`, `clip`, clip-relative `frame`, `goal`,
`playing`, and the one-shot `step` flag. The smoke script checks a full
sandbox dispatch/hold transition and confirms changed viewport pixels while
the document remains clean.

Backward wall/step/ledge/slope collision and hop-fall recovery remain game
controller rules over the same sector and authored animation data. This
milestone adds no editor format or dependency; the existing preview can
inspect clips61/62/93/82 without simulating Lara's collision or input goals.
The no-op save/edit/undo/reopen and animation smoke still passes unchanged.

Home attendant motion uses the existing authored clips280–283 and type260
registry metadata. Preview placement9 in the Home level to inspect its starts,
stops and state goals; local follow thresholds remain in the existing per-type
JSON table. The preview continues to suppress game controllers.

Animated contact reuses the existing clip bounds, joint trees and mesh sphere
fields; no level format changes are needed. Model posing remains shared by
the full-resolution preview and game. The preview does not simulate sphere
contacts or game-specific push decisions.

Navigation corridor queries read the existing `boxes` and `overlaps` fields.
Box maxima are exclusive sector boundaries, matching the existing camera
math. The authoring view and byte-exact save contract retain all navigation
data; game-specific path decisions are not simulated in the editor.

Submerged motion uses Lara's existing authored starts, strokes, glide links
and tread clips. Preview placement8 with clip86 or108 in the Home level.
The new rules add per-type tuning fields in the existing object-type JSON,
without changing persistent level data or making the editor depend on the
game. The preview remains an animation inspector rather than a swim simulator.

Underwater palettes are game presentation policy over the existing atlas and
room water flag. The engine offers optional per-mesh texture overrides while
keeping source lighting; the editor retains ordinary authored materials and
unchanged level files. Home stroke preview and untouched save were checked.

Room policies now receive stable authored corner indices and room identity
for optional animated vertex shading. This metadata is generated in memory
from existing level geometry. The editor keeps static authored shades; Home
tread preview and byte-exact untouched save/reopen still pass.
