# Template provenance

The project started from the psx `template_create` Bevy starter as a single
`game/` crate. Its last merge into that layout, commit `c3561ad`, brought it to
template commit `53b1eac` (merged from `99dae8d`).

It now uses the template's workspace layout and is tracked by
`.psx-template.toml`, written by `psx template adopt`. The current template build is
psx `0.3.8+src-0b26c73b885b1004-release`. Package `tomb-raider-ii`, region
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

- `psx-bevy/src/psx/mod.rs`: `resize_screen` marks the `Projection` of every
  camera it retargets as changed (asserted in `screen_targets_follow_set_size`).
  Bevy recomputes a camera's target size only on a projection change, a new
  camera or an image asset event, not on a `RenderTarget` change. A camera
  whose projection also changed that frame (the engine fits the level view
  to the new screen) took the new size a frame before the HUD and menu
  cameras on the same canvas. Bevy shares one depth texture per target, so
  the frame failed wgpu validation ("Attachments have differing sizes") and
  the game exited when Aspect was switched to match window. Upstream
  candidate: any game that changes a projection on resize hits it.

- `psx-bevy/src/controls/options.rs` (new), `controls/mod.rs`,
  `controls/view.rs`, `controls/tests.rs`, `README.md`: a second controls-menu
  page for game settings. `ControlsOptions` holds labelled choice rows, which
  the menu drafts with the bindings, applies on Save and resets with Restore
  defaults. Tab, L1/R1 or a click on the title row's tabs turns the page.
  With no options registered (the template default) the menu is unchanged.
  The game uses it for the free camera's CAMERA page
  ([free-camera.md](free-camera.md)). The 240-line screen had no room for
  more rows, hence a page. Additive: `ControlsMenu` gained `page` and
  `options_draft()`, and the command rows moved into a shared `command`
  function. Upstream candidate: any game with a few input settings (invert,
  sensitivity) needs somewhere to show them.

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

## Update 2026-10-19

From psx `0.1.0+src-41f296d388d06491-release` to
`0.3.0+src-a3beb4f814bc417d-release`.

- New template modules `display` (F2 display menu: resolution, aspect,
  fullscreen, `PsxLook`; saved to `display.json`), `pointer` (mouse on the PS1
  screen) and `settings` (player settings files), re-exported from `engine`.
  New `examples/screen_size.rs` GPU check. The launcher adds `DisplayPlugin`,
  `--display`, `--display-settings`, `--fullscreen`, `--aspect` and F11.
- `PsxScreen::set_size`, `PsxAspect` and `PsxAnchor`: the framebuffer may now
  change size at runtime. Mouse input on `ControllerState::mouse`, in replays
  and in `game_step`/`game_set_input`; a MOUSE column in the controls menu.
- Conflicts: upstream did not adopt `use_scene_lighting`. `material.rs` kept
  as is (no upstream change); `psx/mod.rs` and `README.md` took the new
  version with the conditional sync, its regression test and the README
  paragraph reapplied.
- Seeded files: merged the display/mouse notes into the root `README.md` and
  `game/AGENTS.md`, and the new re-exports into `engine/src/lib.rs`.
  `.gitignore` already covered the template's entries; `game/src/demo/` was
  replaced by this game long ago, so its change does not apply.

Adapted afterwards: the level camera follows screen resizes
(`ViewSettings::sub_view`/`margin`, `engine::render::fit_view_to_screen`):
the original's 384x240 projection cropped to 368x240, scaled by the screen's
height, with extra width split equally between the sides. The room
tessellator's off-screen rejection widens by the same margin (`RoomView::margin`).
The HUD and title use an overlay camera in the original's 240-line units
(`engine::render::overlay`), with `OverlayAnchor` for edge elements. At
368x240 the captures from `tools/screen_size_smoke.py` are byte-identical to
before the change. `--aspect window` needs a real window and was not run
headless; it resizes through the same `PsxScreen::set_size` path, and the view
test covers arbitrary widths, narrower ones included.

Validation: `cargo test --workspace --all-features` passed, as did all five
ignored GPU/MCP tests in `psx-bevy/tests/dev_mcp.rs` (ALSA null output). The
game (`--level wall`) and the engine sandbox start headless without errors.

## Update 2026-10-20

From psx `0.3.0+src-a3beb4f814bc417d-release` to
`0.3.4+src-b72afc8e8efe891f-release`.

