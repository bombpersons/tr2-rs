# Level editor design: rooms, tools and lighting

The plan for growing the editor from a field browser with a mesh mode into
a tool for editing and creating levels. [editor.md](editor.md) describes what
exists; this file describes where it is going and why. Update both as pieces
land, and mark each section's status.

## How a level is put together

A level is a set of **rooms**. Each room is a rectangle of 1024-unit
**sectors** (`x_sectors * z_sectors`, x-major) with its own render mesh,
lights, ambient and flags. There is no global map: collision, the camera,
triggers and rendering first decide which room they are in and then look in
that room's grid.

Rooms exist because each sector holds exactly one floor and one ceiling
height. One room cannot express:

- anything with two floors in one column: overhangs, bridges, upper storeys;
- a change of water, wind or ambient light (per-room fields);
- visibility. The renderer walks the portal graph from the avatar and the
  camera (`WorldGeometry::visible_rooms`) and draws only rooms it reaches.

### How rooms join

- **Sideways.** Every room has a ring of solid border sectors. Neighbouring
  rooms overlap by two sectors: A's border ring lies over B's first walkable
  row and the reverse. The doorway sectors in A's border carry
  `floor_info.portal = B`, with the same floor height as B's sector at that
  spot (WALL: room 0 sector (1,7) is room 1 sector (1,1), both floor 0).
  `floor_at` follows the portal when the position falls in that sector.

  ```
  Room A sectors:  [wall][ A  ][ A  ][door]
  Room B sectors:               [door][ B  ][ B  ][wall]
  ```

- **Vertically.** A sector's `below` / `above` link stacks rooms. The upper
  sector's floor equals the lower sector's ceiling (WALL: room 0 sector (8,5)
  over room 77 at height 0). Holes, shafts and multi-storey spaces work this
  way. `floor_at` follows them by height; the collision floor is at the
  bottom of the stack.
- **Render portals.** `room.portals` are polygons, one per opening, used
  only for visibility. They must agree with the sector links; nothing
  derives one from the other yet.

### Other facts the tools must respect

- Rooms may overlap in space when no portal joins them (ASSAULT has 13 such
  pairs). Impossible geometry is a feature to keep.
- `alternate` pairs a room with its flipped version (CATACOMB has 10). The
  engine does not flip rooms yet. Every shipped alternate pairs with one
  room on the same grid, and links never name it: when the level flips, the
  original's room number holds the alternate's data.
- `placement.room` is stored, not derived from position.
- Shipped rooms are at most 20 sectors on a side. The PS1 vertex packing
  limits a room to 32 sectors per axis and its height to 31 * 256 units
  below `top`. The JSON engine has no such limit; a "PS1-compatible" lint can
  warn instead of refusing.
- Navigation boxes (`boxes`, `overlaps`, `zones`) are not rebuilt by any
  editing tool yet.

## Principle: edit the world, keep rooms as the saved format

Rooms stay the file format, so the engine and game are untouched and an
untouched level still saves byte for byte. The editor's tools work in world
space, and room bookkeeping (portals, border rings, sector links,
`placement.room`) is derived or offered as a fix, not typed by hand.

Operations live in `engine::editor` beside `extrude` and `push_wall`, as
functions over the typed `Level` with unit tests. UI lives in `editor/`.
Every operation validates the whole document and refuses with a reason in
the status bar rather than leaving a half-edited level.

## Toolbar (built; Rooms mode dropped)

Rooms mode was to make clicks pick whole rooms, with the room tools as its
actions. Those tools ended up in the Rooms and Map windows (Split and Merge
work on the current room, New room is drawn on the map), so the greyed-out
button was removed and Play took its place.

One strip after the menus, on the same bar, in four groups:

```
[Objects][Mesh][Rooms] | Select Move Rotate Paint | <mode actions> | Snap  Room  Brush
```

1. **Mode** (one of): Objects, Mesh, Rooms. Tab cycles them; the status bar
   still names the mode.
