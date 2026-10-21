# Port verification and completion

Agreed 2026-10-18. Baseline committed and pushed as `5bedba0` before starting.
Use only the supplied disc and `psx` for new reverse-engineering evidence.
Existing inherited behavior is provisional until independently checked.

Check an item only after implementing it, retaining PSX evidence, adding
meaningful regression coverage and completing the applicable workspace,
sandbox, editor and visual checks. Link the evidence and tests when checking
it off. Record partial progress below without checking the parent item.

## Recent rendering and collision fixes

- [x] Airborne side/corner contacts for forward jumps and reach: whole-body
  GetCollisionInfo with per-handler limits, the side-contact 910 turn,
  ceiling stop, and reach's front-floor landing contact. Seven controlled-spawn
  routes compare 727 ticks, three independent. A released run's wall stop
  now predicts its stop clip. [Evidence](docs/corner-jump-evidence.md),
  [tests](game/tests/airborne.rs). Vertical-jump/fast-fall sides, clamp and
  static meshes remain separate work.

- [x] Animated item pushes beside a raised room-sector corner: use the native
  directional body probes and restore the last accepted position on contact.
  Four controlled-initial-placement routes compare380 controller ticks,
  including three independent validations and a nearby clearance control.
  Positions and authored behavior match; per-mesh sphere-mask differences
  remain explicit diagnostics. [Evidence](docs/item-push-evidence.md),
  [tests](game/tests/winston_contact.rs). Static, slope and other collision
  contexts remain separate work.

- [x] Frontal oblique running room-wall contact: restore the tick's previous
  tangent coordinate; use verified quantized ground/braking motion. The former
  navigation diagnostic and five additional routes match2745 ticks, including
  two fresh combined-rule validations. [Evidence](docs/oblique-wall-evidence.md),
  [tests](game/tests/winston_contact.rs). Side/corner/static/ceiling combinations
  remain separate work.

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

- [x] Ordinary forward walking against flat room walls, ledges and one-click
  steps: blocked starts, collision stops with standing control, both ascending
  and descending feet, release during steps and an ignored recovery jump.
  Nine complete PSX routes include five independent recordings.
  [Evidence](docs/forward-walk-evidence.md), [tests](game/tests/airborne.rs).
  Angular/static/ceiling/slope and walk/run transition branches remain.

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
  Unassisted fast falls, slope landings into slides, slide-edge falls and the
  back-fall wall rebound now match six routes
  ([evidence](docs/fall-recovery-evidence.md)). Slide jumps, the back jump,
  mid-air twists, back-flight fast falls and the shared ledge fall match
  twelve more ([evidence](docs/slide-jump-evidence.md)). Side and
  directional standing jumps, the running twist and the hop fall's reach
  match sixteen more ([evidence](docs/side-jump-evidence.md)). The stop
  collision's ceiling/clamp restore and wading compression remain.
- [x] Ordinary grounded run-start room-wall stop and held-jump retry.
  The retained diagnostic and two fresh independent routes pass; see
  [evidence](docs/run-start-evidence.md) and [tests](game/tests/airborne.rs).
  Angular/corner/ceiling and static-obstruction sweeps remain separate work.

## Completed traversal milestones

- [x] Fast-fall impact damage boundary and authored fatal ground pose:
  controlled initial-motion fixtures verify140/141/142/147/153/154/155,
  retained lethal contact and terminal pose. Native first-death wait timing
  is a separate diagnostic, not a fixed-rate parity claim.
  [Evidence](docs/fall-damage-evidence.md), [tests](game/tests/airborne.rs).

- [x] Ordinary authored rolling: entry offset, turn effect, deceleration,
  wall/step correction, held repetition, recovery walk and a second-half
  backward departure/landing. Ten complete PSX routes include six independent
  recordings. [Evidence](docs/rolling-evidence.md), [tests](game/tests/airborne.rs).
  Corner/static/ceiling/slope and other launch/departure cases remain.

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
  Ordinary submerged stroke/glide/tread, exact turning/pitch/bank, authored
  dispatch and oxygen consumption now match700 ticks across six complete
  replays, including three independent validations. Wall, floor and ceiling
  collision (0x80045CB8) matches four further independent routes and both
  back-hop entries to their end. Current/roll branches remain. Shallow-water stand-up and wading were
  subsequently ported in [a separate milestone](docs/wading-evidence.md). [Evidence](docs/swimming-evidence.md),
  [tests](game/tests/swimming.rs).
