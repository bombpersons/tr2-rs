# The assault course

Three runs on the assault course at Lara's Home, each played by a person in
a `psx handoff` and replayed through the port tick for tick. The port matches
every tick of all three, with x, y, z, yaw, clip, frame and room all exact:
[tests](../game/tests/home_lap.rs). The first lap (4921 ticks) was the one the
port was built against. The second (10312 ticks, from the house) was played
afterwards to check it, and [what it took](#the-held-out-lap) is below. The
third (6520 ticks) [works the ladders' edges](#ladder-edges). A fourth
[runs into the lattice wall and walks along walls](#walls-and-windows),
and matches every tick, Winston and the control random included.

## Recording

- Handoff `.psx/handoffs/home_lap` (12126 fields), started from
  `.psx/contact_home/state.bin` (Lara in the house). The person ran out to the
  course and played one lap of it, finishing on the zip-line. Replaying the
  handoff's `input.json` reproduces the run.
- `tools/record_wall_traps.py --case lap` (run `home`) replays the whole run
  once and writes one row per control tick
  (`game/tests/data/original/home_lap.csv`, 5303 rows). Each row has Lara's
  item (base `0x8017C87C`, Lara is item 0), her water mode, air, turn rate and
  move angle, her gun status, and the input word the tick read (`0x8008AAD4`).
  The rows are rebuilt from watched stores, as for the Wall runs.
- The test starts Lara standing where the original first stood still (row
  191, by the first block). Each tick it gives her the buttons the
  recorded input word holds, and places Home's objects as the level does.

The lap covers:
- running, walking and turning, with stops while braking;
- vaults, standing, running and up jumps, reaches and grabs;
- hanging, pull-ups and the shimmy;
- two climbable walls: climbing up, down and sideways, dropping off them,
  and climbing on from the ground, from a jump and from the wading pool;
- slides, and the tilted bridges;
- the swan dive into the pool, swimming, climbing out and wading;
- the zip-line.

## What it took

Each item below was the first divergence at the time; the routines are from
the Wall RAM disassembly (`.psx/wall_game`).

**Run and walk dispatch.** A walk request during the run's braking clip (10),
and a run request during the walk's foot stop (clip 3), wait for those clips'
own state changes, as stop requests already did. Neither clip has a way into
the other gait, so the original finishes the clip first.

**Run collision `0x8003A844`.** It runs, in order:
- the ceiling test;
- the vault test `0x8003D924`;
- the wall deflection;
- the ledge fall;
- the step-up: a floor 129 to 384 above enters clip 56 on run frames 3 to 14,
  else clip 55;
- TestLaraSlide `0x8003DD30`, at the position the move left her, so a slide
  starts on the same tick without settling onto the floor;
- otherwise `y += min(floor, 50)`: running down onto a lower floor descends at
  most 50 a tick.

**GetCollisionInfo's height.** Floors are asked for from 160 under the body's
top (762 tall), not at the feet. A bridge whose top lies between the two
counts, as the original's does. The port's ground, walk, back and roll
collisions now probe there (`body_floor`).

**Bridges (types 117, 118, 119).** These have only floor and ceiling routines
(object table `0x800936D0`: `0x8004D0B4`/`0x8004D0D4`,
`0x8004D144`/`0x8004D1A4`, `0x8004D204`/`0x8004D264`). The top is the item's
height, dropping across each sector along its facing by GetOffset
`0x8004D0F4`, shifted right 2 for type 118 and 1 for type 119. The ceiling is
256 under the top. `ObjectSurface` gained a `Ramp` for this.

**Climb flag.** TestTriggers (`0x8001EE84..0x8001EF5C`) sets `0x8008C4F6`
when the climb entry of Lara's floor data has the bit for the direction she
faces (bit q of `climb`, q the quadrant of her yaw). Each tick's collision
reads the value from the end of the tick before.

**Catching a climbable wall.**
- The vertical catch `0x8003D3C0` and the reach catch `0x8003D5BC` both use
  the edge test `0x8003D2AC`. It returns −1 when Lara's hands (her clip's
  interpolated top bound) cross a click line this tick and there is no ledge.
- LaraTestHangOnClimbWall `0x8003CAD8` then shifts her onto the wall and
  tests the climb at her two sides and the middle.
- The vertical catch enters clip 96 at its 12th frame and adds the collision
  shift again. The reach needs a 200 drop, enters clip 96 (or 150 when
  `0x8003D808` finds a drop and headroom ahead) and keeps speed 2 and fall 1.
  Its next animation pass applies gravity once more.

**Climbing.** States 56 to 61 follow their handlers
(`0x8003E950..0x8003EB14`) and collisions
(`0x8003EB40..0x8003F154`):
- LaraCheckForLetGo `0x8003F394`;
- the climb tests `0x8003F480`, `0x8003F764` and `0x8003F994`;
- the side steps `0x8003F864`;
- hanging from the wall `0x8003CC74`.

The hang (state 10) and the shimmies (30, 31, `0x8003BA74`/`0x8003BAC8`, which
move along yaw ±0x4000) use HangTest `0x8003CDE4`'s climbable branch and the
hang collision `0x8003AFCC`. That collision lets Up climb onto the wall (clip
187) and Down step down (188). This is `game/src/player/ladder.rs`, in the
original's integer units and order: control, AnimateLara, collision.

**Climbing on from the ground.** The vault test's climbable branch runs from
the walk, run and wade collisions (`0x8003A69C`, `0x8003A8A0`, `0x8003C084`):
- In the water, or where the wall is within reach, Lara climbs straight on
  (stand clip 11 toward goal 56).
- Otherwise she jumps up to it (goal 28, launch 116).

AnimateLara dispatches before the collision, so the collision is the one for
the state the step reaches.

**Dropping beside a ladder.** DeflectEdgeJump `0x8003C648` skips the rebound
when the climb flag is set and speed is 2.

**Busy hands.** Clip 174's EmptyHands command and effect 12 (`0x8002ADF0`,
in pull-up clip 97) free the hands.

**Dives.**
- The swan dive's control `0x80039C04` steepens into the fast dive (53) at
  fall speed 132.
- The fast dive's control `0x80039C48` brakes speed by 95/100, and Roll
  twists.
- Both collisions (`0x8003BE64`, `0x8003BF14`) probe as the forward jump's
  does and land with goal 2; the fast dive's lands at fall speed 134 or more
  with goal 8 (see [ladder edges](#ladder-edges)).
- Entering water from a dive (`0x80042690..0x800426D8`) pitches −8190, or
  −15470 from the fast dive. It runs one AnimateLara pass toward goal 35 (clip
  154) and doubles the fall speed.

**Fast fall.** Its collision `0x8003AEEC` probes like the reach's, sliding
along edges with `0x8003C7F4`.

**Wading.**
- Stop+Jump in the water dispatches compression 73 too.
- A wading wall stop leaves state 12 on the stop clip until its end. Its
  handler `0x8003909C` neither steers nor stops the turn, so the turn decays
  only.
- A walk's foot stop runs as state 2, whose handler gives Up to the walk
  handler and keeps steering.

**Rooms and the zip-line.**
- The pull-up updates Lara's room each tick (`0x8001C0E8`).
- The zip-line grab reads her state with the walk stop's override, and steps
  her clip as far as it takes.

## The held-out lap

- Handoff `.psx/handoffs/home_lap2` (20625 fields), from the same
  `.psx/contact_home/state.bin`. The request asked for climbing down a ladder
  to its foot, jumping back off one, jumping up to one from a step away, a
  running reach onto one, and grabs, pull-ups and shimmies.
- `tools/record_wall_traps.py --case lap2` (run `home2`) writes
  `game/tests/data/original/home_lap2.csv` (10312 rows). At Home the game
  time `0x800DE670` is the course's timer: it resets while Lara stands on the
  start and stops at the finish. So this run closes its ticks at the control
  loop's own countdown store (`0x80089428` at `0x8001DD20`, every tick).
- The test starts at its first row, Lara standing in the house. The step
  bits `0x400`/`0x800` of the input word are the pad's R2 with Left/Right.

What it covers beyond the first lap:
- the back jump off a ladder (clips 182, 183);
- letting go at a ladder's foot in the wading pool;
- climbing on from the pool;
- rolls, three-click vaults (also from standing and from a turn), sidesteps
  along walls, hop-backs off ledges, back slides;
- landing in the pool, and walking and running into walls.

The fixes, each the first divergence at the time:

**Turns.**
- The turn handlers (`0x80038D94`, `0x80038E64`) set only run, walk, the
  fast turn and stop. So a back hop, a sidestep or a jump asked for during a
  turn stops through clip 11 first.
- The stop handler only asks for a turn. The slow turn handlers push 409
  their own way whatever is held, and they ask for the fast turn only when
  they run. So the first tick of a turn never asks for the fast turn, even
  with a run's rate left over.
- A turn asked for from a clip with no dispatch to one (the vault's end clip
  51) waits for the clip, and the rate does not grow meanwhile.
- The run handler starts a roll before it steers.

**Stand and turn collisions.**
- The stop collision (`0x8003AA20`, also the fast turn's `0x8003B59C`) shifts
  Lara out of what its probe meets along her yaw. Its state is the one this
  tick's animation step reaches.
- The slow turns' collision (`0x8003AD50`) never shifts.

**Run and walk collisions.**
- GetCollisionInfo now takes the slope flags (`0x8001B310..0x8001B37C`): a
  probe on a tilt over 2 reads −32767 above Lara (slopes are walls) or 512
  below her (pits).
- The run collision sets walls; the walk collision sets walls and pits
  (`0x8003A62C`); the wade collision (`0x8003C028`) sets walls. The trace of
  the fountain shows why its rim is no vault: all three probes read −32767.
- The run collision probes its moved body:
  - A side contact turns her 910 (LaraDeflectEdge `0x8003C544`) after the
    shift. Her move angle was written before that turn, so the next run move
    goes along the old yaw.
  - A front contact shifts her back and stops her. TestLaraSlide still runs
    after the stop, and the slide wins.
  - TestLaraSlide runs whether the floor is above or below her. The older
    tick-start slope rule no longer starts slides for a run.
- The walk collision's front contact stops her the same way, with no
  hit-wall clips. The side steps (`0x8003B5BC`) probe along yaw ±16384 and
  turn off a side contact.
- The walk's and run's vault test needs only the probe's front contact. The
  vault sets its clip and goal 2 itself, also from the stand.

**Jumps and landings.**
- A rebound on the take-off tick lands at once (`0x8003ABC4` tests fall speed
  above 0).
- Wading, a landing always stops (`0x8003ABA4`).
- The landing's second AnimateLara pass moves along the heading the
  collision refreshed.
- A landing still in the fall clips keeps the landing's goal.
- The jump collision finds floors from 160 under the body's top, so bridges
  catch a falling Lara.

**Ladders.**
- The push-off clip 182 holds Lara on the wall until its end command
  launches clip 183. Clip 183's end offset (256 back) applies.
- The reach's catch snaps her yaw but not her move angle. The next pass moves
  the retained speed along the reach's heading.
- Letting go of a hang on a climbable wall drops her from where her hips are
  (GetJointAbsPosition `0x8005BE3C` for joint 0, `0x8003CF40..0x8003CF80`),
  along the wall.

**Wading and rooms.**
- In the wading stop handler (`0x80038A94`), Up or Down replace the jump's
  goal.
- The wading vault test runs its step vaults, not only its climbable branch.
- A hop back that falls off a ledge updates her room on that tick.

## Ladder edges

- Handoff `.psx/handoffs/home_ladders` (13041 fields), from the same
  `.psx/contact_home/state.bin`. The request asked for:
  - jumping up to a ladder that starts out of reach;
  - a swing-in catch, where there is nothing under the edge;
  - walks brushing walls at the side;
  - climbing to a ladder's edges.

  The person found no place at Home for the first two. The brushes were
  played wading and running, not walking. They ended the run with a swan
  dive onto dry ground.
- `tools/record_wall_traps.py --case ladders` (run `home3`) writes
  `game/tests/data/original/home_ladders.csv` (6520 rows). It closes ticks at
  the countdown, as `home2` does. The test starts at its first row.

The fixes, each the first divergence at the time:
- **Fast turn.** The fast turn's handler (`0x80039370`) sets only stop, so
  Up during a fast turn stops through clip 11 before walking. The slow turns
  go straight to the walk or run.
- **Wading into a wall from a turn.** The wade collision's front contact
  calls LaraCollideStop (`0x8003A56C`). From a stand or a turn, it puts back
  the clip, frame and state from before this tick's step. It takes the goal
  from Left or Right (7, 6, else 2) and animates again.
- **A missed hold on a climbable wall.** LaraTestHangOnClimbWall
  (`0x8003CAD8`) steps Lara onto the wall by the collision's shift before
  testing her hands, and never undoes it. When the hold fails, the reach's
  ShiftItem adds the shift again.
- **Jumping while wading.** In water mode 4 the compression handler
  (`0x800390C8`) tries no direction, so the jump is straight up whatever is
  held.
- **Sidestepping while wading.** The side steps' collision deflects off its
  sides (±910) in the water too.
- **Diving onto the ground.** The fast dive's collision (`0x8003BF14`) lands
  with goal 8 at fall speed 134 or more. Clip 153 dispatches that to death
  clip 155, and the death collision (`0x8003AEC0`) sets her health to −1.

## Walls and windows

- Handoff `.psx/handoffs/home_walls` (12919 fields), from
  `.psx/contact_home/state.bin`. The request asked for:
  - running and walking into the blue lattice wall with Up+Action, without
    Jump;
  - walking along walls with Walk held.
- `tools/record_wall_traps.py --case walls` (run `home4`) writes
  `game/tests/data/original/home_walls.csv` (6459 rows), with Winston
  (the recorder's item 1), his AI slot and the idle search's slot visits.
  The test compares every row: Lara, Winston's pose and the control seed.

All three jump-ups at the lattice wall took the ledge jump's branch (state
19, clip 26), so the climbable branch is still unrecorded. The fixes, each
the first divergence at the time:
- **Wading into a low step.** The wade collision's front contact is a wall
  hit (state 12) only when the front floor is more than 640 above her
  (`0x8003C0C0`). Lower than that, LaraCollideStop only stands her.
- **Turning through a vault.** The vault test never writes the turn rate,
  so a turn goes on decaying (364 a tick) and turning her through the
  climb. Its end command (+128 forward) is turned by her yaw as it is then,
  with the integer sine table.
- **Windows (type 72).** Each window's collision is TrapCollision
  (`0x8001C65C`): touching the glass pushes Lara back out (ItemPushLara
  `0x8001C890`), and when the pushed spot is blocked she goes back to where
  she was. Their control (`0x8004C034`) runs only once a trigger lists the
  window: it smashes the glass (`0x8004BEE0`) when she touches it at speed
  50 or more along its facing. Only the push is ported.
- **The walk's side turn.** The walk collision's side contact now comes
  from GetCollisionInfo (walls and pits, 384 either way), with the move
  angle from before the turn. The run has 73 ticks of these ±910 turns.
- **Pushes before the walk collision.** Lara's object collisions
  (ItemPushLara `0x8001CA78`) run between her move and her state's
  collision, as the run's already did here. A push that cannot clear puts
  her back where she was, and the walk collision (`0x8003A62C`) probes from
  there. At tick 5082 a window sends her back 1 unit, so her left probe
  stays out of the sill's sector and she does not turn.
- **Down while walking.** The walk and run handlers read only Up, so Down
  during the walk's stop clip 3 is a release, not a hop back.
- **Winston.** He follows Lara through the house and, at tick 5457, pushes
  her as she starts a run beside the windows. He now runs on the shared
  creature routines ([winston-evidence](winston-evidence.md#on-the-shared-creature-routines)).
  The save has him active at x 21342 in room 16 (clip 282 frame 134), not
  at his placement, so the harness starts him from the recording's first
  row, rebuilds his slot from its recorded boxes, seeds the control random
  (his wandering boxes draw from it) and makes the original's idle-search
  visits between ticks. With his search completed at once, as the port
  otherwise does, he first parts at tick 2858: there the original's search
  had not yet marked his box unreachable, so it kept its required box.
- **A walk stopped by a wall, turning.** The walk collision's deflection
  (`0x8003A6AC`) calls LaraCollideStop (`0x8003A56C`) outside its two
  foot-stop windows. From a stand it puts back the stand and steps it
  toward the turn Left or Right holds: with Up, Right and Walk held at a
  wall she turns right (state 6, clip 12). The run, wade and walk now share
  that routine.
- **Walk and run releases follow the clip.** The walk and run handlers are
  the clip's state's, not the last request's: Up alone during a walk asks
  for the run but she walks until the clip changes, and a release then
  finishes the walk's stride; a walk's run request reaching the run clip
  after a release finishes the run's.

## Not covered

No run:
- climbs onto a ladder by jumping up to it from where it is out of reach;
- swings in from a reach (clip 150);
- uses the climb test's −1 cases with differing shifts;

These follow the routines above but are not yet checked against a recording.
Where to record them:
- **The out-of-reach jump.** The climbable branch of the vault test
  (`0x8003DBA8..0x8003DC54`) does not need a ladder that starts high. It
  needs:
  - the climb flag, and not wading;
  - front and left floors at least 1920 above her, and the right at least
    2048;
  - the ceiling at least 1158 above her.

  At Home's blue lattice wall (room 45, facing −z), the original took the
  ledge jump's branch every time it was tried: running, walking and
  standing (the walls run).
- **The swing-in.** Running the port's reach at every climbable face of
  every level finds clip 150 only in cramped tunnels: Great Wall room 83,
  Opera House room 172, and a few in Tibetan Foothills, Catacombs, Ice
  Palace and Floating Islands. None is near a saved state.
The first lap's first 191 ticks, Lara running out of the house, are not
compared, because its test starts from a standing pose; the second lap's
are.
