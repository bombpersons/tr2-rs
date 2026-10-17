# Porting a PlayStation game as a reusable engine

You have access to a tool called `psx`, a specialist tool for reverse
engineering PlayStation 1 games. Your working directory holds a game's disc
image (bin/cue).

Your goal is a Bevy port of this game that looks and plays as close as possible
to the original, **built as two things from the start**:

1. **An engine**: the reusable machinery the game runs on. Someone should be
   able to make a *different* game with it, without the original's content and
   without this game's rules.
2. **This game**: the original's rules and content, running on that engine.

The disc is an asset source only. Extract models, textures, levels, audio and
other data with `psx`, convert them, and bake them into the project as
standalone files. The Rust code must not depend on the disc or BIOS at build
or run time.

`psx` provides a template Rust project (`template_create`) with a developer MCP
server for driving the game, taking screenshots and comparing against the
original. Use it as the base.

## Evidence

Use the supplied disc and the `psx` tool as your only evidence source. Do not
consult other reimplementations, decompilations or fan documentation of this
game. Every behaviour you port should be backed by runtime observations or
binary evidence, and the docs should say which.

## Why the engine comes first

Matching the original and building a reusable engine don't conflict if the
engine is cut along the same lines the original was. Most PS1 games already
separate:

- **Data the engine interprets**: level geometry, collision, animation tables,
  object placement, trigger and script tables, setup values (health, speeds).
- **Code written per object type**: the player's state handlers, each enemy's
  AI, special traps and bosses.

If the engine's data formats express what the original's tables express, and
per-object code stays as code, generalising costs no accuracy. Accuracy breaks
when you invent abstractions the original doesn't have (a generic "enemy
behaviour" schema, a scripting language for traps, a physics component that
replaces hand-written movement rules), because they lose the quirks that make
the original feel right. Examples: the order objects update in, when things
happen within a tick, integer rounding, special cases inside one controller.

Restructuring after the port is finished is much harder: every feature added
before then has to be unpicked while keeping its behaviour identical. So set
up the structure on day one and put each piece in the right place as you port
it.

## Step 1: find the original's engine model

Before writing gameplay code, work out from evidence what the original treats
as data and what it treats as code. Write the result to `docs/engine-model.md`
and keep it current:

- What is the world made of (rooms and portals, a tile grid, BSP, a
  heightmap, free polygon soup)? How are collision and visibility derived from
  it?
- How do characters move: animation-driven (root motion, state machines in
  data) or code-driven physics?
- How are objects defined: a type table with a control function per type?
  Which fields come from tables and which are hardcoded?
- How do levels script events: trigger tables, bytecode, hardcoded per level?
- How is a frame run: update order, tick rate, which work happens per field
  and which per game tick?

This document defines the engine's scope. Anything the original drives from
tables belongs in the engine as data. Anything the original hardcodes per type
belongs in the game crate as code.

## Project layout

Create a Cargo workspace with one shared `target/`:

```text
engine/        reusable crate: no names, numbers or assets from this game
game/          this title: controllers, rules, HUD, menus; lib + bin
tools/         offline importers: disc data -> engine-native assets
assets/        baked content in the engine's own formats
docs/          engine-model.md, format notes, evidence
```

**`engine/`** holds the systems every game on this engine needs, as Bevy
plugins:

- world representation, collision queries and visibility
- the animation system (including state dispatch, if the original's is
  data-driven)
- the object/item framework: spawning from placements, a type registry,
  activation, triggers
- camera framework (modes and rules that come from data; a specific camera
  controller can live in the game if it's hardcoded in the original)
- the PS1 look, logic tick and input, controls menu, dev MCP and testing
  helpers that come from the template
- asset types and loaders for the engine-native formats

**`game/`** registers this title's code with the engine:

- per-type controllers (the player, each enemy, each special object)
- trigger and script action handlers that are specific to this game
- HUD, inventory, menus, save format, level order
- the binary that combines engine, game plugins and content

**`tools/`** holds the importers. The original's raw formats (fixed-point
units, packed bitfields, offsets into word arrays, CLUT/VRAM layouts) live
**only** here. Importers convert them exactly into the engine format and
record the source file hash and size for each output.

