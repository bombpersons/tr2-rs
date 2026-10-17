# Chase camera: PSX-only investigation

Evidence: supplied disc, psx native core, no pokes. Capture
`game/tests/data/original/climb_camera.csv` samples eye `0x8008b258` and target
`0x8008b268` every field along the climb_two_click route
(`.psx/climb_camera`); `.psx/climb_camflags` shows mode `0x8008b278` and flags
`0x8008b280` stay 0 (pure chase), rooms stay 35. Write watch
`.psx/chase_watch` gives the eye writer (0x80017ee4..f7c); traces
`.psx/chase_trace`, `.psx/los_trace2` record the smart shift and line of sight
on the first tick.

## Camera globals (0x8008b258)

eye xyz, eye room `B264`; target xyz `B268`, room `B274`; mode `B278`; render
height offset `B27C`; flags `B280`; target distance `B294` (reset 1536 each
tick at 0x8001a23c); horizontal distance squared `B298`; angle/elevation
offsets `B29C`/`B2A0` (reset 0); divisor `B2AA` (reset 10).

## Pipeline

1. `0x80019bf8`: sets camera geometry flag `0x80087a80`; target x/z = Lara,
   y approaches bounds height by a quarter (existing evidence); target room by
   GetFloor; flag cleared if the target is below its floor.
2. `0x80019214` ideal eye: elevation = `B2A0` + Lara pitch (±15470 clamp);
   h = 1536*cos>>14; ideal = target - h*(sin,cos)(yaw+`B29C`)>>14, y raised
   by 1536*sin(elevation)>>14; ideal room = current eye room.
3. `0x80018800` smart shift: LOS target->ideal (clips ideal); target's
   navigation box (ideal's own box when the ideal lies outside it); each side
   extended through open neighbours (`0x80018768`) or fixed at the solid
   boundary; shrunk 256. If the ideal crosses a solid-backed edge (dominant
   axis first), callback `0x80018594` builds two candidates on the edge at
   distance h (sqrt branches, 341^2 corner rule); preference by eye side, then
   LOS decides.
4. `0x80017e80` eye move: clears the flag; eye += (ideal-eye)/divisor
   (truncating, x/z/y); re-clamps by LOS if eye and ideal are both below
   floor-256; render offset `B27C` keeps the view 256 inside floor/ceiling
   (midpoint if they cross); the stored eye is unchanged.
5. LOS `0x8001fb54`: dominant-axis order of sweeps `0x80020078` (x) and
   `0x8001fc70` (z) across sector boundaries, testing floor/ceiling on both
   sides; ClipTarget `0x80020480` pulls the end to a crossed floor/ceiling.
6. Trig `0x80063f74`: quarter table at RAM `0x80083ff8`, equal to
   round(sin(i*pi/2048)*16384) for all 1025 entries (generated, not copied).

## Camera geometry flag

With `0x80087a80` set, GetHeight (0x8001ea8c) skips a floor slope when either
tilt is 3 or more, and GetCeiling (0x8001f850) a ceiling slope beyond 2. The
first-tick trace shows the Wall ramp (tilt z -3) read as flat 4608 at
z25599, clipping the ideal at z25600; the shift then lands on (63743,25855),
within one divisor step of the settled eye, which is why the original eye
stays still for 55 ticks while Lara turns. The same code confirms the port's
per-sign slope rounding.

## Bounds for the look target

GetFrames `0x80028824` interpolates toward the next stored pose; clips store a
closing key after their last interval (clip50 has 15 poses, the extractor's
`poses` list 14). Its last-interval rate test compares a clip-relative frame
with the absolute end frame, so it only fires for clips starting at frame 0.
The port reads bounds from raw frame data accordingly.

## Verification and limits

`chase_camera_matches_original_through_turn_climb_and_wall_stop` matches eye
and target exactly on every tick to field 217. Rendered frames along the route
(`tools/climb_smoke.py`) line up with the original's cave geometry. Not yet
verified: look (L1, still provisional), combat/fixed-target modes, elevation
and angle requests from Lara states, swimming pitch, LOS through room portals
with different heights, and the render offset path (not exercised for long
here). Lara herself renders without her braid and, at field 61, about 30 px
right of the original despite identical eye/target.
