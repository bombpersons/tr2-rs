#!/usr/bin/env python3
"""Replay the attendant's local follow in the real standalone Bevy game.

The PSX recorder retains camera samples and screenshots for local_run.
Other cases compare the complete gameplay replay; no avatar positions are
injected after the starting alignment. Captures preserve the MCP transcript.
"""
import argparse
import csv
import json
import math
import shutil
from port_smoke import Client, ROOT, REFERENCES, align_reference, convert


def main():
    report = {'status': 'running', 'cases': {}}
    out = ROOT / 'validation/winston/smoke'
    out.mkdir(parents=True, exist_ok=True)
    level = json.loads((ROOT / 'game/assets/levels/assault/level.json').read_text())
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', choices=['local_run', 'heldout_short', 'heldout_turn', 'heldout_walk'])
    args = parser.parse_args()
    for name in ([args.case] if args.case else ['local_run', 'heldout_short', 'heldout_turn', 'heldout_walk']):
        rows = list(csv.DictReader((ROOT / f'game/tests/data/original/winston_{name}.csv').open()))
        meta = json.loads((ROOT / f'game/tests/data/original/winston_{name}.json').read_text())
        start = rows[0]
        client = Client(ROOT / 'target/debug/tomb-raider-ii', out / name, 'assault')
        checks = report['cases'][name] = []
        try:
            client.wait_loaded()
            client.call("game_step", {"frames": 1, "buttons": []})
            ref = dict(REFERENCES['home'],
                       position=[int(start[f'lara_{c}']) for c in 'xyz'], yaw=int(start['lara_yaw']),
                       animation=int(start['lara_animation']), absolute_animation_frame=int(start['lara_animation_frame']))
            align_reference(client, ref)
            yaw = int(start['actor_yaw']) * math.tau / 65536
            client.edit('Object260_9', 'Transform', {'translation': convert([int(start[f'actor_{c}']) for c in 'xyz']),
                        'rotation': [0, math.sin(yaw / 2), 0, math.cos(yaw / 2)]})
            clip = int(start['actor_animation'])
            client.edit('Object260_9', 'ObjectAnimation', {'animation': clip, 'frame': int(start['actor_animation_frame']) - level['animations'][clip]['frame_start']})
            client.edit('Object260_9', 'AnimationGoal', {'state': int(start['actor_goal'])})
            client.edit('Object260_9', 'Winston', {'speed': int(start['actor_speed'])})
            fields = sorted(set([2, 58, 64, 102, 160, 240, 360, len(rows)-1]))
            tick = 0
            for field in fields:
                while 2*tick < field:
                    buttons = [b.lower() for event in meta['input'] if event['frame'] <= 2*tick < event['until'] for b in event['buttons']]
                    # Stop the batch at an input or requested capture boundary.
                    end = min(field, *(e[k] for e in meta['input'] for k in ['frame', 'until'] if e[k] > 2*tick)) if any(e[k] > 2*tick for e in meta['input'] for k in ['frame', 'until']) else field
                    count = (end - 2*tick)//2
                    client.call('game_step', {'frames': count, 'buttons': buttons})
                    tick += count
                row = rows[field]
                state = client.query(name='Object260_9', components=['Winston', 'Transform', 'ObjectAnimation', 'AnimationGoal', 'WorldObject'])
                c = next(e for e in state['entities'] if e['name']=='Object260_9')['components']
                t, a = c['Transform'], c['ObjectAnimation']
                got = [round(t['translation'][0]*1024), round(-t['translation'][1]*1024), round(t['translation'][2]*1024)]
                assert got == [int(row[f'actor_{k}']) for k in 'xyz'], (name, field, got, row)
                assert a['animation'] == int(row['actor_animation']), (name, field, a, row)
                assert level['animations'][a['animation']]['frame_start'] + a['frame'] == int(row['actor_animation_frame'])
                assert c['Winston']['speed'] == int(row['actor_speed'])
                assert c['AnimationGoal']['state'] == int(row['actor_goal'])
                if 'camera_x' in row:
                    client.edit('ChaseCamera', 'CameraRig', {'manual': True,
                                'position': convert([int(row[f'camera_{k}']) for k in 'xyz']),
                                'target': convert([int(row[f'look_{k}']) for k in 'xyz'])})
                capture = client.capture(f'f{field:04}')
                entry = dict(field=field, state=state, image=capture)
                original = ROOT / f'validation/winston/{name}/screens/f{field+4:06}.png'
                if original.exists():
                    target = client.out / f'f{field:04}-original.png'
                    shutil.copyfile(original, target)
                    entry['original'] = str(target.relative_to(ROOT))
                client.edit('ChaseCamera', 'CameraRig', {'manual': True,
                            'position': convert([got[0]+1800, got[1]-1400, got[2]+1800]),
                            'target': convert([got[0], got[1]-450, got[2]])})
                entry['attendant_view'] = client.capture(f'f{field:04}-attendant')
                checks.append(entry)
        finally:
            client.close()
            (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    report['status']='complete'
    (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(status=report['status'], checks=sum(map(len,report['cases'].values())))))


if __name__ == '__main__':
    main()
