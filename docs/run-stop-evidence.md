# Authored running stops and restart requests

Evidence is only the supplied Tomb Raider II USA Rev 3 disc through `psx`.
This milestone covers neutral release of Up during ordinary running,
foot-dependent braking clips and Up re-held during braking. Direction
reversal, run-to-walk, jump/roll interruptions and exceptional collisions
remain separate work.

## The mismatch and the rule

The Reach ceiling release recording first exposed the problem after landing:
`ceiling_reach_late_release_heldout.csv` field160 is clip0/frame20, x28542,
speed47. At field162 the PSX requests goal2 while staying in clip0/frame21
and moving to x28589. The former port immediately entered idle103 and stopped
at x28542. The original wraps the run loop at164, dispatches to braking8 at166,
and gradually stops at x29005 by194. Clip11 begins at198, then idle103 at200.

Run control `0x800387B0`, re-read through `psx disasm --ram
.psx/ceiling_low_start --addr 80038948 --count 21 --text`, requests goal2
when Up is absent (`0x80038954..0x80038994`); it does not force a standing
animation or clear speed. AnimateLara's established advance/dispatch/end-link
order and the extracted authored ranges own the timing:

- Run loop0 accepts goal2 at frames0..5 into clip8/frame131, or10..14 into
  clip10/frame169. Other frames continue running until a valid range.
- Run-start6 accepts goal2 only at absolute frames102..104, into clip10/frame168.
- Braking8 and10 remain state1, with their authored negative acceleration.
  Both naturally finish into clip11/frame185, state2. Neither offers a
  goal1 dispatch, so re-pressing Up cannot restart run-start6 mid-brake.
- Once standing is reached, a still-held Up enters run-start6 on the next
  tick. A request must be reconsidered even if its requested clip id did not
  change across the natural end link.

The Bevy controller keeps `PlayerMode::Run` and its authored speed while the
stop request waits. Movement and animation use the same prospective clip
frame. Ordinary run/stand requests use authored dispatch on every tick;
forced collision clips enter directly, including vault50 whose declared
state2 must not be mistaken for a request to brake. Existing climb, wall,
camera and other trajectory tests retain their exact results.

## Independent routes and calibration provenance

The calibration above is retained unchanged and explicitly labelled
`used_to_refine_rules`. It uses the controlled room7 starting fixture described
in [ceiling-evidence.md](ceiling-evidence.md). The five additional ordinary pad
routes use the existing `.psx/home_runway_start/state.bin` with no pokes.
`tools/record_airborne.py --case NAME` records them, keeping input, identities,
checkpoint/sample hashes and complete heading-watch hashes in their sidecars.
The historical `airborne_` prefix is retained by the shared trajectory harness.

| Case | Input (fields) | Observed result / role |
|---|---|---|
| `run_stop_left_heldout` | Up[0,60),200 fields | Independent clip10 braking at62; standing94 |
| `run_stop_right_heldout` | Up[0,80),220 fields | Independent clip8 braking at82; standing114 |
| `run_stop_rehold_heldout` | Up[0,56), Up[70,114),240 fields | Calibration: exposed forced restart when Up was re-held during braking; clip10 must finish at90 before restart92 |
| `run_stop_rehold_validation_heldout` | Up[0,64), Up[76,136),260 fields | Independent shifted re-hold and second release |
| `run_stop_restart_heldout` | Up[0,66), Up[90,138),260 fields | Fresh independent later restart and second stop |

The five new routes cover595 sampled logic ticks across13 columns, including
exact x/z despite the oblique starting yaw-15647. The test applies only the
existing0.02-source-unit y allowance. Clip8/10 are identified by their asset
ids; the left/right case names distinguish captures. All retained calibration
filenames preserve their original recording identity, with JSON and test names
marking their role.

## Next investigated gap: forward ledge falls

A separate no-poke diagnostic `.psx/fall_forward_probe` starts from
`.psx/home_hang_start/state.bin`, rolls with Circle[0,2), waits, then holds
Up[90,180), and runs300 fields. The PSX leaves the ledge into clip34/state3 at
field138, changes to35 at148, and lands into clip92/state1 at170, with speed47.
Releasing Up while92 continues requests goal2 and eventually brakes via8.
The port's ordinary fall path still uses its provisional landing timer and
`freefall_landing`; this is a concrete next investigation, not verified parity.
The subsequent [ledge-fall milestone](ledge-fall-evidence.md) uses a checkpoint
after the roll and now verifies ordinary running ledge departure and both
authored landings. The broader unassisted-fast-fall/slide landing TODO remains open.

## Verification

`cargo test --workspace --all-features` passes208 tests, including all prior
climb and camera trajectories, with six opt-in tests ignored. Workspace Clippy
and all-target builds pass with the existing unrelated warnings; the changed
Rust files and `git diff --check` pass. Workspace-wide formatting retains the
previously documented differences in unchanged files.

`tools/airborne_smoke.py` passes160 aligned poses across19 routes, including54
new run-stop poses. Together with the120 ceiling poses, both full-game sweeps
pass280 poses. Original and port screenshots at braking/restart frames were
inspected. The smoke verifies positions and authored clip/frame timing; it does
not claim pixel parity, HUD parity or complete Home NPC rendering. Reports,
paired images and MCP transcripts are under `validation/{airborne,ceiling}-smoke/`.

The engine-only sandbox passed a30-tick movement and rendering check. The
editor passed sandbox and Home byte-exact untouched save, undo/redo, save and
reopen checks, plus Home placement8 clip10 preview. No engine format, editor
or template change was required. Build/test/Clippy logs and sandbox evidence
are under `validation/reach-run-stop/`.
