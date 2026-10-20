# Slide jumps, mid-air twists and ledge falls

Evidence comes only from the supplied Tomb Raider II USA Rev 3 disc, through
`psx` (build `src-88132a8eecb2461c-release`). This covers jumping out of a
forward or back slide, the back jump's flight, the mid-air twist between the
forward and back jumps, back-flight fast falls, and the stop collision's
remaining branches. It follows the
[fast-fall and slide milestone](fall-recovery-evidence.md) and reuses its
ramp and pit spawns.

## Native rules

Lara's state tables are at `0x80082C98` (control) and `0x80082DE0`
(collision). Input bits are read from `0x8008AAD4`:
- 0x1 Forward
- 0x2 Back
- 0x10 Jump
- 0x40 Action
- 0x80 Walk
- 0x1000 Roll

**Slide controls.** Forward slide `0x80039598` requests goal 3 when
`input & 0x12 == 0x10` (Jump without Back). Back slide `0x80039808` requests
goal 25 when `input & 0x11 == 0x10` (Jump without Forward). The slide clips'
authored ranges enter the takeoff clips: 70 into 76 at its second frame
(1233), and 104 into 74 at 1189. Neither takeoff has gravity, so Lara holds
her height over the falling slope while moving at the clip speed (99). The
takeoff's end command `jump_velocity [-100, 50]` launches 77 or 75; that
tick's gravity makes the first sampled fall speed −94.

**Back jump.** State 25 collision `0x8003B71C` sets the collision heading to
yaw + 0x8000 and calls `0x8003C250`. That routine:
- uses limits +32512/−384/192;
- deflects through DeflectEdgeJump `0x8003C5F4`;
- once the fall speed is positive and the centre floor is not below, lands
  with goal 2 (8 if fatal), clears fall speed and gravity, and settles.

This is the hop-back fall's handler as well, which the port already had.

Back-jump control `0x800395D4` requests fast fall 9 at fall speed 132.
Otherwise, unless the goal is 1 or 2, it requests twist 68 on
`input & 0x1001` (Forward or Roll). Hop-fall control `0x80039720` requests
9 at 132 and reach 11 on Action with free hands.

**Forward-flight control.** `0x80038B88` first turns a pending 52 or 11 back
into 3. Unless the goal is then 8, 1 or 2, it requests in turn, each
overriding the last:
1. Action (hands free): reach 11
2. `input & 0x1002` (Back or Roll): twist 68
3. Walk (hands free): dive 52
4. Fall speed at least 132: fast fall 9

A twist request otherwise persists.

**Twists.** Goal 68 is accepted by:
- running/standing jump 77 for frames 1235–1239 (into 210);
- back jump 75 for frames 1192–1196 (into 212).

The fall clips 34/35 have no range for it. Twist clips 210 and 212 end with
flip effect 0, which turns yaw by 0x8000, and chain into the other flight
state:
- 210 (state 3) → 211 (state 25) → 75;
- 212 (state 25) → 213 (state 3) → 77.

The speed is unchanged, so Lara keeps travelling the same way. The back
jump's fast fall enters 83 and the hop fall's enters 36.

**Stop collision.** `0x8003AA20` (state 2) clears fall speed and gravity,
probes the body with limits +384/−384/0, then tries in order:
1. The ceiling/clamp restore `0x8003C4C4`.
2. The ledge-fall test `0x8003A500`.
3. TestLaraSlide.

If none applies, ShiftItem applies the shift and the centre floor offset
settles the feet.

The ledge-fall test is shared with the walk, run, back-step, side-step,
roll and wade collisions:
- It applies when the centre floor is more than 384 below and the water
  state (`0x8008C4F4`) is not 4 (wading).
- It enters clip 34 with state/goal 3, zero fall speed and gravity on, and
  keeps the current speed.

On land, the walk, back-step and side-step limits (bad_pos 384 or 128) stop
Lara at an edge before the centre can drop.

## Port

- **Slide controls** (`player.rs`): Jump without the opposite direction
  requests goal 3 or 25. The slide clip's own dispatch enters the takeoff
  at the frame its range names. That tick's move and collision then belong
  to forward flight or back flight, through `Traversal::enter`, which now
  carries a clip and frame.
