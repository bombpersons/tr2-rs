# Object type table and main loop: PSX-only investigation

All findings here are **PSX**: from the supplied disc (USA Rev 3) with `psx`
disassembly, Ghidra over the RAM snapshot `.psx/wall_game/ram.bin`, and two
runs from `.psx/wall_game/state.bin`. No other reference was consulted.
Pointers in the table are stored as KUSEG addresses (`0x000257b4`); they are
written here with the `0x8...` prefix.

Runs:

- `.psx/object_table_reads`: 4 fields from `wall_game`, read watch on the
  whole table `0x800936D0..0x80096880`.
- `.psx/main_loop_trace`: 3 fields from `wall_game`, instruction trace of
  fields 21001-21002 restricted to the control phase and the game loop.

## The table

`0x800936D0`, **265 records of 48 bytes** (type id = record index; the
placement's type id at item+0x0C indexes it). Size and count come from the
reset loop in `0x8005717C` (`do { ... } while (i < 0x109)`, stride 0x30) and
from every indexed access (`type*0x30 + 0x800936xx`).

| Offset | Type | Field | Source of the value | Evidence |
|---|---|---|---|---|
| +0x00 | s16 | mesh count | level file | Lara 15, braid (type 2) 6; negative for sprite types |
| +0x02 | s16 | first mesh index | level file | read by mesh draw / hair (`0x800305A0`, `0x80027360`) |
| +0x04 | s32 | bone (joint tree) index | level file | read at `0x8002F7F0` (hair), `0x800272E0` |
| +0x08 | ptr | frame base | level file | `0x80027328` |
| +0x0C | fn | initialise | setup code | called by InitialiseItem `0x80037690` (at `0x80037958`) if non-null |
| +0x10 | fn | control | setup code | item and effect loops in the control phase (`0x8001DC44`, `0x8001DCC0`) |
| +0x14 | fn | floor | setup code | `0x8001EC20` (height query over objects) |
| +0x18 | fn | ceiling | setup code | `0x8001FA3C` |
| +0x1C | fn | draw | setup code; default `0x800257B4` | room item draw `0x800253F8`; sprites use `0x800256C8` |
| +0x20 | fn | collision | setup code | object collision `0x8001C258` (called from Lara's control) |
| +0x24 | s16 | first animation | level file | InitialiseItem copies it to item+0x14; Lara 0, types without animation -1 |
| +0x26 | s16 | hit points | setup code; default -16384 | InitialiseItem copies it to item+0x22 |
| +0x28 | s16 | (unnamed) creature value | setup code; default 0 | tiger 200, type 15 300, type 214 1800; reader not located |
| +0x2A | s16 | radius | setup code; default 10 | creature values 102/204/341; read at `0x800163D4`, `0x80016ACC` |
| +0x2C | s16 | shadow size | setup code; default 0 | Lara 160, creatures 128 or 64; read at `0x80026D9C` in Lara's draw |
| +0x2E | u16 | flags (bits 16-31 of the word at +0x2C) | bit 16 level loader, others setup code | see below |

Flags word at +0x2C (bit numbers of the 32-bit word):

- bit 16: the type is present in the loaded level. Every setup block tests it
  (`if (rec+0x2C & 0x10000)`) and the reset loop keeps it.
- bit 17: set for creatures (`|0x3E0000`). InitialiseItem sets item status
  bits 1-2 (`|6`) when it is set; `0x80020BB0` uses it with hit points <= 0.
  Also tested at `0x8001F23C`, `0x800377F4`, `0x80046C94`, `0x800471B4`,
  `0x8004B410`, `0x800572AC`. Meaning beyond "creature-like" not established.
- bits 18-21: set in different combinations per type (creatures `0x3E`,
  Lara `0x3C`, doors `0x30`, pickups `0x14`). Readers not traced.
- bit 22: set for several effect-like types (`|0x400000`).
- bit 23: set for some creatures (`|0xBE0000`); tested in creature routines
  `0x8001682C`, `0x8001721C`, `0x8001E028`.

Reset defaults (`0x8005717C`): init, control, floor, ceiling, collision = 0;
draw = `0x800257B4`; hit points = -16384; +0x28 = 0; radius = 10; shadow = 0;
flags &= `0xFF41FFFF`. Then four setup routines run in this order:
`0x800538A0` (Lara and creatures), `0x80055774` (traps and effects),
`0x80055FE4` (switches, doors, pickups, keyholes, special objects), and
`0x8002F594` (InitialiseHair). The caller is the level loader `0x8006A6AC`,
which then initialises every placed item through InitialiseItem
`0x80037690` (call at `0x8006B870`).

The setup routines are straight-line code: per type, a test of the loaded
bit, then constant stores of function pointers, hit points, +0x28, radius,
shadow size and flag bits, plus bone-rotation flags (`|8` or `|0x10` on
entries of the bone table `0x8008AA5C`) for creature heads and necks. So the
original keeps **setup values in a table but writes them from code**: they
are not in the level file.

### Runtime confirmation

`.psx/object_table_reads/reads.jsonl` (32 reads in 4 fields): reads happen in
fields 21001 and 21003 only (one logic tick per 2 fields). Each tick:
`0x8001DC44` reads the control pointer of types 253 (`0x800602DC`) and 107
(`0x8004C954`, the door controller); `0x8001DCC0` reads the control pointer
of effect type 252 (`0x800603D0`); later `0x800253F8` reads Lara's draw
pointer (`0x800257AC`) during drawing. A dump of the 103 loaded records in
Wall is in the shared notes folder (`object-table-wall.txt`); its values agree
with the setup code.

### Setup values against `game/data/object_types.json`

From the setup code (all levels) and the Wall dump:

| Type | Control | Collision | HP | Radius | +0x28 | JSON |
|---|---|---|---|---|---|---|
| 36 | `0x8005C1EC` | `0x8001C598` | 5 | 102 | 0 | health 5, radius 102: matches |
| 37 | `0x8005C4DC` | `0x8001C598` | 40 | 341 | 0 | health 40, radius 341: matches |
| 38 | `0x800128A4` (init `0x800127DC`) | `0x8001C598` | 15 | 204 | 0 | not listed |
| 39 | `0x80021FCC` | `0x8001C598` | 20 | 341 | 200 | health 20, radius 341: matches |

`damage` (25, 100) is not a table field. It must be a constant inside each
controller; not yet located, so it stays Inherited.

Shared function pointers (from the three setup routines):

| Function | Types |
|---|---|
| switch control `0x800516B8` | 92, 93, 103, 104, 105 |
| switch collision `0x800507E4` | 92, 93, 103, 104 (105 uses `0x80050AC4`) |
| door init/control/draw/collision `0x8004C358`/`0x8004C954`/`0x8004AB98`/`0x8001C70C` | 106-113 |
| pickup collision `0x800502D4` (sprite draw `0x800256C8`) | 135-152, 174-177, 190-196, 205, 206 |
| puzzle-hole collision `0x800512CC` | 182-185 |
| keyhole collision `0x80050F80` | 201-204 |
| spikes collision `0x8005F378`, no control | 59 |
| generic creature collision `0x8001C598` | all creatures above |

These match the JSON's grouping of ids into kinds (switch, underwater switch,
door, pickup, keyhole, spikes). Type 152 is a pickup by collision but also has
its own control `0x8004212C` and draw `0x80041760`; type 191 has an extra
control `0x8005193C`.

## The main loop

Game loop `0x8006DD98`:

```text
fields = 1
while ControlPhase(fields, demo) == 0:   # 0x8001D7FC
    fields = DrawPhase()                   # 0x80023DEC, returns fields elapsed
```

ControlPhase `0x8001D7FC` (gp = `0x800878FC`):

1. `fields` is clamped to 10 and added to an accumulator (gp+0x1A8). While
   the accumulator is > 0, one tick runs and 2 is subtracted, so a 3-field
   frame runs 2 ticks and carries -1.
2. Per tick, in order:
   1. Level hook `0x80011F2C` (skipped when `0x8008C398 & 4`): per-level code
      chosen by level number `0x80089CF4` (level 0 calls `0x80012614`).
   2. Exit if `0x80089CFC` is set.
   3. Input `0x80069320`: pad to action bits at `0x8008AAD4` through the
      binding table `0x800847FC`.
   4. Demo, inventory and pause requests (`0x80021314`, `0x80030F30`,
      `0x80068F20`); each can end the phase.
   5. **Active items**: from `0x8008B542` along item+0x1C (items at
      `*0x8008AAAC`, 0x4C bytes). For each, control(item index) through
      type+0x10. The next index is read **before** the call, so an item that
      leaves the list during its control does not break the walk.
   6. **Active effects**: from `0x8008C6E4` along fx+0x1A (effects at
      `*0x8008BB7C`, 0x28 bytes; type at fx+0x16), same table, same rule.
   7. **Lara** `0x800424F0(0)`: dispatches on water state `0x8008C4F4`
      through the jump table at `0x80010B3C` to `0x80038194` (above water),
      `0x80044FC4`, `0x80044264` or `0x80043564`, then adds the distance
      moved to a statistic (`0x800DE67C`). Lara is not in the active item
      list; her type record has no control pointer.
   8. Hair `0x8002F674(0)` (HairControl, `docs/braid-evidence.md`).
   9. Camera `0x80019BF0` (`docs/chase-camera-evidence.md`).
   10. Sound sources and flip effect `0x800290E4`: level sound sources
       (`*0x8008C438`, 16 bytes, count `0x8008B24C`, flag 0x80, or 0x40 when
       flipped per `0x8008B544`), then the active flip effect `0x80087A94`
       through `0x800827B8`, then sound update `0x8005B7F0`.
   11. Counters: `0x80089428` -= 1, game time `0x800DE670` += 1.

Inside Lara's above-water control `0x80038194` (after the state handler and
turn/yaw, `docs/locomotion-evidence.md`):

