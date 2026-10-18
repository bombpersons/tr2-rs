# Port verification and completion

Agreed 2026-10-18. Baseline committed and pushed as `5bedba0` before starting.
Use only the supplied disc and `psx` for new reverse-engineering evidence.
Existing inherited behavior is provisional until independently checked.

Check an item only after implementing it, retaining PSX evidence, adding
meaningful regression coverage and completing the applicable workspace,
sandbox, editor and visual checks. Link the evidence and tests when checking
it off. Record partial progress below without checking the parent item.

## Traversal

- [ ] Jump, grab and hang: original grab conditions, alignment, animation and
  collision behavior; climb from a hang and release to drop. Include a complete
  recorded route and an independently recorded validation route.
  Ordinary vertical/forward routes are done; static obstruction, angular
  boundaries and climbable-wall/alternate-catch variants remain.
- [x] Three-click climbs: ordinary entry, authored timing and final offset;
  original and independent release/turn routes pass. See
  [evidence](docs/traversal-evidence.md) and [tests](game/tests/traversal.rs).
- [ ] Backward movement against walls, steps, drops and slopes.
- [ ] Landing recovery: original animation dispatch and input recovery windows.
  Standing/forward jumps and hang drops are done; unassisted/fast falls,
  slide landings and backward/sideways jump collision remain.

## Completed traversal milestones

- [x] Ordinary vertical/forward ledge jump, reach, grab, hang, pull-up and
  drop; independent early-climb, early-drop and missing-Action validation.
  [Evidence](docs/traversal-evidence.md), [tests](game/tests/traversal.rs).
- [x] Authored standing/forward/running jump preparation and landing recovery;
  both running feet, landing into a run and a second jump through an authored
  cancel window. [Evidence](docs/traversal-evidence.md),
  [tests](game/tests/traversal.rs).

## Water

- [ ] Swimming: acceleration, braking, turning, pitch, collision and animation.
- [ ] Water transitions: entry, surface, submerge and climb out; oxygen behavior.

## Combat and interactions

- [ ] Pistols: draw/holster, firing cadence, targeting and shot/damage timing.
- [ ] Tiger and spider behavior: attack selection, contact and damage windows.
- [ ] Pickups: alignment, animation and collection timing.
- [ ] Switches and key/puzzle use: alignment, animation and trigger timing.
- [ ] Doors: collision and floor/portal changes during opening/closing,
  including occupancy override.

## Appearance

- [ ] Distance darkening: original room/object depth shading and boundaries.
- [ ] Distant texture LOD: original >=8192 room policy and texture selection.
- [ ] Home sky: extract and render with the original placement and behavior.

## Progress

- 2026-10-18: Baseline `cargo test --workspace --all-features` passed before
  checkpoint commit/push. GPU/MCP tests are opt-in and were not run in that check.
  Ledge traversal is the first investigation.
- 2026-10-18: First traversal milestone: sixteen PSX recordings, 1771 sampled
  ticks and thirteen compared columns; independent validation retained apart
  from the explicitly labelled alternate-takeoff calibration capture. The game
  MCP smoke passed52 aligned poses over nine routes. The workspace tests,
  engine sandbox run and editor save/animation smoke passed. The editor still
  preserves untouched files byte for byte; no format or template changes were
  required. Existing workspace formatting differences and Clippy warnings
  outside the changed Rust files remain.
- Next: backward wall/step/drop/slope collision, followed by the remaining
  traversal branches and water. Combat, interactions and appearance remain
  queued above; none is marked complete by the traversal work.
