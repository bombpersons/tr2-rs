#!/usr/bin/env python3
"""Extract PSX water light tables and native per-corner shading observations."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def record(heldout):
    name = 'heldout' if heldout else 'calibration'
    out = ROOT/'validation/caustics'/name
    command = ['psx', 'run', '--from-state', 'validation/swim/level_start/state.bin',
               '--frames', '8', '--trace', '--trace-range', '62734-62790',
               '--trace-range', '6d1f4-6d210', '--watch', '89cd0',
               '--sample', '89cd0:i32:phase', '--snapshot-every', '2',
               '--max-events', '100000', '--out', str(out), '--summary']
    if heldout:
        command += ['--poke', '89cd0=17']
    subprocess.run(command, cwd=ROOT, check=True)
    report = json.loads((out/'report.json').read_text())
    assert report['status'] == 'complete' and report['trace']['lines_dropped'] == 0
    ram = (out/'ram.bin').read_bytes()
    wave = list(struct.unpack_from('<64b', ram, 0x8b4c0))
    phases = list(ram[0x8b500:0x8b540])
    # Binary 0x21024 seeds draw randomness; 0x6d178 builds the table.
    seed = 0xd371f947
    generated = []
    for _ in range(64):
        seed = (seed*0x41c64e6d+12345)&0xffffffff
        generated.append(((seed>>10)&0x7fff)//512)
    assert generated == phases
    observations = []
    phase = index = base = None
    for line in (out/'trace.txt').read_text().splitlines():
        m = re.match(r'(\d+) \d+ ([0-9a-f]+) ', line)
        if not m:
            continue
        frame, pc = int(m[1]), int(m[2], 16)
        if pc == 0x6d1f4:
            phase = int(line.split('=>')[-1], 16)
        elif pc == 0x62734:
            index = int(line.split('=>')[-1], 16)//4
        elif pc == 0x62768:
            base = int(line.split('=>')[-1], 16)
        elif pc == 0x62788:
            value = int(line.rsplit('=', 1)[-1], 16)
            observations.append(dict(frame=frame, phase=phase, vertex=index, base=base, shaded=value))
    assert len(observations) > 500
    for item in observations:
        expected = max(0, min(255, item['base']+wave[(phases[item['vertex']%64]-item['phase']-1)%64]))
        assert expected == item['shaded'], item
    # Retain every distinct observed input/result, rather than repeated faces.
    unique = {tuple(item[k] for k in ['phase', 'vertex', 'base', 'shaded']): item for item in observations}
    ranges = [(0x21024, 0x2105c), (0x6d178, 0x6d220), (0x6d234, 0x6d248),
              (0x6e54c, 0x6e580), (0x62734, 0x62794), (0x61a08, 0x61a18), (0x61d34, 0x61d44)]
    code = []
    for start, end in ranges:
        args = ['psx', 'disasm', '--ram', str(out), '--addr', hex(start), '--count', str((end-start)//4), '--text']
        listing = subprocess.check_output(args, cwd=ROOT, text=True)
        (out/f'{start:05x}.txt').write_text(listing)
        code.append(dict(start=hex(start), end_exclusive=hex(end), sha256=hashlib.sha256(ram[start:end]).hexdigest(), command=args))
    fixture = dict(source='Supplied USA Rev3 disc via psx native', independent=heldout,
                   command=command, pokes=report.get('pokes', []), code=code,
                   table_sha256=hashlib.sha256(ram[0x8b4c0:0x8b540]).hexdigest(),
                   wave=wave, vertex_phase=phases, native_observations=len(observations),
                   observations=list(unique.values()))
    phase_out = ROOT/'validation/caustics'/f'phase-{name}'
    phase_command = ['psx', 'run', '--from-state', 'validation/swim/level_start/state.bin',
                     '--frames', '140', '--sample', '89cd0:i32:phase', '--out', str(phase_out), '--summary']
    if heldout:
        phase_command += ['--poke', '89cd0=17']
    subprocess.run(phase_command, cwd=ROOT, check=True)
    phase_bytes = (phase_out/'samples.csv').read_bytes()
    (ROOT/f'game/tests/data/original/caustics_phase_{name}.csv').write_bytes(phase_bytes)
    fixture['phase_trajectory'] = dict(command=phase_command, initial_phase=17 if heldout else 0,
                                     samples_sha256=hashlib.sha256(phase_bytes).hexdigest())
    (ROOT/f'game/tests/data/original/caustics_{name}.json').write_text(json.dumps(fixture, indent=2)+'\n')
    if not heldout:
        (ROOT/'game/data/caustics.json').write_text(json.dumps(dict(wave=wave, vertex_phase=phases), indent=2)+'\n')
    print(json.dumps(dict(case=name, native=len(observations), unique=len(unique))))


if __name__ == '__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--heldout', action='store_true')
    record(p.parse_args().heldout)
