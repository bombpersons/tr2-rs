#!/usr/bin/env python3
"""Retain PSX trap and creature routes from the Great Wall and Venice, cut from played runs.

Each run is a `psx handoff` playthrough of Wall (the person's pad input, no
memory pokes): `traps`, from `.psx/use_wall_game`, in which traps were set
off on purpose (`.psx/handoffs/wall_traps`), and `trex`, from the cave
before the zip-line, down into the T-Rex valley (`.psx/handoffs/wall_trex`),
and `venice`, from the same Wall state through the level-skip cheat into
Venice, its dogs, gunmen, club man and rats fought on foot
(`.psx/handoffs/venice_enemies`); and `home`, a lap of the assault course
at Lara's Home from `.psx/contact_home` (`.psx/handoffs/home_lap`), whose
`lap` case follows Lara alone, and `home2`, a second lap held out from the
port's work on the first (`.psx/handoffs/home_lap2`, case `lap2`), and
`home3`, ladder edges and wall brushes (`.psx/handoffs/home_ladders`,
case `ladders`), and `home4`, jump-ups onto the lattice wall and walks along
walls (`.psx/handoffs/home_walls`, case `walls`). The runs' inputs are
read from their tracked copies in `recordings/handoffs/`, whose start states
`tools/recordings.py rebuild` makes again from power-on.
Replaying a run's input reproduces it exactly; this tool replays it once
with checkpoints every 1000 fields, then records each case from the
checkpoint before it, sampling Lara, the case's items (and the item slots
darts are created in, or the creature AI slots), every field.

Rows (a trajectory CSV) are one per control tick, not per field. The
control loop adds one to the game time (0x800DE670, at 0x8001DD54) at the
end of every tick; the inventory adds to it too (0x80031774) while the
world stands still, and a slow frame runs two ticks in one field. So the
tool watches every store to the sampled memory and replays them in order
from one sample at the start: the memory at each control-loop store of
the game time is that tick's row. At Home the game time is the assault
course's timer, so a Home run can close ticks at the control loop's
countdown store (0x80089428, at 0x8001DD20) instead.

The Wall's cases but `zipline` were recorded after the routines were
written from the disassembly (docs/trap-evidence.md); `zipline` was
recorded first, and `zipline_release` (the same ride with Action let go,
an input the tool edits) after the port, as its held-out case. Venice's
were all recorded before their port (docs/creature-evidence.md#venice).
"""
import argparse
import csv
import hashlib
import io
import json
import subprocess
from typing import NamedTuple

from record_backward_collision import ROOT

DISC = 'rom/Tomb Raider II - Starring Lara Croft (USA) (Rev 3).cue'


class Run(NamedTuple):
    """A played run: where it starts, its pad input, its length in fields,
    and where its checkpoints every 1000 fields are kept; then its level,
    whose item array and creature slots the loader places (pointers at
    0x8008AAAC and 0x8008C1C0), Lara's item, and the names of its outputs."""
    start: str
    input: str
    fields: int
    survey: str
    level: str = 'wall'
    items: int = 0x8019C940
    slots: int = 0x801A29C0
    lara: int = 20
    prefix: str = 'traps'
    validation: str = 'validation/wall_traps'
    # Fields per idle-search chunk: fewer where more creatures are about.
    idle_chunk: int = 25
    # The level's navigation boxes (8 bytes each) and its baked level file:
    # doors set and clear bit 0x4000 of a box's overlap word as they shut
    # and open.
    boxes: int = 0x80194D1C
    level_file: str = 'wall'
    # Close ticks at the control loop's countdown store instead of the game
    # time's: at Home the game time is the assault course's timer, which
    # stands still until Lara leaves its start and after she finishes.
    countdown: bool = False


