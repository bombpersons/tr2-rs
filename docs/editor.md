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
cargo run -p editor -- --level game/assets/imported/levels/wall/level.json --types game/data/object_types.json
cargo run -p editor -- --software
cargo run -p editor -- --ui-scale 0.9 --theme "tomb raider"
```

The default opens the hand-authored engine sandbox; no disc, imported game
data or game crate is needed. `--level` takes a file path. The atlas is `atlas.png`
next to the level file. `--types` optionally reads `{"types":[{"id":1,"name":"walker"}]}`
metadata through the engine registry. Extra controller-specific fields in a
game's type table are ignored. The editor never runs the registered controllers.
The interface size and theme you choose in the View menu are remembered in
`<config dir>/level-editor/preferences.json` (`$XDG_CONFIG_HOME` or
`~/.config`, or `%APPDATA%` on Windows). `--ui-scale` (0.5 to 2) and `--theme`
(Tomb Raider, Dark, Light or Orange; Dark by default) override them for one
run. A missing or unreadable file, or a bad field, falls back to the
defaults. `--headless` runs neither read nor write it.

## Current authoring surface

The 3D view fills the window. One bar along the top holds the **File / Edit /
View** menus followed by the toolbar (it wraps onto a second row when the
window is narrow). A status bar along the bottom shows the mode, the last
message and the level's name (hover it for the path), and the tool windows
float over the view. The theme is compact: small text, slim title bars and
tight spacing, so the windows cover as little of the view as possible.
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
- **Textures** shows the texture table cut from the atlas, for painting faces
  in mesh mode (see "Mesh editing").
- **Rooms** (View > Rooms) lists every room with its grid size, height range,
  object count and flags. Click one to make it the current room (the one
  mesh mode edits); double-click frames it. Below the list, the current
  room's water and wind flags and ambient shade are editable, and its
  portals ("-> 4" a doorway, "v 77" a hole in the floor, "^ 13" an opening
  in the ceiling), flip partner and objects are buttons that jump to them.
  The **x** beside a portal disconnects that room: every doorway and hole
  between the two closes (see "Joining rooms"). **Split and merge** splits
  the current room across x or z at a sector line, or at a height, with the
  planned cut drawn in magenta in the 3D view, and merges it with another
  room (see "Splitting and merging rooms").
- **Map** (View > Map) draws the rooms from above: open sectors filled (blue
  for water rooms), the current room outlined in yellow and the rooms its
  portals open into in cyan, its doorways as green lines and its floor and
  ceiling openings as orange and purple boxes, objects as dots (selected in
  yellow), and the camera as a white arrow. Drag to pan and scroll to zoom;
  **Fit** shows everything. Click a room to make it current. Rooms stack
  and overlap, so clicking the same spot again steps to the next room under
  it, highest first; double-click frames it. **Slice** limits the map (and
  its clicks) to rooms overlapping a height range, starting from the current
  room's. While Rooms or Map is open, the 3D view outlines the current
  room's box and its portals in the same colours.
  **New room** arms drawing: drag a rectangle of open floor (a click takes
  one sector; right- or middle-drag pans meanwhile). The drawn floor shows in
  green with the solid border ring it will get, and a row above the map sets
  its floor and ceiling (whole 256-unit steps, starting from the current
  room's usual heights) and says which rooms it shares an edge with, or
  overlaps. **Create** builds it in the current room's style (its usual
  floor, ceiling and wall textures, middle vertex shade and ambient) and
  makes it current. Draw the floor beside another room's floor, sharing an
  edge, and the two grids overlap by two sectors as imported neighbours do,
  so Connect can join them (see "Joining rooms"). New rooms have no
  navigation boxes yet, which Diagnostics reports.
- **Flipped** (in the toolbar and on the map, on levels with flip rooms)
  shows the level as it is once it flips. A room's `alternate` is a second
  version of it, such as a flooded or drained pool, that the game swaps in
  for it. Shipped alternates sit on the same grid as their room, and
  nothing links to them by number: their doorways and portals name the
  rooms their originals do, and once the level flips the original's number
  holds the alternate. So the editor shows one version at a time. Without
  Flipped, alternates are hidden from the 3D view and the map (and dimmed
  in the Rooms list); with it, the rooms they replace are. Objects stay
  shown when their room flips. Choosing an alternate anywhere (the Rooms
  list, its "Alternate of" and "Flips to" links, the toolbar's room, a
  Diagnostics row) switches to the state it is in, and turning Flipped on
  or off moves the current room to the version shown. Showing a state
  never changes the file.
- **Diagnostics** (View > Diagnostics, or the status bar's "N new problems")
  checks how rooms join, where objects stand and the navigation boxes:
  one-sided portals; doorway, floor and ceiling links with no render portal,
  or into solid ground; heights that differ across a link; objects standing
  in another room or in solid ground; sectors outside their navigation box
  or open without one; and boxes left stale by a floor that changed since the
  level was opened. Imported levels break most of these rules on purpose
  (WALL has 744 such problems, many of them doorways into rooms on another
  layer), so problems the file had when it was opened are counted as
  inherited and hidden until **Show inherited** is ticked; the status bar
  counts only new ones. **Go to** selects the object, or chooses and frames
  the room (and in mesh mode selects the sector's floor). **Fix** appears
  where only one fix makes sense: adding the mirror of a one-sided portal to
  the other room (the way imported portal pairs are built: same opening,
  opposite normal, reversed winding, one unit short of the plane on the
  positive side), or setting an object's room to the one it stands in.
  **Fix all new** applies every fix offered for new problems as one undo
  step. There is no fix for heights. **Rebuild navigation** (see below)
  resolves every navigation-box problem, new or inherited, in one step.
  Flip rooms are checked in both states. Problems found only once the level
  flips (in an alternate, or in a room next to one) read "When flipped:" and
  are listed against the alternate when they are in its data; they offer no
  fix, since a fix would change the level before it flips too. A separate
  check reports alternates that do not pair up: on another grid, shared by
  two rooms, with an alternate of their own, or named by a link. No shipped
  level has any of these, and a navigation rebuild still changes nothing in
  any of them.

The **toolbar** has four groups. Each button runs the same function as its
key and menu item, and a button that does not apply is greyed out with the
reason in its tooltip. [editor-design.md](editor-design.md) has the plan it
grows into.

- **Mode:** Objects or Mesh (Tab switches), then **Play** (F5, see "Play
  from view"). The room tools that a Rooms mode was planned for live in the
  Rooms and Map windows instead.
- **Tool:** Select (1) only selects; Move (2) shows the arrows; Rotate (3)
  shows the ring (objects only); Paint (4) paints faces (mesh only).
  Leaving a mode drops a tool that does nothing in the other one, back to
  Move.
- **Actions:** in object mode Frame and Select all; in mesh mode Raise,
  Lower, Push wall, Fill (paint the selected faces) and Turn texture.
- **Settings:** in object mode Snap; in mesh mode the edited room (with
  Frame room) and the brush swatch, which opens the Textures window. Levels
  with flip rooms also get Flipped, in both modes.

In the 3D view, click an object to select it. Shift- or Ctrl-click adds it
to the selection, or removes it. Clicking a room or empty space clears the
selection. Selected objects get boxes drawn through walls; the active one is
yellow. Room shells cull their outward faces, so from outside a room you
look and click straight into it. Imported rooms wind their faces clockwise
as seen from the side they are drawn for (Bevy's back face), so the editor
culls Bevy's front faces; the sandbox is wound the same way. Clicks pass
through a room's outer side to the objects inside. Objects without a rendered model can still
be selected through the Objects window.

With the Move tool, the selection's centre carries arrows. Drag one to move
every selected object along level X (red), height (green, up is negative Y)
or Z (blue). With the Rotate tool it carries a ring instead: drag it to turn
the selection about its centre, which also turns each object's heading.
Moves snap to 256 units (a quarter sector) and turns to 15 degrees while
the toolbar's Snap is on; holding Ctrl while dragging inverts it. A drag shows live and becomes one undo step when you
let go. Moves and turns update the objects in place; other edits rebuild the
preview.

A placement's room is authored data, so moving an object keeps it unless
the move crosses into another room. The editor looks the room up from the
object's old and new positions, the way collision does (doorway sectors,
then the rooms below and above), and changes `room` only when that lookup
changed; the status bar names the new room, and undo restores both. A move
that ends inside solid ground or outside every room keeps the room and
says so. Typing the room yourself is never overridden. The imported levels
keep 57 placements in rooms that a strict lookup would not give them (such
as items floating over water rooms), which is why the room is never simply
recomputed (`engine::editor::moved_room`).

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
| F5 / File > Play from view / toolbar Play | Run the game on the level as it is now, with Lara where the view looks (see "Play from view") |
| Ctrl+Z, Ctrl+Y or Ctrl+Shift+Z | Undo / redo (also under Edit) |
| Ctrl+A / Edit > Select all objects | Select every placement |
| Tab / Edit > Mesh mode | Switch between object and mesh mode (after clicking the view) |
| 1 / 2 / 3 / 4 | Select / Move / Rotate / Paint tool (after clicking the view) |
| Click / Shift-click / Ctrl-click | Select / add or remove (viewport and Objects window) |
| F / View > Frame selection | Frame the selection (or the Data window's room) |
| View > Rooms / Map | Room list and details / top-down map of the rooms |
| View > Overview | Frame the first room |
| Ctrl + / Ctrl - / Ctrl 0, View > Larger / Smaller / Default size | Interface size, in 10% steps |
| View > Theme | Tomb Raider, Dark, Light or Orange colours |
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

Static scenery belongs to its room. Unrendered placements and fixed camera
markers are reached through the Data window; room geometry is edited in mesh
mode (below). Sound tables are
editable, but audio playback and atlas/image editing belong to future tools.
There are no authored tuning RON assets in the current port; add an authoring
surface when those formats become used, rather than inventing a controller
schema for the editor.

## Play from view

**Play** (the toolbar's ▶, F5 or File > Play from view) runs the game on the
level as it is in the editor, unsaved edits included. Lara starts on the
floor where the centre of the 3D view first meets a room (looking from
inside a room, the floor under the camera), facing the way the view looks.
The level as it starts is used, not the flipped one. The editor writes
`levels/<name>/editor-play.json` under `game/assets` (`<name>` is the
level's folder, so a copy of WALL plays as WALL with its atlas, sky, audio
and place in the level order), never touching the level's own files, and
runs `cargo run -p tomb-raider-ii -- --level <name> --level-file
levels/<name>/editor-play.json` from the workspace, building the game first
if it needs to. Playing again closes the previous game. A level without a
Lara model (the sandbox) cannot be played. Lara's placement moves to the
spot, or one is added if the level has none.

## Mesh editing

Rooms hold two layers: the render mesh (`vertices`, `quads`, `triangles`)
and the sector grid (`sectors`, `floor_infos` and `portals`). Collision,
camera and trigger rules read only the grid (`WorldGeometry::floor_at` and
friends); nothing reads the render mesh but the renderer. Mesh mode edits
the mesh on the grid and re-derives the grid from it, so the two never
disagree.

The rule that links them: a sector's floor (or ceiling) is the one quad
covering exactly that sector whose winding faces up (or down). Its height is
the face's highest floor corner (lowest ceiling corner) and its floor-data
slope is the corner difference across the sector in 256-unit steps, the
inverse of `floor_at`'s slant arithmetic (`engine::editor::plane` and
`corners`, unit-tested against `floor_at`). Every imported level agrees with
this rule except 52 authored faces in CUT4, LIVING, OPERA and RIG (decorative
faces away from their sector's floor, one not flat), which `level_assets.rs`
pins. Sectors without a face of their own, such as doorway sectors, keep
their stored heights.

Press **Tab** (or Edit > Mesh mode) to edit the active object's room, or the
room open in the Data window; the Properties window becomes the **Mesh**
panel, and the status bar shows the mode. The room's sector grid is drawn at
its lowest floor and its vertices as small crosses.

- Click a vertex (within a few pixels) or a face to select it; Shift- or
  Ctrl-click adds or removes. Clicking another room's face switches to that
  room. Esc clears. Faces are hit from the side they are drawn for.
- With the Move tool, drag an arrow to move the selection: x and z move in whole sectors
  (1024 units) and height in 256-unit steps. The move shows in orange while
  you drag and is applied as one undo step when you let go. Moving a vertex
  moves every face that uses it, as imported rooms share corners: raising
  one line of floor corners tilts the sectors on both sides of it.
- **Raise / Lower** (+ / -) extrude the selected floor or ceiling faces by
  256: their corners are split from their neighbours, moved, and joined back
  with step faces seen from the lower side. Steps that would hide inside a
  solid neighbour are left out, and lowering beside a wall only fills what
  the wall does not already cover. Extruding back removes the steps again.
- **Push wall out** (P) opens the solid sector behind a selected wall face
  (one sector wide): the faces stacked on that edge move one sector out, and
  the new sector gets a floor and ceiling at the wall's foot and top, plus
  side walls against solid neighbours. When that sector was on the room's
  border, the grid first grows by a row of solid sectors on that side; growing
  toward negative x or z moves the room origin and re-expresses vertices and
  portals so everything stays in place.
- The **Textures** window (View > Textures, or click the toolbar's brush)
  shows the texture table cut from the atlas. Click one to make it the brush.
  With the **Paint** tool (4), click a face or drag across faces to paint
  them, in any room; the face under the cursor is outlined in cyan. Each face
  shows painted at once, and the whole stroke is one undo step. **Fill**
  (toolbar) or **Fill selection** (Textures window) paints the selected
  faces. **Alt-click** a face with any tool to make its texture the brush.
  Painting sets a quad's fifth (a triangle's fourth) field.
- **Turn texture** (R) turns the selected faces' textures a quarter turn by
  cycling their corners, which carry the texture coordinates; positions,
  winding and the sectors they cover stay as they were. Mirroring would
  reverse the winding and so the side a face is drawn from, so it is not
  offered.

### Joining rooms

Rooms join the way imported levels join them (WALL rooms 0 and 4 for a
doorway, 0 and 77 for a hole), and the Diagnostics window should show no new
problems afterwards.

- **Connect** (toolbar, mesh mode) opens a doorway through each selected wall
  face into the room beyond. The rooms' grids must overlap by two sectors, so
  each room's solid border lies over the other's open floor: that border
  sector becomes the doorway, naming the other room in its floor data and
  taking the heights of the floor under it, so collision walks across. Each
  room keeps wall in the shared plane only where its own space reaches past
  the opening (above it, or a riser up to a higher floor beyond), and a
  mirrored pair of render portals covers the opening, widened as more of the
  same wall opens. Connect refuses a wall with no overlapping room beyond, or
  where the two rooms' spaces do not meet.
- **Open hole** opens each selected floor face into the room below, whose
  ceiling there must be at exactly the floor's height: the floor face and the
  ceiling face below go, the sectors link `below` and `above`, and a mirrored
  pair of horizontal portals covers the hole.
- **Close hole** closes part of a hole, for a bridge or an overhang: select
  the floor faces seen through it from above (they belong to the room below)
  or the ceiling faces seen through it from below (they belong to the room
  above). Each of those sectors gets the upper room's floor and the lower
  room's ceiling back at their stored heights, the links between them go, and
  the portal pair shrinks to the holes left (or goes, with none left). The
  new floor's sector loses the navigation box it shared with the floor
  beneath, so Diagnostics asks for a navigation rebuild. With Split at a
  height, this makes a bridge: split the room where the bridge's surface
  goes, then close the hole wherever the bridge stands.
- **Disconnect** (the **x** in the Rooms window) reverses both: doorway
  sectors become solid border again with full walls, holes get their floor
  and ceiling faces back, and the portals between the two rooms go. On WALL,
  disconnecting rooms and opening them again through the closing faces leaves
  no new diagnostics problems (the smoke test does this for rooms 0 and 3,
  and for the 90-sector hole between rooms 6 and 13).

Closing a hole or disconnecting rooms preserves any floor or ceiling face
already covering the linked sector, including its texture and vertex shades.
Missing faces are restored without adding overlapping duplicates.

### Navigation boxes

Creatures path and the chase camera keeps its eye inside navigation boxes:
rectangles of world sectors with a floor height, joined by overlap lists and
grouped into zones (see `engine::editor::rebuild_navigation` for what the
imported levels show about each). **Rebuild navigation** in the Diagnostics
window rebuilds only what is missing or stale: open sectors with no box,
sectors outside their box, and every sector of a box whose floor changed
since the level was opened. All other boxes keep their bytes; on every
imported level a rebuild changes nothing.

Rebuilt boxes are rectangles of sectors in one room with one floor and
slope (a sloped box takes the slope's lowest corner; the imported data
follows no single rule for sloped boxes). A replaced box's index is reused,
extra boxes are appended, and boxes left over are removed and renumbered.
Overlap lists join a box to the boxes reached by stepping off each of its
edge sectors, through doorways and down holes, at any height difference;
changed lists are written after the existing ones so unchanged boxes keep
their entries. Zones extend the connected areas with the step limits the
imported data shows (1, 2, 3 and 4 steps, and any height for flying, for
both the normal and flipped arrays), merging zones a new box joins. Sectors
over a hole take the box of the floor beneath. Rebuilt boxes lose the
authored blockable and blocked bits that doors use.

### Splitting and merging rooms

A split **across x or z** at a sector line leaves two rooms overlapping by
two sectors, as imported neighbours do: each part's border sector over the
other's open floor becomes a doorway wherever their space meets, and a
mirrored portal pair covers the opening. Where the space does not meet (a
wall stands between), the border stays solid. Each part keeps at least one
sector of open floor. A split **at a height** (a whole 256-unit step) stacks
two rooms over the same grid: every sector reaching across the height
becomes a hole, the upper part's floor and the lower part's ceiling linked
`below` / `above` under a horizontal portal pair; sectors wholly above or
below go to that part. A floor or ceiling sloping across the height is
refused, as are rooms that flip with another room.

Faces crossing the cut are cut in two (positions and shades interpolated).
Everything that pointed at the room follows the part it now belongs to:
other rooms' doorways, holes and portals (clipped at the cut), objects,
cameras and static meshes; lights are copied to both parts, so lit objects
look the same. The part below the line (or the lower part) keeps the room's
number; the other is added at the end.

**Merge** is the inverse: two rooms joined only by doorways along one line
(or sitting side by side unjoined, with no floor open in both), or only by
holes over the same grid, with the same water, wind and flags. Their grids
are united, keeping each sector's open floor over the other's border or
doorway; portals between them go; everything that named the removed room
names the merged one, and later rooms are renumbered.

Checked on WALL, BOAT, CATACOMB, VENICE and OPERA by splitting every room at
its middle across x, across z and at its middle height: in all 1,561
accepted splits collision finds the same floor and ceiling at every open
sector, and every merge restores the room's sectors. 1,550 leave no new
diagnostics problems; 11 report a doorway whose stored heights differ from
the part it now enters, where an imported doorway's heights never matched
its opening (collision is unchanged there). Diagnostics identifies problems
by world place rather than room number, so splitting and merging do not make
a file's own problems look new.

Connect cuts a wall that spans several sectors (the sandbox's walls each
span the room) into one face per sector first, so only the doorway's sector
opens.

The operations are `engine::editor::{connect_faces, open_holes,
close_holes, disconnect, new_room, split_along, split_height, merge_rooms}`.

Each edit re-derives every sector whose floor or ceiling face it touched,
and refuses (with the reason in the status bar, leaving the document as it
was) anything the grid cannot express: a floor that is not flat or evenly
sloped, a slope that is not whole steps, a face no longer covering one
sector, two floors in one sector, a floor above its ceiling, an open sector
on the room border, or a wall face left standing in open floor between two
sectors. That last rule (faces between open sectors may only fill their
height difference) holds for all but 10 faces across the imported levels.
Floor data shared between sectors is copied before its slope changes, and a
sector that loses both its faces becomes solid. Navigation boxes are not
rebuilt, so creature pathing over an edited area keeps its old boxes, and
pushing a wall does not join it to a neighbouring room.

The operations are engine code (`engine::editor::{move_vertices, extrude,
push_wall, paint, rotate_texture}`) over the typed `Level`; `LevelDocument::apply_level`
turns the edited copy into JSON pointer edits for only the fields that
changed, keeping properties the format does not know, as one undo step.

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
and final newline. Edit followed by undo does too. A changed save rewrites
only what changed: every unchanged value keeps its original text, and changed
arrays and objects keep the file's separators, key order and spelling, so a
compact imported level stays compact and an edit to one room leaves every
other room's bytes as they were. Unknown properties are kept because the document holds a JSON tree,
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
python3 tools/editor_smoke.py --level game/assets/imported/levels/wall/level.json --pose-placement 72 --pose-clip 334 --pose-steps 61
```

