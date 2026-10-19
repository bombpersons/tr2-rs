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
avoidance, obstacle avoidance, controller-triggered voice
sounds, random sounds and creature activation scheduling remain unported.
The current floor guard refuses movement onto a different height; it is a
bounded fallback, not a verified reconstruction of creature navigation.
The animated contact milestone below verifies ordinary open-space Lara pushing.

Milestone checks: `cargo test --workspace --all-features` and Clippy completed;
Clippy's existing warnings remain outside this controller. The engine sandbox
ran offscreen and produced a nonblank frame. The editor Home placement9
preview/start-to-moving check and exact untouched save passed. The full-game
smoke passed32 ordinary checkpoints, followed by eight additional local-run
views centered on the attendant. Original and rebuilt route views, the posed
attendant, editor preview and sandbox screenshots were visually inspected.
The workspace formatting check reports pre-existing differences elsewhere;
the new controller/tests and changed heading module pass rustfmt directly.

## Animated contact and avatar pushing

The contact milestone replaces pass-through for type260 with the sequence
observed in its collision callback `0x8001C65C`: coarse animated bounds,
posed mesh spheres, then an expanded-box push. It does not use an always-solid
capsule or the whole model's static bounds.

`0x8001CCA4` chooses the nearest authored key for both vertical intervals;
strict touching endpoints do not overlap. It rotates Lara's origin into the
obstacle's frame and tests XZ inclusively against that key's bounds expanded
by the100-unit Lara context radius (`0x800381E8`). `0x800289D4` chooses the
next key only beyond halfway; ties choose the earlier key. The shortened
last-interval test retains the source's comparison with absolute clip end.
The engine exposes this separately from interpolated clearance bounds.

The sphere generator `0x8005BAF8` uses the same single key, its root offset,
authored joint tree and every mesh's center/radius. The overlap test
`0x8005B990` uses strict3D squared distance and stores an obstacle-mesh mask.
No extra runtime joint rotations are read in the four retained calls; the
attendant's skeleton flags use only stack push/pop bits. The Bevy implementation
uses ordinary posed transforms. The82 independently captured sphere centers
are within1.022 source units of the original, with exact radii and masks
0x10000/0x110000 in the two complete actor-pair fixtures. A4-unit regression
bound explicitly allows intermediate GTE truncation differences; this is not
an implementation of GTE arithmetic. `tools/inspect_winston.py
--record-contact-spheres` reproduces the two native instruction traces and
exports the actual output stores, without reconstructing the transforms.
The checked-in JSON retains commands, frame/clip/root identity and trace hashes.

On sphere overlap, `0x8001C890` selects the closest expanded-box edge with
tie priority minX, maxX, maxZ, minZ. Local/world products are summed before
the arithmetic shift; this makes the final integer edge corrections exact.
Type260 passes the non-stagger option, preserving Lara's animation and speed.
AnimateItem clears last tick's contacts at `0x8001DDC8`, after the attendant's
controller has had a chance to read them. Contact then publishes this tick's
new mask. The port keeps those boundaries explicitly.

`game/tests/winston_contact.rs` runs both real controllers and the same pad
inputs for285 ticks across four complete ordinary routes: airborne backward
contact, a delayed airborne hop, sustained backward walking and walking
release/recovery. The last three were first recorded after implementation.
All18 compared columns match each tick, including Lara position, signed speed,
yaw, animation, state and goal, plus the attendant's pose and mesh contact mask.
No state or position is supplied after tick0. Engine tests also distinguish
nearest-key contact from clearance interpolation and retain joint ids when a
mesh has no positive-radius sphere.

World validation currently bounds pushes using floor/body-clearance probes
and restores the tick's original XZ if rejected. The original's general
collision call uses body height762, upward tolerance384 and a temporary
movement heading (`0x8001CBC8..0x8001CC6C`). These four routes accept their
candidates in open space. Wall/static/step rejection, room-list traversal,
context-bit changes in special player modes, multiple-object ordering and the
source's object-contact-before-avatar-state-collision boundary still require
separate runtime verification. The current combined avatar motion/collision
phase has not been generalized on the strength of these open-space routes.

Contact checks: the all-features workspace tests and Clippy completed; existing
unrelated warnings remain. All four full-game MCP contact replays pass14
checkpoints each (56 total), including sampled masks, exact Lara corrections,
authored poses and separate attendant-centered views. The immediate walking
fixture seeds the checkpoint's sampled prior collision heading after the
bootstrap tick; no gameplay correction is applied during replay. Native/rebuilt
contact views and attendant poses were visually inspected. The rebuilt engine
sandbox ran and the editor Home idle-pose preview and untouched save passed.
No template or persistent level-format changes were needed.