RUNS = {
    # Traps set off on purpose, and the crows.
    'traps': Run('.psx/use_wall_game/state.bin', 'recordings/handoffs/wall_traps/input.json', 27183,
                 '.psx/wall_traps_survey'),
    # From the cave before the zip-line (field 24240 of the traps run): down
    # into the valley, the far T-Rex shot dead, the hidden one kills Lara.
    'trex': Run('.psx/wall_before_zipline/state.bin', 'recordings/handoffs/wall_trex/input.json', 16738,
                '.psx/wall_trex_survey'),
    # Venice (level file `boat`), from the Wall by the level-skip cheat: dogs,
    # the gunmen, a club man and rats fought on foot and shot dead.
    'venice': Run('.psx/use_wall_game/state.bin', 'recordings/handoffs/venice_enemies/input.json', 29264,
                  '.psx/venice_enemies_survey', 'venice', 0x801C19C4, 0x801C9174, 0, 'venice',
                  'validation/venice_enemies', 10, 0x801BCDC4, 'boat'),
    # Lara's Home (level file `assault`), from the house out to the
    # assault course and one lap of it (`.psx/handoffs/home_lap`).
    'home': Run('.psx/contact_home/state.bin', 'recordings/handoffs/home_lap/input.json', 12126,
                '.psx/home_lap_survey', 'home', 0x8017C87C, 0, 0, 'home', 'validation/home_lap',
                level_file='assault'),
    # A second, held-out lap from the same state (`.psx/handoffs/home_lap2`):
    # down a ladder to its foot, jumps back off and up onto the ladders.
    'home2': Run('.psx/contact_home/state.bin', 'recordings/handoffs/home_lap2/input.json', 20625,
                 '.psx/home_lap2_survey', 'home', 0x8017C87C, 0, 0, 'home', 'validation/home_lap2',
                 level_file='assault', countdown=True),
    # Ladder edges, walks brushing walls at the side and a fatal swan dive onto
    # the ground (`.psx/handoffs/home_ladders`), from the same state.
    'home3': Run('.psx/contact_home/state.bin', 'recordings/handoffs/home_ladders/input.json', 13041,
                 '.psx/home_ladders_survey', 'home', 0x8017C87C, 0, 0, 'home', 'validation/home_ladders',
                 level_file='assault', countdown=True),
    # Running and walking into the lattice wall with Up+Action (its climbable
    # jump-up), and walks along walls (`.psx/handoffs/home_walls`).
    'home4': Run('.psx/contact_home/state.bin', 'recordings/handoffs/home_walls/input.json', 12919,
                 '.psx/home_walls_survey', 'home', 0x8017C87C, 0x801826F4, 0, 'home', 'validation/home_walls',
                 level_file='assault', countdown=True),
}
GAME_TIME = 0x800DE670
# The store that counts a control tick (ControlPhase 0x8001D7FC).
CONTROL_TICK_PC = 0x8001DD54
# The countdown the control loop takes one from every tick, unconditionally,
# just before the game time's add (which the course timer can skip).
COUNTDOWN = 0x80089428
COUNTDOWN_PC = 0x8001DD20

ITEM = [
    (0x00, 'i32', 'floor'), (0x04, 'x32', 'touch'), (0x0E, 'i16', 'state'), (0x10, 'i16', 'goal'),
    (0x12, 'i16', 'required'), (0x14, 'i16', 'animation'), (0x16, 'i16', 'frame'), (0x18, 'i16', 'room'),
    (0x1E, 'i16', 'speed'), (0x20, 'i16', 'fall_speed'), (0x22, 'i16', 'health'), (0x26, 'i16', 'timer'),
    (0x28, 'u16', 'flags'), (0x34, 'i32', 'x'), (0x38, 'i32', 'y'), (0x3C, 'i32', 'z'),
    (0x40, 'i16', 'pitch'), (0x42, 'i16', 'yaw'), (0x44, 'i16', 'roll'), (0x48, 'x32', 'status'),
]
CREATED = [(0x04, 'x32', 'touch'), (0x0C, 'i16', 'kind'), (0x18, 'i16', 'room'), (0x34, 'i32', 'x'), (0x38, 'i32', 'y'),
           (0x3C, 'i32', 'z'), (0x40, 'i16', 'pitch'), (0x42, 'i16', 'yaw'), (0x48, 'x32', 'status')]
# Creature AI slots (five of 68 bytes) and the fields the tests compare.
# The idle search's round-robin slot cursor (0x800492B4 stores it), and
# the store that steps it past a slot it visited (a wrap stores 5, then 0).
SEARCH_CURSOR = 0x80089628
SEARCH_VISIT_PC = 0x80049350
CREATURE = [
    (0x00, 'i16', 'head'), (0x02, 'i16', 'neck'), (0x04, 'i16', 'max_turn'), (0x06, 'i16', 'flags'), (0x08, 'i16', 'item'),
    (0x0C, 'i32', 'mood'), (0x24, 'i16', 'target_box'), (0x26, 'i16', 'required_box'),
    (0x34, 'i32', 'target_x'), (0x38, 'i32', 'target_y'), (0x3C, 'i32', 'target_z'),
]
# Item slots the run's darts were created in (the level places 106 items).
DART_SLOTS = range(106, 136)

