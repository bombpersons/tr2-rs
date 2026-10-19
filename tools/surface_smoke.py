#!/usr/bin/env python3
"""Replay PSX surface-swimming routes in the full standalone game over developer MCP.

Run after `cargo build -p tomb-raider-ii --features dev-mcp`. Each route starts
from its first recorded row, replays the same pad input and checks complete-game
poses; the camera is aligned to the recorded PSX camera for side-by-side frames.
"""
import argparse
import csv
import json
import math
import shutil
from port_smoke import Client, ROOT, align_reference, convert
from record_surface import CASES

# Routes and last fields the headless tests claim (see game/tests/surface.rs).
LIMITS = {'entry_hop_heldout': 62, 'entry_angled_hop_heldout': 82}
MODES = {0: 'Idle', 1: 'Swimming', 2: 'Surface'}


def binary(v):
    return (round(v * 65536 / math.tau) + 32768) % 65536 - 32768


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--case', choices=CASES, action='append')
    args = p.parse_args()
    out = ROOT / 'validation/surface/smoke'
    out.mkdir(parents=True, exist_ok=True)
    report = {'status': 'running', 'cases': {}}
    level = json.loads((ROOT / 'game/assets/levels/assault/level.json').read_text())
    try:
        for name in args.case or CASES:
            rows = list(csv.DictReader((ROOT / f'game/tests/data/original/surface_{name}.csv').open()))
            meta = json.loads((ROOT / f'game/tests/data/original/surface_{name}.json').read_text())
            first = rows[0]
            mode = int(first['water_mode'])
            client = Client(ROOT / 'target/debug/tomb-raider-ii', out / name, 'assault')
            poses = report['cases'][name] = []
            try:
                client.wait_loaded()
                ref = dict(level='assault', position=[int(first[k]) for k in 'xyz'], yaw=int(first['yaw']),
                           room=int(first['room']), animation=int(first['animation']),
                           absolute_animation_frame=int(first['animation_frame']),
                           camera=[int(first['camera_' + k]) for k in 'xyz'],
                           target=[int(first['target_' + k]) for k in 'xyz'])
                align_reference(client, ref)
                client.edit('Player', 'Player', {'mode': MODES[mode], 'health': int(first['health'])})
                client.edit('Player', 'Motion', {'grounded': mode == 0, 'velocity': [0, 0, 0],
                                                 'pitch': int(first['pitch']) * math.tau / 65536, 'turn_rate': 0,
                                                 'turning': 0, 'movement_heading': int(first['yaw']) * math.tau / 65536,
                                                 'oxygen_seconds': int(first['air']) / 30})
                client.edit('Player', 'SubmergedMotion', {'speed': int(first['swim_speed']) * 30 / 4096,
                                                          'bank': 0.0, 'air_ticks': int(first['air']), 'dive_hold': 0})
                client.edit('Player', 'Traversal', {'goal': int(first['goal'])})
                last = min(LIMITS.get(name, len(rows) - 1), len(rows) - 5)
                fields = sorted({f for f in range(6, last + 1, 30)} | {last - last % 2})
                tick = 0
                for field in fields:
                    while 2 * tick < field:
                        buttons = [b.lower() for e in meta['input'] if e['frame'] <= 2 * tick < e['until'] for b in e['buttons']]
                        boundaries = [e[k] for e in meta['input'] for k in ['frame', 'until'] if e[k] > 2 * tick]
                        end = min([field, *boundaries])
                        count = (end - 2 * tick) // 2
                        client.call('game_step', {'frames': count, 'buttons': buttons})
                        tick += count
                    row = rows[field]
                    state = client.query(components=['Player', 'Motion', 'SubmergedMotion', 'AnimationState', 'Traversal'])
                    c = state['entities'][0]['components']
                    actor, water, a = c['Player'], c['SubmergedMotion'], c['AnimationState']
                    clip = level['animations'][a['animation']]
                    actual = dict(x=round(actor['position'][0] * 1024), y=round(-actor['position'][1] * 1024),
                                  z=round(actor['position'][2] * 1024), yaw=binary(actor['yaw']),
                                  animation=a['animation'], animation_frame=clip['frame_start'] + a['frame'],
                                  state=clip['state'], room=actor['room'])
                    if int(row['water_mode']):
                        actual['swim_speed'] = round(water['speed'] * 4096 / 30)
                    for key, value in actual.items():
                        assert value == int(row[key]), (name, field, key, value, row[key])
                    client.edit('ChaseCamera', 'CameraRig', {'manual': True,
                                                             'position': convert([int(row['camera_' + k]) for k in 'xyz']),
                                                             'target': convert([int(row['target_' + k]) for k in 'xyz'])})
                    capture = client.capture(f'f{field:04}', min_colors=16)
                    entry = dict(field=field, actual=actual, image=capture)
                    original = ROOT / f'validation/surface/{name}/screens/f{field + 4:06}.png'
                    if original.exists():
                        target = client.out / f'f{field:04}-original.png'
                        shutil.copyfile(original, target)
                        entry['original'] = str(target.relative_to(ROOT))
                    poses.append(entry)
            finally:
                client.close()
        report['status'] = 'complete'
    finally:
        (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(dict(status=report['status'], checks=sum(map(len, report['cases'].values())))))


if __name__ == '__main__':
    main()
