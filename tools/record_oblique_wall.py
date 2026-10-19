#!/usr/bin/env python3
"""Reproduce the PSX frontal oblique-wall shift; export the observed stores.

The route is the former complete-game navigation diagnostic. Independent
collision trajectories are recorded with tools/record_winston.py wall_*.
"""
import csv
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'validation/oblique-wall/calibration-trace'


def main():
    meta=json.loads((ROOT/'game/tests/data/original/winston_nav_turn_heldout.json').read_text())
    command=['psx','run','--from-state',meta['checkpoint'],'--frames','270',
             '--input',json.dumps(meta['input']),'--out',str(OUT),'--trace',
             '--trace-from-frame','224','--trace-frames','2','--watch','8017c8b0:12',
             '--watch-changes-only','--max-events','20000','--summary']
    for interval in ['3A844-3AA20','1B018-1BC00','1C0AC-1C0E8','3C544-3C5F4']:
        command+=['--trace-range',interval]
    subprocess.run(command,cwd=ROOT,check=True)
    (OUT/'command.json').write_text(json.dumps(command,indent=2)+'\n')
    report=json.loads((OUT/'report.json').read_text())
    assert report['trace']['lines_dropped']==0 and report['watch']['complete']
    writes=[json.loads(line) for line in (OUT/'writes.jsonl').read_text().splitlines()]
    contact=[w for w in writes if w['run_frame']==225]
    trace=(OUT/'trace.txt').read_text()
    stores=[]
    for line in trace.splitlines():
        match=re.search(r'^\d+ \d+ (0001b838|0001b840|0001b9b4|0001c0ac|0001c0d4) .*; \[([0-9a-f]+)\]=([0-9a-f]+)',line)
        if match:
            value=int(match[3],16)
            if value&0x80000000:value-=1<<32
            stores.append(dict(pc='0x'+match[1],address='0x'+match[2],value=value))
    assert [(s['pc'],s['value']) for s in stores]==[
        ('0x0001b838',39),('0x0001b840',14),('0x0001b9b4',1),('0x0001c0ac',16485),('0x0001c0d4',95550)]
    rows=list(csv.DictReader((ROOT/'game/tests/data/original/winston_nav_turn_heldout.csv').open()))
    proposed=[next((w['value'] for w in contact if w['address']==address),int(rows[224]['lara_'+axis])) for address,axis in [(0x17c8b0,'x'),(0x17c8b4,'y'),(0x17c8b8,'z')]]
    assert proposed==[16446,2560,95536]
    result=dict(source='Supplied USA Rev3 disc through psx native runtime, no pokes',
                calibration_for='Frontal oblique run wall tangent restoration; nav_turn was not held out for this correction',
                command=command,checkpoint_sha256=meta['checkpoint_sha256'],build=report['build'],
                trace_sha256=hashlib.sha256((OUT/'trace.txt').read_bytes()).hexdigest(),
                writes_sha256=hashlib.sha256((OUT/'writes.jsonl').read_bytes()).hexdigest(),
                trace_complete=report['trace']['lines_dropped']==0,watch_complete=report['watch']['complete'],
                run_field=225,prior_position=[int(rows[224]['lara_'+c]) for c in 'xyz'],
                proposed_position=proposed,observed_context_stores=stores,observed_position_writes=contact,
                settled_position=[int(rows[226]['lara_'+c]) for c in 'xyz'])
    destination=ROOT/'game/tests/data/original/oblique_wall_trace.json'
    destination.write_text(json.dumps(result,indent=2)+'\n')
    print(destination.relative_to(ROOT))


if __name__=='__main__':main()