# name: (run, first field, last field, trap or creature items)
CASES = {
    'crows': ('traps', 6300, 7650, [2, 12, 13]),
    'trapdoor': ('traps', 4300, 4700, [14]),
    'darts': ('traps', 17350, 18950, list(range(31, 45))),
    'tiles_entrance': ('traps', 19200, 19500, [46, 47]),
    'boulders': ('traps', 19300, 20100, [73, 74]),
    'sliding_blades': ('traps', 19900, 20700, [61, 62]),
    'wall_blades': ('traps', 20550, 21600, [49, 50, 55, 64, 65, 66, 67, 51, 56, 57, 53]),
    'tiles_and_blades': ('traps', 21600, 22250, [82, 83, 84, 75, 76, 77, 78, 79, 80, 81]),
    'spindles': ('traps', 22150, 24000, [90, 91, 88]),
    # Lara walks up to the zip-line in the cave, grabs it and rides it down.
    'zipline': ('traps', 24400, 25300, [93]),
    # The same ride, with Action let go part of the way down (below).
    'zipline_release': ('traps', 24400, 25450, [93]),
    # The far T-Rex wakes, chases Lara and is shot dead.
    'trex_shot': ('trex', 8900, 11400, [104]),
    # The hidden T-Rex appears, bites Lara and kills her.
    'trex_kill': ('trex', 12100, 16600, [99]),
    # Venice: the first two dogs, by the start; the second wakes while a
    # gunman (below) is about.
    'dogs': ('venice', 3400, 6300, [1, 11]),
    # The first gunman, shooting at Lara until shot dead.
    'gunman': ('venice', 3900, 6300, [3]),
    # A club man, who hits Lara once.
    'club_man': ('venice', 6900, 7700, [10]),
    # A dog that bites Lara twice and is shot dead.
    'dog_shot': ('venice', 15500, 16200, [32]),
    # A gunman fought at length (Lara takes a medipack meanwhile).
    'gunman_long': ('venice', 22000, 24500, [6]),
    # Rats, two left alive, and a third that bites Lara and is shot.
    'rats': ('venice', 26350, 28000, [36, 37, 45]),
    # A gunman shot dead among the rats.
    'gunman_rats': ('venice', 27200, 27650, [49]),
    # Home: Lara alone, the whole run, for her movement through the course.
    'lap': ('home', 0, 12125, []),
    # The held-out lap, played after the port matched the first.
    'lap2': ('home2', 0, 20624, []),
    'ladders': ('home3', 0, 13040, []),
    # Winston (the recorder's item 1 at Home: the level's item 9, as Home's
    # item base here is Lara's, item 8) follows her through the house.
    'walls': ('home4', 0, 12918, [1]),
}
# Cases whose items are creatures: their AI slots and the idle search too.
CREATURE_CASES = {'crows', 'trex_shot', 'trex_kill', 'dogs', 'gunman', 'club_man', 'dog_shot', 'gunman_long', 'rats',
                  'gunman_rats'}
# Cases that note the creature AI slots and the idle search, but not the
# boxes: Winston, who never leaves his slot, in a house whose doors the
# port runs itself.
SLOT_CASES = {'walls'}
# Cases that also note the control random seed as the control loop
# dispatches the type's control: creatures not recorded draw from it
# earlier in the same tick. The loop reads the type's control pointer
# (object table +0x10) just before calling it.
CONTROL_SEED_KINDS = {'trex_shot': 214, 'trex_kill': 214, 'dogs': 15, 'gunman': 17, 'club_man': 32, 'dog_shot': 15,
                      'gunman_long': 18, 'rats': 21, 'gunman_rats': 18}
