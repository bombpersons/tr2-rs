#!/usr/bin/env python3
"""Record ordinary Home attendant routes using only the supplied disc and psx.

No pokes: every case begins at an unmodified gameplay checkpoint. Independent
cases were first recorded after the matching rules were written. Earlier
investigation captures stay separate from the checked-in validation routes.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
CASES = {
    'nav_short_heldout': (780, [{'frame':0,'buttons':['UP'],'until':146}], True),
    'nav_walk_heldout': (1080, [{'frame':0,'buttons':['UP','R1'],'until':540}], True),
    'nav_turn_heldout': (1300, [
        {'frame':0,'buttons':['UP'],'until':150},
        {'frame':150,'buttons':['LEFT'],'until':174},
        {'frame':174,'buttons':['UP'],'until':238}], True),
    'nav_run': (740, [{'frame':0,'buttons':['UP'],'until':150}], False),
    'contact_walk_heldout': (160, [{'frame': 0, 'buttons': ['DOWN','R1'], 'until': 96}], True),
    'contact_air_delayed_heldout': (150, [
        {'frame': 8, 'buttons': ['SQUARE'], 'until': 20},
        {'frame': 38, 'buttons': ['DOWN'], 'until': 64}], True),
    'contact_walk_release_heldout': (150, [{'frame': 16, 'buttons': ['DOWN','R1'], 'until': 78}], True),
    'contact_back': (110, [
        {'frame': 0, 'buttons': ['SQUARE'], 'until': 8},
        {'frame': 28, 'buttons': ['DOWN'], 'until': 50}], False),
    'local_run': (600, [{'frame': 0, 'buttons': ['UP'], 'until': 86}], False),
    'heldout_short': (520, [{'frame': 0, 'buttons': ['UP'], 'until': 70}], True),
    'heldout_turn': (700, [
        {'frame': 0, 'buttons': ['UP'], 'until': 84},
        {'frame': 84, 'buttons': ['RIGHT'], 'until': 106},
        {'frame': 106, 'buttons': ['UP'], 'until': 130}], True),
    'heldout_walk': (700, [{'frame': 0, 'buttons': ['UP', 'R1'], 'until': 210}], True),
}


def record(name):
    fields, inputs, independent = CASES[name]
    checkpoint = ROOT / '.psx/home_runway_start/state.bin'
    out = ROOT / 'validation/winston' / name
    command = ['psx', 'run', '--from-state', str(checkpoint), '--frames', str(fields),
               '--input', json.dumps(inputs), '--out', str(out), '--save-state', '--summary', '--snapshot-every', '2']
    samples = []
    for prefix, base in [('actor', 0x8017c8c8), ('lara', 0x8017c87c)]:
        for offset, fmt, label in [(14, 'i16', 'state'), (16, 'i16', 'goal'),
                (20, 'i16', 'animation'), (22, 'i16', 'animation_frame'),
                (24, 'i16', 'room'), (30, 'i16', 'speed'), (52, 'i32', 'x'),
                (56, 'i32', 'y'), (60, 'i32', 'z'), (66, 'i16', 'yaw')]:
            samples.append(f'{base+offset:x}:{fmt}:{prefix}_{label}')
    for addr, fmt, label in [(0x801826f8, 'i16', 'max_turn'),
            (0x80182728, 'i32', 'target_x'), (0x80182730, 'i32', 'target_z')]:
        samples.append(f'{addr:x}:{fmt}:{label}')
    samples.append('8017c8cc:u32:actor_contact_mask')
    samples.append('8008c572:i16:lara_movement_heading')
    samples += ['8017c8ec:i16:actor_box','8017c8a0:i16:lara_box','80182718:i16:goal_box']
    for prefix, base in [('camera', 0x8008b258), ('look', 0x8008b268)]:
        for i, axis in enumerate('xyz'):
            samples.append(f'{base+4*i:x}:i32:{prefix}_{axis}')
    for sample in samples:
        command += ['--sample', sample]
    subprocess.run(command, check=True, cwd=ROOT)
    metadata = dict(source='Supplied Tomb Raider II USA Rev 3 disc, psx native runtime',
                    checkpoint=str(checkpoint.relative_to(ROOT)),
                    checkpoint_sha256=hashlib.sha256(checkpoint.read_bytes()).hexdigest(),
                    fields=fields, input=inputs, independent=independent, pokes=[], command=command,
                    build=json.loads((out / "report.json").read_text())["build"],
                    samples_sha256=hashlib.sha256((out / "samples.csv").read_bytes()).hexdigest(),
                    scope=('Local navigation-box corridor and authored follow' if name.startswith('nav') else 'Animated attendant contact and avatar pushing' if name.startswith('contact') else 'Local same-floor follow, authored start/stop and turning; no contact or cross-box navigation'))
    destination = ROOT / 'game/tests/data/original'
    (destination / f'winston_{name}.csv').write_bytes((out / 'samples.csv').read_bytes())
    (destination / f'winston_{name}.json').write_text(json.dumps(metadata, indent=2)+'\n')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('case', choices=CASES)
    record(p.parse_args().case)
