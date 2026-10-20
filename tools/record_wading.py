#!/usr/bin/env python3
"""Supplied-disc shallow-water evidence. Only fixture placement uses pokes.

Rebuild Home with --boot if older checkpoints cannot load in the current core.
The original enters/stands up in water itself; never poke mode or animation.
"""
import argparse
import hashlib
import json
import struct
import subprocess
from record_swimming import ROOT, SAMPLES as WATER_SAMPLES
SAMPLES = WATER_SAMPLES + ["8008b264:i16:camera_room", "8008b2a0:i16:camera_elevation"]

HOME = 'validation/wading/home/state.bin'
START = 'validation/wading/start/state.bin'


def event(buttons, until, frame=0):
    return dict(frame=frame, buttons=buttons, until=until)


CASES = {
    'action_wall_stop': (START, 360, [event(['UP','CROSS'],260)], False),
    'forward_stop': (START, 260, [event(['UP'], 120)], False),
    'turn': (START, 260, [event(['UP', 'RIGHT'], 64), event(['UP'], 120, 64)], False),
    'stand_up': ('validation/wading/start/state_f000002.bin', 180, [], False),
    'back_side': (START, 320, [event(['DOWN'], 60), event(['R2', 'LEFT'], 130, 80),
                               event(['R2', 'RIGHT'], 210, 160)], False),
    'walk_modifier_heldout': (START, 220, [event(['UP', 'R1', 'LEFT'], 52),
                                          event(['UP', 'R1'], 90, 52)], True),
    'restart_heldout': (START, 260, [event(['UP'], 54), event(['UP'], 114, 70)], True),
    'dry_exit_heldout': ('validation/wading/dry_exit_start/state.bin', 180,
                          [event(['UP'],20)], True),
    'idle_right_validation_heldout': (START, 240, [event(['RIGHT'],74), event(['UP','LEFT'],150,100)], True),
    'surface_validation_heldout': ('validation/wading/surface_shallow_start/state.bin', 220,
                                   [event(['UP','RIGHT'],102,40), event(['UP'],160,102)], True),
    'surface_step_out': ('validation/wading/surface_shallow_start/state.bin', 200,
                          [event(['UP'],150,80)], False),
    'deep_entry_heldout': ('validation/wading/deep_entry_start/state.bin', 200,
                            [event(['UP'],80)], True),
    'turn_in_place_heldout': (START, 300, [event(['LEFT'], 82), event(['CIRCLE'], 120, 100),
                                         event(['UP'], 190, 140)], False),
}


DIAGNOSTICS = {
    'dry_exit_static_diagnostic': ('validation/wading/static_diagnostic_start/state.bin',
                                   180, [event(['UP'],20)], False),
}