### Engine-native formats

- Use normal units: `f32` metres or a documented world unit, radians,
  seconds or ticks, linear colour where it applies. State unit conversions once,
  in the importer and in `docs/`.
- Make formats readable (RON or JSON for tables, PNG, WAV, glTF where it
  fits) so people can author them by hand or with ordinary tools.
- The conversion must be exact enough that trajectory tests still match.
  Where float conversion would change behaviour (fixed-point accumulation,
  rounding), keep the integer quantity in the format and document why.

## Rules for placing code

When adding anything, decide where it goes before writing it:

1. **Would a different game on this engine need it?** If so, it goes in
   `engine/`, named generically (`Character`, `ObjectType`, `TriggerAction`),
   not after this game's characters or items.
2. **Did the original read it from a table?** Then it's data in an asset,
   and engine code interprets it. Don't hardcode it.
3. **Did the original hardcode it per object type?** Then it's a controller in
   `game/`, registered with the engine's type registry, ported faithfully,
   quirks included. Don't generalise it into a schema.
4. **Is it a design number inside a controller** (a speed, a damage value, a
   timer)? Put it in a per-type config asset the controller reads, so it can
   be modded without code changes. Leave structural details (update order,
   rounding) in code.

The engine exposes extension points; the game fills them:

- **Object type registry**: type id → controller system set, collision,
  config asset. Adding a new type means registering it, never editing a
  central `match`.
- **Trigger/script action registry**: action id → handler. Unknown actions
  are reported (logged and exposed in a resource), never silently ignored or
  mapped to something else.
- **Schedule sets**: the engine defines named system sets in the original's
  update order; game systems join those sets rather than ordering themselves
  ad hoc.

Prohibited in `engine/`: this game's title, character or item names, asset
paths, magic ids from the original's tables, and `cfg`s or branches that test
which game is running.

## Keeping the engine honest

Keep a second, tiny game next to the port: `engine/examples/sandbox.rs` (or a
`sandbox/` crate) built **only** from `engine/` and a few hand-authored
assets in the engine format: a small room, one controllable character, one
object type with a trivial controller, one trigger. It must not depend on
`game/` or on anything extracted from the disc.

Build and run it in every verification pass. If adding a feature to the port
forces you to touch the sandbox in a way a new game's author wouldn't expect,
or the sandbox can't be made without reaching into `game/`, the boundary is
in the wrong place. Fix the boundary, not the sandbox.

## Fidelity is the contract

- Record the original with `psx` (inputs, per-tick positions, angles,
  animation states, camera) and check the port against it tick by tick in
  headless tests, using the template's trajectory helpers.
- Every restructuring step must keep every trajectory test passing exactly.
  If a refactor changes a number, the refactor is wrong or the test was
  relying on an accident; find out which from evidence before changing the
  test.
- Compare rendered frames against the original with screenshots, and look at
  them.

## Template updates

The `psx` template is updated over time. Its modules (the PS1 look, logic tick
and input, controller bindings, controls menu, dev MCP, testing helpers) move
into `engine/`. Record in `docs/template.md` which template commit you started
from and where each template file now lives, so later template updates can be
merged three ways file by file.

## Work order

1. Identify the game and region; boot it in `psx`; record reference
   checkpoints.
2. Write `docs/engine-model.md` from evidence (step 1 above).
3. Create the template, then turn it into the workspace above, with the
   sandbox building, before porting gameplay.
4. Write the importers and the engine-native formats; validate them
   structurally (every byte of the source accounted for, every reference in
   range).
5. Port feature by feature. For each: record the original, place the code by
   the rules above, add a trajectory or screenshot check, and keep the
   sandbox building.
6. Keep `docs/` current: what's verified, against what evidence, and what's
   still provisional.

## Done means

- The port matches the original's recorded behaviour and looks like it.
- The sandbox shows the engine is usable without this game: a new game would
  add a crate like `game/`, author assets in the engine format, and never
  edit `engine/` to do it.
- A modder can change levels, object setup and tuning values by editing
  assets, and add new object types or trigger actions by registering them.