OBJECTS = 0x800936D0
SEED = 0x80089D54
# Cases that note Lara's gun status and flags without being creature cases,
# and the input word (0x8008AAD4) the tick's control read: the zip-line's
# grab and Lara's ride both test Action (0x40).
LARA_GLOBAL_CASES = {'zipline', 'zipline_release', 'lap', 'lap2', 'ladders', 'walls'}
# Cases whose input is the run's with Action (CROSS) let go from this
# field on: no longer a person's play, but still no pokes.
ACTION_RELEASED = {'zipline_release': 25200}
# Cases recorded before the port's rules for them were written (from the
# disassembly, but not yet ported).
RECORDED_BEFORE_IMPLEMENTATION = {'lap', 'lap2', 'ladders', 'walls', 'zipline', 'dogs', 'gunman', 'club_man', 'dog_shot', 'gunman_long', 'rats',
                                  'gunman_rats'}
INPUT = '8008aad4:x32:input'
# Lara's fields the creatures read, noted again as CreatureAIInfo first
# reads her x after the type's control began (`seen_lara_*`): her pose at
# the end of the tick before, unless an item that ran first, such as a boat
# she drives, moved her.
SEEN = {'x', 'y', 'z', 'yaw', 'room', 'speed', 'health'}
AI_READ_PC = 0x80014EB0
# Lara's gun status and her flags word (bit 0x40000: no object collision).
LARA_GLOBALS = ['8008c4ea:i16:lara_gun_status', '8008c510:x32:lara_flags']
# The rest of Lara's own state, for cases that follow her movement: her
# water mode and air, turn rate and movement heading.
LARA_MOVEMENT = {'lap', 'lap2', 'ladders', 'walls'}
LARA_MOVEMENT_GLOBALS = ['8008c4f4:i16:lara_water_mode', '8008c4fe:i16:lara_air', '8008c570:i16:lara_turn_rate',
                         '8008c572:i16:lara_heading']


def item(run, index, prefix, fields):
    base = run.items + index * 76
    return [f'{base + offset:08x}:{fmt}:{prefix}_{name}' for offset, fmt, name in fields]


def samples(run, items, darts, creatures, lara_globals, movement=False):
    columns = [f'{GAME_TIME:08x}:u32:game_time', '80089d54:x32:seed'] + item(run, run.lara, 'lara', ITEM)
    if run.countdown:
        columns.insert(1, f'{COUNTDOWN:08x}:i32:countdown')
    if movement:
        columns += LARA_MOVEMENT_GLOBALS
    for index in items:
        columns += item(run, index, f'i{index}', ITEM)
    if creatures or lara_globals:
        columns += LARA_GLOBALS
    if lara_globals:
        columns.append(INPUT)
    if creatures:
        for slot in range(5):
            base = run.slots + slot * 68
            columns += [f'{base + offset:08x}:{fmt}:s{slot}_{name}' for offset, fmt, name in CREATURE]
    if darts:
        for index in DART_SLOTS:
            columns += item(run, index, f'd{index}', CREATED)
    return columns


def shifted_input(run, start, fields, release=None):
    """The run's pad input from field `start`, for `fields` fields; with
    `release`, Action is let go from that field on."""
    events = json.loads((ROOT / run.input).read_text())
    if release is not None:
        held = max((e for e in events if e['frame'] <= release), key=lambda e: e['frame'])
        events = [e for e in events if e['frame'] != release] + [dict(held, frame=release)]
        events = sorted(events, key=lambda e: e['frame'])
        events = [dict(e, buttons=[b for b in e['buttons'] if b != 'CROSS']) if e['frame'] >= release else e
                  for e in events]
    held = max((e for e in events if e['frame'] <= start), key=lambda e: e['frame'])
    later = [dict(e, frame=e['frame'] - start) for e in events if start < e['frame'] < start + fields]
    return [dict(held, frame=0)] + later


def survey(run):
    """Replay the whole run once, keeping a checkpoint every 1000 fields."""
    command = ['psx', 'run', DISC, '--from-state', run.start, '--replay', run.input, '--frames', str(run.fields),
               '--timeout', '3000', '--save-state', '--out', run.survey, '--summary']
    for field in range(1000, run.fields, 1000):
        command += ['--save-state-at', str(field)]
    subprocess.run(capped(command), cwd=ROOT, check=True, stdout=subprocess.DEVNULL)


def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


SIZES = {'i16': 2, 'u16': 2, 'x16': 2, 'i32': 4, 'u32': 4, 'x32': 4}


def parse_column(column):
    address, fmt, name = column.split(':')
    return int(address, 16) & 0x1FFFFF, fmt, name


def encode(value, fmt):
    number = int(value, 16) if value.startswith('0x') else int(value)
    return (number & ((1 << (8 * SIZES[fmt])) - 1)).to_bytes(SIZES[fmt], 'little')


