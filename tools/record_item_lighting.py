#!/usr/bin/env python3
"""Retain the first avatar light calculation in Home and Wall, using psx only."""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CASES = [('assault', 'home_start', 'item_light_home_complete'), ('wall', 'wall_game', 'item_light_wall')]


def signed(value):
    return value if value < 2**31 else value - 2**32


def summarize(level, checkpoint, run, command):
    report = json.loads((ROOT / run / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == 2
    assert report['trace']['lines_dropped'] == 0
    trace_path = ROOT / run / 'trace.txt'
    trace = trace_path.read_text()
    # Both selected rooms have one static light and take the mode-0 path.
    # Reconstruct the input Z from the observed light Z and subtraction.
    first = trace.split('-> 00064954', 1)[0]

    def value(pc, pattern):
        line = next(line for line in first.splitlines() if len(line.split()) > 2 and line.split()[2] == pc)
        return signed(int(re.search(pattern, line).group(1), 16))

    point = [value('0006c6f8', r't8=([0-9a-f]+)'), value('0006c6fc', r't9=([0-9a-f]+)'),
             value('0006c778', r'=>\s*([0-9a-f]+)') + value('0006c780', r'v1=([0-9a-f]+)')]
    ambient = value('0006cb6c', r'\]=([0-9a-f]+)')
    strength = value('0006cb1c', r't3=([0-9a-f]+)')
    denominator = value('0006cb74', r'\]=([0-9a-f]+)')
    room = value('0006c704', r'a3=([0-9a-f]+)')
    mesh = trace.split('-> 00064954', 1)[1].split('-> 00064954', 1)[0]

    def mesh_value(pc):
        line = next(line for line in mesh.splitlines() if len(line.split()) > 2 and line.split()[2] == pc)
        return signed(int(re.search(r'=>\s*([0-9a-f]+)', line).group(1), 16))

    columns = [[mesh_value(pc) for pc in column] for column in [
        ['0006497c', '0006498c', '000649a4'], ['000649c8', '000649dc', '000649f0'],
        ['00064a14', '00064a28', '00064a3c'],
    ]]
    direction = [mesh_value(pc) for pc in ['00064978', '00064988', '000649a0']]
    vertices = []
    normal = []
    for line in mesh.splitlines():
        words = line.split()
        if len(words) < 3:
            continue
        if words[2] in ['00064a64', '00064a68', '00064a74']:
            raw = int(re.search(r'=>\s*([0-9a-f]+)', line).group(1), 16) & 0xffff
            normal.append(raw if raw < 32768 else raw - 65536)
        elif words[2] == '00064ac0':
            assert len(normal) == 3
            vertices.append({'normal': normal, 'shade': int(re.search(r'\]=([0-9a-f]+)', line).group(1), 16)})
            normal = []
    return {
        'level': level, 'point': point, 'room': room, 'ambient': ambient, 'strength': strength,
        'directional_denominator': denominator, 'command': command, 'build': report['build'],
        'mesh_matrix_columns': columns, 'view_light_direction': direction, 'vertices': vertices,
        'frames': report['frames'], 'checkpoint': checkpoint,
        'checkpoint_sha256': hashlib.sha256((ROOT / checkpoint).read_bytes()).hexdigest(),
        'trace': str(trace_path.relative_to(ROOT)), 'trace_sha256': hashlib.sha256(trace_path.read_bytes()).hexdigest(),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--psx', default='psx')
    parser.add_argument('--summarize-only', action='store_true')
    args = parser.parse_args()
    cases = []
    for level, checkpoint_name, run_name in CASES:
        checkpoint = f'.psx/{checkpoint_name}/state.bin'
        run = f'.psx/{run_name}'
        command = [args.psx, 'run', '--from-state', checkpoint, '--frames', '2',
                   '--input', '[{"frame":0,"buttons":[]}]', '--trace',
                   '--trace-range', '8006c6f4-8006cc14', '--trace-range', '80064954-80064b18',
                   '--max-events', '16000', '--out', run, '--summary']
        if not args.summarize_only:
            subprocess.run(command, cwd=ROOT, check=True)
        cases.append(summarize(level, checkpoint, run, command))
    result = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native, no pokes',
        'method': 'First CalculateLight call before the first avatar mesh; input Z reconstructed from its observed light-Z subtraction',
        'cases': cases,
    }
    path = ROOT / 'game/tests/data/original/item_lighting.json'
    path.write_text(json.dumps(result, indent=2) + '\n')
    print(path.relative_to(ROOT))


if __name__ == '__main__':
    main()
