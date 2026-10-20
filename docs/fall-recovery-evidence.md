# Natural fast falls and slide-edge landings

Evidence comes only from the supplied Tomb Raider II USA Rev 3 disc, through
`psx` (build `src-88132a8eecb2461c-release`). This covers a long forward drop
entering fast fall and its heavy landing, landing on a steep slope, forward and
backward slides leaving the slope's edge, and the back-fall wall rebound. The
[fall-damage milestone](fall-damage-evidence.md) already verified the impact
curve and fatal landing with a controlled falling actor. Here every route
reaches its fall from ordinary pad input.

## Native rules

Lara's state tables are at `0x80082C98` (control) and `0x80082DE0`
(collision), one word per state.

**Fast-fall request.** Forward-flight control `0x80038B88` (state 3) first
sets its reach/dive/other requests. Then, if the fall speed at the start of
the tick is 132 or more, it requests goal 9 (`0x80038C4C..0x80038C60`). It
skips this when the goal is already 1, 2 or 8. The authored dispatch from
clip 77 enters fast-fall clip 45. Fast-fall control and collision were
already ported (braking 95/100, heavy landing 24).

**Stop collision and the slide test.** State-2 collision `0x8003AA20` clears
fall speed and gravity and uses limits +384/−384/0. It probes the body, then
tests ceiling (`0x8003C4C4`), ledge fall (`0x8003A500`) and slide
(`0x8003DD30`). If none applies, it shifts and settles onto the floor.
TestLaraSlide does nothing when both floor tilts are under 3. Otherwise:
- **Heading:** it picks a cardinal downhill heading. x wins unless |tilt_z|
  is strictly larger (`0x8003DDAC..0x8003DE70`).
- **Shift:** it applies the collision shift (`0x8003DE7C`).
- **Forward slide:** when facing within 90° of downhill, it enters clip 70
  with state/goal 24 and yaw set to downhill.
- **Back slide:** otherwise it enters clip 104 with state/goal 32 and yaw set
  to downhill + 0x8000.
- **Heading and entry speed:** either way the movement heading becomes
  downhill and the clip starts with speed 0.
On the ramp the shift is the front probe crossing the lip: −41 in
`fall_ramp_run` tick 36.

**Slide collision.** Both slide states share `0x8003C364` (limits
+32512/−512/0; heading downhill). Once the centre is more than 200 above the
floor (`0x8003C3B4`), a forward slide enters clip 34 with state/goal 3 and
a back slide clip 93 with state/goal 29. Fall speed becomes 0 and gravity is
set, while the slide speed is kept. Otherwise the slide follows the floor and
requests stop when the floor is no longer steep. Slide speed is the clip's
authored speed: 70 moves at 50, and 104 accelerates 6, 12, 18 … before
looping into 105.

**Back-fall collision.** State-29 collision `0x8003B9A8` uses forward-jump
collision's limits (+32512/−384/192), faces yaw + 0x8000, and calls the same
DeflectEdgeJump `0x8003C5F4`. A frontal contact quarters speed and reverses
the movement heading for one move. Collision resets the heading every tick,
so Lara re-meets the wall: speed 50 → 12 → 3. Below 129 the impact enters
low impact 31 (state 14). Landing sets goal 2 (8 if fatal) without a second
animation advance.

## Port

- `traversal.rs`: forward-flight control requests goal 9 at
  `FAST_FALL_SPEED`. Landing recovery in state 2 runs `start_slide` (the slide
  test with the stop collision's shift; now `stop_collision`). The frontal rebound is a shared
  `rebound` function.
- `backward.rs`: the back fall classifies the body with `collision::info`,
  applies the shift and uses `rebound`/`deflect`. A landing whose dispatch
  reaches state 2 runs the slide test on the same tick, as the original does.
- `player.rs`: an ongoing slide advances its own authored clip and moves at
  that clip's speed. A slide whose centre drops more than 200 enters forward
  flight or the back fall, replacing the generic `Fall`.

The first calibration (`pit_jump`) failed at tick 54: the port kept clip 77
instead of entering 45. `ramp_run` then failed on the landing-to-slide tick,
and `ramp_hop_back` on the back slide's speed and the back fall's wall. No
engine, format, editor or template change was needed.

## Recordings

`python3 tools/record_fall_recovery.py --prepare SPAWN` writes position,
room and yaw at frame 0 into the rebuilt `.psx/home_start` checkpoint and runs
90 neutral fields. This is a controlled spawn on ordinary floor. `--case NAME`
replays pad input with no pokes, tracing slide collision. Sidecars keep
preparation, checkpoint and source hashes, disc/BIOS identity, toolkit build and
sample/write/trace hashes.

| Case | Spawn / input | Role |
|---|---|---|
| `pit_jump` | room 5 block, Up+Square[0,22) | Calibration: 2048 drop, fast fall at 132, heavy landing |
| `ramp_run` | room 36 top, Up[0,40) | Calibration: run off, slope landing, slide, slide-off, wall low impact |
| `ramp_hop_back` | room 36 top facing away, Down[0,30) | Calibration: hop-fall onto the slope, back slide, back fall, wall rebound |
| `pit_angled_heldout` | new angled spawn, Up+Square[8,30), Up[150,200) | Independent |
| `ramp_run_late_heldout` | Up[20,60), Up[190,240) | Independent |
| `ramp_back_near_heldout` | new spawn, Down[0,20) | Independent |

The six routes compare 706 logic ticks across 13 columns with exact values,
apart from the existing 0.02-unit y conversion allowance.

**Native stall.** The mirrored ramp (room 48 over room 16, x ≈ 23500) stalls
the native core permanently on its slide-off tick: logic and display stop and
never recover in 900 fields. The CPU keeps running ordinary culling code
(`0x80065DF4`). Whether real hardware does this is unknown. Redux cannot load
native states, so that route is not used and no behavior is fitted to it.

## Verification

`cargo test --workspace --all-features` passes 468 tests (8 opt-in
ignored), including every existing hop-fall, slide (Wall entry), tiger and
fall-damage route. All-target/all-feature Clippy passes with existing
unrelated warnings. Changed Rust files are formatted, and `git diff --check`
passes.

`python3 tools/fall_recovery_smoke.py` checks 53 aligned poses across all six
routes in the standalone game. Paired slide and fast-fall views in
`validation/fall-smoke/` were inspected. The port does not draw the original's
blob shadow; HUD and raster parity are not claimed.

## Not covered

- The stop collision's slopes-as-walls/pits probe flags (`collision.rs` notes this).
- Its ceiling and ledge-fall branches during landing recovery.
- The slide's same-angle no-reentry check.
- Jumping out of a slide and walking off a ledge: since covered by the
  [slide-jump milestone](slide-jump-evidence.md).
- Fast-fall side contacts.
