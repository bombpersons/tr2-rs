# psx end-to-end test: Tomb Raider II

## Progress

1. Identified the source as Tomb Raider II (USA, Rev 3), NTSC-U, volume TOMB_RAIDER_2, 61 tracks (1 data and 60 audio).
2. Created the standalone Bevy starter in `game/` with `template_create`. Selected NTSC and provisionally 2 fields per logic tick; cadence is not yet measured.
3. Installed the pinned, hash-verified NTSC-U BIOS in the project `bios/` directory for reference runs.
4. Extracted all 83 disc files into `research/disc/`; extraction reported no failures. Custom `.PSX` level decoding and original gameplay recording remain outstanding.
5. Paused port work to move the MCP registration to this project at the user's request.

## Pain points and bugs

### MCP project registration points at another game

Observed: global Codex `mcp_servers.psx` launches the server with `--root .../external_agent_tests/kings_field`, although this session works in `tomb_raider`. Tool descriptions therefore advertise the wrong default directory.

Workaround used: explicitly pass `project_root` on each tool call. Calls with the explicit root worked, including disc inspection, extraction, and template creation.

Requested correction: remove the global registration and install a project-local `.codex/config.toml` containing the server command, this folder's `--root`, and this folder as `cwd`. Prepared configuration: `psx-project.config.toml`.

Initial session limitation: `codex mcp remove psx` failed with `Read-only file system (os error 30)` while trying to persist `~/.codex/config.toml`. The project `.codex` directory was also explicitly mounted read-only. The user applied the prepared configuration outside the sandbox. On reconnection, verified that global `mcp_servers.psx` is absent, local `.codex/config.toml` sets this project's root/cwd, and `context({})` returns this project without any override. Registration correction validated.

### Initial context requires BIOS even for orientation

Observed: the first `context` call with the disc and `save:true` returned `bios_required`, preventing the normal context overview. The error did include a useful `next_action` for the appropriate region. `disc`, `ls`, and `template_create` worked independently. BIOS installation succeeded in a caller-selected writable directory.

### Custom assets need an external decoder

The disc contains game-specific `.PSX` level files. The tool guide explicitly limits built-in asset discovery to supported formats and recommends external parsers for custom containers. This is a format coverage limitation, not evidence of a scanner bug. No claim of recovered or validated world geometry yet.

## Validation remaining

- Done: confirmed the reconnected server defaults to this project without `project_root` overrides.
- Boot and inspect original title/gameplay; record checkpoints and trajectories.
- Decode and validate all levels, textures, objects, animations, collision, and audio into standalone assets.
- Implement and test the Bevy game against original behavior and rendered reference captures.
- Verify the port builds and runs without disc images or BIOS. The starter alone is not a completed port.
