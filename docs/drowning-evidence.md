# Oxygen exhaustion and water death

Evidence comes only from the supplied Tomb Raider II USA Rev3 disc through
`psx`. The game now keeps a drowning actor in the water controller, follows
the authored death dispatch and holds the terminal underwater or surface pose.
It no longer substitutes the generic grounded death mode on oxygen damage.

## Binary rules

`tools/record_drowning.py --inspect` retains the exact disassembly commands,
Home RAM hash, function-byte hashes and listing hashes in
`game/tests/data/original/drowning_code.json`. Listings are regenerated under
`validation/drowning/code/`.

- LaraControl `0x80042B10` normalizes previously dead health to -1.
  `0x80042BF8..0x80042C40` skips oxygen accounting for negative health;
  otherwise it decrements air, clamps exhausted air to -1 and subtracts five
  health per tick. Air reaching zero causes no damage until the next tick.
  The subtraction is signed, rather than clamped through the generic damage
  helper. This matters when the last five points leave health zero or negative
  before authored dispatch can enter the death state.
- Tread/stroke/glide request goal44 when health is nonpositive and skip active
  steering and speed changes. The underwater wrapper still decays bank and
  turn rate, clamps pitch and updates yaw. The current animation may continue
  moving until its extracted state ranges accept the death goal.
- State44 uses control `0x8004570C`: brake by eight quarter-units per tick
  (two original units/tick) and move pitch toward level by364 binary units.
  State control ignores pad requests. The wrapper still owns angular recovery.
- Death collision `0x80045810` sets health and air to -1, raises the body five
  units if the water surface is more than100 units above it, then calls the
  ordinary submerged collision `0x80045CB8`. The body can stop just inside the
  100-unit margin; it does not snap to a fabricated exact offset.
- The surface wrapper uses the same control/collision tables at `0x80044364`
  and `0x80044478`. An already dead actor that rises out of a wet room still
  follows LaraControl's surfacing transition. Surfacing114 finishes into
  tread110, whose authored ranges select surface death132. Underwater death
  uses124; their different final frames and entry frames remain level data.
  Negative-health surface actors do not regain oxygen.

Controller design values are in Lara's `swimming` entry in
`game/data/object_types.json`. The shared death motion and collision live in
`game/src/player/swimming.rs`; surface and submerged wrappers preserve their
own integration and room-update order. No engine format or template change
is needed.

The accompanying editor verification exposed a separate authoring bug:
closing a hole whose linked sector already had a visible surface added a
duplicate face. `engine::editor::mesh::close_link` now reuses that surface,
preserving its texture and vertex shades. A hand-authored engine-only test
covers both hole closure and room disconnection. This is an editor fix, not a
claim about original gameplay. The editor smoke also reads the current exported
geometry after disconnection instead of selecting a stale pre-edit face index.

## Retained recordings

The starting submerged checkpoint and its original placement/pitch setup are
documented in [swimming evidence](swimming-evidence.md). Moving starts are
reached from it using ordinary pad input. Most fixtures then set only initial
oxygen and health, run two fields and save a checkpoint. Sidecars retain all
setup commands, source/checkpoint hashes, builds, identities and sample hashes.
Replays never poke memory, change code or alter the world.

`full_air_heldout` does not change either counter. It records4380 fields from
the existing submerged checkpoint through the entire remaining air countdown,
health depletion, death animation and settled floating pose.

| Recording | Scope and provenance |
|---|---|
| `exhaustion` | Calibration: zero-air boundary and continued five-point damage |
| `tread_death` | Calibration: death dispatch, buoyancy margin and held terminal124 pose |
| `moving_death` | Calibration: glide must finish before death, retained momentum, pitch/bank recovery and early rejected pad input |
| `exhaustion_heldout` | Independent: exhausted survivors can still stroke, steer and collide |
| `opposite_death_heldout` | Independent: opposite bank/pitch, braking and held terminal pose |
| `full_air_heldout` | Independent: full remaining countdown and death, no counter pokes |
| `rising_death_heldout` | **Calibration** despite its historical name: exposed the surface-death branch and was used to refine it |
| `surface_death_validation_heldout` | Fresh independent validation of combined submerged/surface death rules |
| `surface_death_late_heldout` | Fresh independent validation with later rise and an earlier lethal boundary |
| `air_recovery_heldout` | Fresh independent survivor: exhausted stroke reaches the surface, health remains980 and air refills |

`game/tests/swimming.rs` compares3710 logic ticks across these ten complete
recordings, six independent. All17 gameplay columns are checked at every tick:
position, room, water mode, angles, turn rate, both speed fields, authored
clip/frame/state/goal, oxygen and health. Inputs and initial states are the
only supplied gameplay values.

## Limits and diagnostic

The separately retained `drowning_moving_death_menu_diagnostic` keeps the
original longer post-death pad hold. Its last gameplay update is at field128.
`tools/record_drowning.py --death-flow` retains a complete focused native trace
and counter samples in `drowning_death_flow.json`: at field129 ControlPhase
reads death ticks61, input0x16 and gate0, then returns0x500 through
`0x8001DA74`/`0x8001DD90`. This explains the stopped player updates; it is an
outer game-flow boundary rather than delayed water animation. The subsequent
outer-loop transition remains outside this controller comparison. The ordinary
motion replay releases pad input before that boundary and continues through
its full declared220 fields. No arbitrary pause is fitted to gameplay.

These records verify death caused by oxygen depletion. Combat, traps or other
damage sources selecting water death, the death menu/camera, underwater weapons,
audio and exact water-surface rasterization remain outside this claim. Native
water distortion and bubbles are still separate appearance work.

## Verification

The real-game MCP route is `python3 tools/swimming_smoke.py --suite drowning`.
It checks reflected gameplay values and the rendered body quaternion, aligns
only the comparison camera after each gameplay checkpoint, and pairs native
snapshots with the existing four-field display offset. The long countdown uses
its retained20-field native screenshot cadence; trajectory tests still compare
every logic tick.

All374 workspace tests pass, as do all-target builds and Clippy (existing
unrelated warnings remain). Changed Rust files pass rustfmt directly;
workspace-wide formatting differences outside these files remain. All182
full-game MCP checkpoints pass across the ten routes. Native/rebuilt submerged
death, moving death and final surface-death views were inspected. Body motion
and authored poses agree; HUD styling, water distortion and exact raster
parity are not claimed.

The engine sandbox passes its real movement, step/wall and rendering check.
The full editor sandbox and Home smokes pass byte-exact untouched saves,
undo/redo, save/reopen, mesh/link operations and Lara's terminal clip124
preview. The Home check uses placement8, the level's Lara placement. Original
level files remain unchanged. Reports and exact MCP operations are retained
under `validation/drowning/` and `validation/editor-smoke/`.
