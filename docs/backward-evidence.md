# Backward hop and walk

PSX-only evidence from the supplied Tomb Raider II USA Rev 3 disc, executed
with psx native. No memory pokes or external reimplementations were used.
The existing constant-speed backward prototype is replaced for these moves;
unrelated inherited player rules retain their previous provenance.

## Binary observations

The state table at `0x80082C98` selects stop `0x8003899C`, hop-back
`0x80038D00` (state 5), and walk-back `0x80039270` (state 16).

- Stop's `0x80038B44..0x80038B6C` requests hop-back on Down alone. With
  walk held (input bit `0x80`, physical R1), it calls the walk-back handler.
  The separate wading branch is outside this increment's coverage.
- Hop-back always requests standing (goal 2, `0x80038D0C`), regardless of
  Down being held or released. Its clips have no standing dispatch ranges;
  their end links finish the complete move. The handler pushes turn rate by
  409 and clamps it to ±1092 before the shared 364 decay, giving a steady
  turn of ±728 yaw units per tick.
- Walk-back requests state 16 only while Down+walk remains held, otherwise
  goal 2 (`0x80039294..0x800392C0`). Its turn cap is ±728 before decay.
  Releasing input asks for a stop; the animation table decides when it occurs.
- AnimateLara `0x80042E30..0x80042FD8` increments the frame, tries the
  authored goal ranges, then follows end links. `0x80043190..0x800431C4`
  obtains speed from the selected clip and its acceleration.
- Horizontal motion at `0x80043248..0x800432A4` uses the movement heading
  from the previous collision pass. Walk-back collision writes yaw+half-turn
  at `0x8003B454`; hop-back does so at `0x8003AC84`. Write watches in the
  retained runs confirm these writers. Thus the first walk-back tick still
  moves **forward** at speed 2 before subsequent ticks move backward.

Reinspect with, for example:

```sh
psx disasm --ram .psx/home_side --addr 8003899c --count 123 --text
psx disasm --ram .psx/home_side --addr 80038d00 --count 37 --text
psx disasm --ram .psx/home_side --addr 80039270 --count 57 --text
psx disasm --ram .psx/home_side --addr 80042e08 --count 307 --text
```

## Authored motion and recordings

The already baked animation tables supply every clip duration, speed,
acceleration and stop window. No new asset format or editor field is needed.
This remains a game controller over the engine's authored animation data.

All routes start at the unmodified Home-side checkpoint (field 14992),
position `[19968,2560,95744]`, yaw -15647, room 16, clip 103/frame 1636.
The JSON sidecars in `game/tests/data/original/` retain the full commands,
input schedules, psx build, checkpoint/sample hashes and watch hashes.

| Recording | Input, relative video fields | Result |
|---|---|---|
| `backward_hop` | Down [0,40), then neutral through 100 | Clip 88 at field 2, 89 at 22, 90 at 46, standing clip 11 at 58, idle 103 at 60 |
| `backward_walk` | Down+R1 [0,100), then neutral through 180 | Start clip 41 at 2, walk 40 at 34, stop 39 at 144, standing 11 at 162 |
| `backward_hop_heldout` | Down [0,4), Left [6,42), neutral through 100 | Early release still completes the hop; turn accelerates, caps and coasts |
| `backward_walk_heldout` | Down+R1+Right [0,44), Down+R1 [44,58), neutral through 160 | Turns from the first tick; stops through the other foot's clip 38 at 84, standing 11 at 102 |

Both held-out routes were recorded after implementation. Neither was used
to fit the movement rules. The walking route passes with exact x/z position,
yaw, turn rate, speed, state, clip and absolute frame. The two baseline
routes also match those fields exactly. All four remain level at y2560.

The turning-hop route exposes one bounded numerical difference, retained
explicitly in the test: at tick 9, old yaw -16322 gives native x+37 but float
sine x+36. `0x80063F74` folds yaw into a quarter turn, discards four low
angle bits, and reads the signed sine table at `0x80083FF8`. Its entry at
`0x800847F0` is 16384, rounding this nearly cardinal direction to 1.0.
The port keeps float trigonometry; x remains one original unit (1/1024 world
unit) behind for the rest of this route. Other compared fields match exactly.
This is a declared numerical bound, not a rule fitted to that held-out path.

`game/tests/backward.rs` compares all 270 recorded logic ticks, including
the stop windows and idle recovery. Input schedules are read from the retained
metadata, never inferred from the original's animation samples.

## Reproduction and visual limits

```sh
python3 tools/record_backward.py --case hop --camera
python3 tools/record_backward.py --case walk --camera
python3 tools/record_backward.py --case hop_heldout
python3 tools/record_backward.py --case walk_heldout
cargo test --workspace --all-features
cargo build -p tomb-raider-ii --all-features
python3 tools/backward_smoke.py
```

Use `--psx /path/to/psx` when the CLI is not on PATH. MCP recordings can be
retained with `--summarize-only` using the same run names and arguments.

The offscreen smoke replays inputs in the full standalone game, checks
position/clip/frame at 13 checkpoints, and renders with the sampled original
camera poses. It retains the MCP transcript, native and port images under
`validation/backward-smoke/`. As in earlier comparisons, original screen
field N+4 approximately corresponds to port state field N. The hop and walk
views were inspected: room views align, while Lara's lighting differs and the
nearby butler obscures much of her in the original. These pictures verify
successful rendering, not pixel parity or unobstructed pose parity; movement
evidence is the sampled RAM trajectory.

Workspace all-features tests and Clippy pass (existing lint warnings remain).
The engine-only sandbox was built, stepped and visually inspected offscreen.
Workspace formatting checks still report existing differences in unrelated
code; those edits were preserved. The new trajectory test is rustfmt-formatted.

The later [backward collision milestone](backward-collision-evidence.md)
adds ordinary walls, steps, ledges, slopes and hop-fall recovery; the limits
below describe this earlier clear-ground increment.

Unverified here: wading, backward slope/step and wall collisions, dropping
off an edge, transitions into these moves from other active movement states,
and steering with analog input. Holding Down can request another hop once
standing is reached; repeated hops into obstacles are not covered by these
clear-courtyard recordings. The existing prototype handles unverified collision
branches; this increment does not establish their parity.
