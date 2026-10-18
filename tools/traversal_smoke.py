#!/usr/bin/env python3
"""Replay ledge traversal through Bevy MCP with recorded PSX cameras.

Record references with tools/record_traversal.py;
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
    'forward_climb': [18, 40, 82, 84, 120, 164, 240, 312, 314],
    'forward_landing': [18, 24, 86, 88, 122],
    'running_jump': [62, 76, 98, 138, 140],
    'landing_cancel_heldout': [88, 106, 114, 120, 190, 192, 232],
    'three_click': [10, 60, 140, 142, 190],
    'hang_climb': [34, 40, 56, 100, 144, 200, 292, 294],
    'hang_drop': [102, 120, 122, 160],
    'hang_early_climb_heldout': [40, 44, 100, 192, 194],
    'hang_early_drop_heldout': [46, 64, 66, 108],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=ROOT / 'validation/traversal-smoke')
    parser.add_argument('--cases', default=','.join(CAPTURES))
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    report = {'cases': {}, 'status': 'running'}
    try:
        for case in args.cases.split(','):
            fields = CAPTURES[case]
            source = ROOT / f'game/tests/data/original/traversal_{case}'
            rows = list(csv.DictReader(source.with_suffix('.csv').open()))
            inputs = json.loads(source.with_suffix('.json').read_text())['input']
            first = rows[0]
            reference = dict(REFERENCES['home'],
                             position=[int(first[c]) for c in ['x', 'y', 'z']], yaw=int(first['yaw']),
                             absolute_animation_frame=int(first['animation_frame']), room=int(first['room']),
                             camera=[int(first[f'camera_{c}']) for c in 'xyz'],
                             target=[int(first[f'target_{c}']) for c in 'xyz'])
            client = Client(ROOT / 'target/debug/tomb-raider-ii', out / case, 'assault')
            poses = report['cases'][case] = []
            try:
                client.wait_loaded()
                align_reference(client, reference)
                tick = 0
                level = json.loads((ROOT / 'game/assets/levels/assault/level.json').read_text())
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
                    original = ROOT / f'.psx/traversal_{case}/screens/f{field+4:06}.png'
                    target = client.out / f'f{field:03}-original.png'
                    shutil.copyfile(original, target)
                    poses.append({'field': field, 'state': state, 'image': image,
                                  'original': str(target.relative_to(ROOT)), 'original_screen_field': field + 4})
                log = (client.out / 'game.log').read_text()
                assert not any(s in log for s in ['panicked at', 'Shader compilation error', 'B0001', 'Failed to load asset'])
            finally:
                client.close()
        report['status'] = 'complete'
    except Exception as error:
        report.update(status='failed', error=str(error))
        raise
    finally:
        (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'report': str(out / 'report.json')}))


if __name__ == '__main__':
    main()
