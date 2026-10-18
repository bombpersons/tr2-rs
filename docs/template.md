# Template provenance

The project started from the psx `template_create` Bevy starter as a single
`game/` crate. Its last merge into that layout, commit `c3561ad`, brought it to
template commit `53b1eac` (merged from `99dae8d`).

It now uses the template's workspace layout and is tracked by
`.psx-template.toml`, written by `psx template adopt`. The current template build is
psx `0.1.0+src-41f296d388d06491-release`. Package `tomb-raider-ii`, region
NTSC, 2 fields per tick.

## Updating

```sh
psx template update --dry-run
psx template update
```

`psx-bevy/` is owned by the template: unedited files are replaced, and edited
ones get a `<file>.psx-new` beside them. Seeded files (everything else) are
reported as diffs to merge by hand. Ours have diverged on purpose: `engine/` is reshaped to
`docs/engine-model.md` and `game/` is this title.

## Editing psx-bevy

Editing `psx-bevy/` is allowed. The goal is only that later template updates
stay easy to merge:

- First try building on it from `engine/` or `game/` (wrap, re-export, add
  plugins in the launcher's `add` closure). Edit the template when that
  can't be done cleanly, e.g. a missing hook or pipeline option.
- Keep edits small and local. Add rather than rewrite, don't move, rename or
  reformat code, and keep the existing public API working.
- Only general PS1 machinery goes there. Nothing specific to this game:
  that is `game/` (or `engine/` behind a hook the game fills).
- List every edited file below with the reason. On update, merge
  `<file>.psx-new` into the edited file three ways, then delete the
  `.psx-new`.
- If an edit would help any game, note it for the psx toolkit too, so it can
  arrive upstream and the local edit can be dropped.

### Local edits

- `psx-bevy/src/psx/material.rs`: added `use_scene_lighting` (default true),
  allowing a controller to own per-object lighting without the global light
  sync overwriting it. No shader or default rendering change.
- `psx-bevy/src/psx/mod.rs`: global material sync respects that flag while
  continuing to sync `PsxLook`; includes a regression test covering scene and
  look changes, and opting back into scene lights.
- `psx-bevy/README.md`: documents that material option.

This is an upstream candidate for psx: object-specific light probes need an
explicit lighting owner. Scheduling an engine system after the template's
private material-sync system would be fragile and would mutate every affected
material twice each frame. On template updates, retain the option, default,
conditional sync and test unless upstream provides equivalent support.

## Update 2026-10-18

- Added upstream `PsxDepthBias` and `PsxMaterial::depth_bias`, including the
  `depth_bias` example and GPU/MCP check. This supplies the pipeline option
  used by the game's room meshes and filler copies (see
  [subdivision-evidence.md](subdivision-evidence.md)).
- Adopted `launcher::Game::screen`: the game passes `game::SCREEN` (368x240),
  and the engine sandbox passes 320x240. `ViewSettings` still supplies the
  game's 384x240 projection and its crop to the display window.
- Reviewed the seeded files without replacing the project's engine,
  controllers or tests. Merged the screen setting and editor guidance;
  retained the project's more specific architecture and evidence rules.

Validation: `cargo test --workspace --all-features` passed, as did the
template's depth-bias GPU/MCP test with ALSA null output configured for this
headless machine. Inspected the coplanar-copy screenshot. The Wall and engine
sandbox smoke runs rendered at 368x240 and 320x240 respectively; the sandbox
also moved its walker through MCP input. Captures and transcripts are under
`validation/template-update/`. Clippy completed with existing warnings;
`cargo fmt --all -- --check` reports existing formatting differences outside
the changed code.

## Where the template's files live

| Old location (single crate, template `53b1eac`) | Now | Note |
|---|---|---|
| `game/src/psx/**` (look, materials, shaders, TIM/TMD, scene, font) | `psx-bevy/src/psx/**` | identical to template; re-exported as `engine::psx` |
| `game/src/shell.rs` | `psx-bevy/src/shell.rs` | `engine::shell` |
| `game/src/controller.rs`, `controller/bindings.rs` | `psx-bevy/src/controller*` | `engine::controller`; `BindingStore::default_path` now takes the package name |
| `game/src/controls/**` | `psx-bevy/src/controls/**` | `engine::controls` |
| `game/src/testing.rs`, `testing/trajectory.rs` | `psx-bevy/src/testing*` (+ new `testing/mcp.rs`) | `engine::testing` |
| `game/src/viewer.rs`, `model_viewer.rs`, `dev_mcp.rs` | `psx-bevy/src/` | `engine::{viewer, model_viewer, dev_mcp}` |
| `game/examples/depth_cue.rs` | `psx-bevy/examples/depth_cue.rs` | |
| window/renderer/flags in `game/src/main.rs` | `psx-bevy/src/launcher.rs` | `game/src/main.rs` and the sandbox call `engine::launcher::run` |
| `game/README.md` (template part) | `psx-bevy/README.md` | project part is the root `README.md` |
| `game/AGENTS.md` | `game/AGENTS.md` (new template text), `engine/AGENTS.md`, root `AGENTS.md` | |
| `game/Cargo.lock`, `game/rustfmt.toml` | workspace root | |
| — | `engine/src/config.rs` | template seed (`TuningPlugin`, `load_ron`), unchanged |

Template skeleton files that this engine replaces with its own, reshaped from
evidence: `engine/src/level.rs` → `engine/src/world/`;
`objects.rs`, `triggers.rs` and `schedule.rs` are our own versions (see
`docs/engine-model.md`).

## Resolved template limits

- The launcher now accepts the display size through `Game::screen`. The game
  no longer replaces its screen targets or window size in `configure_view`.
  That helper remains available for hand-built engine apps.
