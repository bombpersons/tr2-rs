#!/usr/bin/env python3
"""Record ordinary Home attendant routes using only the supplied disc and psx.

Replays do not poke memory. Ordinary cases use unmodified gameplay checkpoints;
blocked-contact cases explicitly retain controlled initial placement. Independent
cases were first recorded after the matching rules were written. Earlier
investigation captures stay separate from the checked-in validation routes.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
FRESH_CHECKPOINT = '.psx/contact_runway/state.bin'
BLOCKED_PLACEMENTS = {
    'contact_blocked_roll_target_heldout': (21570, 92360, 21810, 92535),
    'contact_blocked_roll': (21550, 92350, 21800, 92540),
    'contact_blocked_roll_offset_heldout': (21580, 92370, 21820, 92530),
    'contact_blocked_roll_late_heldout': (21550, 92350, 21800, 92540),
    'contact_blocked_corner': (21550, 92350, 21800, 92540),
    'contact_blocked_offset_heldout': (21580, 92370, 21820, 92530),
    'contact_blocked_release_heldout': (21550, 92350, 21800, 92540),
    'contact_blocked_clear_heldout': (21620, 92350, 21800, 92540),
}


def prepare_blocked_checkpoint(name):
    """Place both actors beside the fountain corner; never alter level/code.

    Two neutral fields settle the original room/box and collision context.
    Every subsequent recording starts from that settled state without pokes.
    """
    px, pz, ax, az = BLOCKED_PLACEMENTS[name]
    output = ROOT / f'.psx/{name}_start'
    pokes = [f'8017c8b0={px}', f'8017c8b8={pz}', '8017c8be=0:2',
             f'8017c8fc={ax}', f'8017c904={az}']
    command = ['psx', 'run', '--from-state', FRESH_CHECKPOINT, '--frames', '2',
               '--input', '[]', '--save-state', '--out', str(output), '--summary']
    for poke in pokes:
        command += ['--poke', poke]
    subprocess.run(command, check=True, cwd=ROOT)
    report = json.loads((output / 'report.json').read_text())
    if report['status'] != 'complete' or report['frames']['reached'] != 2:
        raise RuntimeError(f'Controlled checkpoint preparation did not complete: {name}')
    ancestry = ROOT / '.psx/contact_runway/preparation.json'
    preparation = dict(controlled_initial_placement=True, command=command,
                       source_checkpoint=FRESH_CHECKPOINT,
                       source_sha256=hashlib.sha256((ROOT / FRESH_CHECKPOINT).read_bytes()).hexdigest(),
                       state_sha256=hashlib.sha256((output / 'state.bin').read_bytes()).hexdigest(),
                       build=report['build'], identity=report['identity'], pokes=report['pokes'],
                       source_preparation=json.loads(ancestry.read_text()) if ancestry.exists() else None)
    (output / 'preparation.json').write_text(json.dumps(preparation, indent=2)+'\n')
    return str((output / 'state.bin').relative_to(ROOT))


def prepare_checkpoint():
    """Rebuild Home from the disc when local save states use an older format.

    These menu boundaries were observed with psx 0.3.2; inspect the retained
    screenshots if a later toolkit build changes boot/loading timing.
    """
    media = 'rom/Tomb Raider II - Starring Lara Croft (USA) (Rev 3).cue'
    recipes = [
        (None, 'contact_boot', 7800, []),
        ('contact_boot', 'contact_title', 3840, [
            {'frame': 0, 'buttons': ['CROSS'], 'until': 10},
            {'frame': 3600, 'buttons': ['CROSS'], 'until': 3610}]),
        ('contact_title', 'contact_home_start', 3240, [
            {'frame': 0, 'buttons': ['CROSS'], 'until': 10},
            {'frame': 120, 'buttons': ['CROSS'], 'until': 130}]),
        ('contact_home_start', 'contact_menu', 300, [
            {'frame': 0, 'buttons': ['CROSS'], 'until': 10}]),
        ('contact_menu', 'contact_home', 3240, [
            {'frame': 0, 'buttons': ['CROSS'], 'until': 10},
            {'frame': 120, 'buttons': ['CROSS'], 'until': 130}]),
        ('contact_home', 'contact_side', 112, [
            {'frame': 0, 'buttons': ['RIGHT'], 'until': 52}]),
        ('contact_side', 'contact_runway', 380, [
            {'frame': 0, 'buttons': ['DOWN', 'R1'], 'until': 300}]),
    ]
    steps = []
    for source, output, fields, inputs in recipes:
        command = ['psx', 'run']
        command += (['--from-state', f'.psx/{source}/state.bin'] if source else
                    [media, '--bios', 'bios/scph5501.bin'])
        command += ['--frames', str(fields), '--input', json.dumps(inputs),
                    '--save-state', '--out', f'.psx/{output}', '--summary']
        subprocess.run(command, check=True, cwd=ROOT)
        report = json.loads((ROOT / f'.psx/{output}/report.json').read_text())
        if report['status'] != 'complete' or report['frames']['reached'] != fields:
            raise RuntimeError(f'Checkpoint preparation did not complete: {output}')
        steps.append(dict(command=command, state_sha256=hashlib.sha256(
            (ROOT / f'.psx/{output}/state.bin').read_bytes()).hexdigest()))
    (ROOT / '.psx/contact_runway/preparation.json').write_text(json.dumps(steps, indent=2)+'\n')
    return FRESH_CHECKPOINT


CASES = {
    'contact_blocked_roll_target_heldout': (240, [
        {'frame': 32, 'buttons': ['CIRCLE'], 'until': 44}], True),
    'contact_blocked_roll': (180, [
        {'frame': 16, 'buttons': ['CIRCLE'], 'until': 24}], False),
    'contact_blocked_roll_offset_heldout': (200, [
        {'frame': 24, 'buttons': ['CIRCLE'], 'until': 34}], True),
    'contact_blocked_roll_late_heldout': (220, [
        {'frame': 40, 'buttons': ['CIRCLE'], 'until': 54}], True),
    'contact_blocked_corner': (160, [], False),
    'contact_blocked_offset_heldout': (180, [], True),
    'contact_blocked_release_heldout': (220, [
        {'frame': 16, 'buttons': ['UP', 'R1'], 'until': 70}], True),
    'contact_blocked_clear_heldout': (200, [
        {'frame': 16, 'buttons': ['DOWN', 'R1'], 'until': 60}], True),
    'contact_vertical_backward': (240, [
        {'frame': 0, 'buttons': ['SQUARE'], 'until': 8},
        {'frame': 28, 'buttons': ['DOWN'], 'until': 50}], False),
    'contact_vertical_reverse_heldout': (280, [
        {'frame': 16, 'buttons': ['SQUARE'], 'until': 24},
        {'frame': 44, 'buttons': ['DOWN'], 'until': 50},
        {'frame': 50, 'buttons': ['UP'], 'until': 60}], True),
    'contact_vertical_late_heldout': (260, [
        {'frame': 12, 'buttons': ['SQUARE'], 'until': 20},
        {'frame': 58, 'buttons': ['DOWN'], 'until': 80}], True),
    'wall_oblique_right_validation_heldout': (900, [
        {'frame':0,'buttons':['UP'],'until':148},
        {'frame':148,'buttons':['RIGHT'],'until':176},
        {'frame':180,'buttons':['UP'],'until':250}], True),
    'wall_oblique_start_validation_heldout': (850, [
        {'frame':0,'buttons':['UP'],'until':160},
        {'frame':160,'buttons':['LEFT'],'until':184},
        {'frame':190,'buttons':['UP'],'until':250}], True),
    'wall_oblique_left_delay_heldout': (820, [
        {'frame':0,'buttons':['UP'],'until':142},
        {'frame':142,'buttons':['LEFT'],'until':166},
        {'frame':174,'buttons':['UP'],'until':254}], True),
    'wall_oblique_right_heldout': (820, [
        {'frame':0,'buttons':['UP'],'until':150},
        {'frame':150,'buttons':['RIGHT'],'until':174},
        {'frame':174,'buttons':['UP'],'until':238}], True),
    'wall_oblique_start_heldout': (800, [
        {'frame':0,'buttons':['UP'],'until':158},
        {'frame':158,'buttons':['LEFT'],'until':182},
        {'frame':182,'buttons':['UP'],'until':248}], True),
    'nav_short_heldout': (780, [{'frame':0,'buttons':['UP'],'until':146}], True),
    'nav_walk_heldout': (1080, [{'frame':0,'buttons':['UP','R1'],'until':540}], True),
    'nav_turn_heldout': (1300, [
        {'frame':0,'buttons':['UP'],'until':150},
        {'frame':150,'buttons':['LEFT'],'until':174},
        {'frame':174,'buttons':['UP'],'until':238}], True),
    'nav_run': (740, [{'frame':0,'buttons':['UP'],'until':150}], False),
    'contact_walk_heldout': (160, [{'frame': 0, 'buttons': ['DOWN','R1'], 'until': 96}], True),
    'contact_air_delayed_heldout': (150, [
        {'frame': 8, 'buttons': ['SQUARE'], 'until': 20},
        {'frame': 38, 'buttons': ['DOWN'], 'until': 64}], True),
    'contact_walk_release_heldout': (150, [{'frame': 16, 'buttons': ['DOWN','R1'], 'until': 78}], True),
    'contact_back': (110, [
        {'frame': 0, 'buttons': ['SQUARE'], 'until': 8},
        {'frame': 28, 'buttons': ['DOWN'], 'until': 50}], False),
    'local_run': (600, [{'frame': 0, 'buttons': ['UP'], 'until': 86}], False),
    'heldout_short': (520, [{'frame': 0, 'buttons': ['UP'], 'until': 70}], True),
    'heldout_turn': (700, [
        {'frame': 0, 'buttons': ['UP'], 'until': 84},
        {'frame': 84, 'buttons': ['RIGHT'], 'until': 106},
        {'frame': 106, 'buttons': ['UP'], 'until': 130}], True),
    'heldout_walk': (700, [{'frame': 0, 'buttons': ['UP', 'R1'], 'until': 210}], True),
}


def record(name, checkpoint=None):
    fields, inputs, independent = CASES[name]
    checkpoint = ROOT / (checkpoint or '.psx/home_runway_start/state.bin')
    out = ROOT / 'validation/winston' / name
    command = ['psx', 'run', '--from-state', str(checkpoint), '--frames', str(fields),
               '--input', json.dumps(inputs), '--out', str(out), '--save-state', '--summary', '--snapshot-every', '2']
    if name in BLOCKED_PLACEMENTS:
        command += ['--watch', '8017c8b0:4', '--watch', '8017c8b8:4',
                    '--trace', '--trace-range', '8001cbc8-8001cc70', '--max-events', '100000']
    if name.startswith('contact_blocked_roll'):
        command += ['--watch', '8017c8b4:4',
                    '--trace-range', '8003bc50-8003be1c',
                    '--trace-range', '8003c318-8003c364']
    samples = []
    for prefix, base in [('actor', 0x8017c8c8), ('lara', 0x8017c87c)]:
        for offset, fmt, label in [(14, 'i16', 'state'), (16, 'i16', 'goal'),
                (20, 'i16', 'animation'), (22, 'i16', 'animation_frame'),
                (24, 'i16', 'room'), (30, 'i16', 'speed'), (52, 'i32', 'x'),
                (56, 'i32', 'y'), (60, 'i32', 'z'), (66, 'i16', 'yaw')]:
            samples.append(f'{base+offset:x}:{fmt}:{prefix}_{label}')
    for addr, fmt, label in [(0x801826f8, 'i16', 'max_turn'),
            (0x80182728, 'i32', 'target_x'), (0x80182730, 'i32', 'target_z')]:
        samples.append(f'{addr:x}:{fmt}:{label}')
    if name == 'contact_blocked_roll_target_heldout':
        samples.append('8018272c:i32:target_y')
    samples.append('8017c8cc:u32:actor_contact_mask')
    samples.append('8008c572:i16:lara_movement_heading')
    samples += ['8017c89c:i16:lara_fall_speed', '8017c89e:i16:lara_health', '8008c570:i16:lara_turn_rate']
    samples += ['8017c8ec:i16:actor_box','8017c8a0:i16:lara_box','80182718:i16:goal_box']
    for prefix, base in [('camera', 0x8008b258), ('look', 0x8008b268)]:
        for i, axis in enumerate('xyz'):
            samples.append(f'{base+4*i:x}:i32:{prefix}_{axis}')
    for sample in samples:
        command += ['--sample', sample]
    subprocess.run(command, check=True, cwd=ROOT)
    report = json.loads((out / 'report.json').read_text())
    if report['status'] != 'complete' or report['frames']['reached'] != fields:
        raise RuntimeError(f'Reference recording did not complete: {name}')
    if name in BLOCKED_PLACEMENTS and (not report['watch']['complete'] or report['trace']['lines_dropped']):
        raise RuntimeError(f'Incomplete collision evidence: {name}')
    metadata = dict(source='Supplied Tomb Raider II USA Rev 3 disc, psx native runtime',
                    checkpoint=str(checkpoint.relative_to(ROOT)),
                    checkpoint_sha256=hashlib.sha256(checkpoint.read_bytes()).hexdigest(),
                    fields=fields, input=inputs, independent=independent, pokes=[], command=command,
                    build=report['build'], identity=report['identity'], frames=report['frames'],
                    samples_sha256=hashlib.sha256((out / "samples.csv").read_bytes()).hexdigest(),
                    scope=('Frontal oblique avatar room-wall correction and recovery' if name.startswith('wall_') else 'Local navigation-box corridor and authored follow' if name.startswith('nav') else 'Animated attendant contact and avatar pushing' if name.startswith('contact') else 'Local same-floor follow, authored start/stop and turning; no contact or cross-box navigation'))
    if name.startswith('contact_blocked_roll'):
        metadata['navigation_target_comparison'] = 'exact'
        metadata['navigation_target_independent'] = name == 'contact_blocked_roll_target_heldout'
        metadata['navigation_target_note'] = ('Post-implementation validation of bounded-height corridor targeting.'
            if metadata['navigation_target_independent'] else
            'Navigation calibration for the later bounded-height corridor query; original contact independence is unchanged.')
    if name in ['wall_oblique_right_heldout','wall_oblique_start_heldout']:
        metadata['independent']=False
        metadata['validation_note']='Recorded after tangent restoration; reused to diagnose ground trigonometry, so calibration for the combined rule.'
    preparation = checkpoint.parent / 'preparation.json'
    if preparation.exists():
        metadata['checkpoint_preparation'] = json.loads(preparation.read_text())
        if isinstance(metadata['checkpoint_preparation'], dict) and metadata['checkpoint_preparation'].get('controlled_initial_placement'):
            metadata['source'] += '; controlled initial actor placement, no replay pokes'
            metadata['scope'] = 'Animated item pushing beside a raised room-sector corner; directional world validation'
            # Ordinary Bevy bone transforms bound GTE sphere-center error but
            # can differ on individual mesh bits at a tangency. Retain the raw
            # mask; these world-validation routes compare contact presence.
            metadata['contact_mask_comparison'] = 'presence'
    if name in BLOCKED_PLACEMENTS:
        metadata['writes_sha256'] = hashlib.sha256((out / 'writes.jsonl').read_bytes()).hexdigest()
        metadata['trace_sha256'] = hashlib.sha256((out / 'trace.txt').read_bytes()).hexdigest()
    if name.startswith('contact_blocked_roll'):
        metadata['scope'] = 'Animated roll/item contact beside a raised sector corner; local bounded-height corridor targets, no physical creature height transition parity'
    destination = ROOT / 'game/tests/data/original'
    (destination / f'winston_{name}.csv').write_bytes((out / 'samples.csv').read_bytes())
    (destination / f'winston_{name}.json').write_text(json.dumps(metadata, indent=2)+'\n')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('case', choices=CASES)
    p.add_argument('--checkpoint', help='Project-relative checkpoint; retained in capture provenance')
    p.add_argument('--prepare-checkpoint', action='store_true', help='Boot the supplied disc and prepare a fresh runway checkpoint')
    p.add_argument('--prepare-blocked-checkpoint', action='store_true', help='Prepare controlled initial placement for a blocked-contact case')
    args = p.parse_args()
    if sum([args.prepare_checkpoint, args.prepare_blocked_checkpoint, bool(args.checkpoint)]) > 1:
        p.error('Choose one checkpoint preparation or --checkpoint')
    if args.prepare_blocked_checkpoint and args.case not in BLOCKED_PLACEMENTS:
        p.error('--prepare-blocked-checkpoint requires a contact_blocked case')
    checkpoint = (prepare_checkpoint() if args.prepare_checkpoint else
                  prepare_blocked_checkpoint(args.case) if args.prepare_blocked_checkpoint else args.checkpoint)
    record(args.case, checkpoint)
