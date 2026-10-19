#!/usr/bin/env python3
"""Retain pistol draw, fire and holster evidence from ordinary PSX pad routes.

Every case starts from the settled Wall checkpoint (pistols selected, hands
free) and uses no pokes. Cases named *_heldout were recorded only after the
rules in game/src/player/weapons.rs were written and never used to fit them.
"""
import argparse
import hashlib
import json
import shutil
import subprocess

from record_backward_collision import ROOT, SAMPLES, event

# Lara's item is at 0x8019cf30 in Wall (0x8017c87c in Home).
LARA = [f'{int(s[:8], 16) + 0x206b4:08x}{s[8:]}' if s.startswith('8017') else s for s in SAMPLES]
# Lara's gun globals (LaraGun 0x8003FC4C and the pistol handlers).
WEAPON = [
    '8008aad4:x32:input', '8008c4ea:i16:gun_status', '8008c4ec:i16:gun_type',
    '8008c4ee:i16:request_gun', '8008c584:i16:left_frame', '8008c598:i16:right_frame',
    '8008c590:i16:left_flash', '8008c5a4:i16:right_flash', '8008c5a8:i32:pistol_ammo',
    '8008c568:x32:target', '8008c58a:i16:left_yaw', '8008c58c:i16:left_pitch',
    '8008c59e:i16:right_yaw', '8008c5a0:i16:right_pitch',
    '8008c574:i16:head_yaw', '8008c576:i16:head_pitch',
    '8008c57a:i16:torso_yaw', '8008c57c:i16:torso_pitch',
]
DRAW = event(['TRIANGLE'], 14, 10)

# name: (fields, input)
CASES = {
    'draw_fire_holster': (200, [DRAW, event(['CROSS'], 120, 70), event(['TRIANGLE'], 154, 150)]),
    'held_draw': (200, [event(['TRIANGLE'], 160, 10)]),
    'aim_taps': (200, [DRAW, event(['CROSS'], 72, 70), event(['CROSS'], 106, 100),
                       event(['CROSS'], 150, 140)]),
    'rapid_taps': (200, [DRAW] + [event(['CROSS'], f + 2, f) for f in range(70, 130, 4)]),
    'holster_while_firing': (220, [DRAW, event(['CROSS'], 86, 60), event(['CROSS', 'TRIANGLE'], 90, 86),
                                   event(['CROSS'], 110, 90)]),
    'turn_ready': (200, [DRAW, event(['RIGHT'], 130, 70), event(['LEFT'], 180, 150)]),
    'run_draw': (160, [event(['UP'], 20), event(['UP', 'TRIANGLE'], 24, 20), event(['UP'], 100, 24)]),
    # Held out: recorded after weapons.rs was written.
    'retap_heldout': (200, [DRAW, event(['CROSS'], 82, 66)]
                      + [event(['CROSS'], f + 2, f) for f in range(84, 140, 4)]),
    'walk_turn_ready_heldout': (200, [DRAW, event(['R1', 'RIGHT'], 120, 64), event(['R1', 'LEFT'], 170, 140)]),
    'draw_retap_heldout': (200, [DRAW, event(['TRIANGLE'], 24, 20), event(['TRIANGLE'], 42, 40),
                                 event(['CROSS'], 70, 64), event(['TRIANGLE'], 72, 70)]),
    # Recorded after the aiming camera's resting-distance fallback was fixed.
    'draw_turn_holster_heldout': (240, [event(['TRIANGLE'], 4), event(['LEFT'], 100, 50),
                                        event(['TRIANGLE'], 114, 110), event(['UP'], 200, 150)]),
    'run_fire_heldout': (220, [event(['TRIANGLE'], 4), event(['UP'], 60, 4), event(['UP', 'CROSS'], 140, 60),
                               event(['CROSS'], 170, 140)]),
}


# Tiger 18 (Wall placement 18, item 0x8019ce98): pad-only checkpoint after
# its floor trigger in room 36/37 activates it (creature slot 0 = 18).
TIGER_START = '.psx/pistols_tiger_start'
TIGER_PREP = [event(['UP'], 420), event(['RIGHT'], 500, 450), event(['UP'], 580, 530)]
TIGER = [
    '8019cecc:i32:tiger_x', '8019ced0:i32:tiger_y', '8019ced4:i32:tiger_z', '8019ceda:i16:tiger_yaw',
    '8019ceac:i16:tiger_animation', '8019ceae:i16:tiger_frame', '8019ceb0:i16:tiger_room',
    '8019ceba:i16:tiger_hp', '8019cea6:i16:tiger_state', '8019cee0:x32:tiger_flags',
    '801a29c8:i16:slot0', '8008c56c:i16:aim_yaw', '8008c56e:i16:aim_pitch',
    '8008c588:i16:left_lock', '8008c59c:i16:right_lock', '80089d54:x32:seed',
]
# name: (fields, input)
TIGER_CASES = {
    'tiger_volley': (300, [event(['TRIANGLE'], 4), event(['RIGHT'], 92, 40), event(['CROSS'], 300, 100)]),
    'tiger_lock': (240, [event(['TRIANGLE'], 4), event(['RIGHT'], 92, 40)]),
    # Held out: recorded after the targeting rules were written.
    'tiger_left_late_heldout': (300, [event(['TRIANGLE'], 4), event(['LEFT'], 110, 40),
                                      event(['CROSS'], 300, 150)]),
    # Re-taps at a locked tiger; it survives (locked arms keep full recoil).
    'tiger_kill_heldout': (300, [event(['TRIANGLE'], 4), event(['RIGHT'], 92, 40), event(['CROSS'], 120, 100)]
                           + [event(['CROSS'], f + 2, f) for f in range(124, 220, 4)]),
    'tiger_kill_long_heldout': (700, [event(['TRIANGLE'], 4), event(['RIGHT'], 92, 40), event(['CROSS'], 700, 92)]),
    'tiger_release_heldout': (300, [event(['TRIANGLE'], 4), event(['RIGHT'], 92, 40), event(['CROSS'], 130, 100),
                                    event(['CROSS'], 200, 160), event(['CROSS', 'RIGHT'], 230, 200),
                                    event(['CROSS'], 300, 230)]),
}


