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
| Turn-rate rules per state | handlers via table `0x80082c98`, decay at `0x80038194` ([locomotion-evidence](locomotion-evidence.md)) | `original_turn_run_grab_climb_and_wall_stop_match_every_tick` | PSX, verified for states 0,1,2,3,6,16,20 |
| Vault, two-click climb, wall stop | `0x8003d924`, `0x8003c91c`, `0x8001b018`, `.psx/climb_probe` ([locomotion-evidence](locomotion-evidence.md)) | same test | PSX; 3-click and jump-up branches unverified |
| Wall entry fall and slides | `wall_entry.csv`, 207 fields neutral input ([original-evidence](original-evidence.md)) | `original_wall_entry_falls_slides_and_turns_across_every_recorded_tick` | PSX, verified; backward and diagonal slides unexercised |
| Fixed camera shots, timers, easing | [camera-evidence](camera-evidence.md) | `game/tests/camera.rs` | PSX, verified |
| Chase camera | ideal eye, smart shift, LOS, eye move ([chase-camera-evidence](chase-camera-evidence.md)) | `chase_camera_matches_original_through_turn_climb_and_wall_stop` | PSX, verified on the recorded path; L1 look provisional |
| Room and Lara lighting | [lighting-evidence](lighting-evidence.md) | screenshots via `tools/port_smoke.py` | PSX, visual; depth darkening not ported |
| Braid | HairControl ([braid-evidence](braid-evidence.md)) | screenshots via `tools/climb_smoke.py` | PSX, visual only |
| Pickups, switches, doors, keys, triggers | trigger chains (Structural); WALL camera operand | `game/tests/interactions.rs` | Partly Inherited (creature setup, type ids); see [format-notes](format-notes.md) |
| Creatures (tiger, spider) | health/radius: setup code `0x800538A0` and the Wall table dump ([object-table-evidence](object-table-evidence.md)); damage and tiger rules still from OpenLara/TR2Main | `game/tests/interactions.rs` (combat counters only) | health and radius for 36/37/39 PSX, matching the JSON; damage and AI Inherited |
| Object type table | 265 x 48 bytes at `0x800936D0`, reset `0x8005717C`, setup `0x800538A0`/`0x80055774`/`0x80055FE4`, InitialiseItem `0x80037690`; read watch `.psx/object_table_reads` ([object-table-evidence](object-table-evidence.md)) | not checked by a test (docs only) | PSX; +0x28 and flag bits 17-23 not named |
| Object kinds (switch, door, pickup, keyhole, spikes) by id | shared control/collision pointers in the setup routines ([object-table-evidence](object-table-evidence.md)) | `object_types.json` grouping | PSX for 59, 92-93, 103-113, 135-152, 174-177, 182-185, 190-196, 201-206 |
| Main loop and tick order | game loop `0x8006DD98`, ControlPhase `0x8001D7FC` (items → effects → Lara → hair → camera → sound/flip), Lara control internals (animation → object collision → state collision → triggers); trace `.psx/main_loop_trace` ([object-table-evidence](object-table-evidence.md)) | none; `engine::schedule` differs (see engine-model, Frame) | PSX; port order not yet changed |
