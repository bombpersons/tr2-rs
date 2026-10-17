# Original disc runtime evidence

Observed with psx MCP native build `src-84bc9f72af323b47-release`, supplied USA Rev 3 disc and BIOS SHA256 `11052b6499e466bbf0a709b1f9cb6834a9418e66680387912451e971cf8a1fef`. All states use this exact identity. No memory pokes were used.

## Boot/menu checkpoints

The original emulator reached gameplay successfully; black movie/transition frames were not proof of a hang.

| Checkpoint | Absolute field | Observed screen / next action |
|---|---:|---|
| `.psx/boot/state.bin` | 1800 | Earlier boot/movies; continue |
| `.psx/progress1/state.bin` | 7800 | Budget launcher, Tomb Raider II selected; CROSS launches |
| `.psx/launch_tr2/state.bin` | 11400 | No formatted memory card warning; CROSS continues |
| `.psx/title/state.bin` | 11640 | Main menu, Lara's Home selected |
| `.psx/home_start/state.bin` | 14880 | Home courtyard, player standing beside fountain |
| `.psx/passport_new/state.bin` | 15600 | Passport New Game selected; CROSS starts Wall movie |
| `.psx/wall_early/state_f001200.bin` | 16800 | Wall automatic entry slide underway |
| `.psx/wall_game/state.bin` | 21000 | Wall cave, player standing after entry slides |
| `.psx/home_side/state.bin` | 14992 | Home standing, turned across clear flat courtyard |

To get Home from title: CROSS opens its postcard, another CROSS starts it. To get main game: title LEFT to Game, CROSS selects passport / card warning, CROSS continues warning, RIGHT turns Load Game page to New Game, CROSS starts. START can skip movies but pauses active gameplay, so do not schedule repeated START blindly after loading.

`.psx/wall_early` started from passport New Game with CROSS fields [0,10), START [600,610), START [1200,1210). Frame 600 is still FMV; 750-1050 loading screen. At field1200 the entry slides are underway. The second START pauses, so use state_f001200.bin with explicit neutral input instead. The Wall starts with automatic movement and changes yaw without user input: sample `.psx/wall_initial/samples.csv` documents x62976,y3831,z24690 → x64176,y5376,z26090 in120 fields. Do not mistake settled cave position for the file's spawn x62976,y1536,z23040.

## Player addresses and cadence

Global physical pointer `0x8008c6b8` points to Lara's item. It is `0x0017c87c` in Home and `0x0019cf30` in Wall. KSEG0 aliases are accepted by psx. Runtime item layout confirmed by RAM comparisons and the yaw writer at `0x800383e4`, which stores halfword at item+0x42:

| Item offset | Format | Meaning |
|---|---|---|
| +0x0e | i16 | Current state |
| +0x10 | i16 | Goal state |
| +0x14 | i16 | Animation index |
| +0x16 | i16 | Absolute animation frame |
| +0x18 | i16 | Room |
| +0x1e | i16 | Movement speed |
| +0x20 | i16 | Fall speed |
| +0x22 | i16 | Health |
| +0x34 | 3*i32 | x,y,z original units |
| +0x40 | 3*i16 | x,y,z rotations; yaw is middle at +0x42 |

Video reports NTSC, 368x240 gameplay. Player logic updates every two video fields. Watch `.psx/home_right_watch/writes.jsonl` records yaw stores at relative fields3,5,7,…59, writer PC0x000383e4. Thus template region NTSC and fields_per_tick2 are justified. Raw CPU timing remains the tool's flat instruction-cost model, not cycle-accurate PS1 hardware timing.

Running from stand accelerates through speeds23,25,27,29,31,32,34,36,38,39,41,43,45,46,47, then47 original units per logic tick. Walking uses15 in its steady animation but ramps and varies in transitions and step animations. The original Home fountain route steps up256 units twice, which is excluded from clean run reference.

## Independent replay references

`game/tests/data/original/{run,turn,jump}.csv` contain121 samples each including time0 and120fields. `metadata.json` records exact checkpoint hash, input event schedule, initial and final state, and sample hashes. All start from the same unmodified Home checkpoint prepared by RIGHT[0,52), then neutral through112 fields from home_start. Initial position is [19968,2560,95744], yaw-15647, state2, anim103/frame1636, speed0.

- Run: UP held120fields; y stays2560, no collision/step, ends[17317,2560,95910], speed47.
- Turn: RIGHT held120fields; fixed position, yaw ends-27939 wrapping signed16. Yaw increment starts45/tick, grows45/tick; transition from slow turn to fast animation changes the cap behavior.
- Held-out jump: SQUARE held10fields then neutral; apex y1606, lands y2560, ends[19896,2560,95744]. This is separate from run/turn fitting.

## Camera and screenshot alignment

Camera xyz is at `0x8008b258`, target xyz at `0x8008b268`, room is the halfword after each triple. Function `0x80017d8c` initializes target from item+0x34/+0x38/+0x3c, with target y initially player y-1024; this and rotating input observations corroborate identification.

| Screenshot / checkpoint | Player xyz, yaw | Camera xyz | Target xyz |
|---|---|---|---|
| `.psx/home_start/screen.png` | [19968,2560,95744],32768 | [20817,1967,97024] | [19968,1976,95744] |
| `.psx/home_side/screen.png` | [19968,2560,95744],-15647 | [21478,1969,95685] | [19968,1978,95744] |
| `.psx/wall_game/screen.png` | [64176,5376,26090],0 | [63752,4787,25846] | [64176,4795,26090] |