def prepare_tiger():
    command = ['psx', 'run', '--from-state', '.psx/wall_game/state.bin', '--frames', '640',
               '--input', json.dumps(TIGER_PREP), '--save-state', '--out', TIGER_START, '--summary']
    subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    report = json.loads((ROOT / TIGER_START / 'report.json').read_text())
    assert report['status'] == 'complete' and not report.get('pokes')
    return command


def record_tiger(name):
    frames, inputs = TIGER_CASES[name]
    checkpoint = f'{TIGER_START}/state.bin'
    if not (ROOT / checkpoint).exists():
        prepare_tiger()
    out = f'validation/pistols/{name}'
    samples = LARA + WEAPON + TIGER
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '2', '--watch', '80089d54:4',
               '--save-state', '--out', out, '--summary']
    for sample in samples:
        command += ['--sample', sample]
    subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    report = json.loads((ROOT / out / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames
    assert report['watch']['complete'] and not report.get('pokes')
    target = ROOT / f'game/tests/data/original/pistols_{name}.csv'
    shutil.copyfile(ROOT / out / 'samples.csv', target)
    writes = [json.loads(line) for line in (ROOT / out / 'writes.jsonl').read_text().splitlines()]
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; no pokes',
        'scope': 'Pistol targeting, arm aim, shots and tiger 18 hit points per field',
        'heldout_after_implementation': name.endswith('_heldout'),
        'level': 'wall', 'command': command, 'input': inputs, 'samples': samples,
        'build': report['build'], 'identity': report['identity'], 'frames': report['frames'],
        'checkpoint': checkpoint, 'checkpoint_sha256': sha(checkpoint),
        'checkpoint_preparation': dict(source='.psx/wall_game/state.bin', fields=640, input=TIGER_PREP),
        'samples_sha256': sha(target.relative_to(ROOT)),
        # Every control-RNG store (0x80089d54) with its absolute field.
        'random_writes': [dict(frame=w['frame'], old=w['old'], value=w['value']) for w in writes],
    }
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT))


def sha(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def record(name):
    frames, inputs = CASES[name]
    checkpoint = '.psx/wall_game/state.bin'
    out = f'validation/pistols/{name}'
    samples = LARA + WEAPON
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '2',
               '--save-state', '--out', out, '--summary']
    for sample in samples:
        command += ['--sample', sample]
    subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    report = json.loads((ROOT / out / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames
    assert not report.get('pokes')
    target = ROOT / f'game/tests/data/original/pistols_{name}.csv'
    shutil.copyfile(ROOT / out / 'samples.csv', target)
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; no pokes',
        'scope': 'Pistol gun status, arm frames, flashes, ammo and Lara state per field',
        'heldout_after_implementation': name.endswith('_heldout'),
        'level': 'wall', 'command': command, 'input': inputs, 'samples': samples,
        'build': report['build'], 'identity': report['identity'], 'frames': report['frames'],
        'checkpoint': checkpoint, 'checkpoint_sha256': sha(checkpoint),
        'samples_sha256': sha(target.relative_to(ROOT)),
    }
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT))


def inspect():
    """Retain the disassembly of the pistol code these rules port."""
    out = ROOT / 'validation/pistols/code'
    out.mkdir(parents=True, exist_ok=True)
    ram = (ROOT / '.psx/wall_game/ram.bin').read_bytes()
    entries = []
    for start, end in [(0x3fc4c, 0x40494), (0x404f0, 0x406b4), (0x406b4, 0x40e8c),
                       (0x40e8c, 0x4132c), (0x47e04, 0x484b0), (0x484b0, 0x48b3c),
                       (0x89434, 0x89434 + 9 * 0x30)]:
        listing = ''
        if start < 0x80000:
            command = ['psx', 'disasm', '--ram', '.psx/wall_game', '--addr', hex(0x80000000 | start),
                       '--count', str((end - start) // 4), '--text']
            listing = subprocess.run(command, cwd=ROOT, check=True, capture_output=True, text=True).stdout
            (out / f'{start:05x}.txt').write_text(listing)
        else:
            command = ['psx', 'ram', 'view', '.psx/wall_game', '--addr', hex(0x80000000 | start),
                       '--len', str(end - start), '--as', 'i16', '--text']
        entries.append(dict(start=hex(0x80000000 | start), end_exclusive=hex(0x80000000 | end),
                            command=command, bytes_sha256=hashlib.sha256(ram[start:end]).hexdigest()))
    data = dict(source='Supplied USA Rev3 disc via psx; Wall runtime code (.psx/wall_game)',
                ram_sha256=hashlib.sha256(ram).hexdigest(), ranges=entries,
                lara_gun='0x8003fc4c', pistol_table='0x80089464',
                draw_pistols='0x80047e60', undraw_pistols='0x80047f1c', pistol_handler='0x800484b0')
    (ROOT / 'game/tests/data/original/pistols_code.json').write_text(json.dumps(data, indent=2) + '\n')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--inspect', action='store_true')
    p.add_argument('--case', choices=CASES | TIGER_CASES, action='append', default=[])
    p.add_argument('--all', action='store_true')
    args = p.parse_args()
    if args.inspect:
        inspect()
    for name in (list(CASES) + list(TIGER_CASES) if args.all else args.case):
        if name in TIGER_CASES:
            record_tiger(name)
        else:
            record(name)