- New template module `assets`: layered asset folders (`add_root`, `find`,
  a layered default asset source installed by the launcher), re-exported as
  `engine::assets`. `engine::config::asset_path` now resolves through it.
  The launcher, `lib.rs` and `examples/screen_size.rs` were replaced.
- Conflicts: upstream still does not have `use_scene_lighting`.
  `material.rs` kept as is (only our edit differs); `psx/mod.rs` kept, with
  the one line the template had wrapped wrapped the same way; `README.md`
  took the new version (a "Layered asset folders" section) with our
  per-object lighting paragraph reapplied.
- Seeded files: the template now ships a mod (`mods/example`) and a
  `GamePlugins` group, `replace_object_type` / `replace_action`, and
  guidance on shared routines, asset contracts, plugin independence, mods
  and after-parity layers. Merged into this project as:
  - `GamePlugin` became the `GamePlugins` plugin group;
  - `register_object_type` panics on duplicates, with `replace_object_type`
    beside it, and `register_creature` gained `replace_creature`. Trigger
    actions here are names read from messages, so `replace_action` does not
    apply: a mod handles its own names;
  - the workspace has a `game` dependency and the `mods/example` member;
  - `mods/example` is written for this game (hand-made level, a type and a
    trigger action of its own, a replaced pickup type); the template's demo
    version does not apply;
  - the new guidance went into the root, `engine/` and `game/` `AGENTS.md`
    and the root `README.md`.
  `Cargo.lock` was regenerated by cargo; `game/src/demo/` and
  `game/tests/player.rs` do not apply.

Validation: `cargo test --workspace --all-features` and the mod's ignored
GPU/MCP test (`cargo test -p example-mod --features dev-mcp --test dev_mcp
-- --ignored`) passed.

## Update 2026-10-23

From psx `0.3.4+src-b72afc8e8efe891f-release` to
`0.3.8+src-0b26c73b885b1004-release`.

- Template changes: the controls and display menus are split into logic
  (`mod.rs`) and a replaceable built-in view (`view.rs`), with `ControlsUi` /
  `DisplayUi` to turn the view and shortcuts off and a public API for a game's
  own view (`ControlsMenu::{shown, mode, hint}`, `row_label`, `ROWS`...). The
  material shader keeps the perspective weight positive for vertices behind
  the camera, and `DistanceFog` is now per pixel. New `examples/corridor.rs`.
  New `vendor/` (`psx-media`, `psx-assets`), excluded from the workspace.
- Conflicts:
  - `controls/mod.rs`, `view.rs`, `tests.rs`: merged three ways against the
    0.3.4 files. The options page is kept; its Browse hint moved with the
    others into `ControlsMenu::hint`, and the view's hint test checks both
    pages through it. Both sides' tests are kept.
  - `material.rs`: took upstream's fog doc line; `use_scene_lighting` kept.
  - `psx/mod.rs`: no upstream change; kept as is.
  - `README.md`: took the new version (layout rows, "Making the menus the
    game's own") with the per-object lighting and options-page paragraphs
    reapplied.
- Seeded files: added the `vendor/` exclusion to the workspace, and the menu
  view and `vendor/` guidance to `game/AGENTS.md` and `AGENTS.md`.
- Adopted the import from the player's disc on first start, replacing
  `tools/bake_assets.sh` and its Python extractors:
  - `tools/import` is now the `import` library (and command) on the vendored
    crates. It decodes `DATA/*.PSX` (`level_file.rs`, ported from
    `tools/extract_tr2.py`), the `PIX*/*.RAW` screens (`screens.rs`, from
    `extract_pix.py`) and copies the CD audio tracks (`music.rs`). From the
    supplied disc it reproduces all 2,936 baked files, the WAV and level
    files byte for byte and the PNGs pixel for pixel, in about 10 seconds.
  - `game/src/disc.rs` is the template's, with `data_ready` (no
    `GameData` state counts as ready) gating `GameSystems::Game`, the
    engine's `LevelSystems::Load` and the title picture, and `rom/` folders
    added to the disc search. Its screens are still the template's plain UI.
  - Output goes to `game/assets/imported/`, an asset root of the game (and of
    the mod), so load paths are unchanged; tests read it there.
  - `tools/extract_tr2.py` stays as a research dump of raw tables.
- Not adopted yet:
  - "One Bevy build" (`engine/` depending on `bevy_egui` so the game and
    editor resolve the same Bevy features);
  - the seeded egui editor shell and `game/src/demo/`, which this project's
    editor and game replace.

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