Home start anim103/frame1649 and room16. Home side anim103/frame1636 room16. Wall settled anim103/frame1653 room35. Coordinates use original TR2 y down; camera conversion to Bevy must invert y consistently. FOV was subsequently measured: function `0x80065cb8` calculates `H=192*cos(fov/2)/sin(fov/2)`, stores it at `0x8008c6d4`, then calls `0x8007ac58` which writes GTE control26 (projection distance). Home and Wall RAM both contain H=228. Thus at368x240 the equivalent vertical FOV is `2*atan(120/228)=55.5170812°`, horizontal `2*atan(184/228)=77.8082769°`. The original requested angle14560 is approximately80°, using192 reference halfwidth. Native screenshots are368x240. Further binary evidence at `0x80065c48` passes192 as projection centerX and at `0x80065c90` passes120 as centerY to `0x8007ac38` (SetGeomOffset writes GTE24/25). It also writes draw bounds383 and239. The internal view is384x240 centered192x120, but native capture exposes the first368 horizontal pixels starting display VRAMx0. A368x240 port should shift its principal point8pixels right to match, or render384x240 then crop x0..367. Simply centering at184 leaves an8pixel horizontal error.

## Additional held-out tests

Fresh LEFT stationary and UP+R1 walk captures were recorded once each after the run/turn/jump implementation was calibrated. They are `heldout_left.csv` and `heldout_walk.csv`, with metadata appended to `metadata.json`. Same Home side initial checkpoint. LEFT120fields ends yaw-3355 with fixedposition and anim69/frame1118. Walk120fields ends[19266,2560,95777], yaw-15647, anim1/frame33 speed15. These are held-out validation, not calibration data.

## Wall initial fall and slides

`wall_entry.csv` and its JSON sidecar preserve207fields from the first initialized Wall frame16693, cropped from `.psx/wall_load/samples.csv`; input is neutral throughout. Atframe16693 player is originalspawn[62976,1536,23040], yaw0, state3, anim34/frame492, fallspeed0. Camera[61952,384,23040], target[62976,709,23040]. Fall speed increases6 per logic tick, player y follows accumulated fall speed. Atframe16736 the player reaches the slope and begins speed50 sliding along+z. Further slopes turn player toward+x then+z.

`.psx/wall_spawn/state.bin` and screenshot areframe16696: player[62976,1554,23040], anim34/frame494, fallspeed12; camera[61952,384,23040], target[62976,830,23040]. The native screenshot is close to clipped rocks with Lara barelyvisible; geometry/camera comparison should account for this actual start behavior.

## Tool pain points / bugs

1. **Confirmed: reserved sample label silently overwrites frame.** Calling sample `0x8017c89a:i16:frame` writes speed into CSV's built-in absolute `frame` column. Reproduction is `.psx/home_up120/samples.csv` (run_frame0 frame0, run_frame2 frame23 despite absolute start14880). Labels `run_frame` likely need the same protection. The clean reference CSVs use `animation_frame` and preserve correct absolute frame values. The CLI should reject reserved labels or namespace sample labels, rather than silently discard temporal evidence.
2. Frame numbering is correctly explained by guide, but save_state_at captures at the start of a relative field while ordinary screen is end state. A visible explanation beside checkpoint output would help screenshot alignment.
3. Budget launcher and memory card warnings add several explicit confirmations. `status=complete` only confirms the bounded emulator run, not title/gameplay; images were essential to distinguish these states.
4. START accepted during movie skipping becomes gameplay pause after loading. Repeated sparse button pulses can therefore leave a fully functioning game paused. Explicit checkpoint inspection was necessary.

## Limits

This evidence establishes successful original gameplay execution, player cadence, clear-floor run/turn and one jump trajectory, plus camera position/target snapshots. It does not establish full game mechanics, enemy AI, inventory, weapons, triggers or completion behavior. Wall's early per-field fall/slide and camera trajectory are now sampled, but their complete collision and camera rules remain unimplemented/unverified.

## Subsequent PSX-only camera increment

Authored fixed-shot selection, continuation, once flags, timer expiry, eye
easing, and animated look-target height are now implemented and independently
checked against the original. See [camera-evidence.md](camera-evidence.md) for
addresses, experiments, retained samples, reproduction calls, and limits.
The full camera system still does not match the original; later entrance-slide
obstacle correction, chase behavior, object targets, and cinematic cameras
remain unverified.

Running the retained player comparisons also exposed existing frame-boundary
errors: converting a 30 Hz Duration into f32 seconds produced slightly less
than one animation frame, delaying run/walk/turn dispatch. Source-frame
increments close to integral values now retain those boundaries. The jump
recording also includes a final horizontal step on the landing tick; collision
now tests the upper endpoint of the descending sweep before snapping to floor.
All five original run/turn/jump/held-out left/walk comparisons pass unchanged.

## View orientation and airborne startup

The original camera matrices confirm screen-right along source -Z when facing
+X. The render coordinate boundary now corrects the previously mirrored world.
Four snapshot matrices and 64 room landmark projections are retained, and the
ordinary Bevy view matches them within one pixel. Startup watches also establish
the two initial camera-framing passes and the initial fall animation phase.
See [view-evidence.md](view-evidence.md) for the source addresses, captures,
tests, level-transition checks and remaining visual differences.

## Full Wall entry trajectory

`original_wall_entry_falls_slides_and_turns_across_every_recorded_tick` now
replays all 207 recorded fields with neutral input and matches x, z, yaw,
animation, absolute animation frame and fall speed exactly (y within rounding).
It covers three falls, goal2 landing dispatch to clip82, three ramps (+z, +x,
+z) with speed-0 turn ticks, horizontal speed retained while falling off ramp
lips, and the clip71 speed-30 slide stop. The steeper-axis tie-break, backward
slides (clip105) and diagonal slopes are not exercised by this capture.
