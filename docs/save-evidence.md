# The title, saving, loading and game over: PSX evidence and port

Only the supplied USA Rev 3 disc and `psx` were used. Code was read in
Wall runtime RAM (`.psx/wall_game`, Ghidra with gp `0x800878FC`); the
behaviour was recorded from power-on with a formatted memory card in slot 1
(runs kept in `validation/save/`).

## Checkpoints

Memory cards are part of a save state, so these start from power-on with
`memcard1 = new`:

- `.psx/card_boot`: the disc booted 7800 frames, CROSS at 7700 (the
  disc's launcher, the card inserted).
- `.psx/card_title`: from there, CROSS at 3600 and 5400 frames: the first
  title, its ring open on Lara's Home.
- `.psx/card_wall_saved`: from the title, LEFT (passport), CROSS, RIGHT
  (New Game), CROSS, START to skip the FMV, about 5600 frames to the Great
  Wall; there SELECT, DOWN, CROSS, RIGHT, CROSS (Save Game), CROSS, UP,
  CROSS (the card's "Are you sure ?" Yes); then the passport's Load Game,
  CROSS, CROSS on "Current Position" and 1500 frames.

## The front end and the game loop

- The title (`0x8006D534`) loads the save from the card once
  (`0x80068660`, `0x800726EC`), then calls Display_Inventory(1). It
  returns 0 (Lara's Home) for the photograph (object 122), and for the
  passport (`0x80089032` = 120) reads its page (`0x80089034`): page 0 the
  load list's value (`0x80089038`: a level number to play from its start,
  -1000 for the saved position, negative for "Story so far..."), page 1 a
  new game at level 1. Anything else, 0x700, shows the title again.
- The game loop (`0x8001D7FC`, called by `0x8006DD98` from StartGame
  `0x8006DB80`): Lara's death count `0x8008C502` (LaraControl adds one a
  tick from `0x80042B30` once her health is gone) above 300, or above 60
  with any input, sets `0x80087F9C` to 2; the next tick it is 1 and
  Display_Inventory(5) opens. At Lara's Home (level 0) the game returns to
  the title instead. Select opens Display_Inventory(0) a tick after it
  sets `0x80087F9C` to 0. START (input `0x2000`) pauses.
- StartGame reads the passport after the loop returns: page 0 loads, page
  1 at game over restarts the level, page 2 (Exit to Title) and 0x700
  return to the title. Saving (page 1 in play) returns 0 and play goes on.

## Display_Inventory's modes

- Construct_Inventory (`0x800325E8`): the options ring gains Lara's Home
  (5 items) at the title (gameflow flag 0x200, set on this disc). Its item
  in front (`0x80088BCA`) is 4, Lara's Home, only while `0x80088C04` (set
  at power-on, cleared by any other ring) holds; otherwise 0. Recorded:
  the first title opens on Lara's Home, later ones on the passport.
- RingInit (`0x800349E8`): the title's ring has camera pitch 0x400; the
  other fields as in play (`.psx/card_title` ring struct `0x801FFEC8`).
  The port's ring view had the pitch's sign reversed (it showed in ring
  swaps only); with it corrected the title's items lie where the original
  draws them.
- Modes 3, 4 and 5 choose the passport at once: while a local is clear,
  each pass forces input `0x100000` after the repeat stop, and the Open state's
  Cross sets it (`0x80031ABC`). Nothing in the game asks for modes 3 and
  4 (`0x80087F9C` is never -1 or -2), so they are not ported.
- The Open state's Triangle and Select close the ring except at the title
  (`0x800318A8`). The title draws the title picture instead of the
  backdrop (`0x80032FD4`) and no heading (`0x800331D4`); game over heads
  the options ring GAME OVER (game string 3) without arrows. "Select"
  shows always; "Go Back" not at game over, nor at the title until an item
  is chosen (`0x800317D0`, `0x800321D4`).
- The title picture (384 by 256) is drawn unscaled: the screen shows its
  rows 8..248 and first 368 columns (matched pixel for pixel against
  `.psx/card_title`).

## The passport (`0x8006EB58`)

Pages are five frames apart from its opening frame. Each frame it clears
the item name and draws its page's label (`0x800706F8`: the name between
spaced arrows, `\x11` and `\x12`, for the ways it turns, centred 24 above
the bottom):

| Page | Title | Play | Game over |
|---|---|---|---|
| 0 | Load Game → | Load Game → | Load Game → |
| 1 | ← New Game | ← Save Game → | ← Restart Level → |
| 2 | (not reached) | ← Exit to Title | ← Exit to Title |

