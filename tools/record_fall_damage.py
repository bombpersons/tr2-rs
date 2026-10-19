#!/usr/bin/env python3
"""PSX fast-fall impact fixtures with explicit controlled initial motion.

Only checkpoint preparation changes the actor's position/animation/motion.
Code, level, other items and subsequent replay memory remain unmodified.
"""
import argparse
import hashlib
import json
import shutil
import struct
import subprocess
from record_backward_collision import ROOT, SAMPLES, event

CASES = {
    '140': (140, 140, []), '141': (141, 140, []),
    '153': (153, 140, []), '155': (155, 140, []), '155_impact': (155, 2, []), '155_moving_heldout': (155, 2, []),
    '142_heldout': (142, 160, [event(['SQUARE'], 40, 10)]),
    '147_heldout': (147, 160, [event(['UP', 'R1'], 114, 88)]),
    '154_heldout': (154, 2, []),
    'fatal_pose': (155, 120, []),
    'fatal_pose_heldout': (155, 140, [event(['SQUARE', 'UP', 'RIGHT'], 60, 12)]),
}


def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', choices=CASES, required=True)
    args = parser.parse_args()
    impact, frames, inputs = CASES[args.case]
    source = '.psx/vertical_open_start/state.bin'
    # Preserve every unknown actor bit, setting only the known gravity bit8.
    ram = (ROOT / '.psx/vertical_open_start/ram.bin').read_bytes()
    flags = struct.unpack_from('<I', ram, 0x17c8c4)[0] | 8
    checkpoint_folder = f'.psx/fall_damage_{args.case if args.case == "155_moving_heldout" else impact}_start'
    checkpoint = checkpoint_folder + '/state.bin'
    prepare = ['psx', 'run', '--from-state', source, '--frames', '2',
               '--poke', '8017c8b4=2600', '--poke', '8017c890=23:2',
               '--poke', '8017c892=348:2', '--poke', '8017c88a=9:2',
               '--poke', '8017c88c=9:2', '--poke', f'8017c89a={20 if args.case == "155_moving_heldout" else 0}:2',
               '--poke', f'8017c89c={impact - 2}:2', '--poke', f'8017c8c4={flags}',
               '--save-state', '--out', checkpoint_folder, '--summary']
    subprocess.run(prepare, cwd=ROOT, check=True)
    initial_mode = 'fast_falling'
    if args.case.startswith('fatal_pose'):
        # The original first-death path stalls CPU work for several display
        # fields and catches up its logic ticks afterward. Start this fixture
        # after that wait; the precise cause remains unconfirmed.
        prior_checkpoint = checkpoint
        checkpoint_folder = '.psx/fall_death_pose_start'
        checkpoint = checkpoint_folder + '/state.bin'
        settle = ['psx', 'run', '--from-state', prior_checkpoint, '--frames', '12',
                  '--save-state', '--out', checkpoint_folder, '--summary']
        subprocess.run(settle, cwd=ROOT, check=True)
        prepare = [prepare, settle]
        initial_mode = 'fatal_landing'
    output = f'.psx/airborne_fall_damage_{args.case}'
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '2',
               '--watch', '8008c572:2', '--watch', '8017c89e:2',
               '--trace', '--trace-range', '8003e0a4-8003e124',
               '--max-events', '10000', '--save-state', '--out', output, '--summary']
    for sample in SAMPLES:
        command += ['--sample', sample]
    subprocess.run(command, cwd=ROOT, check=True)
    report = json.loads((ROOT / output / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames
    assert not report.get('pokes') and report['watch']['complete'] and report['trace']['lines_dropped'] == 0
    target = ROOT / f'game/tests/data/original/airborne_fall_damage_{args.case}.csv'
    shutil.copyfile(ROOT / output / 'samples.csv', target)
    metadata = dict(source='Supplied Tomb Raider II USA Rev 3 disc via psx native; controlled initial falling actor, no replay pokes',
                    heldout_after_implementation=args.case.endswith('_heldout'),
                    used_to_refine_rules=not args.case.endswith('_heldout'), diagnostic_only=args.case == '155',
                    level='assault', initial_mode=initial_mode, impact_speed=impact,
                    command=command, input=inputs, samples=SAMPLES, build=report['build'],
                    identity=report['identity'], frames=report['frames'], checkpoint=checkpoint,
                    checkpoint_sha256=digest(checkpoint), checkpoint_preparation=prepare,
                    checkpoint_preparation_source_sha256=digest(source),
                    samples_sha256=hashlib.sha256(target.read_bytes()).hexdigest(),
                    writes_sha256=digest(output + '/writes.jsonl'), trace_sha256=digest(output + '/trace.txt'))
    if args.case == '155':
        metadata['provenance_note'] = 'Timing diagnostic: fatal landing reaches a native first-death wait, delayed logic, then several catch-up ticks in one field. Impact and post-wait authored pose have separate fixtures; this full display-field recording is not a fixed-rate parity test.'
    if frames == 2:
        display_out = f'.psx/fall_damage_{args.case}_display'
        display_command = ['psx', 'run', '--from-state', checkpoint, '--frames', '6',
                           '--snapshot-every', '2', '--out', display_out, '--summary']
        subprocess.run(display_command, cwd=ROOT, check=True)
        display_report = json.loads((ROOT / display_out / 'report.json').read_text())
        metadata['display_capture'] = dict(command=display_command, out=display_out,
                                           build=display_report['build'], identity=display_report['identity'], pokes=[])
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT))


if __name__ == '__main__':
    main()
