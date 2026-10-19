#!/usr/bin/env python3
"""PSX-only surfacing and surface-swimming replays. Never modifies level/code.

--setup starts from the submerged checkpoint of tools/record_swimming.py --setup
(its documented position/pitch pokes are the only pokes) and lets the
original surface by itself with a pad route, saving a settled tread checkpoint.
Replays make no pokes. Cases marked independent were first recorded after
the matching surface rules were written.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
from record_swimming import SAMPLES  # noqa: E402

SUBMERGED = 'validation/swim/level_start/state.bin'
TREAD = 'validation/surface/tread_start/state.bin'
# Standing on the east pool edge, facing away, after the held-out climb.
EDGE = 'validation/surface/climb_east_heldout/state.bin'
RISE = [{'frame': 0, 'buttons': ['SQUARE', 'DOWN'], 'until': 80}]
# name: (checkpoint, fields, inputs, independent)
CASES = {
    'rise': (SUBMERGED, 200, RISE, False),
    'forward_turn': (TREAD, 240, [{'frame': 0, 'buttons': ['UP'], 'until': 60},
                                  {'frame': 60, 'buttons': ['UP', 'RIGHT'], 'until': 90},
                                  {'frame': 160, 'buttons': ['LEFT'], 'until': 180}], False),
    'back_side': (TREAD, 280, [{'frame': 0, 'buttons': ['DOWN'], 'until': 40},
                               {'frame': 40, 'buttons': ['DOWN', 'LEFT'], 'until': 60},
                               {'frame': 100, 'buttons': ['R2', 'LEFT'], 'until': 140},
                               {'frame': 180, 'buttons': ['R2', 'RIGHT'], 'until': 220}], False),
    'dive': (TREAD, 160, [{'frame': 0, 'buttons': ['SQUARE'], 'until': 8},
                          {'frame': 12, 'buttons': ['SQUARE'], 'until': 40}], False),
    'wall_climb_blocked': (TREAD, 360, [{'frame': 0, 'buttons': ['UP'], 'until': 150},
                                        {'frame': 150, 'buttons': ['UP', 'CROSS'], 'until': 300}], False),
    'climb_out': (TREAD, 760, [{'frame': 0, 'buttons': ['LEFT'], 'until': 90},
                               {'frame': 100, 'buttons': ['UP', 'CROSS'], 'until': 640}], False),
    # Recorded after the surface rules were written.
    'climb_east_heldout': (TREAD, 560, [{'frame': 0, 'buttons': ['RIGHT'], 'until': 40},
                                        {'frame': 50, 'buttons': ['UP'], 'until': 200},
                                        {'frame': 200, 'buttons': ['UP', 'CROSS'], 'until': 500}], True),
    'strafe_wall_heldout': (TREAD, 400, [{'frame': 0, 'buttons': ['R2', 'RIGHT'], 'until': 340}], True),
    'dive_steer_heldout': (TREAD, 200, [{'frame': 0, 'buttons': ['LEFT'], 'until': 10},
                                        {'frame': 10, 'buttons': ['SQUARE'], 'until': 34},
                                        {'frame': 34, 'buttons': ['SQUARE', 'UP'], 'until': 40},
                                        {'frame': 40, 'buttons': ['UP', 'LEFT'], 'until': 80}], True),
    # Recorded after the entry rule was written.
    'entry_angled_hop_heldout': (EDGE, 160, [{'frame': 0, 'buttons': ['RIGHT'], 'until': 8},
                                             {'frame': 20, 'buttons': ['DOWN'], 'until': 40}], True),
    'entry_hop_heldout': (EDGE, 240, [{'frame': 0, 'buttons': ['DOWN'], 'until': 30},
                                      {'frame': 120, 'buttons': ['SQUARE'], 'until': 200}], True),
}


def run(command):
    subprocess.run(['psx', 'run', *command, '--summary'], cwd=ROOT, check=True, stdout=subprocess.DEVNULL)


def sha(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def setup():
    if not (ROOT / SUBMERGED).exists():
        # Its retained swimming_fixture.json is rewritten; restore it if only the hash differs.
        subprocess.run([sys.executable, 'tools/record_swimming.py', '--setup'], cwd=ROOT, check=True)
    command = ['--from-state', SUBMERGED, '--frames', '240', '--input', json.dumps(RISE),
               '--out', 'validation/surface/tread_start', '--save-state']
    for sample in SAMPLES:
        command += ['--sample', sample]
    run(command)
    data = dict(source='Supplied USA Rev3 disc via psx native; submerged setup pokes only',
                submerged_setup='tools/record_swimming.py --setup',
                submerged_checkpoint=SUBMERGED, submerged_sha256=sha(SUBMERGED),
                command=['psx', 'run', *command, '--summary'], checkpoint=TREAD, checkpoint_sha256=sha(TREAD),
                scope='Original surfaces and settles into surface tread; no pokes after submerged setup')
    (ROOT / 'game/tests/data/original/surface_fixture.json').write_text(json.dumps(data, indent=2) + '\n')


def record(name, scratch=False):
    checkpoint, fields, inputs, independent = CASES[name]
    out = ROOT / 'validation/surface' / name
    command = ['--from-state', checkpoint, '--frames', str(fields), '--input', json.dumps(inputs),
               '--out', str(out), '--snapshot-every', '10', '--save-state']
    for sample in SAMPLES:
        command += ['--sample', sample]
    run(command)
    report = json.loads((out / 'report.json').read_text())
    assert report['status'] == 'complete' and not report.get('pokes')
    if scratch:
        return
    data = dict(source='Supplied USA Rev3 disc via psx native', checkpoint=checkpoint,
                checkpoint_sha256=sha(checkpoint), command=['psx', 'run', *command, '--summary'],
                input=inputs, independent=independent, pokes=[], build=report['build'],
                scope='Surfacing, surface swimming and water-surface transitions',
                samples_sha256=hashlib.sha256((out / 'samples.csv').read_bytes()).hexdigest())
    destination = ROOT / f'game/tests/data/original/surface_{name}'
    destination.with_suffix('.csv').write_bytes((out / 'samples.csv').read_bytes())
    destination.with_suffix('.json').write_text(json.dumps(data, indent=2) + '\n')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--setup', action='store_true')
    p.add_argument('--case', choices=CASES, action='append', default=[])
    p.add_argument('--scratch', action='store_true', help='record under validation/ only')
    args = p.parse_args()
    if args.setup:
        setup()
    for case in args.case:
        record(case, args.scratch)
