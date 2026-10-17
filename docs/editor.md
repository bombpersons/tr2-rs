# Level editor

The editor is a normal, full-resolution Bevy application. It uses standard
perspective rendering and `StandardMaterial`, with the level's atlas and
vertex colours. It has no PS1 screen pass, affine texture mapping, vertex
snapping, dithering or colour quantization. The game retains its PS1 rendering.

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

The field browser opens every field of `engine::world::format::Level`: rooms,
vertices, faces, portals, sectors and floor data; placements and activation
flags; models, joints and animations; fixed cameras and navigation; texture
coordinates and sound tables; and source provenance. Browse nested tables,
then select a scalar or use Edit JSON on a whole table. Replacing a table also
allows adding and removing entries. Whole-document validation runs before a
change is accepted. Unknown JSON properties survive edits and saves.

| Control | Action |
|---|---|
| Click a row / Enter / Right | Open a table or edit a scalar |
| Up / Down | Select a row |
| Left / Backspace / Parent | Parent table |
| Page Up / Page Down / Previous / Next | Browse large tables |
| E / Edit JSON | Edit selected value or table as JSON |
| Ctrl+A | Select the draft text; typing replaces it |
| Backspace | Remove the last character or selected draft |
| Enter / Apply | Validate and apply draft |
| Escape / Cancel | Discard draft |
| Ctrl+S / Save | Save |
| Ctrl+Shift+S / Save as | Save to a new filename |
| Ctrl+O / Open | Open another level (save or undo changes first) |
| Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z | Undo / redo |
| F / Frame | Frame selected room, placement or camera |
| Hold right mouse + drag | Look around |
| Hold right mouse + WASD, Q/E | Fly camera, descend/ascend |
| Shift while flying | Move faster |
| Ctrl+Q / window close | Quit; unsaved changes block exit |
| Ctrl+Shift+Q | Discard unsaved changes and quit |

Draft input currently supports replacement and appending, rather than a
full text editor with cursor movement or clipboard integration. Geometry is
edited through its fields; viewport selection, transform gizmos, dedicated
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
Open again after saving your work elsewhere. Save as requires a new filename
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
It never edits the source level. Keyboard draft handling also has a headless
UI test. All 25 baked levels pass the editor validator in `level_assets.rs`.

For automation:

```sh
cargo run -p editor --all-features -- --level engine/assets/levels/sandbox/level.json --mcp --headless --software
```

Use `game_query` on the exact entity `Editor` with `EditorStatus`. Set
`EditorCommand` fields through `game_set_component`, e.g.
`{"pointer":"/placements/1/position/0","json":"4700","apply":true}`.
`undo`, `redo`, `save`, and `frame` are one-shot booleans. Set `pointer` and
`navigate: true` to browse a field. These commands use the UI's document
transactions. Headless `game_screenshot(target="screen")` captures the
full-resolution editor; use `target="window"` in a desktop session.

`AnimationPreview` is a reflected component on the `Editor` entity. Automation
can set `active`, `isolate`, `placement`, `clip`, clip-relative `frame`, `goal`,
`playing`, and the one-shot `step` flag. The smoke script checks a full
sandbox dispatch/hold transition and confirms changed viewport pixels while
the document remains clean.
