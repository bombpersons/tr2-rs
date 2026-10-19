#!/usr/bin/env python3
"""PSX oxygen exhaustion and underwater death; explicit initial-counter setup.

Only setup changes oxygen/health. Moving starts are reached by ordinary pad
input from the existing documented submerged fixture. No replay pokes.
"""
import argparse
import hashlib
import json
import subprocess
from record_swimming import ROOT, BASE, SAMPLES


def event(buttons, until, frame=0):
    return dict(frame=frame, buttons=buttons, until=until)


# (preparation input/fields, initial air, health, replay fields/input, independent)
CASES = {
    'exhaustion': ([], 0, 4, 1000, 80, [], False),
    'tread_death': ([], 0, 4, 20, 640, [], False),
    'moving_death': ([event(['SQUARE', 'RIGHT', 'UP'], 34)], 34, 2, 10,
                     220, [event(['SQUARE', 'LEFT', 'DOWN'], 100)], False),
    'exhaustion_heldout': ([], 0, 2, 1000, 160,
                          [event(['SQUARE'], 48), event(['SQUARE', 'LEFT', 'UP'], 76, 48)], True),
    # This exposed surface death and was used to refine that rule. Preserve
    # its historical name and samples, but do not claim it as independent.
    'rising_death_heldout': ([event(['SQUARE', 'DOWN'], 76)], 76, 1, 7,
                            260, [event(['SQUARE', 'RIGHT', 'UP'], 80)], False),
    'opposite_death_heldout': ([event(['SQUARE', 'LEFT', 'DOWN'], 30)], 30, 3, 15,
                              560, [event(['SQUARE', 'RIGHT', 'UP'], 80)], True),
    # No counter pokes: retain the full ordinary oxygen countdown and death.
    'full_air_heldout': ([], 0, None, None, 4380, [], True),
    'surface_death_validation_heldout': ([event(['SQUARE', 'DOWN'], 72)], 72, 1, 12,
                                        460, [event(['SQUARE', 'LEFT', 'UP'], 80)], True),
    'surface_death_late_heldout': ([event(['SQUARE', 'DOWN'], 80)], 80, 1, 5,
                                  460, [event(['SQUARE', 'RIGHT', 'DOWN'], 80)], True),
    'air_recovery_heldout': ([event(['SQUARE', 'DOWN'], 76)], 76, 1, 1000,
                            200, [event(['SQUARE', 'DOWN'], 20)], True),
}

DIAGNOSTICS = {
    'moving_death_menu_diagnostic': ([event(['SQUARE', 'RIGHT', 'UP'], 34)], 34, 2, 10,
                                    220, [event(['SQUARE', 'LEFT', 'DOWN'], 180)], False),
}


