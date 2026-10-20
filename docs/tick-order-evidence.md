# Tick ordering: PSX evidence and port boundaries

This change uses only the supplied USA Rev 3 disc through `psx`. No external
reimplementation was consulted. The original has already been investigated
in [object-table-evidence.md](object-table-evidence.md); this pass independently
checked its call sites and recorded a small trace suitable for regression tests.

## Observed

From `.psx/wall_game/state.bin`, four neutral fields recorded two logic ticks,
at absolute fields 21001 and 21003. `.psx/tick_order_port/trace.txt` contains
440 lines, with no dropped lines. The caller addresses, in execution order:

| Caller | Work |
|---|---|
| `0x8001DC54` (twice) | Active item controllers |
| `0x8001DCD0` | Active effect controller |
| `0x8001DCE8` | Lara control `0x800424F0` |
| `0x800383E0` | Lara animation `0x80042E08` |
| `0x80038400` | Object collision `0x8001C258` |
| `0x80038438` | State collision handler |
| `0x80038444` | Room update `0x8001C0E8` |
| `0x8003844C` | Weapons `0x8003FC4C` |
| `0x80038458` | Floor triggers `0x8001EE30` |
| `0x8001DCF0` | Hair `0x8002F674` |
| `0x8001DCF8` | Camera `0x80019BF0` |
| `0x8001DD00` | Sound sources / flip effect / sound update `0x800290E4` |

`psx disasm --ram .psx/wall_game --addr 0x8001DC20 --count 100 --text`
and the listing at `0x800383D0` confirm those call sites. The item loop reads
its next link before calling the controller (`0x8001DC48`). A trigger that
activates an item runs after that tick's item loop; its controller therefore
first runs on the following tick. This is an inference from the observed
call order, rather than a separate measurement of a particular trap's timing.

After the schedule changes and first regression tests were written, a held-out
run from `.psx/home_side/state.bin` held RIGHT for six fields. Its three ticks
(fields 14993, 14995, 14997) have five item calls and no active effect calls.
The remaining phase order matches the Wall recording. No memory pokes were
used. The trace has 808 lines and no dropped lines.

## Ported

The Bevy schedule now runs:

`Prepare -> Initialize -> Items -> Control -> Motion -> Animation -> Interaction -> SecondaryMotion -> Camera -> Effects`

`LifecycleSystems` resets per-level state and initializes object components
before item controllers can read them. This retains deferred first-tick
initialization; native InitialiseItem itself runs when items are loaded or
spawned, not in a per-tick phase.

`ItemSystems::Control` runs creature and door controllers before Lara.
`ItemSystems::Animation` advances the existing generic object clips afterward.
Lara's pose commit precedes object interaction; visual placement is synchronized
after object pushes, without stepping her pose again. Interaction orders
nearby use, collision, avatar publication, combat, then floor triggers and
reporting. A door's animation target now responds to a floor activation on the
following tick. Triggers read the player position after a door pushes her,
rather than the earlier position. Hair runs before the camera; sounds follow it.

No asset format changed. The editor's level fields, preview and save format
continue to cover the same data, and the editor remains independent of game
controllers and simulation scheduling.

The [rolling item-contact follow-up](item-push-evidence.md#rolling-contact-before-floor-response)
exposed the importance of state collision following object pushes: a lateral
push onto lower floor must change the roll's height on that same tick.
Rolling now splits animation/movement from floor/wall response, scheduling
the latter after `InteractionSystems::Resolve` and before `Publish`. Its
per-player collision context carries the accepted origin through item pushes.
Other locomotion families still perform their level response in `Motion`;
the overall port schedule alone does not establish their object-contact
ordering parity.

## Verification and limits

`game/tests/data/original/tick_order{,_heldout}.json` retain the trace caller
sequence, source build, checkpoint/trace hashes, field numbering, input and
reproduction command. `tools/record_tick_order.py` regenerates them:

```sh
python3 tools/record_tick_order.py
python3 tools/record_tick_order.py --heldout
```

`game/tests/tick_order.rs` projects the implemented phases from those observed
call sequences and compares the actual Bevy execution order. It explicitly
excludes unimplemented active effects and separately ordered state collision /
room update. It samples the item phase once, so repeated calls are collapsed.

`game/tests/interactions.rs` has authored scenarios proving the consequences:
newly activated creatures wait until the next tick; creatures read the player's
previous position; door pushes precede floor triggers; door animation retargeting
waits until the next item pass. These are schedule regression fixtures, not
new evidence for the prototype's damage, AI, collision bounds or clip rules.
All recorded player/camera trajectories remain exact.

Remaining gaps:

- The engine still iterates archetype queries; native active-item and room-list
  ordering, including activation/removal during traversal, is not reproduced.
- There is no active-effect list. Hair timing is supported; the missing flip
  effects are still missing.
- Doors now opt into verified authored dispatch/chaining and hold poses
  ([door-animation-evidence](door-animation-evidence.md)). Legacy creature/scenery
  clips still loop; active-list gating, creature AI/damage, door geometry,
  switch/pickup animation and collision remain provisional.
- Lara's controller continues to predict animation movement within integration
  and commit its pose afterward. Level collision remains inside Motion, before
  object collision; native state collision follows object collision. Exact
  player trajectories are preserved, not a claim that all internal boundaries
  of Lara control have been reconstructed.
- Pickup/switch use and pushes are separate passes, not a single native
  room-list collision walk. Weapon input/shot generation remains in the player
  controller; only combat resolution is ordered after object collision.
