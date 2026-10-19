#!/usr/bin/env python3
"""Retain type260 evidence from the supplied disc's PSX RAM/checkpoint only.
No game source, external reimplementations or web evidence is consulted.
"""
import argparse
import csv
import re
import hashlib
import json
from pathlib import Path
import struct
import subprocess

OUT = Path('validation/winston-investigation')
SAMPLES = [(0x0e, 'i16', 'state'), (0x10, 'i16', 'goal'), (0x14, 'i16', 'animation'),
           (0x16, 'i16', 'animation_frame'), (0x1e, 'i16', 'speed'),
           (0x34, 'i32', 'x'), (0x38, 'i32', 'y'), (0x3c, 'i32', 'z'), (0x42, 'i16', 'yaw')]
CASES = {
    'startup': ('.psx/home/state.bin', 1800, [{'frame': 0, 'buttons': ['CROSS'], 'until': 10}]),
    'idle': ('.psx/home_runway_start/state.bin', 700, []),
    'away': ('.psx/home_runway_start/state.bin', 600, [{'frame': 0, 'buttons': ['UP'], 'until': 150}]),
    'settle': (str(OUT / 'away/state.bin'), 100, []),
    'turn': ('.psx/home_runway_start/state.bin', 900, [
        {'frame': 0, 'buttons': ['UP'], 'until': 140},
        {'frame': 140, 'buttons': ['RIGHT'], 'until': 200},
        {'frame': 200, 'buttons': ['UP'], 'until': 300}]),
    'backward_push': ('.psx/home_runway_start/state.bin', 110, [
        {'frame': 0, 'buttons': ['SQUARE'], 'until': 8},
        {'frame': 28, 'buttons': ['DOWN'], 'until': 50}]),
}


def export(ram):
    b = ram.read_bytes()
    def read(fmt, addr):
        return struct.unpack_from('<' + fmt, b, addr & 0x1fffff)
    def pointer(addr):
        return read('I', addr)[0] & 0x1fffff
    object_addr = 0x936d0 + 260 * 48
    mesh_first = read('H', object_addr + 2)[0]
    mesh_table = pointer(0x8b358)
    animation_base = pointer(0x8aa90)
    state_base, range_base, command_base = map(pointer, [0x8bb78, 0x8b350, 0x8c320])
    clips = []
    for number in range(280, 284):
        addr = animation_base + number * 32
        a = read('IBBHii8H', addr)
        assert a[1] == 1, 'This export currently requires key rate 1'
        keys = [list(read('6h', a[0] + k * a[2] * 2)) for k in range(a[7] - a[6] + 1)]
        poses = [list(read('H' * a[2], a[0] + k * a[2] * 2)) for k in range(a[7] - a[6] + 1)]
        changes = []
        for i in range(a[10]):
            goal, count, off = read('3H', state_base + 6 * (a[11] + i))
            changes.append({'goal': goal, 'ranges': [list(read('4H', range_base + 8 * (off + j))) for j in range(count)]})
        commands = []
        cur = command_base + 2 * a[13]
        for _ in range(a[12]):
            opcode = read('h', cur)[0]
            count = {1: 3, 2: 2, 3: 0, 4: 0, 5: 2, 6: 2}[opcode]
            commands.append([opcode, *read('h' * count, cur + 2)])
            cur += 2 * (count + 1)
        clips.append(dict(number=number, ram_address=hex(addr), state=a[3], speed_raw=a[4], acceleration_raw=a[5],
                          frame_start=a[6], frame_end=a[7], next_animation=a[8], next_frame=a[9],
                          state_changes=changes, commands=commands, frame_bounds=keys, pose_words=poses))
    spheres = []
    for i in range(read('H', object_addr)[0]):
        addr = pointer(mesh_table + 4 * (mesh_first + i))
        spheres.append({'mesh_index': mesh_first + i, 'mesh_ram': hex(addr), 'center_radius': list(read('4h', addr))})
    bone_base = pointer(0x8aa5c) + 4 * read('I', object_addr + 4)[0]
    bones = [list(read('4i', bone_base + 16 * i)) for i in range(len(spheres) - 1)]
    result = dict(source=str(ram), sha256=hashlib.sha256(b).hexdigest(), object_address=hex(object_addr),
                  controller=hex(pointer(object_addr+16)), collision=hex(pointer(object_addr+32)),
                  object_raw=b[object_addr:object_addr+48].hex(), clips=clips, mesh_spheres=spheres, bone_ram=hex(bone_base), bones=bones,
                  limitations=['Spheres are mesh-local; runtime requires authored bone/pose transforms.',
                               'Exported frame bounds are direct runtime animation key data; all four clips have key rate 1.',
                               'Command opcode meanings require animate-item.disasm.txt; no external source used.'])
    (OUT / 'runtime-animation-data.json').write_text(json.dumps(result, indent=2) + '\n')