The smoke script copies the level and atlas under `validation/editor-smoke/`,
checks no-op save, Save As, multi-selection, edit, undo, redo, invalid
references, the unsaved-changes question, clicking a framed object in the 3D
view and Shift-clicking it off with the Select tool (automation's `click`
goes through the same systems as the mouse, so this runs headless), the
Rooms and Map windows with the current room's outline, choosing an alternate
room (which shows the level flipped) and turning Flipped off again (on
levels with flip rooms), carrying an object
through a doorway into the next room (on levels with several rooms) and
undoing it, Diagnostics reporting a deleted return portal as new and Fix all
new restoring it, disconnecting a doorway and a hole and opening them again
with Connect and Open hole (on levels that have them), drawing a room beside
the sandbox room on the map, connecting it and rebuilding navigation,
splitting room 0 across x and at a height and merging it back each time,
Play from view writing its play copy (only Lara's placement differs, and
she stands on an open sector), closing four sectors of a hole from below
and rebuilding navigation (on
levels with holes), mesh mode (raise a floor, a refused unsnapped
move, paint, turn a texture, the Paint tool and brush, the Textures window, push a wall, undo back to the original
bytes, then save a raise and check the file's sectors against its faces and
that every other room kept its bytes), save, and reopen in a new process,
and records real 1280x800 screenshots and the MCP transcript. It never edits
the source level. Unit tests cover click and Shift-click selection in the
viewport (through posed model hierarchies, rooms and empty space) and in the
shared selection model; relative drags and absolute typed values across
several objects as one undo step; gizmo axis moves, snapping and ring turns
about the centre; mesh-mode selection, raising a floor as one undo step,
refused moves, painting and lowering the sandbox block; the engine's sector
derivation, slopes against `floor_at`, extrusion and its cancelling, wall
pushing and grid growth, shared floor data, and format-preserving saves;
JSON drafts keeping their types; Save As replacing a file
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
window. `mode` takes `"mesh"` or `"object"`, `mesh_room` a room number,
`mesh_select` elements such as `"q12,t3,v4"` (quads, triangles, vertices;
`"none"` clears), `mesh_move` an offset `"0,-256,0"`, `mesh_extrude` a
height step (negative raises), `mesh_paint` a texture number for the
selection, and `mesh_push` and `show_textures` are one-shot booleans.
`EditorStatus` reports `selection`, `asking_to_save`, `mode`, `mesh_room`
and `mesh_selection`. These commands use the UI's document
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
Bounded-height corridor queries additionally read the existing box floors and
`zones` arrays. Callers supply their step/drop limits and movement-zone index;
these are query parameters, not new level fields. The flat-only query used
by authoring checks retains its earlier behavior. No editor/game dependency
or persistent-format change is needed.

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

Wading reuses Home Lara's baked gait/start/stop/stand-up clips176–186,190,192;
preview placement8, clip177 to inspect the stride. The per-type `swimming.wading`
tuning lives in the existing game JSON table. Contact and camera elevation are
runtime components, with no new persistent level fields. The editor continues
using ordinary rendering and animation-only previews; sandbox and Home no-op
saves preserve the source bytes.
