#!/usr/bin/env python3
"""Replay PSX submerged motion in the full standalone game over developer MCP."""
import argparse
import csv
import json
import math
import shutil
from port_smoke import Client,ROOT,align_reference,convert
from record_swimming import CASES


def binary(v):return (round(v*65536/math.tau)+32768)%65536-32768

def multiply(a,b):
    x,y,z,w=a;u,v,t,s=b
    return [w*u+x*s+y*t-z*v,w*v-x*t+y*s+z*u,w*t+x*v-y*u+z*s,w*s-x*u-y*v-z*t]


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--case',choices=CASES)
    args=p.parse_args();out=ROOT/'validation/swim/smoke'
    out.mkdir(parents=True,exist_ok=True)
    report={'status':'running','cases':{}}
    report_file=out/(f'report-{args.case}.json' if args.case else 'report.json')
    level=json.loads((ROOT/'game/assets/levels/assault/level.json').read_text())
    try:
        for name in [args.case] if args.case else CASES:
            rows=list(csv.DictReader((ROOT/f'game/tests/data/original/swimming_{name}.csv').open()))
            meta=json.loads((ROOT/f'game/tests/data/original/swimming_{name}.json').read_text())
            first=rows[0]
            client=Client(ROOT/'target/debug/tomb-raider-ii',out/name,'assault')
            poses=report['cases'][name]=[]
            try:
                client.wait_loaded()
                ref=dict(level='assault',position=[int(first[k]) for k in 'xyz'],yaw=int(first['yaw']),room=int(first['room']),
                    animation=int(first['animation']),absolute_animation_frame=int(first['animation_frame']),
                    camera=[int(first['camera_'+k]) for k in 'xyz'],target=[int(first['target_'+k]) for k in 'xyz'])
                align_reference(client,ref)
                client.edit('Player','Player',{'mode':'Swimming','health':int(first['health'])})
                client.edit('Player','Motion',{'grounded':False,'velocity':[0,0,0],
                    'pitch':int(first['pitch'])*math.tau/65536,'turn_rate':int(first['turn_rate']),
                    'turning':0,'movement_heading':None,'oxygen_seconds':int(first['air'])/30})
                client.edit('Player','SubmergedMotion',{'speed':int(first['swim_speed'])*30/4096,
                    'bank':int(first['roll'])*math.tau/65536,'air_ticks':int(first['air'])})
                client.edit('Player','Traversal',{'goal':int(first['goal'])})
                fields=sorted(set([f for f in [2,4,14,26,40,44,46,52,62,70,72,74,86,88,92,94,110,112,140,180] if f<len(rows)-4]+[len(rows)-5]))
                tick=0
                for field in fields:
                    while 2*tick<field:
                        buttons=[b.lower() for e in meta['input'] if e['frame']<=2*tick<e['until'] for b in e['buttons']]
                        boundaries=[e[k] for e in meta['input'] for k in ['frame','until'] if e[k]>2*tick]
                        end=min([field,*boundaries]);count=(end-2*tick)//2
                        client.call('game_step',{'frames':count,'buttons':buttons});tick+=count
                    row=rows[field]
                    state=client.query(components=['Player','Motion','SubmergedMotion','AnimationState','Traversal','Transform'])
                    c=state['entities'][0]['components'];actor=c['Player'];motion=c['Motion'];water=c['SubmergedMotion'];a=c['AnimationState']
                    actual=dict(x=round(actor['position'][0]*1024),y=round(-actor['position'][1]*1024),z=round(actor['position'][2]*1024),
                        yaw=binary(actor['yaw']),pitch=binary(motion['pitch']),roll=binary(water['bank']),turn_rate=motion['turn_rate'],
                        swim_speed=round(water['speed']*4096/30),animation=a['animation'],animation_frame=level['animations'][a['animation']]['frame_start']+a['frame'],
                        state=level['animations'][a['animation']]['state'],goal=c['Traversal']['goal'],room=actor['room'],air=water['air_ticks'],health=actor['health'])
                    for key,value in actual.items():assert value==int(row[key]),(name,field,key,value,row[key])
                    yaw,pitch,bank=[int(row[k])*math.tau/65536 for k in ['yaw','pitch','roll']]
                    expected=multiply(multiply([0,math.sin(yaw/2),0,math.cos(yaw/2)],[-math.sin(pitch/2),0,0,math.cos(pitch/2)]),[0,0,-math.sin(bank/2),math.cos(bank/2)])
                    got=c['Transform']['rotation']
                    assert abs(abs(sum(x*y for x,y in zip(expected,got)))-1)<1e-5,(name,field,expected,got)
                    client.edit('ChaseCamera','CameraRig',{'manual':True,'position':convert([int(row['camera_'+k]) for k in 'xyz']),
                        'target':convert([int(row['target_'+k]) for k in 'xyz'])})
                    capture=client.capture(f'f{field:04}',min_colors=16)
                    original=ROOT/f'validation/swim/{name}/screens/f{field+4:06}.png'
                    target=client.out/f'f{field:04}-original.png';shutil.copyfile(original,target)
                    poses.append(dict(field=field,state=state,image=capture,original=str(target.relative_to(ROOT))))
            finally:client.close()
        report['status']='complete'
    finally:report_file.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(status=report['status'],checks=sum(map(len,report['cases'].values())))))


if __name__=='__main__':main()
