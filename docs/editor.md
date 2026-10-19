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

The editor starts with an Objects list and a room overview. The **Objects**,
**Rooms**, **Cameras**, and **Animations** shortcuts jump directly to those
parts of the scene; the active section is highlighted. Object rows show their
type name (when registry metadata is supplied) and room. Click an object in
the list or the viewport to inspect it. **Frame** focuses the selection;
**Overview** returns to the first room overview.

The object inspector edits position with +/-128-unit buttons or by clicking
the number, heading with +/-45-degree buttons or a degree value, room with a
number field, and initial hidden state with a checkbox. Coordinates are the
level format's units, with Y increasing downward. Edits are validated and
undoable, including adding an optional hidden field that was absent from the
original JSON. Scalar numbers and strings use ordinary values; strings need
no JSON quotes. Boolean fields toggle directly.

**All data** retains the field browser for every field of `engine::world::format::Level`: rooms,
vertices, faces, portals, sectors and floor data; placements and activation
flags; models, joints and animations; fixed cameras and navigation; texture
coordinates and sound tables; and source provenance. Browse nested tables,
then select a scalar or use Edit value on a whole table for advanced JSON editing. Replacing a table also
allows adding and removing entries. Whole-document validation runs before a
change is accepted. Unknown JSON properties survive edits and saves.

| Control | Action |
|---|---|
| Click a row / Enter / Right | Open a table or edit a scalar |
| Up / Down | Select a row |
| Left / Backspace / Parent | Parent table |
| Page Up / Page Down / Previous / Next | Browse large tables |
| E in the field browser / Edit JSON | Edit selected value or table as JSON |
| Ctrl+A | Select the draft text; typing replaces it |
| Backspace | Remove the last character or selected draft |
| Enter / Apply | Validate and apply draft |
| Escape / Cancel | Discard draft |
| Ctrl+S / Save | Save |
| Ctrl+Shift+S / Save copy | Browse a destination folder and choose a new filename |
| Ctrl+O / Open | Browse another level; unsaved work offers Save / Discard / Cancel |
| Click a folder / JSON file in Open | Browse folders / open the file |
| Type in Open / Up, Down, Enter | Filter names / select and open a result |
| Ctrl+L in a file dialog | Optionally type a location |
| Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z | Undo / redo |
| F / Frame | Frame selected room, placement or camera |
| Click the 3D view | Select the nearest room or placed model and focus camera controls |
| Hold right mouse + drag | Look around |
| Middle mouse + drag / mouse wheel | Pan / zoom |
| WASD, Q/E with viewport focused | Fly camera, descend/ascend (right mouse is optional) |
| Shift while flying | Move faster |
| Ctrl+Q / window close | Quit; unsaved work offers Save / Discard / Cancel |
| Ctrl+Shift+Q | Discard unsaved changes and quit |

The viewport starts focused. Click the field browser to use its keyboard
navigation; click the viewport to return to flying. E ascends while the
viewport is focused and opens a JSON draft while the field browser is focused.
Values, file selection and advanced JSON drafts use modal dialogs that capture
typing and block camera movement. Open is browse-first: click the current
folder, Project, Home, a nearby level library, or a recent level. Library
shortcuts discover `<crate>/assets/levels` folders without a game dependency.
Recent levels last for the current session. Click breadcrumb folders or Up
to navigate; search filters names and folders come before files. The results
are paged, with Up/Down and Enter as keyboard alternatives. Type location /
Ctrl+L is optional; relative locations resolve inside the browsed folder.
Invalid files keep the browser open and retain the current document.

Save copy uses the same folder browser and suggests an unused `*-copy.json`
filename. Enter a name, rather than a full path; `.json` is added when no
extension is provided. Existing files are protected. Unsaved-work dialogs
let you save and continue, discard, or cancel. When opening, Discard keeps
the old document available until another valid level actually opens, so a
failed or canceled browse still preserves the draft. A failed save also
keeps the confirmation and document available.

Viewport selection uses the rendered meshes, including posed model parts,
and opens the corresponding placement or room in the field browser. A yellow
bounding box marks the selection. Static scenery selects its containing room;
individual vertices, faces, unrendered placements and fixed camera markers
remain accessible through the field browser.

Draft input currently supports replacement and appending, rather than a
full text editor with cursor movement or clipboard integration. Geometry is
edited through its fields; drag gizmos, dedicated
sector painting and a graphical timeline are future work. Sound tables are
editable, but audio playback and atlas/image editing belong to future tools.
There are no authored tuning RON assets in the current port; add an authoring
surface when those formats become used, rather than inventing a controller
schema for the editor.

## Animation preview

Select an entry under `animations` (or a placement with authored clips), then
click **Preview selection** beneath the viewport. The editor frames the first
placed model using that clip and isolates it. **Play / Pause** runs authored
frames at 30 Hz; **Step** advances one frame including dispatch/end links;
**Frame -** seeks backward within the current clip. **Next goal** cycles the
states of the selected model. **Scene / Isolate** restores the surrounding
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
Undo/redo records field replacements.

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
Open again after saving your work elsewhere. Save copy requires a new filename
and writes just the JSON; atlas and audio files remain in their level folder.
Copy that folder first when making a separately playable level. The game
loads `levels/<name>/level.json`, so use that layout for authored levels.

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
checks no-op save, edit, undo, redo, invalid references, save, and reopen in a
new process, and records real 1280x800 screenshots and the MCP transcript.
It never edits the source level. Headless UI tests cover keyboard drafts and
mouse hit testing on row and button labels, including Apply, Cancel and Undo.
The editor enables Bevy's `ui_picking` feature so pointer events reach both
the field browser and animation controls, and mesh ray casting for viewport
selection. Tests cover camera focus, draft capture, the file browser and
selection through model hierarchies, including mirrored geometry and occlusion.
Smoke screenshots include the object inspector, Open, the level library,
Save copy, and the unsaved-work confirmation. Input tests exercise real UI hit
testing for scene navigation and position nudges, search and keyboard paging,
degree conversion, optional checkbox fields, save copies, cancellation and
external-file conflicts. All 25 baked levels pass the editor
validator in `level_assets.rs`.

For automation:

```sh
cargo run -p editor --all-features -- --level engine/assets/levels/sandbox/level.json --mcp --headless --software
```

Use `game_query` on the exact entity `Editor` with `EditorStatus`. Set
`EditorCommand` fields through `game_set_component`, e.g.
`{"pointer":"/placements/1/position/0","json":"4700","apply":true}`.
`undo`, `redo`, `save`, `frame`, `open`, `save_copy`, and `cancel` are one-shot booleans.
`browse` is a one-shot folder/file path for an open file dialog. Set `pointer` and
`navigate: true` to browse a field. These commands use the UI's document
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