Right does not turn past New Game at the title. Triangle closes it
(backwards to frame 0, or from page 2 on to the last frame) except at game
over, where it is ignored. Cross closes it with its page (`0x80089034`),
which the callers read; on Load Game it first reads the card
(`0x80051C88`) and opens the load list, which takes the input until a line
or Triangle is chosen. On Save Game in play it saves at once (`0x80052214`
and the card writer `0x8007255C`). In play with no TR2 save the card's own
screen says "There are no Tomb Raider II games on the Memory card in
Memory card slot 1"; saving goes through "Save game to ...", "Are you sure
?" and "Saving game to ..." screens.

## The save and the load list

The save is one block (`BASLUS-00437TOMB2`, 0x1CD0 bytes from
`0x800DE250`): for each level 0x2C bytes of what Lara carried as she
started it (bit 0 of +0x14 set when reached) and the saved position
(`0x800DE684` its level, `0x800DE69C` bit 0). The load list
(`0x8006E60C`) offers each level reached after Lara's Home by name, with
"Current Position" (game string 74) after the level it was saved in,
chosen at first, and at the title "Story so far..." (77); empty, "No save
games!" (PlayStation string 12). Recorded after saving in the Wall:
`0x80089D10` = [1, -1000] in play, [1, -1000, -1] at the title.

The requester (`0x800355C8`): lines `+0xE` apart (18), five shown (`+0x6`),
296 wide (`+0xC`), its list's bottom 44 above the screen's (`+0x12`).
Down moves on, scrolling at the last shown line; Up back; Cross returns the
line + 1, Triangle -1 when allowed. The heading is 10 above the list; with
the passport's third argument a panel (width 296, height line + 5 lines +
22) behind both and a heading box (width 292); the chosen line is boxed
(width 284). The GPU capture `validation/save/title_load_gpu` gives the
boxes (T_AddBackground `0x8005CA5C`, T_AddOutline `0x8005CB20`): four
half-transparent gouraud quarters meeting at a brighter centre, and eight
opaque gouraud lines; colour tables `0x800828AC`/`0x800828EC` (panel),
`0x80082910`/`0x80082950` (heading), `0x800829D8`/`0x800829B4` (chosen).

## The pause (`0x80068F20`)

"Paused" (PlayStation string 22) centred 24 above the bottom until START.
Select asks "Exit to title?" (Continue, Quit; `0x80037118`), and Quit
"Are you sure?" (Yes, No, starting on No; `0x80037298`): bare requesters
two lines 15 apart, 80 wide; Yes leaves for the title.

## Port

- `game/src/inventory/ring.rs`: `RingMode::Title` and `RingMode::Death`,
  the passport's pages and labels, its load list, and what it closed on
  (`Passport`). The ring reports the chosen item's shown object (the open
  passport is 120), as `0x80089032` holds.
- `game/src/requester.rs`: the requester; `game/src/text.rs`: the boxes.
- `game/src/save.rs`: `SaveGame`, the levels reached with their start
  inventories and the saved position, written as `savegame.json` in the
  player's config folder. There is no memory card, so the card's screens
  are not ported: Save Game saves at once, and with nothing saved the list
  shows "No save games!".
- `game/src/presentation.rs`: the title in the title level (its ring's
  models and font), new game, Lara's Home, loading, saving, game over
  after the death count, restart, the pause and its questions. Levels are
  numbered and named from the gameflow script (`GameStrings`), which also
  fixes the port's old level list that had Venice and Bartoli's Hideout
  swapped. The ring's Controls item opens the template's controls menu
  in place of the original's page.

## Tests

`game/tests/passport.rs` replays three recordings frame by frame
(`tools/record_passport.py`), comparing the ring, its motion and the
options ring's items, and what it closed on:

| Case | Frames | Covers |
|---|---|---|
| `title` | 530 | the first title from power-on: opening on Lara's Home, tilted; the passport's Load Game and New Game, not past it, back, closed; the ring turned |
| `death` | 337 | game over (the death count poked to 301 once; nothing else): the passport opening by itself, all three pages, Triangle ignored, Restart Level chosen |
| `game_pages_heldout` | 289 | held out: the passport in play turned to its last page and not past it, closed from there onwards, the ring closed |

`game/src/requester.rs`, `game/src/text.rs` and `game/src/save.rs` test
the load list's layout and boxes against the GPU capture, its scrolling,
the list's contents and a save read back. Driven in the game over the
developer MCP: the title, Load Game with nothing saved, New Game, Save
Game in the Wall, the load list, loading the saved position from the title
(health and facing restored), game over, Restart Level, the pause, its
questions and Exit to Title.

## Not ported

The memory card's screens and messages; "Story so far..." (no FMVs);
statistics kept in the save; the save and load modes 3 and 4; the title's
demo after about 30 seconds without input; the extra death count when Lara's item
flag 0x100 is set (`0x80042B54`) and the drowning death count (270,
`0x80042B9C`); the level select of gameflow flag 0x400 (not set on this
disc).
