# Water surface, entry and climb-out

Surface swimming, surfacing, the tread dive, climbing out and deep-water entry
now follow the PSX controller. The provisional "any water floor means swim"
handoffs in the ground, jump and hop-fall paths are gone: entry and exit are
LaraControl's depth tests at the start of a tick. Code: `game/src/player/surface.rs`,
the transitions in `player::control`, and `WorldGeometry::water_surface`.

## Evidence

Only the supplied USA Rev3 disc and `psx` were used (BIOS SHA256
`11052b64…1fef`, as in [original evidence](original-evidence.md)). The Home
checkpoint was rebuilt from power-on with the documented menu route and matches
the documented player pose. `tools/record_surface.py --setup` starts from the
submerged checkpoint of `tools/record_swimming.py --setup` (its position/pitch
pokes are the only pokes) and lets the original surface by itself; every replay
is pad input only. `game/tests/data/original/surface_*.json` keep the commands,
inputs, checkpoint hashes and whether a route is independent.

Static reading (`psx disasm` of Home RAM, then checked against runtime traces):

- LaraControl `0x800424F0`: water-mode transitions `0x80042560..0x80042B00`,
  then the mode table `0x80010B3C` (surface branch `0x80042C58` refills air).
- Surface wrapper `0x80044264`, handlers 33 `0x8004477C`, 34 `0x800444C0`,
  47 `0x80044584`, 48 `0x8004462C`, 49 `0x800446D4`; collision `0x80044A2C`,
  climb-out `0x80044CE0`, wade test `0x80044BB4`; dive handler 35 `0x800456DC`.
- GetCollisionInfo `0x8001B018` classification; water height `0x8001E6BC`.
- A runtime trace of `0x80044CE0` showed why the blocked-wall route does not
  climb: its front probe follows room 13's portal to room 62 and reads a floor
  at 2560, outside the climbable band. The port's portal-following floor query
  reproduces this without special cases.

## Verified rules

- **Surfacing.** Once the previous room update leaves a submerged Lara in a dry
  room within 256 of the surface, she snaps one unit under it into clip114 /
  state33 with speed, pitch and roll cleared; otherwise she falls out (clip34,
  a quarter of the swim speed forward). The surface wrapper then runs that tick.
- **Air.** The surface refills 10 per tick to 1800, before the handler.
- **Handlers.** Tread decelerates 4 quarter-units per tick; strokes add 8 up to
  60 (15 units/tick), even on the tick they ask to tread. Tread and forward
  turn yaw directly by 728/tick, back and sides by 364; there is no turn-rate
  inertia. Up/Down select forward/back, the sidestep inputs select 48/49
  (later write wins). Jump held for exactly 10 tread ticks dives; surfacing
  primes the counter at 11 so a held Jump must be released first.
- **Movement.** Quantized sine table, `(sin(heading) * speed) >> 16` per axis,
  along the heading written by the previous tick's collision (yaw, +180, −90
  or +90 by state).
- **Collision.** Probes 700 below the surface line with an 800 body; a front,
  clamp or top contact restores the old position and stops; a side contact is
  shifted out and turns yaw ±910. The room is then updated 100 below the feet.
- **Dive.** Clip119 / state35, pitch −8190, speed 80; state35 only pitches
  down 182 per tick on Up and keeps its speed until the authored stroke.
- **Climb-out.** Forward stroke front contact with Action, level side probes,
  headroom, a ledge 384..1211 above the probe line and yaw within ±6370 of an
  axis: the body lifts to the ledge, snaps 100 past the edge, faces the axis
  and plays clip111/191/193 (by ledge height) into standing.
- **Entry.** In a water room with water at least 475 deep, feet 384 under the
  surface: clip112 / state35, goal17, pitch −8190, 100 deeper, air 1800, and the
  fall speed becomes 3/2 of a swim speed. The original updates Lara's room from
  381 above her feet, so she stays in the dry room until her body crosses the
  surface; the airborne paths now use that rule.

## Validation

`game/tests/surface.rs` replays eleven complete routes tick by tick on all
columns (position, yaw, pitch, roll, both speeds, clip/frame/state/goal, room,
water mode, air, health): surfacing, forward/turn, back/sidesteps, dive,
blocked wall climb and climb-out, and five independent routes recorded after
the rules were written (angled east climb, right stroke into a wall with the
deflection, steered dive, and two back-hop entries). The climb-out route
continues through the existing ground run/stop rules exactly. A one-unit change
to the stroke acceleration fails four routes.

`tools/surface_smoke.py` replays every route in the full game over the
developer MCP: 121 complete-game poses match, and side-by-side frames with the
PSX camera show the same poses. The existing six submerged routes and the
rest of the workspace tests still pass.

## Limits

- Submerged collision (`0x80045CB8`) was later ported in
  [a separate change](swimming-evidence.md#submerged-collision-0x80045cb8);
  both hop entries now run to their end.
- Wading (water mode 4) and the shallow-water stand-up branch are not ported;
  shallow water is now walked on as ground. Swan-dive entries (states 52/53),
  water currents, splash/sound effects and Look at the surface remain separate
  work. Drowning and the authored surface death were later verified in
  [a separate milestone](drowning-evidence.md).
- The standing back jump (ground) does not match its PSX launch, so it is not
  used as entry evidence.
- The PSX shows the air bar at the surface; the port's HUD shows health.
