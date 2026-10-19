#!/usr/bin/env python3
"""Retain PSX-only sky GPU projection fixtures; no memory pokes.

Run the initial calibration before fitting sky rules; record a new independent
orientation only after implementation. Existing fixture identities stay in Git.
"""
import argparse
import hashlib
import json
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', choices=['home_sky', 'home_sky_turn'], required=True)
    args = parser.parse_args()
    heldout = args.case == 'home_sky_turn'
    frames = 120 if heldout else 4
    output = '.psx/home_sky_turn' if heldout else '.psx/home_sky_gpu'
    command = ['psx', 'run', '--from-state', '.psx/home_start/state.bin', '--frames', str(frames)]
    if heldout:
        command += ['--input', json.dumps([dict(frame=0, buttons=['RIGHT'], until=36)])]
    command += ['--gpu-frame', str(frames - 1), '--save-state', '--out', output, '--summary']
    subprocess.run(command, cwd=ROOT, check=True)
    folder = ROOT / output
    report = json.loads((folder / 'report.json').read_text())
    assert report['status'] == 'complete' and not report.get('pokes')
    gpu = json.loads((folder / 'gpu_frame.json').read_text())
    ram = (folder / 'ram.bin').read_bytes()
    def point(address):
        return list(struct.unpack_from('<iii', ram, address))
    commands = []
    for command_record in gpu['commands'][8:]:
        if not command_record.get('textured') or command_record['texpage']['y'] != 256:
            break
        commands.append(command_record)
    assert commands
    fixture = dict(source='Supplied Tomb Raider II USA Rev 3 disc via psx native; no pokes',
                   heldout_after_implementation=heldout, build=report['build'], identity=report['identity'],
                   command=command, camera=dict(eye=point(0x8b258), target=point(0x8b268)),
                   model=254, mesh=100, animation=278, textured_commands=commands,
                   cap_command=gpu['commands'][7],
                   source_checkpoint_sha256=hashlib.sha256((ROOT / '.psx/home_start/state.bin').read_bytes()).hexdigest(),
                   gpu_frame_sha256=hashlib.sha256((folder / 'gpu_frame.json').read_bytes()).hexdigest())
    target = ROOT / f'game/tests/data/original/{args.case}.json'
    target.write_text(json.dumps(fixture, indent=2) + '\n')
    print(target.relative_to(ROOT))


if __name__ == '__main__':
    main()
