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
  Ordinary room-wall/ledge/step, shallow-slope and steep uphill rejection
  routes are done; static obstructions, corner boundaries, low ceilings,
  steep downhill and airborne deflection remain.
- [ ] Landing recovery: original animation dispatch and input recovery windows.
  Standing/forward jumps, hang drops and low/middle/high forward wall impacts are done;
  unassisted fast falls, slide landings and backward/sideways jump collision
  remain.
- [ ] Grounded run-start wall stop and held-jump retry. The retained
  `run_start_retry_probe` diverges at clip11 entry and the following jump
  request; see [diagnostic](docs/low-airborne-evidence.md).

## Completed traversal milestones

- [x] Ordinary vertical/forward ledge jump, reach, grab, hang, pull-up and
  drop; independent early-climb, early-drop and missing-Action validation.
  [Evidence](docs/traversal-evidence.md), [tests](game/tests/traversal.rs).
- [x] Authored standing/forward/running jump preparation and landing recovery;
  both running feet, landing into a run and a second jump through an authored
  cancel window. [Evidence](docs/traversal-evidence.md),
  [tests](game/tests/traversal.rs).
- [x] Ordinary backward walk/hop collision: wall and ledge stops, one-click
  step up/down, shallow-slope descent, steep uphill rejection and committed
  hop-fall/landing recovery. Four independent validation routes pass.
  [Evidence](docs/backward-collision-evidence.md),
  [tests](game/tests/backward_collision.rs).
- [x] High forward room-wall rebound: retained movement heading, signed speed,
  fast-fall animation and heavy landing recovery; contrasting wall reach,
  airborne steering and two independent validation routes pass.
  [Evidence](docs/airborne-evidence.md), [tests](game/tests/airborne.rs).
- [x] Low/middle forward room-wall impacts: repeated speed reduction without
  heavy landing, low-impact state override and locked recovery, plus blocked
  forward takeoff's vertical fallback. Two new independent validation routes
  pass; the earlier turn-goal calibration remains explicitly labelled.
  [Evidence](docs/low-airborne-evidence.md), [tests](game/tests/airborne.rs).

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
- 2026-10-18: Backward room-collision milestone: fourteen PSX recordings,
  1870 logic ticks and thirteen compared columns; four independent routes
  recorded after implementation pass without fitting. All 53 full-game MCP
  pose checks across eleven routes pass; the workspace tests, Clippy, sandbox
  run and editor byte-exact save/animation smoke pass. Existing unrelated
  formatting differences and lint warnings remain. The broad backward
  collision item remains open for the exceptional branches listed above.
- 2026-10-18: High airborne wall-rebound milestone: five PSX recordings,
  515 sampled logic ticks and thirteen compared columns, with two independent
  validation routes. All 39 full-game MCP pose checks pass. Workspace tests,
  Clippy, sandbox movement/rendering, and editor sandbox/Home save and authored
  landing preview checks pass. Existing unrelated formatting differences and
  Clippy warnings remain; changed Rust files are formatted. No engine format
  or template change was needed.
- 2026-10-18: Low/middle airborne wall-impact milestone: six additional
  passing recordings, 676 additional sampled logic ticks and thirteen compared
  columns. The airborne suite now covers 1191 ticks and 80 full-game MCP poses.
  Low impacts now keep state14 through clip31's end; slow-to-fast
  turns now request their goal before waiting for the authored dispatch range.
  Workspace tests (186 passed), Clippy, sandbox movement/rendering, and editor
  sandbox/Home byte-exact saves and clip31 preview checks pass. Existing
  unrelated formatting/lint differences remain. A separate diagnostic retains
  the grounded run-start wall-stop/held-jump retry mismatch without claiming
  that branch as verified. No engine format or template changes were needed.
- Next: grounded run-start wall stop and held-jump retry, corner/ceiling
  airborne wall impacts and unassisted fast-fall/slide landing recovery, then
  the remaining collision/traversal branches and water. Combat, interactions
  and appearance remain queued above.
