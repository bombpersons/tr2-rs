# Recordings

The playthroughs people played in `psx handoff` windows, kept so the tests
can always be recorded again (`tools/recordings.py`).

`handoffs/NAME/` holds what the handoff wrote: `input.json` (every pad
change from the start, as a `psx run --replay`), `request.json`,
`note.txt` and `report.json`. `start.json` names the state it starts from
and the chain of runs that made that state from power-on (frames, input,
memory card, and each state's sha256), so it can be rebuilt with only the
disc and the BIOS:

```bash
python3 tools/recordings.py rebuild wall_level
```

The start states themselves are in `states/`, a submodule pointing at the
private repository `bombpersons/tr2-rs-states`: a state is the console's
whole memory, the game's own code and data, and this repository is public.
Without access, `git submodule update --init` fails for that path only, and
`rebuild` still makes each state. `tools/recordings.py save` copies each
handoff's input here and its start state to `states/`; run it after every
new handoff, then commit and push in `states/` first.

| Handoff | Fields | Starts | What was played |
|---|---|---|---|
| `wall_level` | 72028 | the Great Wall's start, New Game, a formatted card (`.psx/card_wall`) | the whole Great Wall, to Venice |
| `wall_traps` | 27183 | the Great Wall's start (`.psx/use_wall_game`) | traps set off on purpose, the crows, the zip-line |
| `wall_trex` | 16738 | the cave before the zip-line (`.psx/wall_before_zipline`) | down to the valley, a T-Rex shot dead, the other kills Lara |
| `venice_enemies` | 29264 | the Great Wall's start, then the level-skip cheat | Venice's dogs, gunmen, club man and rats on foot |
| `venice_uw_switch` | 54998 | the Great Wall's start | through the Wall (level-skip cheat) to Venice's underwater wall switch |
| `opera_puzzle_hole` | 116759 | after `venice_uw_switch` | on through Bartoli's Hideout to the Opera House's puzzle hole |
| `rig_timed_switch` | 16831 | after `opera_puzzle_hole` | on into the Offshore Rig to its timed wall button |
| `home_lap` | 12126 | Lara's Home, in the house (`.psx/contact_home`) | a lap of the assault course |
| `home_lap2` | 20625 | the same | a second lap, held out |
| `home_ladders` | 13041 | the same | ladder edges, wall brushes, a fatal swan dive |
| `home_ladders2` | 131 | the same | (abandoned at once) |
| `home_walls` | 12919 | the same | lattice-wall jump-ups, walks along walls |
