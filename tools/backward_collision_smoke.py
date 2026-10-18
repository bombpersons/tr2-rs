#!/usr/bin/env python3
"""Replay backward room collision through Bevy MCP with recorded PSX cameras.

Record references with tools/record_backward_collision.py;
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
    'wall_walk': [300, 314, 316, 360],
    'wall_hop': [90, 98, 102, 132, 150],
    'ledge_walk': [180, 192, 194, 240],
    'ledge_hop': [54, 72, 90, 104, 106, 142, 160],
    'step_up': [160, 168, 180, 352, 354],
    'step_down': [96, 102, 140, 164, 280, 330],
    'slope_walk': [102, 106, 108, 110, 150],
    'slope_hop': [20, 30, 34, 104],
    'angled_wall': [260, 300, 340, 400],
    'shallow_slope': [10, 40, 100, 170],
    'ledge_release_heldout': [72, 104, 106, 144, 174],
}


def main(captures=CAPTURES, prefix='backward_collision'):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=ROOT / f'validation/{prefix.replace("_", "-")}-smoke')
    parser.add_argument('--cases', default=','.join(captures))
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    report = {'cases': {}, 'status': 'running'}
    try:
        for case in args.cases.split(','):
            fields = captures[case]
            source = ROOT / f'game/tests/data/original/{prefix}_{case}'
            rows = list(csv.DictReader(source.with_suffix('.csv').open()))
            metadata = json.loads(source.with_suffix('.json').read_text())
            inputs = metadata['input']
            level_name = metadata['level']
            first = rows[0]
            reference = dict(REFERENCES['home' if level_name == 'assault' else 'wall'],
                             position=[int(first[c]) for c in ['x', 'y', 'z']], yaw=int(first['yaw']),
                             animation=int(first['animation']), absolute_animation_frame=int(first['animation_frame']), room=int(first['room']),
                             camera=[int(first[f'camera_{c}']) for c in 'xyz'],
                             target=[int(first[f'target_{c}']) for c in 'xyz'])
            client = Client(ROOT / 'target/debug/tomb-raider-ii', out / case, level_name)
            poses = report['cases'][case] = []
            try:
                client.wait_loaded()
                align_reference(client, reference)
                # Wall loads in its opening fall. These recordings all begin
                # at a settled idle checkpoint, as does the trajectory harness.
                client.edit('Player', 'Player', {'mode': 'Idle'})
                client.edit('Player', 'Motion', {
                    'grounded': True, 'velocity': [0.0, 0.0, 0.0],
                    'turn_rate': 0, 'turning': 0, 'air_animation': 0,
                    'locomotion_frame': 0.0, 'movement_heading': None,
                })
                tick = 0
                level = json.loads((ROOT / f'game/assets/levels/{level_name}/level.json').read_text())
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
                    original = ROOT / f'.psx/{prefix}_{case}/screens/f{field+4:06}.png'
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
