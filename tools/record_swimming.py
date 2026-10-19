#!/usr/bin/env python3
"""PSX-only submerged motion fixtures and replays. Never modifies level/code.

--setup retains every initial-position/pitch poke. Replays start from the
settled submerged checkpoint, with no further pokes. Independent cases are
first recorded after the matching submerged rules have been written.
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[1]
BASE='validation/swim/level_start/state.bin'
SAMPLES=[f'8017c8{offset}:i16:{name}' for offset,name in [
    ('8a','state'),('8c','goal'),('90','animation'),('92','animation_frame'),('94','room'),
    ('9a','speed'),('9c','swim_speed'),('9e','health'),('bc','pitch'),('be','yaw'),('c0','roll')]]
SAMPLES += ['8017c8b0:i32:x','8017c8b4:i32:y','8017c8b8:i32:z',
            '8008c570:i16:turn_rate','8008c4f4:i16:water_mode','8008c4fe:i16:air']
SAMPLES += [f'{base+4*i:x}:i32:{prefix}_{c}' for prefix,base in [('camera',0x8008b258),('target',0x8008b268)] for i,c in enumerate('xyz')]
CASES={
    'restart_heldout':(240,[{'frame':0,'buttons':['SQUARE'],'until':24},{'frame':38,'buttons':['SQUARE'],'until':60}],True),
    'left_up_heldout':(240,[{'frame':0,'buttons':['SQUARE'],'until':20},{'frame':20,'buttons':['SQUARE','LEFT'],'until':40},{'frame':40,'buttons':['SQUARE','DOWN'],'until':52}],True),
    'idle_reverse_heldout':(260,[{'frame':0,'buttons':['RIGHT','DOWN'],'until':56},{'frame':80,'buttons':['LEFT','UP'],'until':170}],True),
    'straight':(220,[{'frame':0,'buttons':['SQUARE'],'until':70}],False),
    'steer':(220,[{'frame':0,'buttons':['SQUARE'],'until':26},
        {'frame':26,'buttons':['SQUARE','RIGHT'],'until':46},
        {'frame':46,'buttons':['SQUARE','UP'],'until':62}],False),
    'idle_steer':(220,[{'frame':0,'buttons':['RIGHT','UP'],'until':110}],False),
}


def run(command):
    subprocess.run(['psx','run',*command,'--summary'],cwd=ROOT,check=True)


def setup():
    initial=['--from-state','.psx/home_start/state.bin','--frames','120',
        '--poke','8017c8b0=28160','--poke','8017c8b4=4608','--poke','8017c8b8=90624',
        '--poke','8017c894=14:2','--poke','8017c8be=0:2',
        '--out','validation/swim/entry','--save-state']
    for sample in SAMPLES:initial+=['--sample',sample]
    run(initial)
    level=['--from-state','validation/swim/entry/state.bin','--frames','2',
        '--poke','8017c8bc=0:2','--out','validation/swim/level_start','--save-state']
    for sample in SAMPLES:level+=['--sample',sample]
    run(level)
    data=dict(source='Supplied USA Rev3 disc via psx native; controlled initial spawn only',
        commands=[['psx','run',*initial,'--summary'],['psx','run',*level,'--summary']],
        seed_checkpoint='.psx/home_start/state.bin',
        seed_sha256=hashlib.sha256((ROOT/'.psx/home_start/state.bin').read_bytes()).hexdigest(),
        checkpoint=BASE,checkpoint_sha256=hashlib.sha256((ROOT/BASE).read_bytes()).hexdigest(),
        scope='Submerged motion; water-entry sequence is setup, not a parity claim')
    (ROOT/'game/tests/data/original/swimming_fixture.json').write_text(json.dumps(data,indent=2)+'\n')


def inspect_code():
    destination=ROOT/'validation/swim/code';destination.mkdir(parents=True,exist_ok=True)
    functions=[(0x44fc4,0x45384),(0x45384,0x45474),(0x45474,0x4552c),(0x4552c,0x455f4),(0x455f4,0x456dc),(0x45cb8,0x45ed4)]
    ram=(ROOT/'.psx/home_start/ram.bin').read_bytes()
    entries=[]
    for start,end in functions:
        command=['psx','disasm','--ram','.psx/home_start','--addr',hex(start),'--count',str((end-start)//4),'--text']
        result=subprocess.run(command,cwd=ROOT,text=True,capture_output=True,check=True)
        (destination/f'{start:05x}.txt').write_text(result.stdout)
        entries.append(dict(start=hex(start),end_exclusive=hex(end),sha256=hashlib.sha256(ram[start:end]).hexdigest(),command=command))
    record=dict(source='PSX Home runtime RAM obtained from supplied disc; byte ranges only',
                ram_sha256=hashlib.sha256(ram).hexdigest(),functions=entries,
                controls={str(state):hex(struct.unpack_from('<I',ram,0x82c98+4*state)[0]) for state in [13,17,18,35]},
                collisions={str(state):hex(struct.unpack_from('<I',ram,0x82de0+4*state)[0]) for state in [13,17,18,35]})
    (ROOT/'game/tests/data/original/swimming_code.json').write_text(json.dumps(record,indent=2)+'\n')


def record(name):
    fields,inputs,independent=CASES[name]
    out=ROOT/'validation/swim'/name
    command=['--from-state',BASE,'--frames',str(fields),'--input',json.dumps(inputs),
             '--out',str(out),'--snapshot-every','2','--save-state']
    for sample in SAMPLES:command+=['--sample',sample]
    run(command)
    report=json.loads((out/'report.json').read_text())
    assert report['status']=='complete' and not report.get('pokes')
    data=dict(source='Supplied USA Rev3 disc via psx native',checkpoint=BASE,
        checkpoint_sha256=hashlib.sha256((ROOT/BASE).read_bytes()).hexdigest(),
        command=['psx','run',*command,'--summary'],input=inputs,independent=independent,
        pokes=[],build=report['build'],scope='Submerged non-contact motion and authored animation',
        samples_sha256=hashlib.sha256((out/'samples.csv').read_bytes()).hexdigest())
    destination=ROOT/f'game/tests/data/original/swimming_{name}'
    destination.with_suffix('.csv').write_bytes((out/'samples.csv').read_bytes())
    destination.with_suffix('.json').write_text(json.dumps(data,indent=2)+'\n')


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--setup',action='store_true');p.add_argument('--inspect',action='store_true');p.add_argument('--case',choices=CASES)
    args=p.parse_args()
    if args.inspect:inspect_code()
    if args.setup:setup()
    if args.case:record(args.case)
