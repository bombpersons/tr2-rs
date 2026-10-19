# Fast-fall impact damage and the authored fatal ground pose

Evidence is the supplied Tomb Raider II USA Rev 3 disc through `psx` only.
This milestone verifies the integer impact curve at its safe/lethal boundaries,
retains the lethal health sentinel and fixes fast-fall's authored death dispatch.
It uses explicit controlled initial motion, not a claim of a naturally reached
high-drop route. Death from combat, traps, water or other airborne states,
long-drop entry conditions and death-camera behavior remain separate work.

## Controlled initial falling actor

`tools/record_fall_damage.py` starts from the ordinary, settled
`.psx/vertical_open_start/state.bin`, at [19968,2816,90739], yaw0, room16.
Checkpoint preparation changes only eight actor values: feet y2600; animation23
at348; current/goal state9; horizontal speed0 (20 for the moving validation);
fall speed `impact-2`; and the gravity bit8 in the existing actor flag word.
Every other bit of that word is preserved. Two neutral fields advance the
original once, producing a falling checkpoint just above the2816-unit floor.
The following replay has no pokes and hits the floor on its first logic tick.
Code, level, other items and world geometry remain unmodified.

The fixture deliberately supplies an already falling actor near impact. It
isolates the collision rule from unrelated high-drop geometry and entry rules.
All preparation commands, root/final checkpoint hashes, input, source identity,
complete sample/write/trace hashes and toolkit builds are in the sidecars.

## Binary and observed rules

Listings are in `validation/fall-damage/`. Landing damage0x8003E044 reads the
signed fall speed and subtracts140. It returns without damage at/below140
(0x8003E0A4..0x8003E0BC). At141..154 it subtracts
`floor((speed-140)^2 * 1000 / 196)` from health; the multiplication/division is
0x8003E0D8..0x8003E114. Above154 it writes health-1 directly
(0x8003E0CC..0x8003E0D4). At154, a full-health actor reaches exactly0.

Fast-fall collision0x8003AEEC calls that routine on floor contact. A surviving
actor enters heavy landing24/state2 immediately. A dead actor instead requests
8, retaining the fast-fall clip and contact speed for that tick
(0x8003AF54..0x8003AF84). Both clear vertical speed/gravity and settle the feet.
The authored state range subsequently selects ground death25, whose natural end
holds its final frame417. LaraControl clamps any dead health to-1 on its next
tick at0x80042B10, including the speed154 impact's initial health0.

The game now has an explicit `FatalLanding` mode that retains the death goal,
advances the authored pose and rejects input. It no longer replaces this contact
with unrelated clip87. The shared landing-damage helper also replaces the older
duplicate floating-point calculation, and the HUD recognizes health<=0. The
synthetic lethal-fall test now expects the PSX-confirmed -1 sentinel while keeping
its existing assertions that movement and firing remain disabled.

## Native wait and display-field timing limitation

The full140-field speed155 capture is retained as `fall_damage_155`, explicitly
`diagnostic_only: true`. It shows impact at field2, then a native wait before
logic catches up. The focused trace proves that the first-death call at
0x80042B28 enters0x8006DA7C in run field3 and returns at0x80042B30 in field8.
The subsequent death updates catch up in field9. A CD trace recorded zero
commands: the precise wait cause is **unconfirmed**, not established as disc I/O.

This full display-field trace cannot be treated as one30-Hz logic tick per two
fields. It is not fitted with an arbitrary gameplay delay. Two separate fixtures
verify the impact tick and the stable authored death pose after the native wait.
The latter is prepared by running12 fields from the fatal checkpoint, without
additional pokes, then replayed at the original's stable cadence. The port remains
a fixed-tick Bevy game and does not reproduce hardware waits or frame catch-up.
Fatal transition timing across that wait is outside the per-field parity claim.

## Passing fixtures

Five calibration fixtures and five fresh independent fixtures compare513 sampled
logic ticks across thirteen columns, exactly except the existing0.02-source-unit
y allowance. Impact-only fixtures intentionally stop at the contact tick.

| Case | Verified behavior |
|---|---|
| `140` | Health1000; heavy landing24 at2, standing11 at82, idle103 at84 |
| `141` | Five damage, health995; complete heavy recovery |
| `153` |862 damage, health138; complete heavy recovery |
| `155_impact` | Contact health-1, goal8, retained fast-fall23/frame350 and settled feet |
| `fatal_pose` | Stable post-wait death25 advances to417 and holds; health-1 throughout |
| `142_heldout` | Independent20 damage, health980; Square during heavy recovery changes its goal but cannot cancel the committed clip |
| `147_heldout` | Independent250 damage, health750; later Up+R1 uses ordinary authored walking and stop |
| `154_heldout` | Independent lethal boundary: contact health0 and death goal8 |
| `155_moving_heldout` | Independent moving fatal contact: horizontal19 at start brakes to18, moves18 units and retains18 on contact |
| `fatal_pose_heldout` | Independent movement/jump/turn input cannot interrupt or move the stable death pose |

Reproduce with `python3 tools/record_fall_damage.py --case NAME`. The original
full-wait diagnostic remains separate from passing calibration/validation. For
two-field fixtures, an additional six-field native display capture supplies the
paired image at the established display offset; its command/identity are retained.

## Verification

All258 workspace tests pass. All-target builds and Clippy pass with existing
unrelated warnings. Controller and harness Rust files are formatted; pre-existing
formatting in the HUD file is retained. `git diff --check` passes.

Full-game MCP checks65 poses across the ten passing fixtures, including health
as well as position, clip and authored frame. The harness seeds falling/dead
actors from their recorded initial values; ordinary routes retain their existing
initialization. Reports, paired PSX images and exact MCP operations are in
`validation/fall-damage/airborne/`. Contact, reduced-health recovery and held
terminal-body images were inspected. The established four-field display offset
applies to these paired poses; the native wait itself remains diagnostic.
Raster, HUD styling, audio, NPCs and death-camera parity are not claimed.

Engine sandbox movement/rendering and editor sandbox/Home byte-exact saves,
undo/redo, save/reopen and terminal clip25 preview pass. No engine format,
editor or template change was required.
