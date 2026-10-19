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

One strip under the menu bar, in four groups:

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
- Possibly a managed-region mode for new levels: paint columns with several
  floor/ceiling spans and let the editor partition them into rooms. Most
  ergonomic, most complex, and incompatible with byte-exact preservation, so
  only for new content.

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
6. Lights and baking.
