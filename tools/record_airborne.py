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
    'roll_first_half_fall': ('airborne_walk_ledge', 240, [event(['CIRCLE'], 2)]),
    'roll_first_half_walk_heldout': ('airborne_walk_ledge', 300, [event(['CIRCLE'], 20, 18),
                                                             event(['R1'], 64, 24),
                                                             event(['UP', 'R1'], 154, 110)]),
    'roll_first_half_run_heldout': ('airborne_walk_ledge', 280, [event(['CIRCLE'], 14, 12),
                                                            event(['SQUARE'], 30, 20),
                                                            event(['UP'], 126, 90)]),
    'roll_runway': ('home_runway_start', 220, [event(['CIRCLE'], 2)]),
    'roll_step_up': ('backward_step_down_start', 220, [event(['CIRCLE'], 2)]),
    'roll_step_down': ('backward_rolled_start', 220, [event(['CIRCLE'], 2)]),
    'roll_wall': ('vertical_open_start', 220, [event(['CIRCLE'], 2)]),
    'roll_runway_delay_heldout': ('home_runway_start', 240, [event(['CIRCLE'], 16, 14),
                                                         event(['RIGHT'], 38, 22)]),
    'roll_wall_hold_heldout': ('vertical_open_start', 280, [event(['CIRCLE'], 100)]),
    'roll_recovery_walk_heldout': ('backward_rolled_start', 260, [event(['CIRCLE'], 14, 12),
                                                             event(['UP', 'R1'], 126, 62)]),
    'roll_ledge_heldout': ('forward_ledge_start', 240, [event(['CIRCLE'], 2)]),
    'roll_ledge_fall_heldout': ('roll_ledge_start', 280, [event(['CIRCLE'], 2)]),
    'roll_during_walk_heldout': ('home_start', 240, [event(['UP', 'R1'], 40),
                                                 event(['UP', 'R1', 'CIRCLE'], 64, 40),
                                                 event(['UP', 'R1'], 120, 64)]),
    'walk_wall': ('vertical_wall_start', 180, [event(['UP', 'R1'], 100)]),
    'walk_ledge': ('forward_ledge_start', 280, [event(['UP', 'R1'], 220)]),
    'walk_step_up': ('home_start', 420, [event(['UP', 'R1'], 360)]),
    'walk_step_down': ('backward_rolled_start', 380, [event(['UP', 'R1'], 320)]),
    'walk_step_up_other_heldout': ('backward_step_down_start', 260, [event(['UP', 'R1'], 190, 10)]),
    'walk_step_down_release_heldout': ('backward_rolled_start', 240, [event(['UP', 'R1'], 112, 16)]),
    'walk_step_down_mid_release_heldout': ('backward_rolled_start', 240, [event(['UP', 'R1'], 98, 16)]),
    'walk_step_down_other_heldout': ('walk_step_down_other_start', 300, [event(['UP', 'R1'], 240)]),
    'walk_ledge_jump_request_heldout': ('forward_ledge_start', 300, [event(['UP', 'R1'], 170, 18),
                                                                event(['SQUARE'], 186, 176),
                                                                event(['UP', 'R1'], 254, 224)]),
    'vertical_steer_forward': ('home_runway_start', 240, [event(['SQUARE'], 8), event(['UP'], 50, 28)]),
    'vertical_steer_backward': ('home_runway_start', 240, [event(['SQUARE'], 8), event(['DOWN'], 50, 28)]),
    'vertical_steer_wall': ('vertical_wall_start', 240, [event(['SQUARE'], 8), event(['UP'], 50, 28)]),
    'vertical_steer_back_open': ('vertical_open_start', 240, [event(['SQUARE'], 8), event(['DOWN'], 50, 28)]),
    'vertical_steer_reverse_heldout': ('vertical_open_start', 260, [event(['SQUARE'], 28, 18),
                                                               event(['DOWN'], 52, 46), event(['UP'], 86, 52)]),
    'vertical_steer_late_heldout': ('vertical_wall_start', 260, [event(['SQUARE'], 24, 16),
                                                            event(['DOWN'], 80, 62)]),
    'vertical_clearance_heldout': ('vertical_wall_start', 220, [event(['SQUARE'], 8)]),
    'vertical_clearance_delay_heldout': ('vertical_wall_start', 260, [event(['SQUARE'], 28, 18),
                                                                  event(['SQUARE'], 84, 72)]),
    'walk_stop_left_heldout': ('home_runway_start', 240, [event(['UP', 'R1'], 100)]),
    'walk_stop_right_heldout': ('home_runway_start', 220, [event(['UP', 'R1'], 80)]),
    'walk_stop_rehold_heldout': ('home_runway_start', 280, [event(['UP', 'R1'], 80),
                                                        event(['UP', 'R1'], 162, 110)]),
    'forward_ledge_walk_validation_heldout': ('forward_ledge_start', 280, [event(['UP'], 72, 16),
                                                                       event(['UP', 'R1'], 148, 72)]),
    'braking_wall_validation_heldout': ('forward_ledge_start', 260, [event(['UP'], 56),
                                                                 event(['SQUARE'], 68, 62),
                                                                 event(['UP'], 176, 130),
                                                                 event(['SQUARE'], 196, 188)]),
    'braking_wall_release_validation_heldout': ('forward_ledge_start', 280, [event(['UP'], 74, 18),
                                                                         event(['UP'], 198, 152)]),
    'forward_ledge_run': ('forward_ledge_start', 210, [event(['UP'], 90)]),
    'forward_ledge_delay_heldout': ('forward_ledge_start', 240, [event(['UP'], 146, 14)]),
    'forward_ledge_release_heldout': ('forward_ledge_start', 240, [event(['UP'], 58),
                                                               event(['SQUARE'], 70, 64),
                                                               event(['UP'], 184, 138)]),
    'forward_ledge_release_validation_heldout': ('forward_ledge_start', 240, [event(['UP'], 68, 14),
                                                                          event(['SQUARE'], 84, 74),
                                                                          event(['UP'], 160, 146)]),
    'forward_ledge_walk_heldout': ('forward_ledge_start', 240, [event(['UP'], 58),
                                                            event(['UP', 'R1'], 130, 58)]),
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
    parser.add_argument('--prepare-vertical-wall', action='store_true')
    parser.add_argument('--prepare-vertical-open', action='store_true')
    parser.add_argument('--prepare-walk-step', action='store_true')
    parser.add_argument('--prepare-roll-ledge', action='store_true')
    parser.add_argument('--summarize-only', action='store_true')
    args = parser.parse_args()
    if args.prepare_roll_ledge:
        subprocess.run(['psx', 'run', '--from-state', '.psx/forward_ledge_start/state.bin',
                        '--frames', '160', '--input', json.dumps([event(['UP', 'R1'], 60)]),
                        '--save-state', '--out', '.psx/roll_ledge_start', '--summary'],
                       cwd=ROOT, check=True)
        if args.case is None:
            return
    if args.prepare_walk_step:
        subprocess.run(['psx', 'run', '--from-state', '.psx/backward_step_down_start/state.bin',
                        '--frames', '90', '--input', json.dumps([event(['CIRCLE'], 2)]),
                        '--save-state', '--out', '.psx/walk_step_down_other_start', '--summary'],
                       cwd=ROOT, check=True)
        if args.case is None:
            return
    if args.prepare_vertical_open:
        subprocess.run(['psx', 'run', '--from-state', '.psx/vertical_wall_start/state.bin',
                        '--frames', '140', '--input', json.dumps([event(['DOWN', 'R1'], 60)]),
                        '--save-state', '--out', '.psx/vertical_open_start', '--summary'],
                       cwd=ROOT, check=True)
        if args.case is None:
            return
    if args.prepare_vertical_wall:
        subprocess.run(['psx', 'run', '--from-state', '.psx/forward_ledge_start/state.bin',
                        '--frames', '260', '--input', json.dumps([
                            event(['UP'], 56), event(['SQUARE'], 68, 62), event(['UP'], 176, 130)]),
                        '--save-state', '--out', '.psx/vertical_wall_start', '--summary'],
                       cwd=ROOT, check=True)
        if args.case is None:
            return
    if args.prepare_checkpoints:
        subprocess.run(['psx', 'run', '--from-state', '.psx/home_hang_start/state.bin',
                        '--frames', '90', '--input', json.dumps([event(['CIRCLE'], 2)]),
                        '--save-state', '--out', '.psx/forward_ledge_start', '--summary'],
                       cwd=ROOT, check=True)
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
        'used_to_refine_rules': args.case in ('low_cancel_heldout', 'run_start_retry_probe', 'run_stop_rehold_heldout',
                                            'forward_ledge_release_heldout', 'forward_ledge_walk_heldout',
                                            'braking_wall_validation_heldout', 'walk_wall', 'walk_ledge',
                                            'walk_step_up', 'walk_step_down', 'roll_runway', 'roll_step_up',
                                            'roll_step_down', 'roll_wall'),
        'diagnostic_only': args.case == 'vertical_steer_backward',
        'level': 'assault', 'command': command, 'input': inputs, 'samples': SAMPLES,
        'build': report['build'], 'identity': report['identity'], 'frames': report['frames'],
        'checkpoint': checkpoint,
        'checkpoint_sha256': hashlib.sha256((ROOT / checkpoint).read_bytes()).hexdigest(),
        'samples_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
        'writes_sha256': hashlib.sha256((ROOT / output / 'writes.jsonl').read_bytes()).hexdigest(),
    }
    if args.case in ('forward_ledge_release_heldout', 'forward_ledge_walk_heldout'):
        metadata['provenance_note'] = ('Former diagnostic recorded after the ledge-fall implementation; '
                                       'used to refine walking/flight goals and the braking-wall state in the next milestone.')
    if start == 'vertical_wall_start':
        source = '.psx/forward_ledge_start/state.bin'
        metadata['checkpoint_preparation'] = {
            'source': source,
            'source_sha256': hashlib.sha256((ROOT / source).read_bytes()).hexdigest(),
            'frames': 260,
            'input': [event(['UP'], 56), event(['SQUARE'], 68, 62), event(['UP'], 176, 130)],
            'output': checkpoint, 'pokes': [],
        }
    if args.case == 'braking_wall_validation_heldout':
        metadata['provenance_note'] = ('Former walking-stop milestone diagnostic; unchanged CSV used to refine '
                                       'the vertical front-floor clearance margin. Fresh vertical_clearance '
                                       'routes provide independent validation.')
    if args.case == 'vertical_steer_backward':
        metadata['provenance_note'] = ('Signed speeds match, but item-bound pushes at fields40 and88 shift x/z. '
                                       'Position writes at0x8001CA78/0x8001CA90 are retained in '
                                       'validation/vertical-steering/ and .psx/vertical_backward_writes. '
                                       'This recording is diagnostic, not a passing parity claim.')
    if start == 'vertical_open_start':
        source = '.psx/vertical_wall_start/state.bin'
        metadata['checkpoint_preparation'] = {
            'source': source,
            'source_sha256': hashlib.sha256((ROOT / source).read_bytes()).hexdigest(),
            'frames': 140, 'input': [event(['DOWN', 'R1'], 60)], 'output': checkpoint, 'pokes': [],
        }
    if start == 'walk_step_down_other_start':
        source = '.psx/backward_step_down_start/state.bin'
        metadata['checkpoint_preparation'] = {
            'source': source,
            'source_sha256': hashlib.sha256((ROOT / source).read_bytes()).hexdigest(),
            'frames': 90, 'input': [event(['CIRCLE'], 2)], 'output': checkpoint, 'pokes': [],
        }
    if start == 'roll_ledge_start':
        source = '.psx/forward_ledge_start/state.bin'
        metadata['checkpoint_preparation'] = {
            'source': source,
            'source_sha256': hashlib.sha256((ROOT / source).read_bytes()).hexdigest(),
            'frames': 160, 'input': [event(['UP', 'R1'], 60)], 'output': checkpoint, 'pokes': [],
        }
    if start == 'airborne_walk_ledge':
        source = '.psx/forward_ledge_start/state.bin'
        metadata['checkpoint_preparation'] = {
            'source': source,
            'source_sha256': hashlib.sha256((ROOT / source).read_bytes()).hexdigest(),
            'frames': 280, 'input': [event(['UP', 'R1'], 220)], 'output': checkpoint, 'pokes': [],
            'recorded_route': 'game/tests/data/original/airborne_walk_ledge.csv',
        }
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT))


if __name__ == '__main__':
    main()