1. `0x80042E08`: Lara's animation step (also called directly when
   `0x800424F0` forces an animation).
2. Object collision `0x8001C258(lara, coll)`, unless `0x8008C510 & 0x40000`:
   for every item in Lara's room and the rooms its portals lead to (room+0x48
   list, item+0x1A next), if item status has 0x20 and not both bits 1-2, and
   the type has a collision function, and the item is within 4095 units on
   each axis: collision(item index, lara, coll). Pickups, switches, doors,
   keyholes and creature pushes all happen here, in room-list order.
3. State collision handler through the table at `0x80082DE0` (only when
   `0x8008C50A == -1`).
4. Room update `0x8001C0E8`, weapons `0x8003FC4C`.
5. **TestTriggers `0x8001EE30(coll trigger, 0)`**: trigger type through the
   jump table at `0x800100FC` (8 types), actions through `0x8001011C`
   (12 actions, bits 10-13 of each word). Creature and trap controllers also
   call it with a non-zero second argument, which only lets type 5 through.

Runtime confirmation: `.psx/main_loop_trace/trace.txt` shows, in field
21001: `jal 0x8001D7FC` with a0 = 2, then `0x80011F2C`, `0x80069320`, two
item `jalr` at `0x8001DC54`, one effect `jalr` at `0x8001DCD0`,
`0x800424F0`, `0x8002F674`, `0x80019BF0`, `0x800290E4`, and `0x80023DEC`.
One tick per draw.
