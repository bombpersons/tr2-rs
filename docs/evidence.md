# Evidence

What is verified, against what evidence, and what is still provisional. One
row per ported feature; the linked notes hold addresses, `psx` runs and
reproduction steps. Provenance labels (PSX / Structural / Inherited) are
defined in [engine-model.md](engine-model.md).

| Feature | Evidence | Checked by | Status |
|---|---|---|---|
| Region, tick rate | yaw writer `0x800383e4` every 2nd field, `.psx/home_right_watch` ([original-evidence](original-evidence.md)) | `game::FIELDS_PER_TICK`, all trajectory tests | PSX, verified |
| View projection, 368x240 crop | `0x80065cb8` H=228, centre `0x80065c48`/`0x80065c90` ([original-evidence](original-evidence.md), [view-evidence](view-evidence.md)) | `game/tests/view.rs` (64 landmarks within 1 px) | PSX, verified; near/far/fog provisional |
| Level geometry, sectors, floor data, models, animations | complete structural decode of `DATA/*.PSX` ([format-notes](format-notes.md)) | `game/tests/level_assets.rs`, `tools/import` tests | Structural; field names partly Inherited |
| Run, walk, standing turn, jump | `game/tests/data/original/{run,turn,jump}.csv` from Home ([original-evidence](original-evidence.md)) | `game/tests/player.rs` `original_*` | PSX, verified per tick |
| Held-out left turn, walk | recorded after fitting: `heldout_{left,walk}.csv` | `heldout_*` in `game/tests/player.rs` | PSX, verified, not used for fitting |
| Backward hop, walk, turn and authored stops | state handlers `0x8003899C`, `0x80038D00`, `0x80039270`, AnimateLara `0x80042E08`; four no-poke recordings ([backward-evidence](backward-evidence.md)) | `game/tests/backward.rs` (270 ticks, two held-out routes), `tools/backward_smoke.py` (13 poses) | PSX; timing/state/speed/yaw exact, turning-hop x bounded to one original unit; collision branches unverified |
| Turn-rate rules per state | handlers via table `0x80082c98`, decay at `0x80038194` ([locomotion-evidence](locomotion-evidence.md)) | `original_turn_run_grab_climb_and_wall_stop_match_every_tick` | PSX, verified for states 0,1,2,3,6,16,20 |
| Vault, two-click climb, wall stop | `0x8003d924`, `0x8003c91c`, `0x8001b018`, `.psx/climb_probe` ([locomotion-evidence](locomotion-evidence.md)) | same test | PSX; 3-click and jump-up branches unverified |
| Slope height after blocked run start, release and retry | centre floor sampled before collision shift; `0x8003C34C`, `0x8001B0B8`, `0x8003A944`, `0x8003AA04`; fresh no-poke hold/release/retry capture ([locomotion-evidence](locomotion-evidence.md)) | full climb recording and `heldout_wall_stop_release_and_retry_match_every_tick` | PSX, verified on the Wall slope; other collision branches unverified |
| Wall entry fall and slides | `wall_entry.csv`, 207 fields neutral input ([original-evidence](original-evidence.md)) | `original_wall_entry_falls_slides_and_turns_across_every_recorded_tick` | PSX, verified; backward and diagonal slides unexercised |
| Fixed camera shots, timers, easing | [camera-evidence](camera-evidence.md) | `game/tests/camera.rs` | PSX, verified |
| Chase camera | ideal eye, smart shift, LOS, eye move ([chase-camera-evidence](chase-camera-evidence.md)) | `chase_camera_matches_original_through_turn_climb_and_wall_stop` | PSX, verified on the recorded path; L1 look provisional |
| Room and Lara lighting | fresh Home/Wall CalculateLight and normal-lighting traces ([lighting-evidence](lighting-evidence.md)) | `game/tests/lighting.rs` (light parameters and 42 vertex shades), template lighting-ownership test; screenshots via `tools/port_smoke.py` | PSX; per-object lighting preserved, directional scale corrected; depth darkening/dynamic lights remain unported |
| Room polygon subdivision near the camera | room renderer `0x800617b0` splits by max SZ (<5000 2x2, <2500 4x4, projection error 8x8) and adds a crack filler ([subdivision-evidence](subdivision-evidence.md)) | `tests/subdivision.rs`, fourteen recorded GPU leaf packets; `tools/subdivision_smoke.py` | Ported for rooms; float projection and depth-biased filler approximate GTE/OT behavior; distant texture LOD remains unported |
| Braid | HairControl ([braid-evidence](braid-evidence.md)) | screenshots via `tools/climb_smoke.py` | PSX, visual only |
| Pickups, switches, doors, keys, triggers | trigger chains (Structural); WALL camera operand | `game/tests/interactions.rs` | Partly Inherited (creature setup, type ids); see [format-notes](format-notes.md) |
| Creatures (tiger, spider) | health/radius: setup code `0x800538A0` and the Wall table dump ([object-table-evidence](object-table-evidence.md)); damage and tiger rules still from OpenLara/TR2Main | `game/tests/interactions.rs` (combat counters only) | health and radius for 36/37/39 PSX, matching the JSON; damage and AI Inherited |
| Door animation goals, dispatch, links and hold poses | AnimateItem `0x8001DD98`, GetChange `0x8001E250`, DoorControl `0x8004C954`; baseline/cycle and post-implementation reversal captures ([door-animation-evidence](door-animation-evidence.md)) | `game/tests/doors.rs` (306 exact ticks), sandbox and editor pose checks | PSX for Wall type 107; collision/occupancy override and other types unverified |
| Object type table | 265 x 48 bytes at `0x800936D0`, reset `0x8005717C`, setup `0x800538A0`/`0x80055774`/`0x80055FE4`, InitialiseItem `0x80037690`; read watch `.psx/object_table_reads` ([object-table-evidence](object-table-evidence.md)) | not checked by a test (docs only) | PSX; +0x28 and flag bits 17-23 not named |
| Object kinds (switch, door, pickup, keyhole, spikes) by id | shared control/collision pointers in the setup routines ([object-table-evidence](object-table-evidence.md)) | `object_types.json` grouping | PSX for 59, 92-93, 103-113, 135-152, 174-177, 182-185, 190-196, 201-206 |
| Main loop and tick order | ControlPhase `0x8001D7FC`, Lara calls `0x800383E0..0x80038458`; fresh `.psx/tick_order_port` and post-change held-out turn `.psx/tick_order_heldout` ([tick-order-evidence](tick-order-evidence.md)) | `game/tests/tick_order.rs`, four interaction timing tests; all existing trajectories | PSX phase boundaries ported; active lists, state-collision boundary and per-type animation timing remain incomplete |

Editor authoring support is a new tool, not a claim about original gameplay.
It consumes the existing engine format and retains its Structural/Inherited
provenance. The full-resolution standard-rendering preview is deliberately an
authoring view. See [editor.md](editor.md) for format coverage, validation,
byte-exact save tests and the GPU/MCP smoke workflow.
