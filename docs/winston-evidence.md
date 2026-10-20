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
The later [raised-corner validation](item-push-evidence.md) replaces the
five-offset world-clearance approximation with directional body probes.

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

## Adjacent navigation-box corridor targets

The local controller now distinguishes Lara's position (awareness) from the
point Winston steers toward (navigation). `WorldGeometry::adjacent_path_target`
interprets existing authored box rectangles and overlap links. It implements
the flat, directly overlapping corridor subset of `0x80015DD4..0x800161DC`:
when outside the destination on one axis, keep the actor's perpendicular
coordinate within its512-unit clearance margins; geometrically inside the
destination, resume direct aiming at Lara. Geometric containment matters:
boxes overlap, so the actor can enter the destination rectangle before its
sector's selected box id changes.

The binary evidence is the retained Home `path-target.disasm.txt`, plus `psx
 ghidra decompile --analysis .psx/ghidra/81851f7f0f07d25e404efd4e5af7c7665abdb2a13a7af60fbb2b6db01a7e54e5/analysis.json
 --addr 0x15DD4`. The cached analysis used Wall RAM; the whole function's bytes
`0x15DD4..0x16354` match Home RAM exactly, SHA256
`a6b1391fb0a5ead9cfd602a8e777f0089b65fdd83c40a02ea4d8a43edc3c4c12`.
The pseudocode was interpreted against the original MIPS and runtime targets.
Its warnings about load-delay/GTE semantics do not establish runtime behavior;
the retained per-tick target comparisons provide that check here.

In the long running route, Lara moves from sector-box135 into18. Winston
retains target Z95723 for hundreds of fields instead of steering immediately
to Lara's Z95805. Once his geometric position enters box18 during deceleration,
he turns toward Lara again. The new `nav_run` capture extends through the
complete stop. Three post-implementation validations cover shorter running,
walking across the boundary and a turn after the initial departure. Across
1950 new ticks, actor x/y/z, yaw, animation, absolute frame, state, goal, speed,
room and both steering-target coordinates match exactly. The original1260
local ticks now also compare their two steering-target coordinates.

`game/tests/winston.rs` deliberately isolates the item by supplying each
preceding observed avatar pose. The developer-MCP smoke replays the complete
pad route through the real avatar rules instead. The engine's independent
hand-authored boxes check margin clamping, geometric overlap before sector-id
changes and explicit refusal of missing links, blocked destinations or height
transitions. Bounds are half-open sector intervals; the documentation now
matches the existing camera/navigation arithmetic.

This is local adjacency, not a general pathfinder. Multi-box search queues,
zone selection, dynamic blockage, diagonal/corner corridors, stair/drop limits,
creature wall deflection and scheduling remain unported. Unsupported local
queries retain the previous direct-target fallback; floor movement still has
the separately documented flat-height guard. No authored links, placements,
models or level file fields were modified.


Adjacent-corridor checks: the all-features workspace suite and Clippy completed,
with existing unrelated warnings. The engine sandbox ran offscreen and the
editor Home walk-loop preview/save checks passed. Full-game run, short-run and
walking routes pass17 checkpoints each (51), including target coordinates;
route and attendant-centered views were visually inspected. The turned route
matches the isolated item through all650 ticks but exposes an existing avatar
collision diagnostic in the complete game: after an oblique wall contact,
Lara settles at Z95536 rather than95550. Its steering target first exposes
that difference at checkpoint616, after all preceding checked item positions
match. `tools/winston_smoke.py --case nav_turn_heldout` now passes after the
independently verified avatar correction in [oblique-wall-evidence.md](oblique-wall-evidence.md);
no item target or movement correction was added to hide the difference.

The later [bounded-height corridor verification](navigation-height-evidence.md)
extends local targets with caller-supplied step/drop and movement-zone limits.
The flat-only engine query remains available; physical creature steps and
multi-box navigation remain unported.

## Extended vertical-contact verification

Three new no-poke captures exercise both live controllers through vertical
jump preparation, animated item contact, landing, and return to standing.
They extend the short `contact_back` route and revisit the interaction retained
as a player-only [steering diagnostic](vertical-steering-evidence.md).
No runtime controller or collision rule was changed for these recordings.

| Capture | Pad input in display fields | Length |
|---|---|---:|
| `contact_vertical_backward` | Square[0,8), Down[28,50) | 240 |
| `contact_vertical_reverse_heldout` | Square[16,24), Down[44,50), Up[50,60) | 280 |
| `contact_vertical_late_heldout` | Square[12,20), Down[58,80) | 260 |

The latter two routes were recorded after the existing contact and steering
rules, without fitting. The original-style route is labelled calibration.
The tests compare both actors throughout all390 controller ticks. New
fixtures additionally retain and compare Lara's fall speed, health and turn
rate; all contact fixtures now also check the attendant's yaw, speed and room.

The installed toolkit (`psx`0.3.2, build `src-659f5da213e93fb5-release`) cannot
restore the retained format-8 checkpoints. `tools/record_winston.py CASE
--prepare-checkpoint` therefore rebuilds a format-9 runway checkpoint from
the supplied disc and BIOS. The preparation recipes retain menu/loading
waits and pad releases. Inspect their native screenshots if a later build
changes timing. Existing checkpoints and fixtures are preserved.

To replay the newly prepared checkpoint without booting again, use
`tools/record_winston.py CASE --checkpoint .psx/contact_runway/state.bin`.
Each new sidecar retains tool/disc/BIOS identities and the checkpoint hash;
its preparation history retains state hashes and either exact commands or
the observed input logs and run identities. Lara starts at
[20946,2560,95577], yaw-15647, as in the older fixture. The new checkpoint is
at absolute field18912 and Winston's frame is139 rather than119. Keep this
provenance distinction when comparing the old diagnostic.

Full-game verification uses `tools/winston_smoke.py --case CASE`, with both
controllers active and only their initial states aligned. Camera placement
uses the native samples for visual inspection; this is not a camera parity
claim. Sphere rounding, blocked-world pushes, static obstacles and unusual
player modes remain outside this milestone.
The subsequent [item-push milestone](item-push-evidence.md) verifies the
raised-sector corner rejection and nearby clearance subset of blocked-world
validation; the broader exceptional contacts remain open.
The later [rolling contact verification](item-push-evidence.md#rolling-contact-before-floor-response)
adds 300 ticks with same-tick floor settlement after a lateral push. Its
cross-height navigation target difference is explicitly retained as diagnostic.

Verification: `cargo test --workspace --all-features` passes444 tests, including
all14 contact tests. Workspace all-target/all-feature Clippy passes with
existing unrelated warnings; changed Rust is formatted, Python scripts compile,
and `git diff --check` passes. Standalone MCP replays pass48 checked poses across the
three routes; paired native/game jump-contact screenshots were visually
inspected. Reports and exact MCP transcripts are retained under
`validation/winston/smoke/`. There is no runtime, asset-format, editor or
template change in this follow-up.