def decode(memory, address, fmt):
    raw = int.from_bytes(bytes(memory[address + i] for i in range(SIZES[fmt])), 'little')
    if fmt[0] == 'x':
        return f'0x{raw:0{2 * SIZES[fmt]}X}'
    if fmt[0] == 'i' and raw >= 1 << (8 * SIZES[fmt] - 1):
        raw -= 1 << (8 * SIZES[fmt])
    return str(raw)


def watches(columns):
    """Watch ranges covering every sampled column, merged where adjacent."""
    spans = sorted((address, address + SIZES[fmt]) for address, fmt, _ in map(parse_column, columns))
    merged = []
    for lo, hi in spans:
        if merged and lo <= merged[-1][1] + 8:
            merged[-1][1] = max(merged[-1][1], hi)
        else:
            merged.append([lo, hi])
    return [f'{0x80000000 | lo:08x}:{hi - lo}' for lo, hi in merged]


# The idle search stores its cursor some 5000 times a field, too many to
# keep in one run (the emulator holds every event): those are counted in
# short chunks (`Run.idle_chunk` fields), each run under a memory cap and
# streamed from disk.
MEMORY_CAP = '1G'


def capped(command, cap=MEMORY_CAP):
    """Run a psx command in a scope that cannot take the machine's memory."""
    return ['systemd-run', '--user', '--scope', '-q', '-p', f'MemoryMax={cap}', '-p', 'MemorySwapMax=0'] + command


