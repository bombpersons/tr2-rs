# Template provenance

The project started from the psx `template_create` Bevy starter as a single
`game/` crate. Its last merge into that layout, commit `c3561ad`, brought it to
template commit `53b1eac` (merged from `99dae8d`).

It now uses the template's workspace layout and is tracked by
`.psx-template.toml`, written by `psx template adopt`. The current template build is
psx `0.1.0+src-7abefa01a0da12ad-release`. Package `tomb-raider-ii`, region
NTSC, 2 fields per tick.

## Updating

```sh
psx template update --dry-run
psx template update
```

`psx-bevy/` is owned by the template: unedited files are replaced, and edited
ones get a `<file>.psx-new` beside them. Never edit `psx-bevy/`; extend it from
`engine/` or `game/`. Seeded files (everything else) are reported as diffs to
merge by hand. Ours have diverged on purpose: `engine/` is reshaped to
`docs/engine-model.md` and `game/` is this title.

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

## Workarounds for template limits

- The launcher always creates a 320x240 screen. Tomb Raider II displays
  368x240 of a 384x240 projection (`docs/original-evidence.md`), so the game
  calls `engine::render::configure_view` in the launcher's `add` closure. It
  replaces the screen targets and window size. Worth raising upstream as a
  `Game` field.
