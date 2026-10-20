# Item pushes beside a raised room-sector corner

Evidence comes only from the supplied Tomb Raider II USA Rev3 disc through
`psx`0.3.2, build `src-659f5da213e93fb5-release`. This verifies the world
validation of Winston's ordinary animated push beside the Home fountain.
Static meshes, slopes, other player collision contexts and general creature
navigation remain unverified.

## Native rule and observed failure

`psx disasm --ram .psx/contact_runway --addr 8001c890 --count 260 --text`
retains the push function. At `0x8001CBC8..0x8001CC1C` it sets floor limits
+32512/-384, ceiling limit0, computes the heading from the last accepted
position to the candidate, and calls GetCollisionInfo `0x8001B018` with
height762. The ordinary collision context supplies radius100. Any nonzero
contact class restores the accepted x/z at `0x8001CC30..0x8001CC44`.
Clear candidates instead become the accepted position before room updating
at `0x8001CC48..0x8001CC6C`. The caller does not apply the computed wall shift.
The source bytes `0x1C890..0x1CCA4` in the fresh checkpoint have SHA256
`c5e7dd2fa1be497df29234cb1cf52cd723849c8705094f0951f70fd2a0acc621`.

The calibration checkpoint places Lara at [21550,2560,92350], yaw0, and
Winston at [21800,2560,92540], yaw-16384. Native stores first propose
[21550,2560,92234], then restore Z92350. The candidate faces -Z; its right
leading corner [21450,92134] lies on the fountain sector with floor2048,
a512-unit rise. This exceeds the384-unit permitted rise. Centre/front/other
side probes remain on floor2560. The trace loads contact type4 (right) at
`0x8001CC20` and executes the rollback on every tick of this route.

The port's former five fixed offsets missed that diagonal corner and accepted
Z92234 on the first tick. The retained failing regression is
`validation/item-push/before.log`. `player::collision::item_push_is_clear`
now uses the existing directional body probes with the native push limits.
Winston retains its existing animation/sphere contact and accepted-origin
handling. No new collision implementation, level fields or baked assets
were introduced.

## Complete controller recordings

All routes use controlled initial actor placement, explicitly distinguished
from the earlier unmodified-checkpoint contact evidence. Preparation changes
only the two actors' horizontal positions and Lara's yaw, then runs two neutral
fields so the original resolves rooms, boxes and collision context. The pad
replays themselves have no pokes. Each JSON sidecar retains the preparation
command, pokes, source/state hashes, source checkpoint history, tool/disc/BIOS
identities, replay command and sample/write/trace hashes.

| Case | Initial Lara x/z; Winston x/z | Pad input, display fields | Fields |
|---|---|---|---:|
| `contact_blocked_corner` | 21550/92350; 21800/92540 | Neutral | 160 |
| `contact_blocked_offset_heldout` | 21580/92370; 21820/92530 | Neutral | 180 |
| `contact_blocked_release_heldout` | 21550/92350; 21800/92540 | Up+R1[16,70) | 220 |
| `contact_blocked_clear_heldout` | 21620/92350; 21800/92540 | Down+R1[16,60) | 200 |

The last three were recorded after the directional validation rule was
written, without fitting it. Together these routes compare380 controller
ticks with both actors running their real rules. Native watches retain280
rejected pushes and9 accepted pushes. The walking route stays blocked through
its authored movement and release; the clearance control settles at Z92234
during preparation and later walks backward to Z91936. Its push calls return
contact type0 and never execute the rejected-candidate stores.

The additional two-field `validation/item-push/corner-probes/` trace includes
GetCollisionInfo itself. Its first call stores relative floors0/0/0/-512
for centre/front/left/right, then returns type4. Reproduce with `psx run
--from-state .psx/contact_blocked_corner_start/state.bin --frames 2 --input '[]'
--trace --trace-range 8001b018-8001b768 --trace-range 8001cbc8-8001cc70
--max-events 10000 --out validation/item-push/corner-probes --summary`.

Reproduce from the freshly prepared runway checkpoint with
`python3 tools/record_winston.py CASE --prepare-blocked-checkpoint`.
If that checkpoint is missing, first run the earlier disc boot recipe:
`python3 tools/record_winston.py contact_vertical_backward --prepare-checkpoint`.
To replay a settled controlled checkpoint directly, use
`--checkpoint .psx/CASE_start/state.bin`. Native output, complete position-write
watches and focused validator traces are in `validation/winston/CASE/`.

## Explicit sphere-mask limitation

These fixtures retain the exact native mesh-contact masks, but compare their
nonzero/contact condition rather than claiming exact per-mesh bits. For
example, calibration field72 has port mask15797568 versus native15796544:
only bit10 differs, and both still trigger the same push. A subsequent native
sphere-output capture bounds this specific pose as described below. Exact
mesh-bit parity remains separate work.

The sidecars explicitly declare `contact_mask_comparison: "presence"`.
Positions, yaw, animations, authored frames, states, goals, room, horizontal
and vertical speeds, health and turn rate retain the existing exact comparison
contract (the prior0.02-source-unit y allowance remains). All earlier fixtures
continue to compare their full mesh masks exactly. No observed positions or
animation states are injected after initial setup.

## Verification

`cargo test --workspace --all-features` passes448 tests, including18 contact
tests. The all-feature/all-target workspace build and Clippy pass, with the
existing unrelated Clippy warnings. Changed Rust is formatted, both Python
scripts compile, fixture hashes match and `git diff --check` passes.