def sha(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def run(args):
    command = ['psx', 'run', *args, '--summary']
    subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    return command


def boot():
    # Retained original menu route, replayed from power-on with this core.
    events = [{'buttons': ['START'], 'frame': 1800},
     {'buttons': [], 'frame': 1815},
     {'buttons': ['START'], 'frame': 2800},
     {'buttons': [], 'frame': 2815},
     {'buttons': ['START'], 'frame': 3800},
     {'buttons': [], 'frame': 3815},
     {'buttons': ['START'], 'frame': 4800},
     {'buttons': [], 'frame': 4815},
     {'buttons': ['START'], 'frame': 5800},
     {'buttons': [], 'frame': 5815},
     {'buttons': ['CROSS'], 'frame': 7800},
     {'buttons': [], 'frame': 7820},
     {'buttons': ['START'], 'frame': 9300},
     {'buttons': [], 'frame': 9320},
     {'buttons': ['START'], 'frame': 10300},
     {'buttons': [], 'frame': 10320},
     {'buttons': ['CROSS'], 'frame': 11400},
     {'buttons': [], 'frame': 11410},
     {'buttons': [], 'frame': 11880},
     {'buttons': ['CROSS'], 'frame': 13080},
     {'buttons': [], 'frame': 13090}]
    out = ROOT / 'validation/wading/boot_home'
    out.mkdir(parents=True, exist_ok=True)
    (out / 'input.json').write_text(json.dumps(events, indent=2) + '\n')
    run(['--frames', '14880', '--replay', str(out / 'input.json'),
         '--out', str(out), '--save-state', '--timeout', '600'])
    # Core format9 reaches the Home postcard at this boundary.
    return run(['--from-state', str(out / 'state.bin'), '--frames', '1800',
                '--input', json.dumps([event(['CROSS'], 10)]), '--save-state',
                '--out', 'validation/wading/home'])


def setup():
    # Home fountain: 512 deep. Start with dry standing rules and let the
    # original's deep-entry / submerged shallow-water test select stand-up192.
    command = ['--from-state', HOME, '--frames', '100', '--out', 'validation/wading/start',
               '--poke', '8017c8b0=19968', '--poke', '8017c8b4=2816', '--poke', '8017c8b8=75776',
               '--poke', '8017c894=17:2', '--poke', '8017c8be=0:2', '--save-state', '--save-state-at', '2']
    for sample in SAMPLES:
        command += ['--sample', sample]
    command = run(command)
    # Prepare native gait, then move only the fixture's initial placement.
    # One display field has no controller call at this even boundary: the
    # saved fixtures therefore retain mode4 for the replay's transition test.
    gait = run(['--from-state', START, '--frames', '50', '--input', json.dumps([event(['UP'],50)]),
                '--save-state', '--out', 'validation/wading/gait'])
    placements = []
    for name, xyz, room in [('dry_exit_start', [26112,512,65024],33),
                            ('deep_entry_start',[28160,4352,90624],14),
                            ('static_diagnostic_start',[25088,512,65024],33)]:
        command_place = ['--from-state', 'validation/wading/gait/state.bin', '--frames','1',
                         '--save-state','--out',f'validation/wading/{name}']
        for address,value in zip(['8017c8b0','8017c8b4','8017c8b8','8017c894'],[*xyz,room]):
            command_place += ['--poke',f'{address}={value}'+(':2' if address=='8017c894' else '')]
        placements.append(run(command_place))
    surface_prep=run(['--from-state','validation/wading/deep_entry_start/state.bin','--frames','79',
                      '--out','validation/wading/surface_prep','--save-state'])
    surface_place=run(['--from-state','validation/wading/surface_prep/state.bin','--frames','1',
                       '--poke','8017c8b0=19968','--poke','8017c8b4=2561','--poke','8017c8b8=75776',
                       '--poke','8017c894=17:2','--out','validation/wading/surface_shallow_start','--save-state'])
    metadata = dict(source='Supplied USA Rev3 disc via psx native', command=command,
                    seed=HOME, seed_sha256=sha(HOME), checkpoint=START, checkpoint_sha256=sha(START),
                    scope='Initial placement only; native water entry and stand-up, then settled wading',
                    gait_preparation=gait, transition_placements=placements,
                    surface_preparation=[surface_prep,surface_place])
    (ROOT / 'game/tests/data/original/wading_fixture.json').write_text(json.dumps(metadata, indent=2) + '\n')


def inspect():
    ram = ROOT / 'validation/wading/home/ram.bin'
    raw = ram.read_bytes()
    ranges = [(0x424f0, 0x42b00), (0x39d24, 0x39e54), (0x3c028, 0x3c200),
              (0x44bb4, 0x44ce0), (0x45ba4, 0x45cb8), (0x3899c, 0x38b88),
              (0x38194,0x38468),(0x386c4,0x3899c),(0x38d94,0x38f34),
              (0x39370,0x393d8),(0x39ce0,0x39d24),(0x3c208,0x3c250)]
    entries = []
    for start, end in ranges:
        command = ['psx', 'disasm', '--ram', str(ram), '--addr', hex(start),
                   '--count', str((end-start)//4), '--text']
        listing = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=True).stdout
        entries.append(dict(start=hex(start), end_exclusive=hex(end), command=command,
                            sha256=hashlib.sha256(raw[start:end]).hexdigest(), listing=listing))
    data = dict(source='Supplied disc Home runtime RAM through psx', ram_sha256=sha(str(ram)), ranges=entries,
                handlers={str(s): [hex(struct.unpack_from('<I', raw, b+4*s)[0])
                                  for b in [0x82c98, 0x82de0]] for s in [2, 55, 65]})
    (ROOT / 'game/tests/data/original/wading_code.json').write_text(json.dumps(data, indent=2) + '\n')


def record(name):
    checkpoint, frames, inputs, independent = (CASES | DIAGNOSTICS)[name]
    out = f'validation/wading/{name}'
    command = ['--from-state', checkpoint, '--frames', str(frames), '--input', json.dumps(inputs),
               '--out', out, '--save-state', '--snapshot-every', '2',
               '--watch', '8008c572:2', '--watch', '8008aad4:4']
    for sample in SAMPLES:
        command += ['--sample', sample]
    command = run(command)
    report = json.loads((ROOT / out / 'report.json').read_text())
    assert report['status'] == 'complete' and not report.get('pokes')
    # A collision writes move_angle once per completed controller tick. The
    # current core can pause a display field and catch up afterward. Preserve
    # every field sample, plus the actual input and tick boundary, rather than
    # fitting a display delay into gameplay.
    ticks = []
    input_reads = 0
    current_input = 0
    for line in (ROOT / out / 'writes.jsonl').read_text().splitlines():
        write = json.loads(line)
        if write['address'] & 0x1fffff == 0x8aad4:
            input_reads += 1
            current_input = write['value']
        elif write['address'] & 0x1fffff == 0x8c572:
            buttons = [button for bit, button in [(1, 'UP'), (2, 'DOWN'), (4, 'LEFT'), (8, 'RIGHT'),
                       (16, 'SQUARE'), (64, 'CROSS'), (128, 'R1'), (4096, 'CIRCLE')] if current_input & bit]
            if current_input & 0xc00:
                buttons += ['R2', 'LEFT' if current_input & 0x400 else 'RIGHT']
            ticks.append(dict(field=write['run_frame'], input=current_input, buttons=buttons, pc=write['pc']))
    assert input_reads == len(ticks), (name, input_reads, len(ticks))
    dest = ROOT / f'game/tests/data/original/wading_{name}'
    dest.with_suffix('.csv').write_bytes((ROOT / out / 'samples.csv').read_bytes())
    metadata = dict(source='Supplied USA Rev3 disc via psx native', checkpoint=checkpoint,
                    checkpoint_sha256=sha(checkpoint), command=command, input=inputs,
                    independent=independent, pokes=[], build=report['build'], identity=report['identity'],
                    logic_ticks=ticks, logic_watch_sha256=sha(out + '/writes.jsonl'),
                    samples_sha256=sha(out + '/samples.csv'),
                    verify_camera=name not in ['stand_up','surface_step_out','surface_validation_heldout','deep_entry_heldout'])
    if name in DIAGNOSTICS:
        metadata.update(diagnostic_only=True, verify_camera=False,
                        provenance_note='Placement overlaps room61 static mesh47 at [25088,-128,65024]. The native -99X/-910 yaw deflection is not implemented by the room-only collision query. Retained separately; the ordinary exit fixture is one tile east.')
    if not metadata['verify_camera'] and name not in DIAGNOSTICS:
        metadata['camera_limitations']='Gameplay is compared throughout. Stand-up flags1 and the general surface camera remain unported; these complete transition routes do not claim camera parity.'
    if name == 'turn_in_place_heldout':
        metadata['provenance_note']='Historical filename retained; this route refined the left fast-turn goal and is calibration. Fresh idle_right_validation_heldout verifies both turn directions afterward.'
    dest.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(dest.relative_to(ROOT))


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--boot', action='store_true')
    p.add_argument('--setup', action='store_true')
    p.add_argument('--inspect', action='store_true')
    p.add_argument('--case', choices=CASES | DIAGNOSTICS, action='append', default=[])
    args = p.parse_args()
    if args.boot:
        boot()
    if args.setup:
        setup()
    if args.inspect:
        inspect()
    for name in args.case:
        record(name)