2. **Tool** (one of): Select (1), Move (2), Rotate (3), Paint (4). Letters
   are taken by camera flight (WASD, Q/E). The gizmo shows only the current
   tool's handles. Rotate applies to objects; Paint to mesh faces. A tool
   that does not apply to the mode is greyed out.
3. **Actions** for the mode, as buttons. Each calls the same function as its
   key and menu item, and is greyed out with a reason when it does not apply:
   - Objects: Frame, Select all.
   - Mesh: Raise (+), Lower (-), Push wall (P), Fill, Turn texture (R).
   - Mesh also has Connect and Open hole, since they start from selected
     faces; Disconnect is in the Rooms window.
   - Rooms: New room is on the Map; Split and Merge are in the Rooms window.
4. **Settings**: Snap in object mode (Ctrl inverts while dragging); in mesh
   mode the edited room with Frame room, and the brush swatch.

Drag-type tools (such as New room) are armed by their button and completed
in the view; Esc disarms them and the status bar says what to do next. Labels
are text with the shortcut in the tooltip; icons can come later. The strip
wraps instead of scrolling on narrow windows.

## Texture painting (built, except the items under "Later")

- **Paint is a tool**, not a checkbox. In mesh mode, clicking a face paints
  it and dragging paints every face the cursor crosses. One stroke is one
  undo step.
- **The brush** shows in the toolbar as a swatch and index; clicking it opens
  the Textures window. Choosing a texture sets the brush without changing
  tool.
- **Eyedropper:** Alt-click (any tool in mesh mode) picks a face's texture
  as the brush.
- **Fill selection** paints the selected faces (the current Paint
  selection).
- **Turn texture** (R) cycles a quad's corners (`[v0..v3]` order carries the
  UVs), turning the texture 90 degrees without touching positions or the
  sector derivation. Mirroring would reverse the winding and so the drawn
  side; it needs a mirrored texture-table entry and is left out for now.
- Later: a hover preview of the brush on the face under the cursor; a
  Textures filter and "used in this room"; a vertex-shade brush (see
  lighting).

## Rooms mode (understanding and bookkeeping built; editing planned)

### Understanding (built)

- **Rooms window:** every room with its size, flags (water, wind), ambient,
  neighbours and flip partner. Click to select and frame.
- **Map view:** a top-down view of room rectangles on the sector grid, with
  portals as edges (sideways) and dots (vertical), and a height-range slider
  to slice through stacks.
