#!/usr/bin/env python3
"""Keep the people's `psx handoff` playthroughs, and how to rebuild the
states they start from, in `recordings/`.

`.psx/` holds emulator states, which carry the console's whole memory (the
game's own code and data), so it stays out of git and can be wiped. What a
test needs from a handoff is small: the person's pad input and the state it
starts from. Every start state was itself made from power-on by runs of
scripted input or earlier handoffs, so `save` writes, per handoff:

- `recordings/handoffs/NAME/`: `input.json` (the replay), `request.json`,
  `note.txt` and `report.json` as the handoff wrote them;
- `recordings/handoffs/NAME/start.json`: the chain of runs from power-on to
  its start state (each run's frames, input, memory card and the sha256 of
  the state it ended in), read back from the runs' reports;

and copies each start state (with the report it reloads by) to
`recordings/states/STATE/`, named as its `.psx/` checkpoint. That folder is
a submodule, the private repository `bombpersons/tr2-rs-states`: this one is
public, and the states are the game's. `rebuild NAME` replays the chain from
power-on into `.psx/rebuild/NAME/` and checks each step against its noted
digest, for when the states are out of reach or psx no longer loads them.

    tools/recordings.py save
    tools/recordings.py rebuild wall_level
"""
import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DISC = 'rom/Tomb Raider II - Starring Lara Croft (USA) (Rev 3).cue'
HANDOFFS = ROOT / '.psx/handoffs'
OUT = ROOT / 'recordings'
KEPT = ['input.json', 'request.json', 'note.txt', 'report.json']
# Handoffs whose window closed before psx wrote a report: their start and
# length in fields (as tools/record_wall_traps.py replays them).
NO_REPORT = {'wall_trex': ('.psx/wall_before_zipline/state.bin', 16738)}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def relative(path):
    path = Path(path)
    return str(path.relative_to(ROOT)) if path.is_absolute() else str(path)


def chain(state):
    """The runs from power-on that made `state`, first to last."""
    steps = []
    while state:
        state = relative(state)
        run = ROOT / state
        report_path = run.parent / 'report.json'
        report = json.loads(report_path.read_text())
        frames = report['frames']['reached']
        # A checkpoint saved part-way through a run (`--save-state-at`).
        if run.name.startswith('state_f'):
            frames = int(run.stem.removeprefix('state_f'))
        if report['input'].get('file') and report.get('request'):
            # A handoff: its input is the person's replay.
            name = run.parent.name
            input_ = {'replay': f'recordings/handoffs/{name}/input.json'}
        else:
            input_ = {'events': [e for e in report['input'].get('events') or [] if e['frame'] <= frames]}
        memcards = report.get('memcards') or []
        step = {
            'from': report['identity'].get('from_state') and relative(report['identity']['from_state']),
            'frames': frames,
            **input_,
            'pokes': report.get('pokes') or [],
            # A card inserted at power-on (`new`); later runs carry it in
            # their state.
            'memcard1': 'new' if not report['identity'].get('from_state') and any(m['slot'] == 1 for m in memcards)
            else None,
            'pad': (report.get('pad') or {}).get('kind', 'digital'),
            'state': state,
            'state_sha256': digest(run),
        }
        steps.append(step)
        state = report['identity'].get('from_state')
    steps.reverse()
    assert not steps[0]['from'], 'a chain starts at power-on'
    return steps


def save():
    for handoff in sorted(p for p in HANDOFFS.iterdir() if (p / 'input.json').exists()):
        name = handoff.name
        out = OUT / 'handoffs' / name
        out.mkdir(parents=True, exist_ok=True)
        for file in KEPT:
            if (handoff / file).exists():
                shutil.copy2(handoff / file, out / file)
        if (handoff / 'report.json').exists():
            report = json.loads((handoff / 'report.json').read_text())
            start, frames = report['identity']['from_state'], report['frames']['reached']
        else:
            start, frames = NO_REPORT[name]
        steps = chain(start)
        state = ROOT / relative(start)
        kept = OUT / 'states' / state.parent.name
        (out / 'start.json').write_text(json.dumps({
            'start': relative(start), 'start_sha256': digest(state), 'kept': relative(kept / 'state.bin'),
            'frames': frames,
            'media': DISC, 'bios_sha256': '11052b6499e466bbf0a709b1f9cb6834a9418e66680387912451e971cf8a1fef',
            'chain': steps,
        }, indent=1) + '\n')
        # The start state, with the report psx reloads it by.
        kept.mkdir(parents=True, exist_ok=True)
        shutil.copy2(state, kept / 'state.bin')
        report = state.parent / 'report.json'
        shutil.copy2(report, kept / 'report.json')
        print(f'{name}: {frames} fields from {relative(start)} ({len(steps)} runs from power-on)')


def rebuild(name):
    """Replay a handoff's chain from power-on; returns its start state."""
    start = json.loads((OUT / 'handoffs' / name / 'start.json').read_text())
    work = ROOT / '.psx/rebuild' / name
    previous = None
    for n, step in enumerate(start['chain']):
        out = work / f'{n:02d}'
        command = ['psx', 'run', start['media'], '--frames', str(step['frames']), '--timeout', '3600',
                   '--save-state', '--out', relative(out), '--summary', '--pad', step['pad']]
        if previous:
            command += ['--from-state', relative(previous)]
        elif step['memcard1']:
            command += ['--memcard1', step['memcard1']]
        if 'replay' in step:
            command += ['--replay', step['replay']]
        elif step['events']:
            command += ['--input', json.dumps(step['events'])]
        for poke in step['pokes']:
            command += ['--poke', poke if isinstance(poke, str) else poke['spec']]
        subprocess.run(['systemd-run', '--user', '--scope', '-q', '-p', 'MemoryMax=4G'] + command, cwd=ROOT,
                       check=True, stdout=subprocess.DEVNULL)
        previous = out / 'state.bin'
        same = digest(previous) == step['state_sha256']
        print(f'{n:02d} {step["state"]}: {"same" if same else "DIFFERS from the noted state"}')
    return previous


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest='command', required=True)
    sub.add_parser('save')
    sub.add_parser('rebuild').add_argument('name')
    args = parser.parse_args()
    if args.command == 'save':
        save()
    else:
        rebuild(args.name)


if __name__ == '__main__':
    main()