def idle_steps(run_name, checkpoint, checkpoint_field, fields):
    """The idle search's slot visits after each control tick from the
    checkpoint, as (field the tick ended in, cursor then, visits)."""
    run = RUNS[run_name]
    chunks = f'{run.validation}/{run_name}_idle_f{checkpoint_field:06d}'
    chunk = run.idle_chunk
    starts = list(range(chunk, fields, chunk))
    replay = ROOT / chunks / 'input.json'
    replay.parent.mkdir(parents=True, exist_ok=True)
    replay.write_text(json.dumps(shifted_input(run, checkpoint_field, fields)))
    states = ROOT / chunks / 'states'
    if not (states / 'report.json').exists():
        command = ['psx', 'run', DISC, '--from-state', checkpoint, '--replay', str(replay.relative_to(ROOT)),
                   '--frames', str(fields), '--timeout', '1200', '--save-state', '--out', f'{chunks}/states', '--summary']
        for field in starts:
            command += ['--save-state-at', str(field)]
        subprocess.run(capped(command), cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    cache = ROOT / chunks / 'ticks.json'
    if cache.exists():
        return [tuple(t) for t in json.loads(cache.read_text())]
    ticks, cursor, count = [], 0, 0
    # The store that closes a tick, as `record` reads them.
    tick_address, tick_pc = (COUNTDOWN, COUNTDOWN_PC) if run.countdown else (GAME_TIME, CONTROL_TICK_PC)
    for start in [0] + starts:
        length = min(chunk, fields - start)
        state = checkpoint if start == 0 else f'{chunks}/states/state_f{start:06d}.bin'
        part = ROOT / chunks / 'part.input.json'
        part.write_text(json.dumps(shifted_input(run, checkpoint_field + start, length)))
        out = f'{chunks}/part'
        command = ['psx', 'run', DISC, '--from-state', state, '--replay', str(part.relative_to(ROOT)),
                   '--frames', str(length), '--timeout', '1200', '--max-events', '2000000', '--out', out, '--summary',
                   '--watch', f'{SEARCH_CURSOR:08x}:4', '--watch', f'{tick_address:08x}:4']
        subprocess.run(capped(command), cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
        assert json.loads((ROOT / out / 'report.json').read_text())['watch']['complete']
        path = ROOT / out / 'writes.jsonl'
        with path.open() as lines:
            for line in lines:
                w = json.loads(line)
                if w['writer'] != 'cpu':
                    continue
                if w['address'] == SEARCH_CURSOR & 0x1FFFFF:
                    if w['pc'] & 0x1FFFFFFF == SEARCH_VISIT_PC & 0x1FFFFFFF:
                        if not ticks:
                            cursor = w['old_signed']
                        count += 1
                elif (w['address'], w['pc'] & 0x1FFFFFFF) == (tick_address & 0x1FFFFF, tick_pc & 0x1FFFFFFF):
                    if ticks:
                        ticks[-1][2] = count
                        cursor = (ticks[-1][1] + count) % 5
                    ticks.append([w['run_frame'] + start + checkpoint_field, cursor, 0])
                    count = 0
        path.unlink()
    if ticks:
        ticks[-1][2] = count
    cache.write_text(json.dumps(ticks))
    return [tuple(t) for t in ticks]


def record(name):
    run_name, first, last_field, items = CASES[name]
    run = RUNS[run_name]
    darts = name == 'darts'
    creatures = name in CREATURE_CASES
    checkpoint_field = first // 1000 * 1000
    checkpoint = run.start if checkpoint_field == 0 else f'{run.survey}/state_f{checkpoint_field:06d}.bin'
    if not (ROOT / checkpoint).exists():
        survey(run)
    fields = last_field - checkpoint_field + 1
    inputs = shifted_input(run, checkpoint_field, fields, ACTION_RELEASED.get(name))
    replay = ROOT / f'{run.validation}/{name}.input.json'
    replay.parent.mkdir(parents=True, exist_ok=True)
    replay.write_text(json.dumps(inputs))
    out = f'{run.validation}/{name}'
    columns = samples(run, items, darts, creatures or name in SLOT_CASES, name in LARA_GLOBAL_CASES,
                      name in LARA_MOVEMENT)
    # One sample at the start gives every value; the watches give every
    # store after it.
    command = ['psx', 'run', DISC, '--from-state', checkpoint, '--replay', str(replay.relative_to(ROOT)),
               '--frames', str(fields), '--timeout', '1200', '--max-events', '20000000',
               '--sample-every', str(fields), '--out', out, '--summary']
    # Creatures steer by the boxes doors leave open: their blocked bits
    # are watched too, and noted where they change.
    box_columns = []
    if creatures:
        count = len(json.loads((ROOT / f'game/assets/imported/levels/{run.level_file}/level.json').read_text())['boxes'])
        box_columns = [f'{run.boxes + k * 8 + 6:08x}:x16:box{k}' for k in range(count)]
    for watch in watches(columns + box_columns):
        command += ['--watch', watch]
    for column in columns + box_columns:
        command += ['--sample', column]
    control = CONTROL_SEED_KINDS.get(name)
    lara_x = run.items + run.lara * 76 + 0x34
    if control is not None:
        command += ['--watch-reads', f'{OBJECTS + control * 48 + 0x10:08x}:4', '--watch-reads', f'{lara_x:08x}:4']
    subprocess.run(capped(command, '3G'), cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    report = json.loads((ROOT / out / 'report.json').read_text())
    assert report['status'] == 'complete' and not report.get('pokes') and report['watch']['complete']
    start = next(csv.DictReader((ROOT / out / 'samples.csv').open()))
    assert start['run_frame'] == '0'
    parsed = [parse_column(c) for c in columns]
    boxes = [parse_column(c) for c in box_columns]
    memory = {}
    for address, fmt, column in parsed + boxes:
        for i, byte in enumerate(encode(start[column], fmt)):
            memory[address + i] = byte
    writes = [json.loads(line) for line in (ROOT / out / 'writes.jsonl').read_text().splitlines()]
    if control is not None:
        reads = [json.loads(line) for line in (ROOT / out / 'reads.jsonl').read_text().splitlines()]
        writes += [dict(r, read=True) for r in reads]
    writes.sort(key=lambda w: w['cycle'])
    # Replay the stores in order; each store of the control loop's tick
    # count closes a tick, and the watched memory then is that tick's row.
    # A read of the control pointer notes the seed as the control starts.
    rows, control_seeds, control_seed = [], [], -1
    tick_address, tick_pc = (COUNTDOWN, COUNTDOWN_PC) if run.countdown else (GAME_TIME, CONTROL_TICK_PC)
    tick_store = (tick_address & 0x1FFFFF, tick_pc & 0x1FFFFFFF)
    # Lara as the type's control starts: an item earlier in the active list
    # (a boat she drives) can move her before the creatures think.
    seen_fields = [(a, f, n) for a, f, n in parsed if n.removeprefix('lara_') in SEEN]
    seen_rows, seen, at_control = [], None, None
    blocked_boxes, blocked = [], None
    for w in writes:
        if w.get('read'):
            if w['address'] == lara_x & 0x1FFFFF:
                # CreatureAIInfo reading her x, after the type's control began.
                if w['pc'] & 0x1FFFFFFF == AI_READ_PC & 0x1FFFFFFF and at_control is not None and seen is None:
                    seen = [decode(memory, a, f) for a, f, _ in seen_fields]
            elif control_seed == -1:
                control_seed = int(decode(memory, SEED & 0x1FFFFF, 'u32'))
                at_control = [decode(memory, a, f) for a, f, _ in seen_fields]
            continue
        if w['writer'] != 'cpu':
            continue
        for i, byte in enumerate(int(w['value_hex'], 16).to_bytes(w['width'], 'little')):
            if w['address'] + i in memory:
                memory[w['address'] + i] = byte
        field = w['run_frame'] + checkpoint_field
        if (w['address'], w['pc'] & 0x1FFFFFFF) == tick_store:
            if field >= first:
                rows.append([str(field)] + [decode(memory, a, f) for a, f, _ in parsed])
                control_seeds.append(control_seed)
                seen_rows.append(seen or at_control or [decode(memory, a, f) for a, f, _ in seen_fields])
                now = [k for k, (a, f, _) in enumerate(boxes) if int(decode(memory, a, f), 16) & 0x4000]
                if box_columns and now != blocked:
                    blocked_boxes.append([len(rows) - 1, now])
                    blocked = now
            control_seed = -1
            seen, at_control = None, None
    if creatures or name in SLOT_CASES:
        ticks = [t for t in idle_steps(run_name, checkpoint, checkpoint_field, fields) if t[0] >= first]
        assert [t[0] for t in ticks] == [int(r[0]) for r in rows], 'idle pass ticks differ'
    # Trajectory CSV: one row per control tick; `field` is the run field
    # it ended in, `idle_steps` the idle search's slot visits after it,
    # `control_seed` the seed as the type's control started (-1: it did not
    # run).
    target = ROOT / f'game/tests/data/original/{run.prefix}_{name}.csv'
    out_csv = io.StringIO()
    idle = creatures or name in SLOT_CASES
    header = ['tick', 'field'] + (['search_cursor', 'idle_steps'] if idle else [])
    header += (['control_seed'] if control is not None else []) + [n for _, _, n in parsed]
    header += [f'seen_{n}' for _, _, n in seen_fields] if control is not None else []
    out_csv.write(','.join(header) + '\n')
    for tick, row in enumerate(rows):
        extra = [str(ticks[tick][1]), str(ticks[tick][2])] if idle else []
        extra += [str(control_seeds[tick])] if control is not None else []
        seen_columns = seen_rows[tick] if control is not None else []
        out_csv.write(','.join([str(tick), row[0]] + extra + row[1:] + seen_columns) + '\n')
    target.write_text(out_csv.getvalue())
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; '
                  + ('a person played, Action then let go by the tool, no pokes' if name in ACTION_RELEASED
                     else 'a person played, no pokes'),
        'scope': f'{run.level.capitalize()} items {items} and Lara, one row per control tick, rebuilt from watched stores',
        'recorded_after_implementation': name not in RECORDED_BEFORE_IMPLEMENTATION,
        'action_released_from_field': ACTION_RELEASED.get(name),
        'level': run.level, 'run_input': run.input, 'run_input_sha256': digest(run.input),
        'run_start': run.start, 'run_start_sha256': digest(run.start),
        'checkpoint': checkpoint, 'checkpoint_field': checkpoint_field, 'checkpoint_sha256': digest(checkpoint),
        'first_field': first, 'last_field': last_field, 'command': command, 'samples': columns,
        'build': report['build'], 'identity': report['identity'],
        'samples_sha256': digest(target.relative_to(ROOT)),
    }
    if box_columns:
        # [tick, every box blocked after it], from the first tick and then
        # each tick the set changed.
        metadata['blocked_boxes'] = blocked_boxes
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT), len(rows), 'ticks')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--survey', choices=RUNS, action='append', default=[], help='rebuild a run\'s checkpoints')
    parser.add_argument('--case', choices=CASES, action='append', default=[])
    parser.add_argument('--all', action='store_true')
    args = parser.parse_args()
    for name in args.survey:
        survey(RUNS[name])
    for name in CASES if args.all else args.case:
        record(name)


if __name__ == '__main__':
    main()