- [x] Oxygen exhaustion and authored submerged/surface death: signed damage,
  delayed animation dispatch, braking, angular recovery, buoyancy and terminal
  poses. Ten complete PSX routes compare3710 ticks across17 gameplay columns,
  six independent, including a full remaining-air countdown without counter
  pokes. The later native death-flow return is a separate diagnostic.
  [Evidence](docs/drowning-evidence.md), [tests](game/tests/swimming.rs).
- [ ] Water transitions: entry, surface, submerge and climb out; oxygen behavior.
  Surfacing, surface strokes/turns/wall contact, the tread dive, climb-out,
  deep-water entry and surface air refill now match eleven complete PSX routes,
  five independent. [Evidence](docs/surface-evidence.md),
  [tests](game/tests/surface.rs). Swan-dive entry and the surface air bar remain; ordinary wading was
  subsequently verified in [a separate milestone](docs/wading-evidence.md).

- [x] Ordinary shallow-water wading: authored gait/start/stops, steering/bank,
  standing turns, backward/sidestep movement, submerged/surface stand-up,
  shallow exit and deeper forward surface entry. Thirteen complete PSX routes
  replay1599 observed controller ticks; six are independent. Nine complete
  chase-camera routes also pass. Native display scheduling, static contacts,
  exceptional collision and transition cameras remain separate.
  [Evidence](docs/wading-evidence.md), [tests](game/tests/wading.rs).

## Combat and interactions

