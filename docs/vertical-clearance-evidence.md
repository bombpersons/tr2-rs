# Vertical jump front-floor clearance

Evidence is the supplied Tomb Raider II USA Rev 3 disc through `psx` only.
This change covers vertical jumps against a flat cardinal room wall, including
clearance while ascending, contact again while descending and authored landing.
Side/corner contacts, static meshes and vertical-jump directional steering remain
separate work.

`psx disasm --ram .psx/home_hang_start/ram.bin --addr 0x8003B7B8 --count 124 --text`
shows jump-up collision setting its front-floor lower limit to -384 at
`0x8003B7E4..0x8003B7E8`, with body height870 at `0x8003B828`.
GetCollisionInfo compares the relative front floor with that limit at
`0x8001B790..0x8001B79C`. In Bevy's upward coordinates the rule is: allow the
front floor up to384 source units above the proposed feet. The old airborne
check used the greater of the previous and proposed feet, refusing the movement
until Lara's feet reached the top of the wall. The game now uses the vertical
jump's own margin. The existing blocked-wall response retains speed2.
Listings are retained in `validation/vertical-clearance/`.

The former walking-stop diagnostic `braking_wall_validation_heldout` now passes
unchanged CSV bytes. Its metadata explicitly marks it as calibration for this
change, preserving original command, sample hash and checkpoint identity. At
field226 its feet are2436 with z91035; field228 reaches2356 and z91037 as the
front floor2560 falls within the384-unit margin. All131 sampled ticks pass
across thirteen columns, with the existing0.02-source-unit y allowance.

Two fresh independent routes were recorded after implementation. Checkpoint
preparation uses ordinary pad input from `forward_ledge_start`, then260 fields
to settle at the wall; no memory pokes or modified level/code. Commands, input,
source/checkpoint/sample/write hashes and disc/BIOS/toolkit identities are in
the sidecars and `tools/record_airborne.py`.

| Case | Input and observed result |
|---|---|
| `vertical_clearance_heldout` | Square[0,8),220 fields: launch30, clearance38, descending wall contact92, landing100, recovery102, idle144 |
| `vertical_clearance_delay_heldout` | Square[18,28), Square[72,84),260 fields: launch48, clearance56, descending contact110, landing118, recovery120, idle162; the airborne jump request is ignored |

Both independent routes pass without further rule changes:242 additional
sampled ticks, for373 across all three routes. Thirteen columns compare with
zero tolerance except the existing y allowance. Reproduce with
`python3 tools/record_airborne.py --prepare-vertical-wall`, then `--case NAME`.

Full-game MCP captures check41 poses across the three routes, with paired PSX
screenshots at the established four-field display offset. Movement and authored
clip/frame are checked; raster, sky, NPC and HUD differences remain outside this
claim. Reports, paired images and transcripts are in
`validation/vertical-clearance/airborne/`.

Workspace tests pass221 tests, and Clippy and all-target builds pass with the
existing unrelated warnings. Changed Rust files and `git diff --check` pass.
The engine sandbox passes30 ticks of movement and rendering. Editor sandbox and
Home checks preserve untouched bytes, undo/redo and save/reopen behavior, with
Home previewing clip28. No engine format, editor or template change was needed.
