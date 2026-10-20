#!/usr/bin/env python3
"""Retain native GetSpheres output at the raised-corner mask diagnostic.

Only the supplied disc's prepared checkpoints and psx runtime are evidence.
No pose arithmetic is reconstructed here.
"""
import csv
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
STORES = {0x5bc58, 0x5bc6c, 0x5bc80, 0x5bc94,
          0x5bda4, 0x5bdbc, 0x5bdd0, 0x5bde4}


def signed(value, bits):
    return value - (1 << bits) if value & (1 << (bits - 1)) else value


def record(name):
    fixture = ROOT / f'game/tests/data/original/winston_{name}'
    metadata = json.loads(fixture.with_suffix('.json').read_text())
    assert hashlib.sha256((ROOT / metadata['checkpoint']).read_bytes()).hexdigest() == metadata['checkpoint_sha256']
    rows = list(csv.DictReader(fixture.with_suffix('.csv').open()))
    out = ROOT / 'validation/item-push' / f'{name}-spheres'
    command = ['psx', 'run', '--from-state', metadata['checkpoint'],
               '--frames', '74', '--input', json.dumps(metadata['input']),
               '--trace', '--trace-from-frame', '70', '--trace-frames', '4',
               '--trace-range', '5BAF8-5BE3C', '--max-events', '30000',
               '--out', str(out), '--summary']
    subprocess.run(command, check=True, cwd=ROOT)
    report = json.loads((out / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == 74
    assert report['trace']['lines_dropped'] == 0
    calls, current, values = [], None, []
    for line in (out / 'trace.txt').read_text().splitlines():
        parts = line.split()
        if len(parts) < 4 or not parts[0].isdigit():
            continue
        pc = int(parts[2], 16)
        if pc == 0x5bb00:
            item = int(re.search(r's1=([0-9a-f]+)', line)[1], 16)
            assert item in (0x17c8c8, 0x17c87c)
            field = int(parts[0]) - report['frames']['start']
            row = rows[field + 1]
            prefix = 'actor' if item == 0x17c8c8 else 'lara'
            assert not values
            current = dict(kind=260 if prefix == 'actor' else 0,
                           run_field=field, animation=int(row[prefix+'_animation']),
                           absolute_frame=int(row[prefix+'_animation_frame']),
                           position=[0, 0, 0], yaw=0, spheres=[])
            calls.append(current)
        if current is None:
            continue
        if pc in {0x5bb40, 0x5bb44, 0x5bb4c, 0x5bb94}:
            value = int(re.search(r'=>[0-9a-f]+', line)[0][2:], 16)
            if pc == 0x5bb94:
                current['yaw'] = signed(value, 16)
            else:
                current['position'][{0x5bb40: 1, 0x5bb44: 0, 0x5bb4c: 2}[pc]] = signed(value, 32)
        if pc in STORES:
            values.append(signed(int(re.search(r'\]=([0-9a-f]+)', line)[1], 16), 32))
            if len(values) == 4:
                current['spheres'].append(values)
                values = []
    assert [len(call['spheres']) for call in calls] == [26, 15, 26, 15]
    cases = []
    for i in range(0, len(calls), 2):
        actor, lara = calls[i:i+2]
        mask = sum(1 << joint for joint, a in enumerate(actor['spheres'])
                   if any(sum((a[k]-b[k])**2 for k in range(3)) < (a[3]+b[3])**2
                          for b in lara['spheres']))
        assert mask == int(rows[actor['run_field']+1]['actor_contact_mask'])
        cases.append(dict(calls=[actor, lara], native_mask=mask))
    return dict(route=name, command=command, checkpoint=metadata['checkpoint'],
                checkpoint_sha256=metadata['checkpoint_sha256'],
                build=report['build'], identity=report['identity'],
                trace_sha256=hashlib.sha256((out / 'trace.txt').read_bytes()).hexdigest(),
                cases=cases)


if __name__ == '__main__':
    captures = [record(name) for name in
                ('contact_blocked_corner', 'contact_blocked_offset_heldout')]
    destination = ROOT / 'game/tests/data/original/winston_corner_spheres.json'
    destination.write_text(json.dumps(dict(
        source='Native GetSpheres output stores, PSX 0x8005BAF8; controlled placement',
        captures=captures), indent=2)+'\n')
