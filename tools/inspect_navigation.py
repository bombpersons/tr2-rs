#!/usr/bin/env python3
"""Retain local height-transition evidence from the supplied disc through psx."""
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'validation/winston-height/path-target'


def main():
    fixture = ROOT / 'game/tests/data/original/winston_contact_blocked_roll.json'
    metadata = json.loads(fixture.read_text())
    checkpoint = ROOT / metadata['checkpoint']
    assert hashlib.sha256(checkpoint.read_bytes()).hexdigest() == metadata['checkpoint_sha256']
    command = ['psx', 'run', '--from-state', str(checkpoint), '--frames', '26',
               '--input', json.dumps(metadata['input']), '--trace',
               '--trace-from-frame', '20', '--trace-frames', '4',
               '--trace-range', '80015dd4-80016354',
               '--trace-range', '8001518c-80015194', '--max-events', '10000',
               '--out', str(OUT), '--summary']
    subprocess.run(command, check=True, cwd=ROOT)
    report = json.loads((OUT / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == 26
    assert report['trace']['lines_dropped'] == 0
    ram = (OUT / 'ram.bin').read_bytes()
    read = lambda fmt, addr: struct.unpack_from('<'+fmt, ram, addr & 0x1fffff)
    pointer = lambda addr: read('I', addr)[0] & 0x1fffff
    trace = (OUT / 'trace.txt').read_text()
    box_base = int(re.search(r'00015e5c .*?=>([0-9a-f]+)', trace)[1], 16)
    overlap_base = int(re.search(r'00015190 .*?=>([0-9a-f]+)', trace)[1], 16)
    level = json.loads((ROOT / 'game/assets/imported/levels/assault/level.json').read_text())
    creature = pointer(0x17c8c8 + 0x30)
    lot = creature + 16
    step, drop, fly = read('3h', lot + 12)
    assert (step, drop, fly) == (256, -512, 0)
    boxes = []
    for index in (24, 138):
        values = read('4BhH', box_base + index * 8)
        box = dict(zip(('z_min', 'z_max', 'x_min', 'x_max', 'floor', 'overlap'), values))
        assert box == level['boxes'][index]
        boxes.append(dict(index=index, address=hex(box_base + index * 8), values=box))
    zone_base = pointer(0x8b4a0)
    zones = list(read('h'*len(level['boxes']), zone_base))
    assert zones == level['zones'][0]
    overlaps = list(read('H'*len(level['overlaps']), overlap_base))
    assert overlaps == level['overlaps']
    neighbors = []
    for entry in overlaps[level['boxes'][24]['overlap'] & 0x3fff:]:
        neighbors.append(entry & 0x7fff)
        if entry & 0x8000:
            break
    assert 138 in neighbors
    next_box = read('h', pointer(lot) + 24*8)[0]
    assert next_box == 138
    result = dict(source='Supplied disc; native navigation RAM and GetTarget trace only',
                  command=command, checkpoint=metadata['checkpoint'],
                  checkpoint_sha256=metadata['checkpoint_sha256'],
                  build=report['build'], identity=report['identity'],
                  trace_sha256=hashlib.sha256((OUT / 'trace.txt').read_bytes()).hexdigest(),
                  ram_sha256=hashlib.sha256(ram).hexdigest(),
                  creature_address=hex(creature), lot_address=hex(lot),
                  step=step, drop=drop, fly=fly, boxes=boxes,
                  next_box_from_24=next_box, neighbors_from_24=neighbors,
                  overlaps_base=hex(overlap_base),
                  overlaps_sha256=hashlib.sha256(ram[overlap_base:overlap_base+2*len(overlaps)]).hexdigest(),
                  zone_pointer_address='0x8b4a0', zone_base=hex(zone_base),
                  zone_table_sha256=hashlib.sha256(ram[zone_base:zone_base+2*len(zones)]).hexdigest(),
                  matching_baked_zone_tables=[i for i, z in enumerate(level['zones']) if z == zones],
                  zones_at_boxes={str(i): zones[i] for i in (24, 138)},
                  code_sha256={f'{a:x}-{b:x}': hashlib.sha256(ram[a:b]).hexdigest()
                               for a, b in ((0x150d8, 0x16354), (0x48ed0, 0x490ec))})
    (OUT / 'evidence.json').write_text(json.dumps(result, indent=2)+'\n')
    (ROOT / 'game/tests/data/original/winston_navigation_height.json').write_text(json.dumps(result, indent=2)+'\n')


if __name__ == '__main__':
    main()
