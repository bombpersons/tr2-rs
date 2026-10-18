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
    'run_stop_left_heldout': ('home_runway_start', 200, [event(['UP'], 60)]),
    'run_stop_right_heldout': ('home_runway_start', 220, [event(['UP'], 80)]),
    'run_stop_rehold_heldout': ('home_runway_start', 240, [event(['UP'], 56), event(['UP'], 114, 70)]),
    'run_stop_rehold_validation_heldout': ('home_runway_start', 260, [event(['UP'], 64), event(['UP'], 136, 76)]),
    'run_stop_restart_heldout': ('home_runway_start', 260, [event(['UP'], 66), event(['UP'], 138, 90)]),
    'middle_wall': ('airborne_mid_wall_start', 200, [event(['UP', 'SQUARE'], 22)]),
    'blocked_takeoff': ('airborne_blocked_wall_start', 200, [event(['UP', 'SQUARE'], 22)]),
    'running_low_wall': ('airborne_wall_start', 220, [event(['UP'], 40),
                                                   event(['UP', 'SQUARE'], 70, 40)]),
    # Retain the former diagnostic's name so its original recording identity
    # stays traceable. It is now a passing calibration route.
    'run_start_retry_probe': ('airborne_mid_wall_start', 220, [event(['UP'], 12),
                                                           event(['UP', 'SQUARE'], 30, 12)]),
    'run_start_release_heldout': ('airborne_mid_wall_start', 180, [event(['UP'], 12),
                                                              event(['UP', 'SQUARE'], 24, 12),
                                                              event(['UP'], 80, 24)]),
    'run_start_hold_heldout': ('airborne_mid_wall_start', 260, [event(['UP'], 8),
                                                           event(['UP', 'SQUARE'], 100, 8)]),
    'middle_delay_heldout': ('airborne_mid_wall_start', 220, [event(['UP', 'SQUARE'], 32, 10),
                                                          event(['SQUARE'], 48, 40)]),
    'low_cancel_heldout': ('airborne_wall_start', 240, [event(['UP'], 40),
                                                     event(['UP', 'SQUARE'], 62, 40),
                                                     event(['SQUARE'], 72, 66),
                                                     event(['RIGHT'], 150, 80)]),
    'low_cancel_validation_heldout': ('airborne_wall_start', 260, [event(['UP'], 40),
                                                                event(['UP', 'SQUARE'], 62, 40),
                                                                event(['SQUARE'], 74, 64),
                                                                event(['LEFT'], 210, 86)]),
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
        for target, buttons, until, frames in [('airborne_mid_wall_start', ['UP', 'R1'], 100, 180),
                                               ('airborne_blocked_wall_start', ['UP'], 40, 140)]:
            subprocess.run(['psx', 'run', '--from-state', '.psx/airborne_wall_start/state.bin',
                            '--frames', str(frames), '--input', json.dumps([event(buttons, until)]),
                            '--save-state', '--out', f'.psx/{target}', '--summary'],
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
        'used_to_refine_rules': args.case in ('low_cancel_heldout', 'run_start_retry_probe', 'run_stop_rehold_heldout'),
        'diagnostic_only': False,
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
