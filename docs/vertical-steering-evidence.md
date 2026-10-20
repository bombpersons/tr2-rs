# Vertical jump directional steering

Evidence is the supplied Tomb Raider II USA Rev 3 disc through `psx` only.
This milestone covers signed forward/backward steering during early vertical
ascent, input reversal, the ascent cutoff, front-wall contact and landing.
It builds on [front-floor clearance](vertical-clearance-evidence.md).

Jump-up collision `0x8003B7B8`, re-read from `.psx/home_hang_start/ram.bin`,
establishes these rules (listing in `validation/vertical-steering/jump-up.txt`):

- `0x8003B7D8..0x8003B820`: animation movement keeps yaw as its heading;
  negative speed reverses only the collision probe heading.
- `0x8003B89C..0x8003B8B4`: any collision retains signed speed+2 for positive
  incoming speed, -2 otherwise. Centre ceiling contacts share this response.
- `0x8003B8B8..0x8003B92C`: without contact, steering is accepted only when
  fall speed is below-70, after gravity. Up adds1 if speed is below5;
  otherwise Down subtracts2 if speed is at least-4. Testing before subtracting
  allows the sequence0,-2,-4,-6. Input changes the next tick's displacement.
- `0x8003B930..0x8003B98C`: descending floor contact requests standing, zeros
  fall speed and clears gravity while retaining horizontal speed until the
  authored landing dispatch on the following tick.

The game uses the existing ECS motion component and authored animation assets.
Vertical motion retains a signed speed along yaw independently of its front/back
collision heading. The trajectory harness now measures that signed projection
for vertical jumps, as it already did for fast-fall rebounds.

## Complete recordings

| Case | Route and observations |
|---|---|
| `vertical_steer_forward` | Oblique Home runway; Square[0,8), Up[28,50),240 fields. Speeds3/4/5 at30/32/34, keeps5 after early ascent, lands100 and recovers102 |
| `vertical_steer_wall` | Cardinal front wall; Square[0,8), Up[28,50),240 fields. Contact holds2 through36; clearance permits3 at38 and4 at40; -68 fall speed freezes4 at42; lands atop the wall |
| `vertical_steer_back_open` | Cardinal open area; Square[0,8), Down[28,50),240 fields. Speeds0/-2/-4/-6 at30/32/34/36, retains-6 through landing100, recovers102 |
| `vertical_steer_reverse_heldout` | Independent: Square[18,28), Down[46,52), Up[52,86),260 fields. Early reversal changes-4 to-3/-2/-1 at54/56/58; later Up cannot accelerate further; landing118/recovery120 |
| `vertical_steer_late_heldout` | Independent: wall, Square[16,24), Down[62,80),260 fields. Late Down does not alter speed2; descending wall contact and landing116/recovery118 |

All625 sampled logic ticks compare thirteen columns, exactly apart from the
existing0.02-source-unit y allowance. The two independent recordings were made
after implementation and pass without further controller changes. All routes
use ordinary pad input and no pokes. Sidecars retain precise commands, input,
checkpoint/sample/write hashes, tool build, disc and BIOS identities.

`tools/record_airborne.py --prepare-vertical-open` prepares the open fixture from
`vertical_wall_start`: Down+R1[0,60),140 fields. It settles at
[19968,2816,90739], yaw0. The source checkpoint was already recorded by the
clearance milestone. Preparation commands and hashes are retained in the new
sidecars. Reproduce each recording with `--case NAME`.

## Item-pushing diagnostic and subsequent combined verification

`vertical_steer_backward` is retained as diagnostic for the isolated player harness. Its oblique runway
route matches animation, signed speeds, y and all other non-position columns,
but x first differs by5 at field40 and z by111 at88. A fresh position-write
watch attributes these changes to `0x8001CA78/0x8001CA90`, inside the animated
item-bounds push routine `0x8001C890`. A focused native trace at field39 shows
that routine receives item pointer0x17C8C8 as the obstacle and Lara0x17C87C.
The obstacle's item kind at+12 is260, at [21342,2560,95744] in the runway
checkpoint; it corresponds to Home placement9. This is a separate dynamic
object collision gap, not a steering mismatch. No passing parity test is
claimed for that route. Position writes, trace, listings and reports are retained
under `validation/vertical-steering/` and `.psx/vertical_backward_*`.

The combined player/attendant harness now covers this interaction with fresh
native recordings; see [Winston's extended vertical-contact verification](winston-evidence.md#extended-vertical-contact-verification).
The older diagnostic remains unchanged: its player-only harness deliberately
does not spawn the obstacle. The fresh checkpoint has the same player placement
but a different attendant animation frame, so these captures are not presented
as a byte-identical reproduction of that older run.

## Verification

Workspace tests pass226 tests. All-target builds and Clippy pass with existing
unrelated warnings; changed Rust files and `git diff --check` pass. Full-game
MCP checks64 poses across the five passing routes, with paired PSX screenshots
using the established four-field display offset. Checked values are position,
clip and authored frame; sky, HUD, NPC and raster parity remain outside this
claim. Reports and exact transcripts are in `validation/vertical-steering/airborne/`.

Engine sandbox movement and rendering pass30 ticks. Editor sandbox and Home
checks preserve untouched bytes and exercise undo/redo and save/reopen; Home
previews clip28. No engine format, editor or template change was needed.
