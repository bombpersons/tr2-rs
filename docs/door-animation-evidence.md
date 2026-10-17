# Door animation: authored dispatch, links and holds

Evidence is exclusively the supplied Tomb Raider II USA Rev 3 disc executed
with `psx native`. No external reimplementation was consulted. The level
format/pose decoder predates this work; this pass verifies the timing and
state semantics independently against the original's RAM and code.

## Binary evidence

`DoorControl` at `0x8004C954` calls TriggerActive `0x8001F6C8`, then changes
the requested state in two cases: active while state 0 requests state 1
(`0x8004CA04..0x8004CA58`); inactive while state 1 requests state 0
(`0x8004CA3C..0x8004CA60`). It calls AnimateItem at `0x8004CA84`. A request
to reopen while a closing clip still has state 1 does not cancel the pending
close. A later tick requests open after the close reaches state 0.

InitialiseItem copies the first clip's state to both current and goal
(`0x80037720`, `0x8003777C`, `0x80037780`); the door registration initializes
its goal from that same table value.

AnimateItem `0x8001DD98`:

1. Increments the absolute frame (`0x8001DDD4` / `0x8001DDE0`).
2. Runs GetChange `0x8001E250` when the clip has state ranges. GetChange does
   nothing when current state equals goal; otherwise it scans for the goal
   and tests the incremented frame against inclusive first/last bounds
   (`0x8001E2EC..0x8001E30C`). A match sets destination clip/frame directly.
3. Updates current state from the dispatched clip (`0x8001DE20..0x8001DE28`)
   while preserving the requested goal.
4. If the resulting frame exceeds that clip's end, processes its end
   commands and assigns its authored next clip/frame (`0x8001DF30..0x8001DF50`).
   Only a change of state resets the goal (`0x8001DF54..0x8001DF6C`).
5. Sound/effect commands compare against the resulting absolute frame;
   e.g. sound equality at `0x8001DFF4..0x8001E000`.

Listings can be repeated with `psx disasm --ram .psx/wall_game --addr ADDR
--count N --text`. No source-format constants were inferred from another port.

## Runtime records

Wall item 72 is a type-107 door. `0x8008AAAC` contains item-array base
`0x0019C940`; stride is 76, giving item address `0x8019DEA0`. A dump confirms
kind 107 at +0x0C, current/goal at +0x0E/+0x10, clip/absolute frame at
+0x14/+0x16, and activation flags at +0x28. At the checkpoint it is open,
clip 333, absolute frame 62, state/goal 1, flags `0x4000`.

`tools/record_door_animation.py` records and copies samples plus metadata:

```sh
python3 tools/record_door_animation.py --case control
python3 tools/record_door_animation.py --case cycle
python3 tools/record_door_animation.py --case heldout
```

The unmodified control holds clip 333/frame 62 for all twelve fields.
The cycle changes only activation flags: `0x7E00` at relative field 20 and
`0x4000` at field 160. With the inversion bit set, these mean inactive and
active respectively. Animation, state and goal fields are never poked.

| Recorded row (relative field) | Clip / absolute frame | State / goal |
|---|---|---|
| 0 | 333 / 62 (open hold) | 1 / 1 |
| 22 | 332 / 1 (start close) | 1 / 0 |
| 144 | 331 / 0 (closed hold) | 0 / 0 |
| 162 | 334 / 63 (start open) | 1 / 1 |
| 284 | 333 / 62 (open hold) | 1 / 1 |

All intermediate clip frames are compared, not just these boundary rows.
Samples are after N fields; a poke at field 20 first appears in row 21 and
the following logic tick appears in row 22. Logic remains one tick per two
fields. The untouched baseline is retained separately.

The held-out run was captured after the engine step and door goal rule were
written. It deactivates at field 0 and reactivates at field 24, during closing.
It still finishes closing at row 124, starts reopening at row 126 and reaches
open hold at row 248. The intervening goals/frames match exactly.

`game/tests/doors.rs` compares all fields at logic-tick boundaries in all three
recordings (306 ticks total). The cycle and held-out checks also verify engine
sound cues for id 64 on clip 332/frame 1 and clip 334/frame 63. Cue frame
semantics come from the animation code and table; native SPU playback,
spatialization and mixing were not measured in this pass.

## Engine, game and editor

`AnimationGoal` opts a placed object into `step_authored_animation`: authored
dispatch, next links and frame/end cues. The game registers it for doors and
runs the verified goal rule before the engine animation step. Legacy creature
and scenery animation is unchanged pending per-type verification. End commands
and effect commands are emitted as `ObjectAnimationCue` messages; this pose
step does not execute movement, jump, kill or effect handlers. Sound cues feed
the engine's existing sound-request path. Room/environment filtering and
non-spatial playback retain the existing audio limitations.

The editor previews placed models with the same engine step, while suppressing
all cue side effects. It can play/pause, step, seek backward within a clip,
cycle the model's goal states and isolate its mesh. This is authored-data
inspection, not simulation of a game's door controller. The engine sandbox
has a hand-authored four-clip lift/hold/return example, independent of game data.
Animation link validation and save/reopen checks accompany the preview.

No level format version changes: dispatch ranges, links and commands already
exist in the engine format. Unknown properties and byte-exact no-op saves remain.

## Remaining scope

Only the ordinary goal/pose path of the type-107 Wall door has recorded
runtime coverage. Other door types share this controller but have not each
been exercised. Its occupancy override at `0x8004C960..0x8004C9C4` is not
ported. Native door floor/portal changes and collision while opening/closing
remain provisional, as do queued required states and active-list handling.
The character animation/movement controller remains separate and its existing
recorded trajectories must stay exact.
