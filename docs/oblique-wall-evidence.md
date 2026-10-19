# Frontal oblique running contacts

The complete Home navigation replay exposed a14-unit sideways error on Lara's
first wall-impact tick. A frontal run stop now restores the perpendicular
coordinate from the tick's beginning while retaining the original101-unit
normal separation. Ground running and braking also use the already verified
quantized sine/cosine table, avoiding occasional one-unit approach drift.

## PSX evidence

Only the supplied USA Rev3 disc and `psx` were used. The former
`winston_nav_turn_heldout` route is calibration for this correction, not an
independent wall validation. It starts at `.psx/home_runway_start/state.bin`
and uses Up[0,150), Left[150,174), Up[174,238), without pokes.

`tools/record_oblique_wall.py` reproduces the decisive field225 instruction
trace and position writes. The checked-in `oblique_wall_trace.json` retains
the command, checkpoint/build identity, trace/write hashes and observed
context/item stores. The trace and watch both report complete coverage.

At sampled field224 Lara is [16492,2560,95550]. AnimateLara writes the
proposed [16446,2560,95536] at `0x80043274` and `0x800432A4`.
GetCollisionInfo's west-front branch stores normal shift39 at `0x8001B838`,
then restores the previous Z using shift14 at `0x8001B840`. It stores frontal
collision type1 at `0x8001B9B4`. Run collision calls LaraDeflectEdge
`0x8003C544`; its frontal branch calls ShiftItem `0x8001C09C`, which writes
X16485 and Z95550 at `0x8001C0AC`/`0x8001C0D4`. Speed becomes zero and the
existing authored hit/recovery path continues. The original source therefore
cancels this tick's sideways displacement on a front impact. The old port
accepted Z95536 through independent-axis sliding even while stopping the run.
Side-only deflections remain a different branch and were not rewritten here.

The opposite-angle and run-start captures also exposed earlier one-unit
position differences. AnimateLara uses the same quantized trigonometry as the
verified airborne/controller motion. Ordinary grounded Run/Idle braking now
uses those helpers and signed product shifts; the other movement families
retain their existing controllers. This changes approach rounding, not clip
speed or animation timing.

## Trajectories and independent validation

`game/tests/winston_contact.rs` now runs the original complete route and five
new ordinary replays: delayed left approach, opposite-angle approach, blocked
run start, then fresh right and start validations recorded after both rules
were written. All2745 ticks match18 sampled columns, including avatar position,
yaw, speed, room, clip/frame/state/goal and the attendant's recorded movement,
authored pose and mesh contacts. The two opposite-angle/start captures used
to diagnose ground rounding are explicitly labelled calibration in metadata
and test names, despite their historical filename suffix. The delayed-left
route validates tangent restoration; the last two validate the combined rule.

The calibration includes the complete authored hit53 recovery. Other routes
also verify the run-start distinction: clip6 stops through the standing path,
without a hit-wall clip. Both tangent directions are exercised. No scenario
uses a memory poke or supplies positions after the initial state.

This closes the complete-game navigation diagnostic as an avatar fix. Winston
needs no fitted target or movement correction. Static-mesh contacts, corner
sweeps, side-only deflection and ceiling combinations remain outside this
milestone's scope.

Verification: all-features workspace tests and Clippy completed, with existing
unrelated warnings. Four complete-game MCP routes pass65 checkpoints: the
former navigation diagnostic, delayed-left approach and both fresh combined
validations. They check the real avatar and attendant without supplying
positions during replay. Native/rebuilt wall-contact and settled views were
visually inspected. The engine sandbox ran, and the editor Home pose/untouched
save check passed. No engine format, template or authored level edits were
required. Changed Rust files pass rustfmt directly.