def record(case):
    checkpoint, fields, inputs = CASES[case]
    destination = OUT / case
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(fields), '--input', json.dumps(inputs), '--out', str(destination), '--save-state', '--summary']
    for offset, fmt, name in SAMPLES:
        command += ['--sample', f'{0x8017c8c8+offset:x}:{fmt}:{name}']
    for addr, fmt, name in [(0x8017c8d4, 'i16', 'kind'), (0x8017c8ec, 'i16', 'actor_box'), (0x8017c8a0, 'i16', 'lara_box'),
                            (0x80182700, 'i32', 'mood'), (0x80182718, 'i16', 'target_box'),
                            (0x8018271c, 'i32', 'goal_x'), (0x80182724, 'i32', 'goal_z'),
                            (0x8017c8b0, 'i32', 'lara_x'), (0x8017c8b4, 'i32', 'lara_y'), (0x8017c8b8, 'i32', 'lara_z'),
                            (0x801826f8, 'i16', 'max_turn'), (0x801826fa, 'x16', 'creature_flags'),
                            (0x80182728, 'i32', 'target_x'), (0x80182730, 'i32', 'target_z')]:
        command += ['--sample', f'{addr:x}:{fmt}:{name}']
    command += ['--watch', '8017c8c8:76', '--watch', '8017c8b0:12', '--watch-changes-only']
    if case == 'backward_push':
        command += ['--trace', '--trace-from-frame', '38', '--trace-frames', '52', '--trace-range', '1C65C-1CE04', '--trace-range', '500A0-502D4', '--trace-range', '5BAC4-5BACC', '--max-events', '40000']
    subprocess.run(command, check=True)
    (destination / 'command.json').write_text(json.dumps(command, indent=2) + '\n')


def record_contact_spheres():
    for name, start in [('spheres_first_trace', 38), ('spheres_second_trace', 86)]:
        destination = OUT / name
        command = ['psx', 'run', '--from-state', '.psx/home_runway_start/state.bin',
                   '--frames', '110', '--input', json.dumps(CASES['backward_push'][2]),
                   '--trace', '--trace-from-frame', str(start), '--trace-frames', '2',
                   '--trace-range', '5BAF8-5BE3C', '--max-events', '15000',
                   '--out', str(destination), '--summary']
        subprocess.run(command, check=True)
        (destination / 'command.json').write_text(json.dumps(command, indent=2)+'\n')


def export_contact_spheres():
    """Parse the actual sphere-output stores, without recreating pose math."""
    reports = []
    stores = {0x5bc58, 0x5bc6c, 0x5bc80, 0x5bc94, 0x5bda4, 0x5bdbc, 0x5bdd0, 0x5bde4}
    native = list(csv.DictReader(Path('game/tests/data/original/winston_contact_back.csv').open()))
    signed = lambda value, bits: value - (1 << bits) if value & (1 << (bits - 1)) else value
    for name in ['spheres_first_trace', 'spheres_second_trace']:
        path = OUT / name / 'trace.txt'
        calls, current, values = [], None, []
        for line in path.read_text().splitlines():
            parts = line.split()
            if len(parts) < 4 or not parts[0].isdigit():
                continue
            pc = int(parts[2], 16)
            if pc == 0x5bb00:
                item = int(re.search(r's1=([0-9a-f]+)', line)[1], 16)
                run_field = int(parts[0]) - 15372
                row = native[run_field + 1]
                prefix = 'actor' if item == 0x17c8c8 else 'lara'
                current = dict(kind=260 if prefix == 'actor' else 0,
                               run_field=run_field, animation=int(row[prefix+'_animation']),
                               absolute_frame=int(row[prefix+'_animation_frame']),
                               position=[0,0,0], yaw=0, spheres=[])
                calls.append(current)
            if current is None:
                continue
            if pc in {0x5bb40, 0x5bb44, 0x5bb4c, 0x5bb94}:
                value = int(re.search(r'=>[0-9a-f]+', line)[0][2:], 16)
                if pc == 0x5bb94:
                    current['yaw'] = signed(value, 16)
                else:
                    current['position'][{0x5bb40:1,0x5bb44:0,0x5bb4c:2}[pc]] = signed(value,32)
            if pc in stores:
                values.append(signed(int(re.search(r'\]=([0-9a-f]+)',line)[1],16),32))
                if len(values)==4:
                    current['spheres'].append(values)
                    values=[]
        assert [len(c['spheres']) for c in calls] == [26,15]
        reports.append(dict(trace=str(path),trace_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                            command=json.loads((path.parent/'command.json').read_text()) if (path.parent/'command.json').exists() else None,calls=calls))
    destination = Path('game/tests/data/original/winston_spheres.json')
    destination.write_text(json.dumps(dict(source='PSX runtime output stores in GetSpheres0x8005BAF8; no pose reimplementation',
        trace_command='psx run --from-state .psx/home_runway_start/state.bin --frames 110 --input <contact_back input> --trace --trace-from-frame <38 or86> --trace-frames 2 --trace-range 5BAF8-5BE3C --max-events 15000',
        cases=reports),indent=2)+'\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--ram', type=Path, default=Path('.psx/home_start/ram.bin'))
    parser.add_argument('--case', choices=CASES)
    parser.add_argument('--export-contact-spheres', action='store_true')
    parser.add_argument('--record-contact-spheres', action='store_true')
    args = parser.parse_args()
    if args.record_contact_spheres:
        record_contact_spheres()
    if args.export_contact_spheres or args.record_contact_spheres:
        export_contact_spheres()
        return
    OUT.mkdir(parents=True, exist_ok=True)
    export(args.ram)
    if args.case:
        record(args.case)


if __name__ == '__main__':
    main()