def sha(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def run(args):
    command = ['psx', 'run', *args, '--summary']
    subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    return command


def record(name):
    inputs, fields, air, health, duration, replay, independent = (CASES | DIAGNOSTICS)[name]
    checkpoint = BASE
    preparation = []
    if fields:
        out = f'validation/drowning/{name}_moving'
        preparation.append(run(['--from-state', checkpoint, '--frames', str(fields),
                                '--input', json.dumps(inputs), '--save-state', '--out', out]))
        checkpoint = out + '/state.bin'
    if air is not None:
        out = f'validation/drowning/{name}_start'
        preparation.append(run(['--from-state', checkpoint, '--frames', '2',
                                '--poke', f'8008c4fe={air}:2', '--poke', f'8017c89e={health}:2',
                                '--save-state', '--out', out]))
        checkpoint = out + '/state.bin'
    out = f'validation/drowning/{name}'
    command = ['--from-state', checkpoint, '--frames', str(duration), '--input', json.dumps(replay),
               '--snapshot-every', '2' if duration < 1000 else '20', '--save-state', '--out', out]
    for sample in SAMPLES:
        command += ['--sample', sample]
    command = run(command)
    report = json.loads((ROOT / out / 'report.json').read_text())
    assert report['status'] == 'complete' and not report.get('pokes')
    destination = ROOT / f'game/tests/data/original/drowning_{name}'
    destination.with_suffix('.csv').write_bytes((ROOT / out / 'samples.csv').read_bytes())
    metadata = dict(source='Supplied Tomb Raider II USA Rev3 disc via psx native',
                    scope='Submerged oxygen exhaustion, authored death, motion and collision',
                    checkpoint=checkpoint, checkpoint_sha256=sha(checkpoint),
                    preparation=preparation, preparation_source=BASE, preparation_source_sha256=sha(BASE),
                    counter_pokes_only=air is not None, command=command, input=replay, independent=independent,
                    pokes=[], build=report['build'], identity=report['identity'], frames=report['frames'],
                    samples_sha256=sha(out + '/samples.csv'))
    if name == 'rising_death_heldout':
        metadata['used_to_refine_rules'] = True
        metadata['provenance_note'] = ('Historical filename retained. This first post-implementation route '
                                       'exposed the surface-death branch and became calibration for it. '
                                       'Fresh surface_death_validation/late routes validate the combined rules.')
    if name in DIAGNOSTICS:
        metadata['diagnostic_only'] = True
        metadata['provenance_note'] = ('Native simulation stops at field128 while post-death pad input remains held. '
                                       'The separately retained death_flow trace observes ControlPhase returning '
                                       '0x500 at field129 once death ticks reach61 with held input. The outer '
                                       'game-flow transition is outside controller parity; no delay is fitted.')
    destination.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(destination.relative_to(ROOT))


def inspect():
    out = ROOT / 'validation/drowning/code'
    out.mkdir(parents=True, exist_ok=True)
    ram = (ROOT / '.psx/home_start/ram.bin').read_bytes()
    entries = []
    for start, end in [(0x1d9fc, 0x1dac8), (0x1dd60, 0x1dd98),
                       (0x429a4, 0x42ca8), (0x44264, 0x444c0),
                       (0x44fc4, 0x45384), (0x45384, 0x458ac)]:
        command = ['psx', 'disasm', '--ram', '.psx/home_start', '--addr', hex(start),
                   '--count', str((end - start) // 4), '--text']
        text = subprocess.run(command, cwd=ROOT, check=True, capture_output=True, text=True).stdout
        (out / f'{start:05x}.txt').write_text(text)
        entries.append(dict(start=hex(start), end_exclusive=hex(end), command=command,
                            bytes_sha256=hashlib.sha256(ram[start:end]).hexdigest(),
                            listing_sha256=hashlib.sha256(text.encode()).hexdigest()))
    data = dict(source='Supplied USA Rev3 disc via psx; Home runtime code',
                ram_sha256=hashlib.sha256(ram).hexdigest(), functions=entries,
                death_state=44, control='0x8004570c', collision='0x80045810')
    (ROOT / 'game/tests/data/original/drowning_code.json').write_text(json.dumps(data, indent=2) + '\n')


def death_flow():
    name = 'moving_death_menu_diagnostic'
    meta_path = ROOT / f'game/tests/data/original/drowning_{name}.json'
    if not meta_path.exists():
        record(name)
    meta = json.loads(meta_path.read_text())
    out = 'validation/drowning/death_flow'
    command = run(['--from-state', meta['checkpoint'], '--frames', '220',
                   '--input', json.dumps(meta['input']), '--trace', '--trace-from-frame', '124',
                   '--trace-frames', '12', '--trace-range', '8001d9fc-8001dac8',
                   '--trace-range', '8001dd60-8001dd98', '--max-events', '20000',
                   '--sample', '8008c502:i16:death_ticks', '--sample', '8008aad4:u32:input',
                   '--sample', '80089cf4:i32:death_flow_gate', '--out', out])
    report = json.loads((ROOT / out / 'report.json').read_text())
    assert report['status'] == 'complete' and report['trace']['lines_dropped'] == 0
    data = dict(source='Supplied USA Rev3 disc via psx native', command=command,
                checkpoint_sha256=sha(meta['checkpoint']), build=report['build'], identity=report['identity'],
                trace=report['trace'], trace_sha256=sha(out + '/trace.txt'),
                samples_sha256=sha(out + '/samples.csv'),
                observed='At run field129 ControlPhase reads death ticks61, input0x16 and gate0, then returns0x500 at0x8001DA74/0x8001DD90. No later player logic is observed in the220-field capture.')
    (ROOT / 'game/tests/data/original/drowning_death_flow.json').write_text(json.dumps(data, indent=2) + '\n')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--inspect', action='store_true')
    p.add_argument('--death-flow', action='store_true')
    p.add_argument('--case', choices=CASES | DIAGNOSTICS, action='append', default=[])
    args = p.parse_args()
    if args.inspect:
        inspect()
    if args.death_flow:
        death_flow()
    for name in args.case:
        record(name)
