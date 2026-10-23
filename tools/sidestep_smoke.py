#!/usr/bin/env python3
"""Replay sidesteps through Bevy MCP; align views to sampled PSX cameras.

Record references with tools/record_sidestep.py --case left_clear/right_base --camera;
build the game's dev-mcp binary first. Original images trail RAM by about
four fields, as in tools/climb_smoke.py. Output includes both images and
the exact MCP transcript; no disc is opened by the port.
"""
import argparse
import csv
import json
import shutil
from pathlib import Path

from port_smoke import Client, ROOT, REFERENCES, align_reference, convert

CAPTURES = {
    'left_clear': [2, 22, 48, 50, 60],
    'right_base': [2, 52, 82, 100, 110],
    'steer_heldout': [24, 50, 60],
    'rehold_heldout': [12, 14, 50, 60, 62, 82, 110, 120],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=ROOT / 'validation/sidestep-smoke')
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    report = {'cases': {}, 'status': 'running'}
    try:
        for case, fields in CAPTURES.items():
            source = ROOT / f'game/tests/data/original/sidestep_{case}'
            rows = list(csv.DictReader(source.with_suffix('.csv').open()))
            inputs = json.loads(source.with_suffix('.json').read_text())['input']
            first = rows[0]
            reference = dict(REFERENCES['home'],
                             position=[int(first[c]) for c in ['x', 'y', 'z']], yaw=int(first['yaw']),
                             absolute_animation_frame=int(first['animation_frame']),
                             camera=[int(first[f'camera_{c}']) for c in 'xyz'],
                             target=[int(first[f'target_{c}']) for c in 'xyz'])
            client = Client(ROOT / 'target/debug/tomb-raider-ii', out / case, 'assault')
            poses = report['cases'][case] = []
            try:
                client.wait_loaded()
                align_reference(client, reference)
                tick = 0
                level = json.loads((ROOT / 'game/assets/imported/levels/assault/level.json').read_text())
                for field in fields:
                    while 2 * tick < field:
                        buttons = [b.lower() for event in inputs if event['frame'] <= 2 * tick < event['until']
                                   for b in event['buttons']]
                        client.call('game_step', {'frames': 1, 'buttons': buttons})
                        tick += 1
                    row = rows[field]
                    client.edit('ChaseCamera', 'CameraRig', {
                        'manual': True,
                        'position': convert([int(row[f'camera_{c}']) for c in 'xyz']),
                        'target': convert([int(row[f'target_{c}']) for c in 'xyz']),
                    })
                    state = client.query(components=['Player', 'AnimationState'])
                    components = state['entities'][0]['components']
                    p, a = components['Player'], components['AnimationState']
                    position = [round(p['position'][0] * 1024), round(-p['position'][1] * 1024), round(p['position'][2] * 1024)]
                    assert position == [int(row[c]) for c in ['x', 'y', 'z']], (field, position, row)
                    assert a['animation'] == int(row['animation']), (field, a, row)
                    absolute = level['animations'][a['animation']]['frame_start'] + a['frame']
                    assert absolute == int(row['animation_frame']), (field, a, row)
                    image = client.capture(f'f{field:03}', min_colors=16)
                    original = ROOT / f'.psx/sidestep_{case}/screens/f{field+4:06}.png'
                    target = client.out / f'f{field:03}-original.png'
                    shutil.copyfile(original, target)
                    poses.append({'field': field, 'state': state, 'image': image,
                                  'original': str(target.relative_to(ROOT)), 'original_screen_field': field + 4})
                log = (client.out / 'game.log').read_text()
                assert not any(s in log for s in ['panicked at', 'Shader compilation error', 'B0001', 'Failed to load asset'])
            finally:
                client.close()
        report['status'] = 'complete'
    finally:
        (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'report': str(out / 'report.json')}))


if __name__ == '__main__':
    main()
