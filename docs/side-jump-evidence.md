# Side jumps, directional standing jumps, running twists and hop-fall reach

Evidence comes only from the supplied Tomb Raider II USA Rev 3 disc, through
`psx` (build `src-88132a8eecb2461c-release`). This covers:
- choosing a standing jump's direction during compression;
- the right, left and standing back jumps;
- the running jump's twist;
- the hop fall's reach;
- reach's fast-fall request.

It follows the [slide-jump milestone](slide-jump-evidence.md) and reuses the
[fast-fall milestone](fall-recovery-evidence.md)'s spawn method around room
5's raised block.

## Native rules

Lara's state tables are at `0x80082C98` (control) and `0x80082DE0`
(collision). Input bits at `0x8008AAD4` add 0x4 Left and 0x8 Right to those
listed in the slide-jump notes.

**Taking Jump.** Of the ground states, only stop and run test Jump.
- **Stop** (`0x8003899C`) requests compression 15 on held Jump. On land it
  returns before delegating Up, so it neither turns nor walks that tick.
  Its earlier Left/Right turn and side-step requests are overridden.
- **Run** (`0x800387B0`) requests goal 3 whatever direction is held, once its
  jump latch is set.
- **Others:** walk, back, side-step and turn controls ignore Jump.

**Compression control** `0x800390C8` (state 15) skips every direction while
wading (`0x8008C4F4` = 4). Otherwise it tries these in order and takes the
first whose floor 256 that way is clear (GetHeightAhead `0x8003DF60`, no more
than 384 above):

| Input | Heading | Goal |
|---|---|---|
| Forward | yaw | 3 |
| Left | yaw − 0x4000 | 27 |
| Right | yaw + 0x4000 | 26 |
| Back | yaw + 0x8000 | 25 |

Taking a direction also writes that heading to `0x8008C572`. With no clear
direction the goal stays 15, and compression ends in vertical jump 91/28.
At fall speed 132 compression requests 9.

Clip 73's ranges (frames 1186–1189) enter the takeoffs:
- 76 for goal 3;
- 74 for 25;
- 80 for 26;
- 78 for 27.

Each side takeoff moves at speed 99 without gravity. Its end command
`jump_velocity [-100, 50]` launches flight clip 81 (right) or 79 (left).

**Side flight.** Right-jump collision `0x8003B750` sets the heading to
yaw + 0x4000; left-jump collision `0x8003B784` sets yaw − 0x4000. Both call
the back jump's `0x8003C250`, with limits +32512/−384/192, deflection and
landing goal 2 or 8.

Right and left controls (`0x80039638`, `0x8003969C`):
- They request fast fall 9 at 132.
- Otherwise, on the opposite direction and unless goal 2 is pending, they
  request twist 68. Clips 79/81 have no range for 68, so nothing happens.
- They also clear `0x8008C510` bit 0x80000, which is not ported.

They land into 82 and fast-fall into 84 (left) or 85 (right).

**Running twist.** Running-jump clips 17 and 19 accept goal 68 in their first
five frames, into twist 207 (state 3). The twist's last frame flips yaw
(effect 0) and links to 209 (state 25), then back jump 75. Forward flight's
existing Back/Roll request drives it.

**Hop-fall reach.** Hop-fall control `0x80039720` requests 9 at 132. Then,
on Action with free hands (`0x8008C4EA` = 0), it requests reach 11, which
overrides the fast fall. Neither request checks the pending goal. Clip 93
accepts 11 at frames 1480–1483 into reach 94. Reach control `0x80039070`
requests fast fall 9 at 132, which clips 94/95 accept into 98.

## Port

- **Ground jumps** (`player.rs`): with clips, a Jump from standing always
  enters compression 73, and from running always requests goal 3. Other
  modes ignore it, so the old synthetic directional launch is now only the
  clipless fallback. On the stop handler's Jump tick the turn update does
  not turn.
