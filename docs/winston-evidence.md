# Home attendant: local follow and authored motion

The type260 attendant previously rendered and looped its first clip without
an object controller. It now follows Lara locally, starts and stops through
the level's authored dispatch ranges, and turns using the previous tick's
movement speed. The controller is in `game::winston`; thresholds are editable
in the existing `game/data/object_types.json` type260 entry. The engine supplies
clip dispatch, model posing and integer-quantized heading/trigonometry.

## Sources and observed rules

Only the supplied Tomb Raider II USA Rev3 disc and `psx` were used. Runtime
RAM `.psx/home_start/ram.bin` binds object260 at `0x800936D0 +260*48` to
control `0x800500A0`. Its collision entry is separate. The retained local
runs start at `.psx/home_runway_start/state.bin`, without memory pokes;
checkpoint digests and exact commands are in the checked-in trajectory JSONs.
`tools/record_winston.py` reproduces them. The original investigation's
listings and captures remain under ignored `validation/winston-investigation/`.

- Control `0x80050128..0x800501AC`: idle state1 requests moving state2
  beyond squared distance2359296 (1536²), or if Lara is behind. Moving requests
  idle strictly below that distance while Lara is ahead. Ahead excludes
  angles of exactly±16384. The decision precedes the turn.
- `0x800172E0..0x80017430`: no turn while the previous speed or turn limit
  is zero. Otherwise aim at the creature's path target, clamp to182 binary
  angle units/tick, and halve the cap when a target behind is inside the
  speed-derived turn radius. Creature-slot init `0x80048F40` sets the limit.
- Heading `0x80063EC4`, lookup at `0x80082FF4`: quantize the minor/major
  ratio to1/2048 before looking up its rounded angle; signed octants have
  asymmetric offsets. Keeping this quantization avoids accumulated drift.
- AnimateItem `0x8001DD98` advances/dispatches before evaluating authored
  speed and moving. Clip283 accelerates into280; clip281 decelerates into282.
  The four clips' data are already in the standalone level file. No new clip
  timing or speed constants are embedded in the controller.
- ControlPhase runs items before Lara. Each item tick therefore follows the
  preceding avatar position. The headless comparison deliberately supplies
  those observed positions; the full-game smoke instead replays pad inputs
  through the real avatar controller after initial alignment.

## Verification

`game/tests/winston.rs` compares1260 item ticks across four complete ordinary
routes: local running departure, shorter running departure, a diagonal turn
and a walking departure. The latter three were first recorded after the local
rules were written. Position x/y/z, yaw, clip, absolute frame, state, goal,
speed and room match exactly at every tick, including full authored stops.
The registry regression in `game/tests/interactions.rs` checks that type260
receives its controller once. `tools/winston_smoke.py` checks the running
standalone game's item poses and captures its views for visual inspection.

The editor already previews these clips through the engine's existing
animation format. Its type names come from the updated registry table; no new
level fields or editor dependency on the game were introduced.

## Limits

This milestone verifies open, flat, local movement with a direct path target.
The older `away`/`turn` investigation routes cross navigation boxes, where the
original can clamp its target to a corridor. Full path search, step/ledge
avoidance, obstacle avoidance, Lara contact/pushing, controller-triggered voice
sounds, random sounds and creature activation scheduling remain unported.
The current floor guard refuses movement onto a different height; it is a
bounded fallback, not a verified reconstruction of creature navigation.
Mesh-sphere contact requires separate verification before extending this claim.

Milestone checks: `cargo test --workspace --all-features` and Clippy completed;
Clippy's existing warnings remain outside this controller. The engine sandbox
ran offscreen and produced a nonblank frame. The editor Home placement9
preview/start-to-moving check and exact untouched save passed. The full-game
smoke passed32 ordinary checkpoints, followed by eight additional local-run
views centered on the attendant. Original and rebuilt route views, the posed
attendant, editor preview and sandbox screenshots were visually inspected.
The workspace formatting check reports pre-existing differences elsewhere;
the new controller/tests and changed heading module pass rustfmt directly.