- [ ] Home attendant navigation and contact. Local flat-floor follow, authored
  start/stop and turning now match1260 PSX ticks across four ordinary routes,
  including three independent recordings. [Evidence](docs/winston-evidence.md),
  [tests](game/tests/winston.rs). Cross-box navigation, room obstacles and
  blocked-world contact and unusual player modes remain. Adjacent flat-floor
  corridor targets now match1950 additional ticks across four complete routes
  (three independent); general multi-box/zone navigation remains. Ordinary animated
  contact/pushing now matches285 additional ticks, with three independent
  routes and retained native sphere-output fixtures.
  Extended vertical-contact routes and fresh-checkpoint provenance are in
  [the contact follow-up](docs/winston-evidence.md#extended-vertical-contact-verification).
  Rolling contact across the fountain's floor-height change now matches 300
  additional ticks, including two independent routes. The roll's floor response
  follows item pushes. The later bounded-height corridor query now also
  matches their navigation targets plus a fresh 120-tick independent route;
  physical creature height transitions remain open.
  [Navigation evidence](docs/navigation-height-evidence.md).
  [Evidence](docs/item-push-evidence.md#rolling-contact-before-floor-response).


- [x] Pistols: draw/holster, firing cadence, targeting and shot/damage timing.
  LaraGun's status machine, held Draw/Action, arm frames, recoil and the
  no-target re-tap quirk, target search/locks/arm swing, head/torso and
  ResetLook, spread from the control random generator, sphere hits,
  HitTarget, the fast turn with guns ready and the combat camera match18
  no-poke routes tick by tick, nine held out, including a pad-only tiger
  checkpoint with every shot's hit/miss and a kill.
  [Evidence](docs/pistols-evidence.md), [tests](game/tests/pistols.rs).
  Other guns, flares, hands-busy climbs/pickups and ricochets/smashables
  remain. The slide's camera flag (exact slopes) now keeps the tiger
  routes' camera exact through the knock-down slide.
- [x] Tiger and spider behavior: attack selection, contact and damage windows.
  Shared creature routines (slots, AI info, mood, box search, turn, tilt,
  head, animation, bad floor, room-ordered creature collisions, death) with a
  control function per type. Doors and blocks block their navigation boxes.
  Seven tiger routes (2721 ticks, four held out) and five spider routes
  (1960 ticks, three held out) match tick by tick.
  [Evidence](docs/creature-evidence.md), [tiger tests](game/tests/tiger.rs),
  [spider tests](game/tests/spider.rs). The other creature types, effect
  pieces, blood, flip zones and flying creatures remain.
- [ ] Pickups, switches, keys, puzzle holes and doors: ported from the
  binary ([evidence](docs/interaction-evidence.md),
  [tests](game/tests/using.rs)). The original bounds and alignment tables,
  the use clips with their camera requests and hands-busy, the pickup taken on
  frame 42, switch status/timer and SwitchControl, switch/key/pickup trigger
  tests (targets change once the switch's own clip ends), door sectors made
  solid while shut, and door pushes through the shared contact pass.
  A dry-land key pickup, Home's front-door button and Wall's first keyhole,
  with the doors they open, match no-poke `psx handoff` recordings tick for
  tick, camera included, and so does running into a closed door: ground
  state collision waits for object pushes, so Lara stays running pinned
  against it. Airborne collision still runs before pushes. The underwater
  key pickup (MoveLaraPosition, clip 130) and Venice's underwater switch
  (SwitchCollision2, clip 129, on the flipped level) match their recordings
  too, and so does the Opera House Relay Box puzzle hole (Lara stands 327
  in front, not 362 as at keyholes). The Offshore Rig's timed cell button
  and door match too, now that a trigger's timer is held by the object and
  counted by its control (`WorldObject::trigger_active`). Still to record:
  the inventory ring.
  Not ported: the inventory ring at keyholes (the matching key is used
  directly), flares, flipping in play, type 92's own clip and trigger,
  Venice's door hold, the object-collision room list.
- [ ] State camera requests: Lara's state handlers set camera angle/elevation
  offsets (`0x8008B29C`/`0x8008B2A0`) and flags 1 every tick: slide
  elevation -8190, and the handlers at `0x8003982C`, `0x8003988C`,
  `0x80039BC8`, `0x80039CE0`, `0x8003A134`, `0x8003EADC`, `0x8005EFD8`.
  Only the slide's flags 2 (exact slopes) and the use states' angle,
  elevation, distance and speed (`Avatar::camera_angle` and friends) are
  ported. Needs chase-mode recordings. [Notes](docs/chase-camera-evidence.md).

## Appearance

- [x] Underwater green palette: exact red5 halving, wet-room/camera selection,
  per-mesh textures preserving live lighting and HUD. Native palettes cover
  all 32 red levels; fresh visual comparison and restoration checks pass.
  [Evidence](docs/underwater-palette-evidence.md).
- [x] Wet-room caustic vertex shades before subdivision, including authored
  vertex phase, animated field clock and dry-room isolation. All 64 wave-table
  entries exercised by native corner observations.
  [Evidence](docs/caustics-evidence.md), [tests](game/tests/caustics.rs).
- [x] Pistol presentation: arm poses from the pistol animation object
  (upper arms oriented from Lara's root, torso/head turns), hand/holster mesh
  swaps at draw frame 13 and per-arm holster frame 12, muzzle flash meshes
  and the gunfire dynamic light on items and room vertices. Draw, grip,
  ready, aim and recoil screenshots line up with native captures.
  [Evidence](docs/pistols-evidence.md#presentation),
  [tests](game/tests/pistol_presentation.rs), `tools/pistols_visual_smoke.py`.
  The additive gun-flash glow sprite (sprite sequences are not imported) and
  the braid's collision with the gun arms (HairControl 0x8002F67C reads the
  arm frames) remain.
- [ ] Water-surface rendering, distortion and bubbles.
- [ ] Distance darkening: original room/object depth shading and boundaries.
- [ ] Distant texture LOD: original >=8192 room policy and texture selection.
- [x] Home sky: render the baked model with original rotation-only placement,
  texture modulation and camera-relative movement. Runtime mesh/pose checks,
  independent projected-vertex validation and full-game on/off/translation
  checks pass. Pixel raster and unusual portal clipping remain unverified.
  [Evidence](docs/sky-evidence.md), [tests](game/tests/sky.rs).

## Extensibility and mods

- [x] Mod machinery: `GamePlugins` group, `replace_object_type`,
  `replace_creature`, layered asset folders (`engine::assets`) and
  `mods/example` (hand-made level, its own type and trigger action, a
  replaced pickup type; headless rules test and GPU/MCP render test).
- [x] Lara's asset contract in `docs/lara-contract.md`: the clip, state,
  state-change and command numbers, frame windows, joints and sound ids her
  controller relies on, named in `game::player::contract` and checked by
  `contract::check` (Wall and every playable level pass; tests in
  `game/tests/lara_contract.rs`).
- [x] A hand-made character meeting the ground-locomotion part of that
  contract (stand, walk, run, turn, stop, wall hits), driven by Lara's
  controller in `mods/example` (`tools/make_character.py`; headless and
  GPU tests).
- [ ] Give the mod's character more of the contract (jumps, backward,
  sidesteps, roll); its other clip numbers are standing fillers today.
- [ ] Let a character's table start anywhere: the controller's clip numbers
  are absolute (a model's clips must start at 0), and walk collision reads
  absolute frame windows (`contract::frame`), so a character's walk loop must
  sit at frames 22..=57.
- [ ] An enemy added to `mods/example` through the shared creature module.
- [ ] Object setup (`data/object_types.json`) loaded as an asset rather than
  compiled in, so a mod can replace it by file.

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
- 2026-10-19: Vertical front-floor clearance: the prior diagnostic now passes
  as calibration, with two fresh independent complete jump/landing routes. All373
  sampled ticks across thirteen columns pass. Workspace tests (221 passed),
  Clippy, all-target builds, sandbox movement/rendering, editor byte-exact saves
  and clip28 preview pass. Full-game MCP checks41 poses across three routes.
  No engine format or template change was needed.
  [Evidence](docs/vertical-clearance-evidence.md).
- 2026-10-19: Vertical jump steering: five complete passing routes,625 sampled
  ticks across thirteen columns, including two fresh independent routes. Signed
  backward acceleration, forward cap, early reversal and late-input cutoff match
  the original. Workspace tests (226 passed), Clippy, builds, sandbox movement
  and rendering, editor byte-exact saves and clip28 preview pass. Full-game MCP
  checks64 poses. A separate diagnostic retains item260 pushing at Home.
  [Evidence](docs/vertical-steering-evidence.md).
- 2026-10-19: Home sky: independently verified the existing baked model against
  PSX RAM and draw code. Two native GPU views validate72 projected cloud-band
  corners. The game renders a rotation-only background behind world geometry;
  full-game checks preserve foreground pixels and verify translation invariance.
  Workspace tests (229 passed), Clippy, builds, sandbox and editor checks pass.
  No engine format or template change. [Evidence](docs/sky-evidence.md).
- 2026-10-19: Forward walking room collision: nine complete PSX recordings,
  1309 sampled ticks across thirteen columns, including five fresh independent
  routes. Both stepping feet and collision-selected standing stops match the
  original. Workspace tests (238 passed), Clippy, builds,100 full-game MCP
  poses, sandbox movement/rendering and editor saves/clip60 preview pass.
  No engine format or template change. [Evidence](docs/forward-walk-evidence.md).
- 2026-10-19: Authored rolling: removed the provisional timer/manual turn.
  Ten complete PSX routes compare1220 sampled ticks across thirteen columns,
  including six fresh independent recordings. Workspace tests (248 passed),
  Clippy, builds,119 full-game MCP poses, sandbox movement/rendering and editor
  saves/146-to147 preview pass. No engine format or template change.
  [Evidence](docs/rolling-evidence.md).
- 2026-10-19: Fast-fall damage and fatal landing: ten passing controlled-motion
  fixtures compare513 sampled ticks across thirteen columns, including five
  fresh independent fixtures. Fixed the death goal/pose and retained the native
  negative-health sentinel. Workspace tests (258 passed), Clippy, builds,65
  full-game MCP poses, sandbox movement/rendering and editor saves/clip25 preview
  pass. The original first-death wait is retained as a timing diagnostic; its
  exact cause and display-field transition parity remain unverified.
  [Evidence](docs/fall-damage-evidence.md).
- 2026-10-19: First-half roll departures: three fresh post-implementation
  no-poke recordings pass without rule changes,413 sampled ticks across thirteen
  columns and44 full-game MCP poses. Forward flight, skipped turn, landing,
  flight goal52 and walk/run recovery match. Workspace tests (261 passed),
  all-target builds and Clippy pass. No runtime or format change was needed.
  [Evidence](docs/rolling-evidence.md#first-half-departure-verification).
- 2026-10-19: Oxygen exhaustion and water death: ten complete PSX routes,
  3710 exact logic ticks, six independent and182 full-game MCP poses. Authored
  submerged and surface death now retain water motion and collision. A focused
  native trace identifies the later held-input death-flow return separately.
  Workspace tests (374 passed), all-target builds, Clippy, sandbox movement
  and rendering, editor sandbox/Home save/reopen and Lara clip124 preview pass.
  Editor verification also fixed duplicate surface faces when closing room
  links, with an engine-only regression covering hole closure and disconnection;
  the smoke now selects the post-edit doorway face. No format or template
  changes. Existing unrelated format/lint warnings remain.
  [Evidence](docs/drowning-evidence.md).
- 2026-10-19: Pistol milestone: LaraGun, pistol draw/holster/aim/fire,
  target search, HitTarget, ResetLook, the guns-ready fast turn and the
  combat camera ported from the Wall runtime code. Eighteen no-poke routes
  (nine held out) match tick by tick; the tiger routes replay the creature's
  recorded pose and inject the recorded control-random state on shot ticks.
  `cargo test --workspace --all-features` (423 tests) passed; Clippy is clean
  for the changed files; the engine sandbox starts headless. Arm-pose
  rendering is queued under Appearance.
- 2026-10-19: Slide camera flag: the forward slide's handler sets camera
  flags 2, which keeps the camera's slope flattening off for that tick
  (`0x80019CF4`). Ported as `Avatar::camera_exact_slopes`; all six tiger
  routes now match the camera on every tick (was windowed to tick 102).
  [Evidence](docs/chase-camera-evidence.md).
- Next: remaining blocked-world/static item contacts, vertical-jump/fast-fall side contacts,
  remaining traversal and water.
  Combat, interactions and appearance remain queued above.

- 2026-10-20: Wading milestone: thirteen complete PSX routes replay1599 native
  controller ticks, six independent, including native input/tick watches for
  display pauses and catch-up groups. All gameplay comparisons pass; nine
  complete chase-camera routes pass. Required water contact is an independent
  component and the engine exposes an optional chase elevation. Workspace tests
  (441 including doc tests), all-target builds and Clippy pass; existing unrelated
  lint warnings remain. Standalone pose checks, sandbox movement/rendering and
  editor sandbox/Home saves plus Lara gait177 preview are retained. No persistent
  format or template change. [Evidence](docs/wading-evidence.md).
- 2026-10-20: Extended vertical item-contact verification: rebuilt a native
  format-9 Home checkpoint from the supplied disc, retaining the preparation
  history separately from older format-8 evidence. Three complete no-poke
  recordings compare390 controller ticks with both actors active, including
  two independent input routes. Existing contact rules pass without changes;
  added coverage for vertical motion, health, turn rate and attendant yaw,
  speed and room. Workspace tests pass444 tests; standalone MCP checks48
  poses and paired contact views were inspected. No runtime, asset-format,
  editor or template changes. Workspace all-target/all-feature Clippy passes
  with existing unrelated warnings.
  [Evidence](docs/winston-evidence.md#extended-vertical-contact-verification).
- 2026-10-20: Raised-corner item-push validation: replaced fixed-offset
  clearance checks with the existing directional body probes and verified
  native rejection/rollback against a retained first-tick failure. Four
  complete controlled-placement routes compare380 controller ticks; three
  are independent. Raw native mesh masks are retained, with contact-presence
  comparison explicitly separated from their bit-rounding diagnostic.
  Workspace tests pass448 tests, all-target builds and Clippy pass with
  existing warnings, and50 standalone poses match. Sandbox movement/rendering
  and editor Home byte-exact save/reopen/clip282 preview checks pass. No
  engine format or template change.
  [Evidence](docs/item-push-evidence.md).
- 2026-10-20: Raised-corner sphere diagnostic: eight native sphere clouds
  directly bound the previously unmeasured mesh-mask discrepancy. Maximum
  coordinate error is1.018 source units; the corner differs only on joint10
  at a0.236-unit native gap, while the nearby offset has full mask parity.
  Added a reproducible PSX recorder and regression retaining that limitation.
  All449 workspace tests and all-target/all-feature Clippy pass with existing
  warnings. No gameplay, presentation, format or editor change.
  [Evidence](docs/item-push-evidence.md#raised-corner-sphere-diagnostic).
- 2026-10-20: Rolling item-contact ordering: fixed the same-tick height response
  after an animated lateral push onto lower floor. Three complete native
  controlled-placement routes compare 300 ticks, two independently recorded
  after the fix; standalone MCP checks pass 57 poses. All 452 workspace tests,
  all-target builds and Clippy pass with existing unrelated warnings. Sandbox
  movement/rendering and editor Home save/reopen checks pass. Cross-height
  attendant targets are retained as an explicit diagnostic; other locomotion
  families' object/floor ordering and static collision remain separate work.
  No engine format or template change.
  [Evidence](docs/item-push-evidence.md#rolling-contact-before-floor-response).
- 2026-10-20: Bounded-height local corridor targets: the engine query now
  takes caller-supplied step/drop limits and an authored zone index
  (`CorridorLimits`); the flat-only query keeps its results. Winston uses the
  native rise256/drop512 and ordinary ground zone, verified against live
  creature, box, overlap and zone data. Four complete routes compare420
  controller ticks with exact navigation targets; a fresh 120-tick route
  independently validates target X/Y/Z. All 455 workspace tests and
  all-target/all-feature Clippy pass with existing unrelated warnings;
  standalone MCP checks pass 76 poses. Physical creature height changes and
  multi-box search remain open. No format or editor schema change.
  [Evidence](docs/navigation-height-evidence.md).
- 2026-10-20: Airborne side/corner contacts: forward-jump and reach collision
  now classify the body with GetCollisionInfo (reach uses bad_neg 0) and
  apply the native deflection tables: side turns of 910, ceiling stop,
  and clamp (binary only). A released run's wall decision now predicts the
  requested stop clip. Seven controlled-spawn routes compare 727 ticks;
  three are independent, and one is labelled calibration for the run-stop
  fix. All 462 workspace tests pass; Clippy has only existing unrelated
  warnings; the standalone smoke matches 69 poses. No engine, format, editor or
  template change. [Evidence](docs/corner-jump-evidence.md).
- 2026-10-20: Natural fast falls and slide-edge landings: forward flight
  requests fast fall at fall speed 132; landing recovery runs the stop
  collision's slide test; slides use authored clip speed and leave an edge
  into forward flight or the back fall; the back fall now shares forward-jump
  deflection. Six controlled-spawn routes compare 706 ticks, three
  independent. All 468 workspace tests pass; Clippy has only existing
  unrelated warnings; the standalone smoke matches 53 poses. A native-core
  stall on the mirrored ramp is documented, not fitted. No engine, format,
  editor or template change. [Evidence](docs/fall-recovery-evidence.md).
- 2026-10-20: Slide jumps, twists and ledge falls: slide controls request
  the forward or back jump through authored takeoffs; the back jump shares
  the hop fall's flight handler; Back/Roll twists between forward and back
  flight; back flight fast falls at 132; the stop collision adds the ledge
  fall and shift; every clip-driven grounded mode uses the shared ledge
  fall, and mid-air spawns and water exits enter forward flight. Twelve
  controlled-spawn routes compare 1372 ticks, seven independent. All 480
  workspace tests pass; Clippy has only existing unrelated warnings; the
  standalone smoke matches 70 poses. No engine, format, editor or template
  change. [Evidence](docs/slide-jump-evidence.md).
- 2026-10-20: Side and directional standing jumps: compression picks
  Forward, Left, Right or Back by the first clear floor; the right, left and
  standing back jumps share back flight at their own headings; Jump is taken
  only from stop and run; the running twist, hop-fall reach and reach fast
  fall match. Sixteen controlled-spawn routes compare 1376 ticks, nine
  independent. All 496 workspace tests pass; Clippy reports nothing in the
  changed files; the standalone smoke matches 67 poses. No engine, format,
  editor or template change. [Evidence](docs/side-jump-evidence.md).
