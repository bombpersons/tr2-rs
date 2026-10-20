# Roadmap: from a faithful port to authoring new games

The long-term aim is to author levels from scratch, and eventually new
enemies, new player models and whole games, on top of a port whose mechanics
match the original. The port comes first: accurate behaviour can only come
from it. This file lists the layers in the order they depend on each other.
Each layer's detailed design lives in its own doc once work starts
(the editor's in [editor-design.md](editor-design.md)).

## Layer 0: finish the port, open to extension

Behaviour waits on the port, but how the port is written decides how easy
custom content is later. These habits cost nothing in accuracy
(`game/AGENTS.md`, "Keep it extensible", has the working rules):

- **Behaviour is registered by type,** through the object type registry
  (`register_object_type`, `ObjectType::on_spawn`), never by matching on
  type ids in shared systems. A new type is then one more registration.
- **Enemies share creature systems.** The original's enemies share
  pathfinding over navigation boxes, mood (attack, chase, flee), senses and
  attack ranges. Port that once as reusable components with per-type
  parameters, not as one-off controllers, so that a new enemy is mostly
  data.
- **Tuning numbers are named and grouped** (speeds, jump velocities, reach,
  damage), each with its evidence. They can move into tuning assets later
  without changing a rule.
- **Every kind of content shares one path per kind.** Pickups and inventory
  items, weapons and ammo, traps, doors and switches, cutscenes, fixed cameras
  and sound cues are each built on shared machinery (a registry entry plus
  per-type data or a per-type plugin), not one-off code per instance. A gun is
  the gun table's entry plus its handlers, a pickup its inventory entry, a
  cutscene its camera, actor and clip data; adding a new one later means
  another entry of the same kind.
- **Clip and state numbers are a written contract.** The clips, states and
  effect commands Lara's controller relies on are listed in one place, so a
  replacement model can be checked against it.

## Layer 1: levels from scratch with existing content

The editor plan ([editor-design.md](editor-design.md), "Building by push and
pull"): File > New, the object library, push/pull geometry with rooms made
automatically, lighting. A complete level also needs:

- **Trigger and floor-data editing:** doors, enemy activation, flip maps,
  secrets, camera cues, level ends. Each trigger kind becomes editable once
  it is ported, so it can be play-tested.
- **Fixed cameras and sound sources,** placed in the view.
- **Level sequence data:** the level order, starting inventory and music as
  a file the game reads, instead of game code.

This layer proceeds alongside the port.

## Layer 2: your own assets

- **Textures and sounds:** PNG and WAV import (part of the object library).
- **Static meshes and decoration:** glTF/OBJ import as statics, and as room
  detail without collision.
- **Referencing instead of copying.** Levels copy every asset into
  themselves today. Authored content is better served by levels referencing
  library entries, so a fix to a model reaches every level using it. That is
  a new format version, resolved by the engine's loader; imported disc levels
  keep their self-contained, byte-exact form. The same change should allow
  more than one 2048 x 2048 atlas per level.

## Layer 3: characters

The original's characters are rigid segmented models (one mesh per joint),
not skinned.

1. **Model import** from glTF: split a skinned mesh into per-joint pieces by
   bone weight (the original's look), or support skinned meshes in the
   engine as a modern option. Bevy supports both; it is a style choice.
2. **Animation import** into clips, with clip links, state changes and
   commands (sounds, effects) authored in the editor's Animation window,
   which grows from a viewer into an editor.
3. **Player models** that meet the Layer 0 contract (joints, clips, states,
   commands) use Lara's controller unchanged: a new look and new animations
   with the same moves. This is the cheapest custom character.
4. **Retargeting** animations from another skeleton, later and optional.

## Layer 4: new behaviour

In order of ambition, and of building:

1. **Configured enemies (data).** On the shared creature systems, a new
   enemy is a model plus a definition: health, speed, senses, attack clips
   and damage, walking, swimming or flying, drops. No compiling. Its exact
   shape is settled once enough enemies are ported to show what they share.
2. **Plugins (code).** Anything truly new (a boss with phases, a trap, new
   player moves) is a Bevy plugin registering types and systems. It needs a
   rebuild and is plain idiomatic Bevy.
3. **Scripting** (Lua, Rhai or WASM) only if data and plugins prove
   insufficient; it brings an API to maintain.

- **Type ids.** Behaviour is keyed by the original's `u16` type ids. Custom
  types get string names in `object_types.json`, mapped to ids when a level
  loads, or a reserved id range.
- **New player characters** follow the same pattern: Lara's tuning as data
  gives variants ("Lara who jumps higher"); new moves are plugin code with
  clips that meet the contract.

## Layer 5: a whole game

Menus, inventory items, save games, cutscenes, music, the level sequence
from Layer 1, and packaging a custom game apart from the original's assets.
Disc-derived content stays generated and gitignored; authored content can be
committed and shared.

## Order

1. The port, following Layer 0, alongside Layer 1 in the editor (trigger
   editing as each trigger kind is ported).
2. Layer 2: asset import, then the move from copying to referencing.
3. Layer 3: characters, starting with player models that meet the contract.
4. Layer 4: configured enemies once enough are ported; plugins after.
5. Layer 5: game structure.