`tools/winston_smoke.py --case CASE` passes50 checked poses across all four
routes. Native-camera and attendant-camera captures are retained under
`validation/winston/smoke/`; the fountain-corner game/native pair was inspected.
The engine sandbox moves for30 ticks and renders its hand-authored room. Its
small colour palette was inspected rather than relying on the game smoke's
generic64-colour threshold. The editor Home save/reopen and undo/redo checks
pass and preserve untouched bytes; placement9's clip282 preview was inspected.
No engine format, editor implementation or template change was required.

## Raised-corner sphere diagnostic

`python3 tools/record_contact_spheres.py` replays the settled corner and nearby
offset checkpoints without pokes and traces GetSpheres output stores at
`0x8005BAF8..0x8005BE3C` during display fields70..73. The exporter checks
checkpoint hashes, complete execution and zero dropped trace lines. It retains
the exact commands, tool/disc identities and trace hashes with eight sphere
clouds in `game/tests/data/original/winston_corner_spheres.json`. Actor root
positions and yaw come from native loads; animation/frame come from the
complete route samples at the corresponding post-controller field. No native
pose arithmetic is reconstructed.

At both fields70 and72, Winston's maximum coordinate error is1.017578125 source
units and Lara's is0.9765625. The corner's native mask is `0xF10940`, versus
Bevy's `0xF10D40`: only joint10 differs. The nearby offset route has full mask
parity (`0xF10D40`) at both fields. The regression bounds each coordinate
error to1.1 units, retains the precise mask discrepancy and checks that every
disagreeing sphere lies within the resulting relative-distance error bound
of tangency (the native joint10 gap is0.2355873 source units). This directly measures the existing approximation at the new
pose; it does not establish exact GTE intermediate rounding or full mask parity.

There are no gameplay, presentation, asset-format or editor changes in this
diagnostic follow-up.

Verification: workspace all-feature tests pass449 tests, including19 contact
tests. Workspace all-target/all-feature Clippy passes with existing unrelated
warnings. The changed Rust is formatted, the recording script compiles and
`git diff --check` passes.

## Rolling contact before floor response

Three complete controlled-placement recordings exercise a roll past Winston
beside the same fountain corner. Preparation changes only the horizontal actor
positions and Lara's yaw, as above; the pad routes have no pokes. Reproduce
with `python3 tools/record_winston.py CASE --prepare-blocked-checkpoint`.

| Case | Placement | Circle held, display fields | Total fields |
|---|---|---|---:|
| `contact_blocked_roll` | Corner calibration | [16,24) | 180 |
| `contact_blocked_roll_offset_heldout` | Nearby offset | [24,34) | 200 |
| `contact_blocked_roll_late_heldout` | Corner, later pose | [40,54) | 220 |

The first route exposed a floor-response ordering bug. At run field16,
AnimateLara writes Z92400 at `0x800432A4`; the item push then writes X21408
at `0x8001CA78`. Its validator accepts the candidate. Only afterward does
the first roll collision handler write Y2816 at `0x8003BCF8`, settling onto
the lower floor. The output row at field18 therefore has position
[21408,2816,92400]. The former port checked the floor before the item push
and retained Y2560. The first failing comparison is retained in
`validation/item-push/roll-before.log`.

The recorder now watches all three position coordinates and retains both
roll collision handlers (`0x8003BC50..0x8003BE1C`), the collision-query wrapper
(`0x8003C318..0x8003C364`) and the push validator. Each sidecar records the
commands, checkpoint ancestry and hashes, samples, complete write watches
and zero-drop traces. The focused four-field trace in
`validation/item-push/roll-order/` independently shows the same ordering.
This agrees with the earlier [tick-order evidence](tick-order-evidence.md).

Rolling now advances its authored animation and movement first, then resolves
its floor/wall response after object contact and before avatar publication,
weapons and triggers. A per-player `CollisionFrame` retains the accepted
origin; accepted pushes update it for the later response, while rejected
pushes restore it. This replaces Winston's single-player origin resource.
Other movement families retain their previous scheduling and remain outside
this ordering claim.

Both actors replay their real controllers for all 300 ticks. Positions, yaw,
animation/frame, state/goal, room, speed, fall speed, health and turn rate match
with the existing comparison tolerances. Mesh masks retain the explicit
contact-presence contract. The two held-out routes were recorded after the
ordering fix and pass without fitting or additional gameplay changes.

The lower floor exposes a separate navigation diagnostic: Lara enters box138
at height2816 while Winston remains in box24 at height2560. His native path
target is clipped toward Z92672, while the port falls back to Lara's position
for this unsupported height transition. The standalone smoke first detected
it at field24 of calibration. These routes explicitly declare
`navigation_target_comparison: "diagnostic"`; raw targets remain in the
fixtures and MCP reports. Winston stays at speed0 throughout, so this target
difference does not change his observed position or yaw. Cross-height
navigation parity is not claimed.

That target diagnostic is superseded by the later
[bounded-height corridor milestone](navigation-height-evidence.md): all three
routes now compare target X/Z exactly, with their navigation provenance
labelled calibration. General cross-height creature movement remains open.

Verification: `cargo test --workspace --all-features` passes 452 tests,
including all 22 contact tests. The all-target/all-feature workspace build and
Clippy pass with existing unrelated warnings. Changed Rust is formatted;
recording/smoke scripts compile, fixture/checkpoint/trace/write hashes match,
and `git diff --check` passes. The standalone smoke passes 57 poses across
the three routes, with native-camera and attendant views retained under
`validation/winston/smoke/`; the field50 native/game pair was inspected.
The sandbox moves for 30 ticks and renders its authored room. The editor's
Home save/reopen/undo checks pass, and the separate placement8 clip146 preview
advances to frame3 without dirtying the document; its save/reopen remains
byte exact. No persistent engine format, editor or template change is needed.
