# Free camera: mouse look and right stick

A port extra, not in the original. The PS1 game has no player-driven camera
beyond the L1 look view. Every option is **off by default**, and the chase
camera then runs exactly as ported (docs/chase-camera-evidence.md). Nothing
here is PSX evidence: the feel (speeds, delays, easing) is a port choice.

## Using it

Open the controls menu (**F1**, or **Start+Select** on a pad) and press
**Tab**, **L1** or **R1** (or click **CAMERA** in the title row) to reach the
CAMERA page:

| Row | Choices | Default |
|---|---|---|
| Mouse look | OFF / ON | OFF |
| Mouse sens | 1..10 | 5 (about 0.055 degrees per mouse count) |
| R stick | OFF / ON | OFF |
| Stick sens | 1..10 | 5 (about 130 degrees a second at full stick) |
| Invert Y | OFF / ON | OFF (mouse or stick up looks up) |
| Recentre | OFF / ON | ON |

Up/down select, left/right or Enter change, Del/West restore one row's
default. As with bindings, nothing applies until **Save and close**, and
**Restore defaults** resets both pages' drafts. The choices are saved to
`camera.json` beside `controls.json` (`$XDG_CONFIG_HOME/tomb-raider-ii/`,
`~/.config/tomb-raider-ii/` or `%APPDATA%\tomb-raider-ii\`).

While mouse look is on and a level is playing, the cursor is hidden and held
in the window. Any menu that takes input focus, or the window losing focus,
gives it back.

## How it fits the original camera

- The orbit is a yaw/pitch offset in binary angle units (`CameraOrbit`).
  The chase camera (`follow` in `engine/src/camera/mod.rs`) adds it on top of
  the facing plus `Avatar::camera_angle`, and of `Avatar::camera_elevation`.
  States that ask for a view (the slide, wading, switch and pickup views, the
  death view) keep it, and the player's orbit adds to it. The ideal eye still
  goes through the original's smart shift, line of sight and floor/ceiling
  clamp, so an orbiting eye slides along walls rather than through them. The
  original's ±15470 elevation limit applies to the total. The orbit's own
  pitch is limited to ±12288 (67.5 degrees).
- Fixed trigger shots, the L1 look view and the combat aim camera ignore the
  orbit. While one of them runs, camera input is ignored and the offset eases
  back to zero, so the chase view returns behind Lara. `CameraState::mode`
  says which view was used.
- **Recentre.** After 45 ticks (1.5 s) without camera input, the offset closes
  1/8 per tick, but only on ticks when Lara moves or turns. You can stand still
  and look around, and once she runs the camera swings back behind her.
- **Responsiveness.** On ticks with camera input, the eye closes 1/3 of its gap
  per tick instead of the original 1/10, so the view keeps up with the mouse.
  A divisor requested by Lara's state (`Avatar::camera_speed`) still wins.
- Input comes from the per-tick `ControllerState` (`mouse.motion`,
  `right_stick`). So it is deterministic, replays and works through
  injected input: `InjectedMouse::motion`, or the developer MCP's `mouse`
  field.

## Extending it

- `FreeCameraSettings` (the saved choices) and `FreeCameraTuning` (speeds,
  pitch limit, delay, easing and eye divisors) are reflected resources. A mod
  can replace either one, or write `CameraOrbit` itself (for a scripted orbit)
  and skip the steering system.
- The rules are added by `CameraRulesPlugin` and are renderer-free. The menu
  rows, `camera.json` and the cursor grab belong to `FreeCameraMenuPlugin`,
  which the executable adds (`game/src/main.rs`). `GamePlugins` and the tests
  never read a player's settings file.
- The CAMERA page is the controls menu's generic options page
  (`engine::controls::ControlsOptions`, a small psx-bevy addition listed in
  docs/template.md). Other settings can add rows there too, up to 14.

## Verification

- `engine/src/camera/free.rs` unit tests: device scaling, invert Y, yaw wrap,
  pitch clamp, the recentre delay and easing, the other modes unwinding the
  orbit, and the menu rows round-tripping the settings.
- `game/tests/camera.rs`, on the Wall from the original's settled chase pose:
  - with the options off, mouse motion leaves the eye identical to no input;
  - mouse look swings the eye about 70 degrees around Lara (she stands in
    a corner, which limits the swing) and holds it there while she stands;
  - L1 unwinds the orbit;
  - turning on the spot eases it back after the delay.
- `psx-bevy/src/controls/tests.rs`: the options page drafts, saves, restores
  and discards, and there is no second page without options.
- All original camera trajectory tests pass unchanged with the options off.
