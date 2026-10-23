# Inventory ring: PSX evidence and port

Only the supplied USA Rev 3 disc and `psx` were used. Disassembly and
Ghidra pseudocode were taken from Wall runtime RAM (`.psx/wall_game`,
`validation/inventory/trace_open`); the item table also matches
`TOMB2.EXE` on the disc.

## What Lara carries

Two lists of item records and counts: the main ring (`0x80088AD8`, counts
`0x80088AA8`, size `0x80088AA0`) and the keys ring (`0x80088B6C`,
`0x80088B3C`, `0x80088B34`). Ammunition for guns Lara has lives in Lara's
info (`0x8008C5AC` autos, `0x8008C5B0` uzis, `0x8008C5B4` shotgun,
`0x8008C5B8` harpoon, `0x8008C5BC` grenades, `0x8008C5C0` M16).

- Inv_GetItemOption `0x80034894` maps placed objects to ring items
  (135→157 pistols … 151/152→173 flares, 174..177→178..181 puzzles,
  193..196→197..200 keys, 205/206→207/208 pickups; pistol clips none).
- Inv_AddItem `0x80033A70`: an item Lara has gains one (six for a box of
  flares, 151). A new gun converts the ammunition boxes she carried for it
  (shotgun 12 rounds a box, autos 40, uzis 80, harpoon 3, M16 40, grenades
  2) and adds one box's worth; ammunition for a gun she has goes into the
  gun, otherwise it is a ring item. Secrets 190..192 set bits of
  `0x800DE682`. New items go in by Inv_InsertItem `0x80034284`: sorted by
  the record's ring position, below 100 on the main ring.
- Inv_RemoveItem `0x8003462C` and Inv_RequestItem `0x80034530` take one
  away and count.
- The Wall start in a new game (`.psx/use_wall_game`): statistics, pistols,
  shotgun with 12 rounds, 2 flares, a large and a small medi pack. The
  original builds it from the gameflow script's ADD2INV entries
  (`0x8002E2E8`, applied by `0x8002E468`), which is not ported;
  `Inventory::new_game` reproduces the observed result.

The 37 item records (`0x80087FCC`, 0x54 bytes) are `game/data/inventory.json`:
model, opening frames, selected tilt/turns/offset, joint mask and ring
position. Names come from the gameflow script.

## The gameflow script

`/DATA/TOMBPSX.DAT`, read by `0x8006BBC8`: version 3, a 256-byte
description, a 128-byte header (counts at +0x40, flags +0x70, XOR key
+0x78), then string tables `[u16 offsets][u16 size][bytes]` XOR'd with the
key (`0x8006BA94`): level names, pictures, titles, FMVs, level files,
cutscenes, the sequence script, demo levels, FMV info, 89 game strings, 80
PlayStation strings, then four puzzle, two pickup and four key name tables
by level. `tools/import/src/gameflow.rs` imports it as `gameflow.json`;
the parser consumes all 6701 bytes. The ring's item text picks the level's
table for puzzles, pickups and keys (`0x80033534..0x800336C0`).

## The ring

Display_Inventory `0x80030F30(mode)`: 0 from Select in play, 2 from a
keyhole or puzzle hole, 1 at the title and 5 at game over (see
[save-evidence.md](save-evidence.md)); 3/4 (save/load), which nothing in
the game asks for, are not ported. Its ring and motion structs live on the stack (`0x801FFDB8`,
`0x801FFD80` from play); templates at `0x800890A8` and `0x80089074`.

Per pass of its loop:

1. RingCalcAdders `0x80034C4C` (24 steps per item turn).
2. The pad reader (`0x80069320`, storing `0x8008AAD4` at `0x800697C0`) and
   the repeat stop `0x80032F8C`: a word counts once until all buttons are
   released.
3. RingDoMotions `0x80034CD0` once per vsync the last frame took (2 in
   play; 12 on the first frame after opening from the Wall): each motion
   adds its rates until its count runs out, then snaps to its targets and
   takes its target status; a ring turn adds a 24th of the item angle 24
   times.
