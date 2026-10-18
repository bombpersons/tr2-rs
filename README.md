# Tomb Raider II, native Bevy port

A port of Tomb Raider II (PlayStation, USA Rev 3) built as a reusable engine
plus this game on top of it. The disc is an asset source only: nothing here
reads it at build or run time.

## Game content is not included

This repository has no game data. Levels, textures, sounds, screens and music
are copyrighted; you bake them from your own copy of the game (bin/cue, one
`.bin` per track) with the psx toolkit on `PATH`, python3
with Pillow, and cargo:

```sh
tools/bake_assets.sh path/to/game.cue   # writes game/assets/{levels,screens,music} (gitignored)
cargo test --workspace --all-features   # includes the tests that read baked content
```

Without baked content the engine, the sandbox and most tests still build and
run: `cargo test --workspace --features dev-mcp`. Tests that read the baked
levels are behind the game crate's `baked-assets` feature.

```sh
cargo run --features dev-mcp -- --level wall   # start at the Great Wall, no title screen
python3 tools/port_smoke.py                    # offscreen screenshots vs the original (after a dev-mcp build)
python3 tools/camera_smoke.py                  # fixed entrance camera through offscreen rendering
python3 tools/backward_smoke.py                # backward hop/walk against retained PSX motion and cameras
python3 tools/backward_collision_smoke.py      # backward walls, steps, ledges, slopes and hop-fall recovery
python3 tools/traversal_smoke.py               # jumps, ledges, pull-ups and authored landing recovery
python3 tools/airborne_smoke.py                # low/middle/high wall impacts, blocked takeoff and landing recovery
python3 tools/item_lighting_smoke.py           # per-object lighting in aligned Home and Wall views
python3 tools/subdivision_smoke.py             # off/split/filled room meshes at two PSX camera poses
python3 tools/room_visibility_smoke.py         # Wall/Home room pixels after repeated camera turns
python3 tools/ceiling_smoke.py                 # forward low-ceiling impacts and authored recovery
```

Evidence for every ported behaviour is indexed in [docs/evidence.md](docs/evidence.md);
what the engine treats as data and what as per-type code is in
[docs/engine-model.md](docs/engine-model.md).
The remaining work and completed verification milestones are tracked in
[TODO.md](TODO.md).

## Level editor

```sh
cargo run -p editor   # hand-authored sandbox; no game data needed
cargo run -p editor -- --level game/assets/levels/wall/level.json --types game/data/object_types.json
```

A full-resolution Bevy editor with standard 3D rendering, searchable file
browsing, scene shortcuts, an object property inspector, validated edits,
undo/redo, exact untouched saves, and authored animation playback/stepping
with isolated model previews. It depends only on the engine. See
[editor controls and workflow](docs/editor.md).
Keep it in step with engine formats as the port grows.

## The template workspace

A standalone Rust / **Bevy 0.20.0** workspace for a native port of a
PlayStation game, created by `psx template create`. It doesn't need an
emulator, a disc or the psx toolkit at runtime. Requires Rust 1.97.1+ and a
desktop GPU driver (or Mesa lavapipe with `--software`).

```sh
cargo run                                # the game (default member: game/)
cargo run -- --software                  # force a software adapter
cargo run -- --region pal                # override REGION (display + tick rate)
cargo run -- --controls                  # open the controls menu (F1 also opens it)
cargo run -p engine --example sandbox    # the engine's own tiny game
cargo test --workspace                   # headless tests: engine, game, importers
cargo run --features dev-mcp -- --mcp --headless --software  # developer MCP, offscreen
cargo test --workspace --all-features    # routine agent check (dev-mcp everywhere)
cargo build --release                    # shipping build; developer MCP is excluded
```

## Layout

| Path | Owner | Contents |
|---|---|---|
| `psx-bevy/` | psx template | PS1 look, ticks and input, controls menu, testing, viewers, developer MCP, launcher. Updated by `psx template update`; edit sparingly and keep edits mergeable ([README](psx-bevy/README.md), [template notes](docs/template.md)) |
| `engine/` | this project | This game's engine family: object type and action registries, update-order sets, level and tuning formats, `examples/sandbox.rs`. No names, numbers or assets from this game ([AGENTS.md](engine/AGENTS.md)) |
| `editor/` | this project | Engine-format authoring, full-resolution UI and standard 3D preview ([guide](docs/editor.md)) |
| `game/` | this project | This title: per-type controllers, action handlers, HUD, menus, baked assets in `game/assets/`, trajectory tests ([AGENTS.md](game/AGENTS.md)) |
| `tools/import/` | this project | Offline importers: files extracted with `psx` → engine formats, with provenance |
| `tools/*.py` | this project | Evidence recording and smoke scripts |
| `docs/` | this project | `engine-model.md` (what the original treats as data vs code), `evidence.md`, `template.md`, per-feature evidence notes |
| `.psx-template.toml` | psx template | Which template version this is and which files it owns |

Level content is data (`game/assets/levels/<name>/level.json`, written by
`tools/import`) spawned through the engine's object type registry; each object
type is code registered by id in `game/`; trigger actions run through the
engine's action registry. `docs/template.md` records where the template's
files went.

## Template updates

```sh
psx template update --dry-run   # what would change
psx template update             # replace unmodified template files
```

Files under `psx-bevy/` that you edited are left alone and the new version is
written beside them as `<file>.psx-new`. Files the template only seeded
(everything outside `psx-bevy/`) are never touched; when their template
version changed, the update reports a diff to apply by hand.
