#!/usr/bin/env python3
"""PSX-only submerged collision replays (0x80045CB8). Never modifies level/code.

Pool routes start from the submerged checkpoint of tools/record_swimming.py
--setup. --setup builds a second checkpoint in the Home water tunnel (rooms
42/46), whose position/yaw pokes are its only pokes; every replay is pad input
only. All cases were first recorded after the collision rules were written.
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

POOL = 'validation/swim/level_start/state.bin'
TUNNEL = 'validation/swim_collision/tunnel_start/state.bin'
# Room 42, west end, facing the 1024-high tunnel (room 46) to the west.
TUNNEL_POKES = ['8017c8b0=28160', '8017c8b4=3072', '8017c8b8=56832', '8017c894=42:2', '8017c8be=49152:2']
# name: (checkpoint, fields, inputs)
CASES = {
    'wall_head_on': (POOL, 300, [{'frame': 0, 'buttons': ['SQUARE'], 'until': 200}]),
    'wall_angled': (POOL, 440, [{'frame': 0, 'buttons': ['RIGHT'], 'until': 52},
                                {'frame': 52, 'buttons': ['SQUARE'], 'until': 200},
                                {'frame': 200, 'buttons': ['SQUARE', 'LEFT'], 'until': 232},
                                {'frame': 232, 'buttons': ['SQUARE'], 'until': 380}]),
    'floor_dive': (POOL, 360, [{'frame': 0, 'buttons': ['UP'], 'until': 40},
                               {'frame': 40, 'buttons': ['SQUARE'], 'until': 260}]),
    'tunnel_ceiling': (TUNNEL, 400, [{'frame': 0, 'buttons': ['DOWN'], 'until': 60},
                                     {'frame': 60, 'buttons': ['SQUARE'], 'until': 200},
                                     {'frame': 200, 'buttons': ['SQUARE', 'DOWN'], 'until': 260},
                                     {'frame': 260, 'buttons': ['SQUARE'], 'until': 340}]),
}


def run(command):
    subprocess.run(['psx', 'run', *command, '--summary'], cwd=ROOT, check=True, stdout=subprocess.DEVNULL)


def sha(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def setup():
    command = ['--from-state', '.psx/home_start/state.bin', '--frames', '240']
    for poke in TUNNEL_POKES:
        command += ['--poke', poke]
    command += ['--out', 'validation/swim_collision/tunnel_start', '--save-state']
    for sample in SAMPLES:
        command += ['--sample', sample]
    run(command)
    data = dict(source='Supplied USA Rev3 disc via psx native; controlled initial spawn only',
                command=['psx', 'run', *command, '--summary'], seed_checkpoint='.psx/home_start/state.bin',
                seed_sha256=sha('.psx/home_start/state.bin'), checkpoint=TUNNEL, checkpoint_sha256=sha(TUNNEL),
                scope='Original enters the room-42 water by itself and settles; the spawn is setup, not parity')
    (ROOT / 'game/tests/data/original/swim_collision_fixture.json').write_text(json.dumps(data, indent=2) + '\n')


def record(name, scratch=False):
    checkpoint, fields, inputs = CASES[name]
    out = ROOT / 'validation/swim_collision' / name
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
                input=inputs, independent=True, pokes=[], build=report['build'],
                scope='Submerged wall, floor and ceiling collision',
                samples_sha256=hashlib.sha256((out / 'samples.csv').read_bytes()).hexdigest())
    destination = ROOT / f'game/tests/data/original/swim_collision_{name}'
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