- **Room outline** in 3D: the current room's box and its portals, drawn
  while the Rooms or Map window is open. (Built as an outline rather than
  the tint first planned: it reads more clearly through walls and leaves
  the preview's materials alone.)

The current room is the one mesh mode edits, so the toolbar, the Rooms
window, the map and clicking faces in mesh mode all choose the same room.

### Bookkeeping

- **Automatic object room (built):** a move changes `placement.room` only
  when looking the room up from the old and the new position (by
  `floor_at`'s rules) gives different answers, so crossing a doorway or a
  vertical link carries the object across and every other move keeps the
  authored room. 57 imported placements sit where a strict lookup would put
  them in another room, so recomputing it outright would rewrite authored
  data. A move into solid ground keeps the room and warns. Lights will do
  the same.
- **Diagnostics (built):** one-sided portals, links without a render
  portal or into solid, mismatched heights across a link, objects in another
  room or in solid, sectors outside or without a navigation box, and boxes
  stale since the level was opened. Imported data breaks most of these on
  purpose, so problems are split into new (since the file was opened) and
  inherited. Fixes: the mirror of a one-sided portal (all 7,478 two-sided
  imported pairs but 6 follow the mirror rule) and an object's room.

### Editing across rooms (Connect, Open hole and Disconnect built)

- **New room (built):** drag a rectangle on the map, choose floor and
  ceiling. The grid gets its solid ring and floor, ceiling and wall faces,
  in the current room's textures, shade and ambient. Drawn beside another
  room's floor it overlaps that room's grid by two sectors, ready for
  Connect. Navigation boxes wait for the rebuild.
- **Connect / open doorway:** select a wall face where two rooms meet. Check
  the two-sector overlap, open the matching sectors in both rooms, set
  `floor_info.portal` both ways, remove the wall faces, add the render
  portal. Disconnect reverses it.
- **Open hole / stack room:** select floor faces over another room (or over
  nothing, creating a room below). Set `below` / `above`, make the heights
  agree, add the vertical portal.
  (Built for a room already below at the floor's height; creating the
  room below belongs with New room.)
- **Split room (built)** along a sector line, or at a height. With **Close
  hole (built)** on part of the resulting hole, that makes an overhang or a
  bridge.
- **Merge (built)** two joined or side-by-side compatible rooms.
- **Push wall** into a neighbouring room offers to connect instead of
  refusing.

### Later

- Rebuild navigation boxes for edited areas. (Built: Diagnostics' Rebuild
  navigation.)
- Flip-room view (built: Flipped shows one state at a time, and
  Diagnostics checks both). Editing `alternate` ("Make alternate" copying a
  room in place; split and merge applied to both versions) waits for the
  game to flip rooms, so authored flips can be seen working.
- A managed-region mode (paint columns with several floor/ceiling spans and
  let the editor partition them into rooms) was considered and dropped in
  favour of push and pull below: it cannot keep imported rooms byte for byte.

## Building by push and pull (planned)

The aim is SketchUp-style building: start from a new level, draw on faces,
push and pull them into rooms and corridors, put Lara down and play. Rooms
stay visible (tint, map, Rooms window) but the editor makes, grows, joins
and splits them as the geometry needs. You can still split, merge or connect
by hand.

Rooms stay the saved format and each gesture is an ordinary edit on the
typed `Level`. That keeps the rest of this file's principle:
- untouched rooms save byte for byte;
- every intermediate state validates and can be played;
- Diagnostics checks each step.

Everything snaps to the game's grid:
- 1024 (one sector) across;
- 256 (one step) in height;
- slopes in whole steps.

Free-angle geometry is out of scope; it cannot carry collision.

### 1. File > New

- **Dialog.** File > New... (Ctrl+N) opens a small dialog with these fields:
  - **Name:** the level's folder name.
  - **Textures:** a texture set from the object library (step 2), each
    shipped level's room textures or the sandbox's (the default, so it works
    without disc data).
  - **Starting room:** size in sectors (default 4 x 4) and height in steps
    (default 8, i.e. 2048).
  - **Add Lara:** checked by default when the library has her.
- **Choosing where it goes.** New asks where to save first, so there is no
  untitled-document state. `LevelDocument::open` needs an existing file, and
  Play names the level by its folder.
- **Before the library.** Until step 2 lands, New uses the sandbox's
  textures and adds no Lara.
- **Engine.** `engine::editor::new_level(&NewLevel) -> Level` starts with no
  models, sounds, cameras or navigation. It takes the texture set's textures,
  then builds one room with `new_room`. With Add Lara, it adds her library
  entry with `add_object` and places her in the middle of the floor, facing
  +z.
- **Files.** The editor writes `level.json`, `atlas.png` and `audio/`, and
  opens the level as Open does (`Editor::open`).
- **Save As fix.** Save As starts copying `audio/` as well as the atlas.
  Play's export also copies `audio/` when the play folder lacks it. Without
  this, a level outside `game/assets/imported/levels` loses its sounds.
- **Checks.**
  - Unit test: a new level from the sandbox set validates and has one open
    room of the asked size.
  - MCP field `new_level: "DEST[,textures,w,d,steps]"`.
  - Smoke: `new_level_smoke` makes a level, saves it, reopens it, and checks
    the room count and Diagnostics.

### 2. Object library

A library of objects (Lara, enemies, pickups, doors, statics) that can be
dropped into any level, and removed again with the assets only they used.

- **An entry** is a folder, `assets/objects/<name>/`, holding:
  - `object.json`: the model or models (type id, mesh list, joints), their
    meshes, their animations with clip links, state changes and commands
    numbered within the entry, and their sound effects keyed by the game's
    effect ids;
  - `textures.png`: only the atlas pieces the entry uses, with UVs into it;
  - `audio/*.wav`: only its sounds;
  - **needs:** the other models the game expects alongside it. Lara needs
    her braid (2), pistols (1) and muzzle flash (240).
- **Texture sets** are entries with only textures: a shipped level's room
  textures, cut out. Painting and New use them.
- **Where the library comes from.** A new step of the disc import
  (`tools/import`) cuts every model, static and room texture set out of the
  imported levels. Like them, it is generated and gitignored, never
  committed. The sandbox's hand-made walker and pillar
  are committed entries, so the library works without the disc.
- **Variants.** The same type id can look different between levels (Lara's
  outfits). Entries are named by the level they came from ("Lara
  (Venice)"), and identical copies are kept once.
- **Needs** come from the game's `object_types.json` (which the editor
  already reads with `--types`), filled in as each object is ported and
  checked. The editor never depends on the game crate. An entry with no
  known needs says so when added.
- **Adding** (`engine::editor::add_object(level, atlas, entry)`, one undo
  step):
  1. Add the meshes, animations and sounds, renumbering into the level's
     arrays.
  2. Pack the texture pieces into free atlas space and add `textures[]`
     entries.
  3. Add the models. If the level already has that type id, skip it (or
     offer to replace it).
  4. Add each need the level lacks the same way.
  5. Copy the WAVs into `audio/`.

  Free atlas space is whatever no texture entry covers, so nothing else
  tracks it. If the 2048 x 2048 atlas fills up, it can be enlarged by
  rescaling every UV; that rewrites the texture table, so only when needed.
- **Removing** (`engine::editor::remove_unused(level, atlas, keep)`):
  1. Drop the models with no placement that are not a need of a model
     still present and not on the game's always-loaded list (`keep`, from
     `object_types.json`).
  2. Drop the meshes, animations, sounds and textures nothing left refers
     to (rooms, statics or remaining models), renumbering the rest.
  3. Clear their atlas pixels, which leaves that space free for the next
     object.

  Deleting the last placement of a type offers this for that type. A whole-
  level **Remove unused assets** command runs it everywhere. On imported
  levels it is only ever offered, never automatic: the game can spawn models
  no placement names (effects, pieces, models triggers swap in), and those
  must be on the always-loaded list before removal is safe there.
- **PNG and WAV import** use the same packing and copying: a PNG becomes a
  texture (and a brush), a WAV a sound in a free effect slot.
- **Limits.** Behaviour belongs to the type id: a wolf acts like a wolf
  because the game runs wolf code for its id. A level cannot hold two
  different models with one id, and new enemy types need game code, not
  only a library entry. Where two entries disagree on an effect id's sound,
  the level's existing sound wins.
- **Checks.**
  - Engine tests: add then remove an entry and get the level back exactly;
    adding the same entry twice adds nothing; removal keeps anything still
    used.
  - Importer test: every exported entry re-added to its own level matches
    the original model's meshes, clips and textures.
  - Smoke: add the walker to a new sandbox level, place it, delete it,
    remove unused, compare with the saved file.

### 3. Placing objects and Lara

- **Add object** (Objects window, key A): pick an entry from the library
  (searchable, with a thumbnail), then click a floor. The entry's assets are
  added if the level lacks them, and the placement goes there:
  - **Position:** the hit point snapped to the sector centre (Shift keeps the
    exact point).
  - **Height and room:** found as `play::spot` finds them.
  - **Heading:** the view's yaw, rounded to 90 degrees.

  One undo step; the new placement is selected, so Move and Rotate fine-tune
  it as usual.
- **Delete** (Del) removes the selected placements, offering to remove the
  assets nothing uses any more (step 2).
- **Place Lara** (key L) is Add object for Lara, except that it moves her
  existing start instead of adding a second one.
- **The marker.** Lara's start always shows an arrow marker, even when the
  level has no model 0 (the sandbox), so you can see where she will be.
- **Play from Lara's start** (Shift+F5, beside Play from view): `export`
  skips `spot` and uses the stored placement as it is.
- **Diagnostics.** New problems for:
  - no Lara start;
  - more than one (the game spawns a Lara for every kind-0 placement);
  - a start inside solid ground;
  - a placement whose model, or one of whose needs, the level lacks.
- **Checks.**
  - MCP fields `add_object: "ENTRY,x,y"` (a viewport click) or
    `"ENTRY,x,y,z,heading"`, `place_lara` likewise, and `delete`.
  - Smoke: place Lara on WALL and check the placement's room and floor
    height, then play-export from the start.

### 4. Push/pull tool

- **The tool.** A new tool, **Push/pull** (key 5), in mesh mode. Like the
  Paint tool, it works on any room's faces.
- **The gesture.**
  - Hover highlights a face. Pressing on it starts a drag along the face's
    normal.
  - The distance snaps to 256 for floors and ceilings and to 1024 for walls,
    and shows as a label ("+3 steps", "2 sectors").
  - Typing a number during the drag sets the distance in those units; Enter
    commits.
  - Release commits, Esc cancels, and each push is one undo step.
  - With faces already selected, pressing on one of them pushes them all.
- **What a push does.** It maps onto existing operations:
  - **Floor or ceiling faces:** `extrude(dy * n)`. Raise and Lower become
    shortcuts for the same thing.
  - **Wall faces outward:** `push_wall` n times, per selected face, with
    walls cut first by `split_walls` so room-wide walls (the sandbox's) push
    one sector at a time.
  - **Wall faces inward (pull):** a new `pull_wall`, the inverse of
    `push_wall`. The sector in front of the wall becomes solid and the wall
    stack moves in. It refuses a sector holding objects, portals or links.
- **Preview.**
  - **First version:** outlines only, as Move does. The faces at the offset
    and the step faces they would make are drawn in orange, with no document
    change.
  - **Second version:** solid. On drag start, cache one typed `Level`. When
    the snapped distance changes (not every frame), run the operation on a
    clone and rebuild only the touched rooms' meshes as a preview overlay,
    hiding the originals. This needs a per-room mesh entry point out of
    `finish_level`'s `MeshBuilder` loop. A full commit (document diff,
    respawn, diagnostics) is too slow to run per frame.
  - **Refusals:** a gesture the operation refuses shows its reason live in
    red and commits nothing.
- **Checks.**
  - Engine tests for multi-step `push_wall`, `pull_wall` and push-then-pull
    round trips, which should restore the room exactly.
  - MCP field `push_pull: "FACE,…;n"`.
  - Smoke: push a sandbox wall out two sectors, pull it back, and compare
    with the saved file.

### 5. Drawing on faces

- **On floors and ceilings,** faces are already one per sector, so a
  rectangle is simply a selection. Drag on them with the Push/pull tool
  (pressing on empty floor, not on a selected face) to select the sector
  faces it covers, then push.
- **On walls,** a rectangle cuts. Drag across a wall to pick sector columns
  along it and a height range in 256 steps. A new `cut_wall(room, plane,
  columns, y0..y1)` splits the wall faces there into pieces above, inside and
  below the range, all still in the same plane. Cutting changes only render
  faces, never sectors.
- **Pushing just the cut piece.** `push_wall` today moves every face stacked
  on that edge. It gains a variant that moves only the given faces:
  - the new sector's floor and ceiling come from that piece's foot and top
    (as now);
  - the pieces left above and below stay as the lintel and the step, as
    `check_walls` already allows between floors and ceilings of different
    heights.
- **Result.** A 1 m (1024) high doorway or corridor in a 2 m wall is then
  one rectangle and one push.
- **Checks.** Engine tests for cut + push on the sandbox wall. Collision
  must find the corridor's floor and ceiling.

### 6. Pushes that cross rooms

The automatic room bookkeeping. Each case maps to an existing operation:

| The push… | Result |
|---|---|
| …of a wall reaches another room's open space at overlapping heights | It stops at the shared plane and runs `connect` (doorway). This replaces "push into a neighbour" refusing (see Editing across rooms). |
| …of a floor down meets the ceiling of the room below exactly | `open_hole`. |
| …of a ceiling up meets the floor of the room above exactly | `open_hole` from above. |
| …would make one of this room's columns hold two separate spaces (a corridor looping over itself, a floor pulled up into a bridge with space under it) | A new room for the moved part, made with `new_room` / `split_height` / `split_along`, then joined. |
| …would pass the PS1 limits (32 sectors per axis, 31 * 256 height) | A new room, joined by a doorway, instead of growing. |
| …goes into a column another room uses at heights that don't meet | Allowed: rooms may overlap in space unjoined. |

- **The status bar** says what happened: "Corridor joined Room 12 through a
  doorway", "New room 40 for the bridge; Ctrl+Z to undo".
- **Locked rooms** never change automatically; pushes into them refuse.
- **Navigation.** Edits leave boxes stale as now. Rebuild navigation runs
  automatically after a push that changed sectors, so play-testing never
  waits on it.
- **Checks.** Engine tests per case. Smoke: on WALL, push between two
  rooms; Diagnostics must show no new problems.

### 7. Keeping rooms visible but out of the way

- **Room tint** (View > Room colours): faces tinted per room, with each
  room's outline in the 3D view and on the Map.
- **Names and locks:** `name: Option<String>` and `locked: bool` on rooms,
  as editor-only fields that the game ignores. They are serialised only when
  set, so untouched levels still save byte for byte. Messages use the name
  when there is one.
- **Status bar:** the room under the cursor and the current one ("Room 12 ·
  Pool hall").
- **Water, wind and ambient stay room settings** in the Rooms window.
  Setting water on part of a room offers to split it at the water's height
  and edge first.

### Decided

- **Units:** sectors (1024) and steps (256), the game's own grid, in labels
  and typed values. Lara is 762 units tall (`PLAYER_HEIGHT`), so a sector is
  roughly 2.3 m at her scale; labels may show that as a hint, but distances
  are never typed in metres.
- **Tools:** Push/pull is its own tool for faces. Move stays for vertices
  and edges, which is how slopes are made.
- **New levels** start with no objects and add them from the library,
  rather than copying a whole base level and pruning it.

## Baked room lighting (planned, after rooms)

Room geometry is lit only by its stored per-vertex shade (`vertices[..][3]`,
low byte, 128 neutral; `lighting::room_vertex_shade`). Room `lights` and
`ambient` currently light only items (`lighting::item_light`). Baking writes
vertex shades from the room's lights, so it adds no format and changes
nothing in the game.

- **Lights are selectable** in the view with the same gizmo, multi-select
  and Properties panel as objects (intensity, falloff, room ambient). A
  marker and falloff sphere show each one. Their room is assigned from
  position.
- **Formula:** start from the item falloff `intensity * f^2 / (d^2 + f^2)`
  with a facing term from the vertex normal. Calibrate by baking imported
  rooms from their own lights and comparing with their shipped shades. The
  original bake tool is not on the disc, so this is a fit, documented as
  such, not a port.
- **Across portals:** gather lights from connected rooms within range, and
  give coincident vertices in neighbouring rooms the same value, so doorways
  have no seam.
- **Shadows (optional):** a per-vertex ray test against room geometry.
  Resolution is the mesh's; a "subdivide for lighting" action could follow.
- **Control:** bake a room, the selection or all, as one undo step, with a
  live uncommitted preview while dragging a light. Combine with the
  vertex-shade brush for touch-ups; a per-vertex lock (an optional field)
  only if re-baking over touch-ups becomes a problem.
- **Toolbar:** a Lights mode (or part of Rooms) with Add light, Bake room,
  Bake all and a lighting-only (untextured) view toggle.

## Order of work

1. Toolbar and Paint tool. (Done.)
2. Rooms window, map view, automatic object room. (Done.)
3. Diagnostics. (Done.)
4. Connect / open hole (done, with Disconnect), new room, split and merge
   (done), close hole (done), flip-room view and checks (done).
5. Navigation box rebuild. (Done: incremental, for missing and stale boxes.)
6. Building by push and pull, in its section's order: File > New, the
   object library (with removal), placing objects and Lara, the Push/pull
   tool, drawing on faces, pushes across rooms, room tint and names.
7. Lights and baking.