- **Compression** (`traversal.rs`): it requests the first clear direction in
  the original order and sets the movement heading. Reach control requests
  fast fall.
- **Flight** (`backward.rs`): `fall` now serves states 25–27 and 29 at their
  own headings (`flight_turn`). It has:
  - each state's fast-fall, twist and reach requests;
  - the side takeoffs;
  - handoffs to forward flight, reach (state 11) and fast fall.

  Forward flight hands states 25–27 to it.

Before the change, four of the five calibration routes failed on their first
directional or reach tick. The running twist already passed. Two held-out
routes then failed and are labelled calibration (`used_to_refine_rules`):
- `forward_over_right_heldout`: the port turned on the Jump tick;
- `hop_reach_late_heldout`: the port lacked reach's fast-fall request.

No engine, format, editor or template change was needed.

## Recordings

`python3 tools/record_side_jump.py --case NAME [--prepare]` replays pad input
from controlled spawns, which are listed in `tools/record_fall_recovery.py`:
- `pit_edge_side` and `pit_edge_side_right`: the block's edge, with the pit to
  Lara's left or right;
- `pit_floor_side` and `pit_floor_side_left`: the pit floor beside the
  block's 2048-unit wall;
- `pit_floor_run`: the pit floor.

Replays poke nothing and trace compression control. Sidecars keep:
- preparation, checkpoint and source hashes;
- disc/BIOS identity and the toolkit build;
- sample, write and trace hashes.

| Case | Spawn / input | Role |
|---|---|---|
| `left_pit` | edge, Left+Square | Calibration: left jump, fast fall 84, heavy landing |
| `right_block` | edge, Right+Square | Calibration: right jump across the block onto a slope, slide |
| `right_wall` | pit floor, Right+Square | Calibration: blocked direction, vertical jump |
| `run_twist` | pit floor run, Square then Down | Calibration: twist 207/209, back-jump wall rebound |
| `hop_reach` | edge behind, Down then Cross | Calibration: reach 94 and ledge grab |
| `forward_over_right_heldout` | Up+Right+Square | Calibration: forward wins, no turn |
| `hop_reach_late_heldout` | Cross after the hop | Calibration: reach fast fall 98 |
| `right_pit_heldout` | new spawn, Right+Square | Independent |
| `left_wall_heldout` | new spawn, Left+Square | Independent |
| `left_over_right_heldout` | Left+Right+Square | Independent: left wins, then a twist request |
| `wall_then_back_heldout` | Right+Down+Square at the wall | Independent: back jump |
| `standing_back_heldout` | Down+Square at the edge | Independent: back jump, fast fall 83 |
| `side_twist_heldout` | left jump, then Right | Independent: no twist range |
| `run_twist_roll_heldout` | running jump, Circle | Independent |
| `run_turn_jump_heldout` | Left during the running jump request | Independent |
| `walk_jump_heldout` | Square while walking | Independent: ignored |

The sixteen routes compare 1376 logic ticks across 13 columns with exact
values, apart from the existing 0.02-unit y conversion allowance.

## Verification

`cargo test --workspace --all-features` passes 496 tests (8 opt-in
ignored), including every existing airborne, slide, fall, roll, wading and
traversal route. All-target/all-feature Clippy reports nothing in the
changed files. Changed Rust files are formatted.

`python3 tools/side_jump_smoke.py` (after `cargo build -p tomb-raider-ii
--features dev-mcp`) checks 67 aligned poses across all sixteen routes in
the standalone game. Paired side-jump, twist, reach and grab views in
`validation/side-jump-smoke/` were inspected. HUD and raster parity are not
claimed.

## Not covered

- Wading: there, stop keeps reading Up after Jump and compression takes no
  direction. The port does neither differently while wading, and no route
  covers it.
- The side controls' `0x8008C510` bit-0x80000 clear.
- The standing jump's request while turning (states 6/7 ignore Jump; the
  port waits for stop, as they do, but no route covers it).
- The stop collision's ceiling/clamp restore (see the slide-jump notes).
