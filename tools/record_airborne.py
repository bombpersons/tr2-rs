#!/usr/bin/env python3
"""Retain airborne collision evidence from ordinary PSX pad routes, without pokes.

Record held-out routes only after implementing the corresponding rules.
"""
import argparse
import hashlib
import json
import shutil
import subprocess

from record_backward_collision import ROOT, SAMPLES, event

CASES = {
    'forward_wall': ('airborne_wall_start', 180, [event(['UP', 'SQUARE'], 22)]),
    'reach_wall': ('airborne_wall_start', 180, [event(['UP', 'SQUARE', 'CROSS'], 22),
                                            event(['CROSS'], 110, 22)]),
    'forward_steer': ('airborne_wall_start', 200, [event(['UP', 'SQUARE'], 22),
                                               event(['RIGHT'], 48, 30)]),
    'forward_delay_heldout': ('airborne_wall_start', 220, [event(['UP', 'SQUARE'], 36, 14),
                                                       event(['SQUARE'], 120, 100)]),
    'reach_release_heldout': ('airborne_wall_start', 240, [event(['UP', 'SQUARE', 'CROSS'], 22),
                                                       event(['CROSS'], 64, 22),
                                                       event(['DOWN', 'R1'], 170, 96)]),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', choices=CASES)
    parser.add_argument('--prepare-checkpoints', action='store_true')
    parser.add_argument('--summarize-only', action='store_true')
    args = parser.parse_args()
    if args.prepare_checkpoints:
        subprocess.run(['psx', 'run', '--from-state', '.psx/backward_rolled_start/state.bin',
                        '--frames', '240', '--input', json.dumps([event(['UP', 'R1'], 160)]),
                        '--save-state', '--out', '.psx/airborne_wall_start', '--summary'],
                       cwd=ROOT, check=True)
        if args.case is None:
            return
    if args.case is None:
        parser.error('--case or --prepare-checkpoints is required')
    start, frames, inputs = CASES[args.case]
    checkpoint = f'.psx/{start}/state.bin'
    output = f'.psx/airborne_{args.case}'
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '2',
               '--watch', '8008c572:2', '--max-events', '10000',
               '--save-state', '--out', output, '--summary']
    for sample in SAMPLES:
        command += ['--sample', sample]
    if not args.summarize_only:
        subprocess.run(command, cwd=ROOT, check=True)
    report = json.loads((ROOT / output / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames
    assert report['watch']['complete'] and not report.get('pokes')
    target = ROOT / f'game/tests/data/original/airborne_{args.case}.csv'
    shutil.copyfile(ROOT / output / 'samples.csv', target)
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; no pokes',
        'heldout_after_implementation': args.case.endswith('_heldout'),
        'level': 'assault', 'command': command, 'input': inputs, 'samples': SAMPLES,
        'build': report['build'], 'identity': report['identity'], 'frames': report['frames'],
        'checkpoint': checkpoint,
        'checkpoint_sha256': hashlib.sha256((ROOT / checkpoint).read_bytes()).hexdigest(),
        'samples_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
        'writes_sha256': hashlib.sha256((ROOT / output / 'writes.jsonl').read_bytes()).hexdigest(),
    }
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT))


if __name__ == '__main__':
    main()
