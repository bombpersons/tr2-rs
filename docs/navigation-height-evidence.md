# Local navigation targets across a floor-height change

Evidence comes only from the supplied USA Rev3 disc through `psx` 0.3.2,
build `src-659f5da213e93fb5-release`. This extends local corridor targeting,
not physical creature movement over steps or a general pathfinder.

## Native rule

The previous [rolling contact milestone](item-push-evidence.md#rolling-contact-before-floor-response)
retained a target diagnostic. Lara rolls from box24 (floor2560) into adjacent
box138 (floor2816), while Winston stays in box24. The port's flat-only query
refused the height difference and fell back to Lara's position. An exact
target comparison first fails at display field20: port Z92400, native Z92672.
The pre-fix failure is retained in `validation/winston-height/before.log`.

`python3 tools/inspect_navigation.py` replays the unchanged controlled corner
checkpoint and captures two target calculations at fields20 and22. Its trace
contains no dropped lines. Commands, identities, checkpoint/RAM/trace hashes,
runtime limits, boxes and table hashes are retained in
`game/tests/data/original/winston_navigation_height.json`. Raw outputs are in
`validation/winston-height/path-target/`. The earlier exploratory
`path-transition/` trace was truncated and is not the verification capture.

Creature initialization writes step256 at `0x80048F68`, drop-512 at
`0x80048F48`, blocked mask0x4000 at `0x80048F6C`, and fly0 at `0x80048F70`.
The live structure independently confirms these values: creature at
`0x801826F4`, navigation context at `0x80182704`, limits at
`0x80182710/0x80182712`. This object type takes the ordinary initialization
branch. The selected movement-zone pointer at `0x8008B4A0` points to
`0x8017BD48`; the complete runtime array matches baked zones[0] and zones[5]
(identical for Home). Both boxes have zone3. Flipped-world zone selection
remains unverified.

Reverse search at `0x800151F0` rejects a different zone. At
`0x800151FC..0x80015224` it compares the candidate floor minus the current
search floor against the signed step/drop limits. Search expands from the
destination back toward the actor: in forward movement terms this permits
a rise of at most256 or a drop of at most512. Equality is accepted because
the comparisons are strict. These threshold values and inclusivity come
from binary evidence; the runtime route observes a256-unit drop.

The exporter verifies both runtime box records against the baked data, and
the complete native overlap table against the baked connectivity. Box24's
neighbor list includes138, and its live next-box node is138. The two-box
target trace follows that node at `0x800161EC`, checks the blocked mask at
`0x80016200..0x80016210`, and then clips against box138. For an actor outside
the destination on X, the perpendicular Z coordinate is retained within
clearance margins. This narrow box begins at Z92160, so the lower margin is
92160+512=92672 (`0x80016164..0x80016184`). The destination goal supplies X
and Y at `0x80016194..0x800161DC`.

## Port and scope

The engine exposes `world::CorridorLimits` and
`WorldGeometry::adjacent_path_target_with_limits`. Callers supply clearance,
maximum step/drop and an optional authored zone-table index. This is a local
query over existing level data. The original `adjacent_path_target` remains
flat-only, with its previous results. Hand-authored engine tests cover exact
step/drop boundaries, their next rejected unit, different zones, missing
zone data, and the existing blocked/missing-link cases.

Winston's controller supplies its verified limits and zone index through
`game/data/object_types.json`. His physical movement still has its existing
flat-floor guard. Multi-box search, queue scheduling, blocked paths,
cross-height creature movement and navigation around corners remain separate
work. No format or editor schema changes are needed: boxes, overlaps and
zones already belong to the engine's level data.

## Complete controller recordings

The three earlier roll recordings now compare target X/Z on every tick.
Their sidecars label them as navigation calibration, preserving their original
independence labels for the earlier contact fix. A fresh route,
`contact_blocked_roll_target_heldout`, was recorded after this query was
implemented and passes without fitting: actors are initially placed at
Lara X21570/Z92360 and Winston X21810/Z92535, with Circle held during
fields[32,44), followed through recovery to field240. Preparation uses the
existing two-field controlled-placement recipe; the replay itself has no pokes.
It additionally retains and compares native target Y at `0x8018272C`.

Reproduce with `python3 tools/record_winston.py
contact_blocked_roll_target_heldout --prepare-blocked-checkpoint`.
The four routes compare420 ticks with both real controllers active and no
post-start pose injection. Existing actor/animation/health/motion comparisons
remain in place, including the previously documented contact-mask presence
contract and0.02-source-unit Lara Y tolerance. Navigation targets compare
exactly. The standalone smoke's previous target-comparison bypass has been
removed; target X/Z (and Y when recorded) are checked directly.

## Verification

`cargo test --workspace --all-features` passes 455 tests, including all 23
contact tests and four engine corridor tests. The all-feature/all-target
workspace build and Clippy pass with existing unrelated warnings. Changed
Rust is formatted, recording scripts compile, native fixture/checkpoint/trace
hashes match, and `git diff --check` passes.

`tools/winston_smoke.py --case CASE` passes 76 checked poses across all four
roll routes, including the independent route's target height. Native/game
field64 views were inspected; the smoke uses native camera samples for
inspection and does not claim camera parity. The engine sandbox moves for
30 ticks and renders its hand-authored room without game content.
