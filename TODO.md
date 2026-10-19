# Port verification and completion

Agreed 2026-10-18. Baseline committed and pushed as `5bedba0` before starting.
Use only the supplied disc and `psx` for new reverse-engineering evidence.
Existing inherited behavior is provisional until independently checked.

Check an item only after implementing it, retaining PSX evidence, adding
meaningful regression coverage and completing the applicable workspace,
sandbox, editor and visual checks. Link the evidence and tests when checking
it off. Record partial progress below without checking the parent item.

## Recent rendering and collision fixes

- [x] Missing room geometry at the Wall entrance and Home training course:
  mesh/material restoration and portal traversal ordering, with pixel-checked
  camera-return regressions. [Notes](docs/room-visibility-fix.md).
- [x] Forward jumps under flat low ceilings: retain heading and horizontal
  speed, stop ascent, then use ordinary authored landing/recovery. Three
  PSX controlled-spawn replays include two independent validation routes.
  [Evidence](docs/ceiling-evidence.md), [tests](game/tests/airborne.rs).
  Side/corner and forward-top contacts remain open.
- [x] Vertical jumps under flat low ceilings:870-unit body correction,
  preserved upward speed on contact, authored standing/landing dispatch and
  repeated held-Jump recovery. Three controlled-spawn PSX routes include two
  independent validation recordings. [Evidence](docs/ceiling-evidence.md),
  [tests](game/tests/airborne.rs).

- [x] Reach against flat room ceilings: preserve clip/goal and forward speed,
  correct the body height and stop ascent. The early-contact control and four
  later-contact recordings distinguish calibration from independent validation.
  [Evidence](docs/ceiling-evidence.md), [tests](game/tests/airborne.rs).
- [x] Neutral running release and Up re-held during authored braking: retain
  movement until the stop range, use both braking clips and wait for their
  end before restarting. Five additional routes, with re-hold calibration
  explicitly labelled. [Evidence](docs/run-stop-evidence.md),
  [tests](game/tests/airborne.rs).
- [x] Authored forward walking release and restart: both foot ranges,
  committed stop/re-hold and release during walk-start/acceleration. Airborne
  Walk requests and braking-wall state overrides also pass the former diagnostics
  and five independent routes. [Evidence](docs/walk-stop-evidence.md),
  [tests](game/tests/airborne.rs).

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
  ordinary running ledge falls now use authored running/standing landings.
  Unassisted fast falls, slide landings and backward/sideways jump collision remain.
- [x] Ordinary grounded run-start room-wall stop and held-jump retry.
  The retained diagnostic and two fresh independent routes pass; see
  [evidence](docs/run-start-evidence.md) and [tests](game/tests/airborne.rs).
  Angular/corner/ceiling and static-obstruction sweeps remain separate work.

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
- [x] Ordinary run-start room-wall stop into the authored standing pose,
  held-Jump retry and release-before-contact behavior. Two independent
  validation routes pass. [Evidence](docs/run-start-evidence.md),
  [tests](game/tests/airborne.rs).
- [x] Ordinary cardinal running ledge departure and authored running/standing
  landing, with release, ignored airborne jump and later restart. Three complete
  routes include two independent validations. The subsequent walking milestone
  resolves its Walk and braking-wall diagnostics. [Evidence](docs/ledge-fall-evidence.md),
  [tests](game/tests/airborne.rs).

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
- 2026-10-18: Run-start room-wall stop and held-jump retry: the unchanged
  diagnostic is now passing calibration, joined by two fresh independent
  routes. All 333 additional sampled logic ticks pass across thirteen columns.
  All 106 full-game MCP pose checks pass across fourteen routes.
  Workspace tests (189 passed), Clippy, sandbox movement/rendering, and editor
  sandbox/Home byte-exact save and clip11 preview checks pass. Changed Rust
  files are formatted; unrelated workspace format/lint differences remain.
  No engine format or template change was needed.
- 2026-10-18: Vertical low-ceiling milestone: three new controlled-spawn PSX
  recordings,333 sampled logic ticks across thirteen columns, including two
  independent validation routes. The first contact now requests authored
  landing without shifting Lara sideways or cancelling her upward speed.
  All63 full-game ceiling poses pass across six forward/vertical routes.
  Workspace tests (198 passed), Clippy, sandbox movement/rendering and editor
  sandbox/Home save and clip28 preview checks pass. Changed Rust files are
  formatted; unrelated workspace format/lint differences remain. No engine
  format or template change was needed. [Evidence](docs/ceiling-evidence.md).
- 2026-10-18: Reach-ceiling and authored run-stop milestones: ten new PSX
  recordings compare1170 sampled ticks across thirteen columns. Reach now
  corrects centre ceiling contacts without a sideways wall shift. Neutral
  Up release waits for the authored braking range; re-holding Up keeps the
  committed brake before restarting. Two recordings used to refine rules
  retain their original names and are explicitly labelled calibration.
  Independent validation passes. All208 workspace tests and280 full-game
  MCP poses pass. Clippy, builds, sandbox movement/rendering and editor
  sandbox/Home save and clip10 preview checks pass; existing unrelated
  lint/format differences remain. No engine format or template changes.
  [Ceiling evidence](docs/ceiling-evidence.md),
  [run-stop evidence](docs/run-stop-evidence.md).
- 2026-10-18: Running ledge-fall milestone: three complete no-poke PSX routes,
  348 sampled ticks across thirteen columns, including two independent validation
  routes. Run departures into clip34 now use the existing authored forward-flight
  controller and land into running92 or standing82. Two additional diagnostic
  recordings preserve Walk/goal52 and later braking-wall state12 gaps without
  parity claims. Workspace tests (211 passed), Clippy, all-target builds, sandbox
  movement/rendering and editor sandbox/Home saves and clip92 preview pass.
  The full airborne smoke passes201 poses across22 routes, including41 new
  ledge-fall poses.
  [Evidence](docs/ledge-fall-evidence.md).
- 2026-10-18: Walking-stop and braking-wall milestone: the two unchanged prior
  diagnostics now pass as explicitly labelled calibration, joined by five fresh
  independent routes. All897 sampled ticks across thirteen columns pass. Walking
  finishes authored strides and stops before restarting; airborne Walk requests52
  without interrupting the fall clip, and braking-wall contact retains state12
  until clip11's natural end. Workspace tests (218 passed), Clippy, all-target
  builds, sandbox movement/rendering, and editor sandbox/Home byte-exact saves
  and clip3 preview pass. No engine format or template change was needed.
  All283 full-game poses across29 routes pass, including82 new poses.
  A fresh diagnostic retains a vertical jump's front-wall clearance mismatch
  beginning at field228. [Evidence](docs/walk-stop-evidence.md).
- Next: vertical jump clearance over the front wall (the retained braking-wall
  jump diagnostic), corner airborne ceiling impacts, fast-fall/slide recovery,
  remaining traversal and water.
  Combat, interactions and appearance remain queued above.
