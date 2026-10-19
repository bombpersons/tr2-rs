#!/usr/bin/env python3
"""Verify the supplied PSX runtime's paired water/normal RGB555 palettes."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import csv
from record_swimming import SAMPLES

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'validation/underwater-color'


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    checkpoints = [('dry', '.psx/home_start/state.bin'), ('underwater', 'validation/swim/level_start/state.bin')]
    captures = []
    representatives = {}
    for name, checkpoint in checkpoints:
        destination = OUT / name
        command = ['psx', 'run', '--from-state', checkpoint, '--frames', '2', '--gpu-frame', '1',
                   '--trace', '--trace-range', '6D1F4-6D220', '--watch', '8c250',
                   '--sample', '8b264:i16:camera_room', '--sample', '8c250:i32:camera_water',
                   '--out', str(destination), '--summary']
        subprocess.run(command, cwd=ROOT, check=True)
        raw = (destination / 'vram.bin').read_bytes()
        words = struct.unpack('<524288H', raw)
        packed = bytearray()
        for y in range(384, 512):
            for x in range(960, 1024):
                normal, water = words[y * 1024 + x], words[(y - 128) * 1024 + x]
                assert water == (normal & ~31) | ((normal & 31) >> 1), (name, x, y)
                representatives.setdefault(normal & 31, dict(x=x, normal_y=y, underwater_y=y-128,
                                                            normal=normal, underwater=water))
                packed += struct.pack('<HH', normal, water)
        captures.append(dict(name=name, checkpoint=checkpoint, command=command,
                             checkpoint_sha256=hashlib.sha256((ROOT/checkpoint).read_bytes()).hexdigest(),
                             vram_sha256=hashlib.sha256(raw).hexdigest(), palette_pairs=8192,
                             pairs_sha256=hashlib.sha256(packed).hexdigest()))
    assert set(representatives) == set(range(32))
    code_ranges = [(0x23f14, 0x23f24), (0x24060, 0x240b0), (0x24fd0, 0x25004),
                   (0x251bc, 0x251f0), (0x6d1f4, 0x6d220), (0x617b0, 0x61808), (0x61934, 0x61960)]
    ram = (OUT/'dry/ram.bin').read_bytes()
    functions = []
    for start, end in code_ranges:
        command = ['psx', 'disasm', '--ram', str(OUT/'dry'), '--addr', hex(start),
                   '--count', str((end-start)//4), '--text']
        listing = subprocess.check_output(command, cwd=ROOT, text=True)
        (OUT/f'{start:05x}.txt').write_text(listing)
        functions.append(dict(start=hex(start), end_exclusive=hex(end),
                              sha256=hashlib.sha256(ram[start:end]).hexdigest(), command=command))
    fixture = dict(source='Supplied USA Rev3 disc, psx native runtime VRAM and code',
                   rule='Underwater red5=floor(normal red5/2); green5, blue5 and STP preserved',
                   selection='Room water flag OR camera room water flag; caustics separate and unported',
                   captures=captures, code=functions,
                   representatives=[representatives[i] for i in range(32)])
    (ROOT/'game/tests/data/original/underwater_palette.json').write_text(json.dumps(fixture, indent=2)+'\n')
    # Fresh view recorded after the palette implementation; no pokes.
    destination = OUT / 'view-heldout'
    inputs = [dict(frame=0, until=18, buttons=['SQUARE', 'LEFT']),
              dict(frame=18, until=32, buttons=['SQUARE', 'DOWN'])]
    command = ['psx', 'run', '--from-state', 'validation/swim/level_start/state.bin',
               '--frames', '48', '--input', json.dumps(inputs), '--snapshot-every', '2',
               '--out', str(destination), '--summary']
    for sample in SAMPLES:
        command += ['--sample', sample]
    subprocess.run(command, cwd=ROOT, check=True)
    row = list(csv.DictReader((destination/'samples.csv').open()))[36]
    view = dict(source='Supplied USA Rev3 disc via psx native', independent=True,
                command=command, pokes=[], sample_field=36, screenshot_field=40,
                screenshot='validation/underwater-color/view-heldout/screens/f000040.png',
                sample={key: int(value) for key, value in row.items()},
                samples_sha256=hashlib.sha256((destination/'samples.csv').read_bytes()).hexdigest())
    (ROOT/'game/tests/data/original/underwater_palette_view.json').write_text(json.dumps(view, indent=2)+'\n')
    print('Verified 16384 palette pairs, covering all 32 red levels')


if __name__ == '__main__':
    main()