- **Back flight** (`backward.rs`): `fall` serves both states 29 and 25 and
  has:
  - the back-jump and hop-fall fast-fall requests and the twist request;
  - the grounded takeoff;
  - the jump command;
  - effect 0;
  - handoffs to forward flight (213) and fast fall.
- **Forward flight** (`traversal.rs`): it uses the original request order
  with goal 68, handles effect 0, and hands off to back flight at 211. A
  handoff into fast fall does not brake on its first tick, because
  back-jump control ran that tick.
- **Stop collision** (`traversal::stop_collision`): it replaces the
  slide-only test with the ledge fall, the slide test and the
  shift-and-settle. The back jump's landing on the ramp's top edge needs
  that shift (+72).
- **Generic floor path:** it settles drops up to 384 and sends larger drops
  into the ledge fall for every clip-driven mode, not just Run. The mid-air
  level spawn and falling out of the water now enter forward flight
  (clip 34, goal 3) instead of the generic `Fall` mode. The avatar's
  level-start `falling` flag follows clip 34.

Before the change, the forward slide jump failed at tick 37 (the old jump
skipped takeoff 76) and the back one at tick 33. After the slide controls,
the back jump failed only on its landing shift, and the forward one on the
takeoff's entry frame. `forward_away_heldout` then failed only on the goal
column (68) and is labelled calibration (`used_to_refine_rules`). The twist
was ported after that and calibrated on `twist_pit` and
`twist_slide_back`. `twist_pit` first failed on the fast-fall handoff's
braking. No engine, format, editor or template change was needed.

## Recordings

`python3 tools/record_slide_jump.py --case NAME` replays pad input from the
fast-fall milestone's controlled spawns. `--prepare` rebuilds a spawn
checkpoint; `pit_edge_back` (room 5, z 79700, facing −z) was added for the
hop-fall route. Replays poke nothing and trace both slide controls. Sidecars
keep preparation, checkpoint and source hashes, disc/BIOS identity, toolkit
build and sample/write/trace hashes.

| Case | Spawn / input | Role |
|---|---|---|
| `forward` | ramp top, Up[0,40), Square[72,90) | Calibration: forward slide jump, wall rebound, fast fall |
| `back` | ramp top facing away, Down[0,30), Square[64,84) | Calibration: back slide jump onto the top edge, landing shift |
| `twist_pit` | pit, Up+Square[0,22), Down[22,30) | Calibration: twist 210/211, back-jump fast fall |
| `twist_slide_back` | ramp back, back slide jump, Up+Square[68,74) | Calibration: twist 212/213, rebound, fast fall |
| `forward_away_heldout` | Down+Square[70,90) on the forward slide | Calibration for goal 68 (no jump; slide-off) |
| `forward_late_heldout` | Square[74,90) | Independent |
| `back_held_heldout` | new spawn, Square held from before the slide | Independent |
| `back_away_heldout` | Up+Square on the back slide | Independent (no jump) |
| `twist_roll_heldout` | pit, Circle[26,30) | Independent |
| `twist_late_heldout` | pit, Down after the twist window | Independent (no twist) |
| `back_twist_roll_heldout` | back slide jump, Circle | Independent |
| `hop_pit_heldout` | pit edge facing away, Down[0,20) | Independent: hop fall fast fall 36 |

The twelve routes compare 1372 logic ticks across 13 columns with exact
values, apart from the existing 0.02-unit y conversion allowance.

## Verification

`cargo test --workspace --all-features` passes 480 tests (8 opt-in
ignored), including every existing airborne, fall, slide, roll, wading and
surface route. All-target/all-feature Clippy reports only existing
unrelated warnings. Changed Rust files are formatted.

`python3 tools/slide_jump_smoke.py` (after `cargo build -p tomb-raider-ii
--features dev-mcp`) checks 70 aligned poses across all twelve routes in the
standalone game. Paired twist, takeoff and landing views in
`validation/slide-jump-smoke/` were inspected. HUD and raster parity are not
claimed.

## Not covered

- The hop fall's reach 11 and the running jump's twist 207: since covered
  by the [side-jump milestone](side-jump-evidence.md).
- The stop collision's ceiling/clamp restore and slopes-as-walls/pits flags.
- The 0x8008C4F6 front-rebound skip.
- Generic `Fall` remains only for restored saves and the clipless fallback.