4. Camera z = radius + 598.
5. Items (`0x800313F4..0x800315D8`), once per vsync: the front item spins
   +0x100 while the ring rests, turns to its chosen angle in 0x400 steps
   while chosen (downwards whenever the chosen angle is below its own: the
   difference is compared unsigned), unwinds ±0x200 while the ring turns;
   the others unwind ±0x100.
6. Drawing, then the vsync wait, which stores the count it took
   (`0x80031748`).
7. The status jump table `0x800103CC` on this pass's input, unless the
   ring is turning; status 13 leaves.

So a vblank sample shows a pass's state step together with the next
pass's motions and items, and the port's `Ring::frame` is cut the same
way: the last pass's state step, then steps 1-5 of the next.

States (motion status): 0 opening, 1 open, 2 closing, 3/4/5/6 swapping
rings, 7 selecting, 8 selected, 9 deselecting, 10 deselect, 11 closing the
item, 12 exiting, 13 done. Opening: 32 vsyncs, radius 0→688, camera y
-1536→-256, half a turn. Right turns to the previous item, Left to the
next (sound 108). Triangle or Select closes (sound 112; 32 vsyncs back).
Up/Down in play swap to the keys or options ring (24 vsyncs out and in,
camera tilt ±0x2000). Cross brings the front item forward over 16 vsyncs
(MotionItemSelect `0x80035140`; sounds 113 statistics, 114 guns, 109
Lara's Home, 111 others) and runs its opening animation, a frame every
other vsync (`0x80032838`). Then DoInventoryOptions `0x8006EA54`: guns,
medi packs, puzzles, keys and pickups confirm themselves; ammunition does
nothing; flares and the rest close on Cross or Triangle. Confirming closes
the item (status 11), deselects it (16 vsyncs) and closes the ring; the
chosen item is `0x80089032`, the object it shows (120 for the open
passport). UseItem `0x800432D4` then asks for the gun
(`0x8008C4EE`) or heals 500/1000 when health is 1..999 (sound 116).

KeyHoleCollision `0x80050F80` (and PuzzleHoleCollision `0x800512CC`):
Action with no item chosen calls Display_Inventory(2) inside Lara's
collision; with no keys it returns at once. The chosen item is tested in
the same call: a fitting key is removed and used, anything else plays
sound 2 once per position. The chosen item is cleared there only.

The game loop opens the ring on Select (`0x8001DAC8`) unless Lara is dying
(`0x8008C502`) or in an extra-animation state (`0x8008C510 & 0x40000`).

Drawing (`0x800328E8`, `0x80034B64`, `0x80034C00`, `0x80032FF4`): the
backdrop is object 223's mesh 12288 below a camera at y -512 looking down,
turned (y 0, x 0x4000, z -0x8000), posed by its first frame and turned a
half turn about y. It ripples: `0x8006759C` with flag 1 moves each vertex
of its 9 by 13 grid across by `sin/cos(c) * 0x140 >> 14` and shades it
`(sin(a) << 4 >> 14) + 16 + (sin(b) << 4 >> 14) + 16 + edge` (the bytes at
`0x80089A88`: 0 on the border, 0x10 one in, 0x20 inside), the gouraud
colour itself (128 neutral). The angles start from three phases
(`0x80089AFE`/`B00`/`B02`, 0/0/0x4000 at power-on), step 0x1422/0x33E7/
0x33E7 along a row and 0x1822/0x3000/0x3000 down the rows, and the phases
move -0x200/-0x267/-0x267 per pass. All 63 vertices of the GPU capture
(`validation/inventory/gpu_ring`) match pass 109 exactly, the screen's
columns running along the grid's 13-vertex rows;
items sit at the ring's radius, tilt by `pt_xrot` and take their offset
and turns; the camera looks at (0, -96, radius), plus the swap tilt; light
from (-1536, 256, 1024). The camera pitch tilts the view down for a
positive pitch (the title's 0x400 puts its items where the original
draws them; the sign was reversed before). Text (`0x800331D4`, `0x800334DC`): heading at
y 26 centred (INVENTORY/OPTION/ITEMS), the font's up and down arrows `[`
and `]` at (±20, 28) from the top corners and (±20, -15) from the bottom
ones, the item name 16 above the bottom centred, "> Select" and "; Go
Back" (`>` and `;` are the Cross and Triangle symbols) at (±25, -40) from
the bottom corners, count at (64, -56) (not seen in a capture) (gun rounds `%5d`, shotgun in boxes of six,
clips doubled, medi packs and keys from 2), the health bar while a medi
pack is in front.

## Recordings

`tools/record_inventory.py` records three no-poke routes from
`.psx/use_wall_game`, sampling the ring and motion structs, the six main
ring items and the globals every vsync, and watching the frame-length
store (`0x80031158` at opening, `0x80031748` at each frame's end) and the
pad reader's input store (`0x800697C0`) to cut exact ring frames:

| Case | Frames | Covers |
|---|---|---|
| `browse` | 253 | turns both ways, the options ring and back, pistols chosen |
| `stats` | 196 | statistics chosen and put back, a turn, closing |
| `medipacks` | 246 | both medi packs at full health, reopening |
| `passport_heldout` | 216 | held out: the options ring, the passport opened and closed with Triangle |

`game/tests/inventory.rs` replays every frame through `Ring::frame` and
compares ring, motion and item fields exactly; `passport_heldout` was
recorded after the port was written and found the open passport's own
handler (`0x8006EB58`, dispatched on the item's current model 120 through
`0x80010E28`): its pages lie five frames apart from the opening frame,
Left/Right turn them (sound 115) and Triangle runs it back to frame 0 (on
to the last frame from the exit page) before it is put back. The same file
checks the backdrop's greys and four vertices' view positions against the
GPU capture (within 40 units of 12800; measured 18, the original's
fixed-point matrices).

## Port

`game/src/inventory/`: `Inventory` and `ItemTable` (rules), `ring.rs`
(Display_Inventory's logic, one `frame` per logic tick of two vsyncs),
`view.rs` (backdrop, items, text). The ring opens at the start of a tick
and the world's rules wait until the tick it closes on, which then runs in
full. A keyhole asks for the ring during its tick; the ring opens on the
next and the keyhole tests the choice on the tick it closes, so Lara's
sampled states after the ring are those of the recordings
(`game/tests/using.rs`).

Started with `--level`, Lara carries the Wall's new-game inventory
(statistics only at Lara's Home), as from the title.

Checked visually with the developer MCP against the PSX frames: backdrop,
item placement and size, the options ring and the open passport agree.
The items' light (ambient and one direction from (-1536, 256, 1024)) is
not measured.

## Text

The font is object 255's sprite sequence in each level file (110 sprites;
the Wall's from sprite 73, as `0x800966A2` holds at run time), imported
with the level's other sprites into its atlas. T_Print `0x8005C7AC` adds a
line (letter spacing 1, word spacing 6) and T_DrawText `0x8005D104` draws
it a sprite per character through the character map at `0x80082F54`
(codes to 10 and 11..18 computed) and the widths at `0x80082F04`; digits
keep to twelve-pixel cells and `(`, `)`, `$` and `~` overlay the next
character. Centring and right alignment use the width from `0x8005CC3C`,
which tests the glyph number rather than the code for digits (w..z and
0..5 count twelve) and rounds down to even. The sprite drawer `0x800697CC`
places each sprite's rectangle from the pen (the baseline). `game/src/text.rs`
ports this; its test reproduces every glyph of the capture's heading,
name, arrows and prompts.

The pause and the passport's requesters use the font through
`ScreenLines`, with their boxes ([save-evidence.md](save-evidence.md)).
Before a level's font is loaded the template's text stands in.

Not ported: the statistics, sound, screen adjust and controls pages (the
template's controls menu stands in for the last), the save and load modes
and the memory card's screens, demo and "controller removed" handling,
flares and guns
other than the pistols (choosing them does nothing), the stopwatch hands,
the large button symbols (codes 0x7F..0x81), converting placed shotgun pickups to
ammunition (`0x80037D78`), the level-start inventory from the gameflow
script, and Lara's last/back gun updates on new guns.
