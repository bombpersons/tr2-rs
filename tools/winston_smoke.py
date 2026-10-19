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
from record_winston import CASES


def main():
    report = {'status': 'running', 'cases': {}}
    out = ROOT / 'validation/winston/smoke'
    out.mkdir(parents=True, exist_ok=True)
    level = json.loads((ROOT / 'game/assets/levels/assault/level.json').read_text())
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', choices=CASES)
    args = parser.parse_args()
    report_file = out / (f"report-{args.case}.json" if args.case else "report.json")
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
            if 'lara_movement_heading' in start:
                # The bootstrap tick used the level spawn's old heading; seed
                # the checkpoint's preceding collision pass before replaying.
                client.edit('Player','Motion',{'movement_heading':int(start['lara_movement_heading'])*math.tau/65536})
            yaw = int(start['actor_yaw']) * math.tau / 65536
            client.edit('Object260_9', 'Transform', {'translation': convert([int(start[f'actor_{c}']) for c in 'xyz']),
                        'rotation': [0, math.sin(yaw / 2), 0, math.cos(yaw / 2)]})
            clip = int(start['actor_animation'])
            client.edit('Object260_9', 'ObjectAnimation', {'animation': clip, 'frame': int(start['actor_animation_frame']) - level['animations'][clip]['frame_start']})
            client.edit('Object260_9', 'AnimationGoal', {'state': int(start['actor_goal'])})
            client.edit('Object260_9', 'Winston', {'speed': int(start['actor_speed'])})
            samples = [2,102,148,174,180,198,212,224,226,228,230,240,256,260,280] if name.startswith('wall_') else [2,8,16,36,38,40,42,78,88,90,94,96,98] if name.startswith('contact') else ([2,102,158,160,162,164,360,548,550,592,594,616,618,644,646,650] if name.startswith('nav') else [2,58,64,102,160,240,360])
            fields = sorted(set([f for f in samples if f < len(rows)] + [len(rows)-1]))
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
                qx,qy,qz,qw=t['rotation']
                yaw=round(math.atan2(2*(qw*qy+qx*qz),1-2*(qy*qy+qz*qz))*65536/math.tau)
                assert (yaw-int(row['actor_yaw']))%65536==0,(name,field,yaw,row)
                if 'target_x' in row:
                    assert [round(c['Winston']['target'][i]*1024) for i in [0,2]]==[int(row['target_x']),int(row['target_z'])],(name,field,c,row)
                if 'actor_contact_mask' in row:
                    assert c['Winston']['contact_mask']==int(row['actor_contact_mask']),(name,field,c,row,client.query(components=['Player','Motion','AnimationState','Traversal']))
                if name.startswith(('contact','wall_','nav')):
                    player=client.query(components=['Player','AnimationState'])['entities'][0]['components']
                    pos=player['Player']['position']
                    got_lara=[round(pos[0]*1024),round(-pos[1]*1024),round(pos[2]*1024)]
                    assert got_lara==[int(row[f'lara_{k}']) for k in 'xyz'],(name,field,got_lara,row)
                    la=player['AnimationState']
                    assert la['animation']==int(row['lara_animation'])
                    assert level['animations'][la['animation']]['frame_start']+la['frame']==int(row['lara_animation_frame'])
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
            report_file.write_text(json.dumps(report,indent=2)+'\n')
    report['status']='complete'
    report_file.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(status=report['status'], checks=sum(map(len,report['cases'].values())))))


if __name__ == '__main__':
    main()
